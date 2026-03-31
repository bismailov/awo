use super::{DiscoveredSkill, SkillInstallState, SkillLinkMode};
use crate::error::{AwoError, AwoResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(super) fn determine_install_state(
    skill: &DiscoveredSkill,
    target_path: &Path,
) -> AwoResult<SkillInstallState> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SkillInstallState::Missing);
        }
        Err(error) => {
            return Err(AwoError::io(
                "inspect runtime skill target",
                target_path,
                error,
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        let linked_to = fs::read_link(target_path)
            .map_err(|source| AwoError::io("read symlink target", target_path, source))?;
        let resolved = if linked_to.is_absolute() {
            linked_to
        } else {
            target_path.parent().unwrap_or(target_path).join(linked_to)
        };
        if same_path(&resolved, Path::new(&skill.source_path))? {
            return Ok(SkillInstallState::Linked);
        }
        return Ok(SkillInstallState::Conflict);
    }

    if metadata.is_dir() {
        let target_skill = target_path.join("SKILL.md");
        if target_skill.exists()
            && same_directory_contents(Path::new(&skill.source_path), target_path)?
        {
            return Ok(SkillInstallState::Copied);
        }
        if target_skill.exists() {
            return Ok(SkillInstallState::Drifted);
        }
    }

    Ok(SkillInstallState::Conflict)
}

pub(super) fn install_skill(
    source_path: &Path,
    target_path: &Path,
    mode: SkillLinkMode,
) -> AwoResult<()> {
    match mode {
        SkillLinkMode::Symlink => create_symlink(source_path, target_path),
        SkillLinkMode::Copy => copy_dir_all(source_path, target_path),
    }
}

pub(super) fn matches_mode(mode: SkillLinkMode, state: SkillInstallState) -> bool {
    matches!(
        (mode, state),
        (SkillLinkMode::Symlink, SkillInstallState::Linked)
            | (SkillLinkMode::Copy, SkillInstallState::Copied)
    )
}

pub(super) fn remove_target(target_path: &Path) -> AwoResult<()> {
    let metadata = fs::symlink_metadata(target_path)
        .map_err(|source| AwoError::io("inspect runtime skill target", target_path, source))?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(target_path)
            .map_err(|source| AwoError::io("remove file", target_path, source))?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(target_path)
            .map_err(|source| AwoError::io("remove directory", target_path, source))?;
    }
    Ok(())
}

fn create_symlink(source_path: &Path, target_path: &Path) -> AwoResult<()> {
    match create_dir_symlink(source_path, target_path) {
        Ok(()) => Ok(()),
        #[cfg(windows)]
        Err(source)
            if source.kind() == std::io::ErrorKind::PermissionDenied
                || source.raw_os_error() == Some(1314) =>
        {
            copy_dir_all(source_path, target_path)
        }
        Err(source) => Err(AwoError::io(
            "create skill symlink",
            format!("{} -> {}", source_path.display(), target_path.display()),
            source,
        )),
    }
}

#[cfg(unix)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source_path, target_path)
}

#[cfg(windows)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source_path, target_path)
}

fn copy_dir_all(source_path: &Path, target_path: &Path) -> AwoResult<()> {
    fs::create_dir_all(target_path)
        .map_err(|source| AwoError::io("create copied skill directory", target_path, source))?;

    for entry in fs::read_dir(source_path)
        .map_err(|source| AwoError::io("read skill source", source_path, source))?
    {
        let entry =
            entry.map_err(|source| AwoError::io("read skill source entry", source_path, source))?;
        let source = entry.path();
        let target = target_path.join(entry.file_name());
        if source.is_dir() {
            copy_dir_all(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|source_err| {
                AwoError::io(
                    "copy skill file",
                    format!("{} -> {}", source.display(), target.display()),
                    source_err,
                )
            })?;
        }
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> AwoResult<bool> {
    let left = dunce::canonicalize(left)
        .map_err(|source| AwoError::io("canonicalize path", left, source))?;
    let right = dunce::canonicalize(right)
        .map_err(|source| AwoError::io("canonicalize path", right, source))?;
    Ok(left == right)
}

fn same_directory_contents(left: &Path, right: &Path) -> AwoResult<bool> {
    Ok(directory_fingerprint(left)? == directory_fingerprint(right)?)
}

fn directory_fingerprint(path: &Path) -> AwoResult<String> {
    let mut entries = Vec::new();
    collect_directory_entries(path, path, &mut entries)?;
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_directory_entries(root: &Path, path: &Path, entries: &mut Vec<String>) -> AwoResult<()> {
    let mut children = fs::read_dir(path)
        .map_err(|source| AwoError::io("read directory", path, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| AwoError::io("read directory entries", path, source))?;
    children.sort_by_key(|entry| entry.file_name());

    for entry in children {
        let child_path = entry.path();
        let relative = child_path
            .strip_prefix(root)
            .unwrap_or(&child_path)
            .display()
            .to_string();
        let metadata = fs::symlink_metadata(&child_path)
            .map_err(|source| AwoError::io("inspect path", &child_path, source))?;
        if metadata.file_type().is_symlink() {
            let link = fs::read_link(&child_path)
                .map_err(|source| AwoError::io("read symlink", &child_path, source))?;
            entries.push(format!("symlink:{relative}:{}", link.display()));
        } else if metadata.is_dir() {
            entries.push(format!("dir:{relative}"));
            collect_directory_entries(root, &child_path, entries)?;
        } else {
            entries.push(format!("file:{relative}:{}", file_hash(&child_path)?));
        }
    }

    Ok(())
}

fn file_hash(path: &Path) -> AwoResult<String> {
    let contents = fs::read(path).map_err(|source| AwoError::io("read file", path, source))?;
    let mut hasher = Sha256::new();
    hasher.update(contents);
    Ok(format!("{:x}", hasher.finalize()))
}
