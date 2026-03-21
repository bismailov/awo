use super::{DiscoveredSkill, SkillInstallState, SkillLinkMode};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(super) fn determine_install_state(
    skill: &DiscoveredSkill,
    target_path: &Path,
) -> Result<SkillInstallState> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SkillInstallState::Missing);
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect runtime skill target {}",
                    target_path.display()
                )
            });
        }
    };

    if metadata.file_type().is_symlink() {
        let linked_to = fs::read_link(target_path)
            .with_context(|| format!("failed to read symlink target {}", target_path.display()))?;
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
) -> Result<()> {
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

pub(super) fn remove_target(target_path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(target_path).with_context(|| {
        format!(
            "failed to inspect runtime skill target {}",
            target_path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(target_path)
            .with_context(|| format!("failed to remove {}", target_path.display()))?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(target_path)
            .with_context(|| format!("failed to remove {}", target_path.display()))?;
    }
    Ok(())
}

fn create_symlink(source_path: &Path, target_path: &Path) -> Result<()> {
    create_dir_symlink(source_path, target_path).with_context(|| {
        format!(
            "failed to create skill symlink from {} to {}",
            source_path.display(),
            target_path.display()
        )
    })
}

#[cfg(unix)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source_path, target_path)
}

#[cfg(windows)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source_path, target_path)
}

fn copy_dir_all(source_path: &Path, target_path: &Path) -> Result<()> {
    fs::create_dir_all(target_path).with_context(|| {
        format!(
            "failed to create copied skill directory {}",
            target_path.display()
        )
    })?;

    for entry in fs::read_dir(source_path)
        .with_context(|| format!("failed to read skill source {}", source_path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source_path.display()))?;
        let source = entry.path();
        let target = target_path.join(entry.file_name());
        if source.is_dir() {
            copy_dir_all(&source, &target)?;
        } else {
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "failed to copy skill file from {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
        }
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> Result<bool> {
    let left = fs::canonicalize(left)
        .with_context(|| format!("failed to canonicalize {}", left.display()))?;
    let right = fs::canonicalize(right)
        .with_context(|| format!("failed to canonicalize {}", right.display()))?;
    Ok(left == right)
}

fn same_directory_contents(left: &Path, right: &Path) -> Result<bool> {
    Ok(directory_fingerprint(left)? == directory_fingerprint(right)?)
}

fn directory_fingerprint(path: &Path) -> Result<String> {
    let mut entries = Vec::new();
    collect_directory_entries(path, path, &mut entries)?;
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_directory_entries(root: &Path, path: &Path, entries: &mut Vec<String>) -> Result<()> {
    let mut children = fs::read_dir(path)
        .with_context(|| format!("failed to read directory {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entries in {}", path.display()))?;
    children.sort_by_key(|entry| entry.file_name());

    for entry in children {
        let child_path = entry.path();
        let relative = child_path
            .strip_prefix(root)
            .unwrap_or(&child_path)
            .display()
            .to_string();
        let metadata = fs::symlink_metadata(&child_path)
            .with_context(|| format!("failed to inspect {}", child_path.display()))?;
        if metadata.file_type().is_symlink() {
            let link = fs::read_link(&child_path)
                .with_context(|| format!("failed to read symlink {}", child_path.display()))?;
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

fn file_hash(path: &Path) -> Result<String> {
    let contents =
        fs::read(path).with_context(|| format!("failed to read file {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(contents);
    Ok(format!("{:x}", hasher.finalize()))
}
