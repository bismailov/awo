use crate::error::{AwoError, AwoResult};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitDiscovery {
    pub git_root: PathBuf,
    pub remote_url: Option<String>,
    pub default_base_branch: String,
}

pub fn discover_repo(path: &Path) -> AwoResult<GitDiscovery> {
    let git_root = run_git(path, "discover repo root", ["rev-parse", "--show-toplevel"])?;
    let git_root = PathBuf::from(git_root.trim());

    let remote_url = run_git_allow_failure(&git_root, ["config", "--get", "remote.origin.url"])?;
    let default_base_branch = detect_default_base_branch(&git_root)?;

    Ok(GitDiscovery {
        git_root,
        remote_url,
        default_base_branch,
    })
}

pub fn clone_repo(remote_url: &str, destination: &Path) -> AwoResult<()> {
    let parent = destination.parent().unwrap_or(destination);
    std::fs::create_dir_all(parent)
        .map_err(|source| AwoError::io("create clone parent directory", parent, source))?;

    let output = Command::new("git")
        .args(["clone", remote_url, &destination.display().to_string()])
        .output()
        .map_err(|source| AwoError::git_invocation("clone", destination, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "clone",
            destination,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn fetch_repo(repo_root: &Path) -> AwoResult<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["fetch", "--all", "--prune", "--tags"])
        .output()
        .map_err(|source| AwoError::git_invocation("fetch", repo_root, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "fetch",
            repo_root,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn create_worktree(
    repo_root: &Path,
    slot_path: &Path,
    branch_name: &str,
    base_branch: &str,
) -> AwoResult<()> {
    let base_ref = resolve_base_ref(repo_root, base_branch)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "worktree",
            "add",
            "-b",
            branch_name,
            &slot_path.display().to_string(),
            &base_ref,
        ])
        .output()
        .map_err(|source| AwoError::git_invocation("worktree add", slot_path, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "worktree add",
            slot_path,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn reuse_worktree(slot_path: &Path, branch_name: &str, base_branch: &str) -> AwoResult<()> {
    let base_ref = resolve_base_ref(slot_path, base_branch)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(slot_path)
        .args(["checkout", "-B", branch_name, &base_ref])
        .output()
        .map_err(|source| {
            AwoError::git_invocation("checkout reused worktree", slot_path, source)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "checkout reused worktree",
            slot_path,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn detach_worktree(slot_path: &Path, base_branch: &str) -> AwoResult<()> {
    let base_ref = resolve_base_ref(slot_path, base_branch)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(slot_path)
        .args(["checkout", "--detach", &base_ref])
        .output()
        .map_err(|source| AwoError::git_invocation("detach worktree", slot_path, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "detach worktree",
            slot_path,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn remove_worktree(repo_root: &Path, slot_path: &Path) -> AwoResult<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "worktree",
            "remove",
            "--force",
            &slot_path.display().to_string(),
        ])
        .output()
        .map_err(|source| AwoError::git_invocation("worktree remove", repo_root, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(
            "worktree remove",
            slot_path,
            stderr.trim(),
        ));
    }

    Ok(())
}

pub fn is_clean(path: &Path) -> AwoResult<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|source| AwoError::git_invocation("status", path, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed("status", path, stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn dirty_files(path: &Path) -> AwoResult<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|source| AwoError::git_invocation("status", path, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed("status", path, stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = vec![];
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && let Some(p) = trimmed.get(3..)
        {
            files.push(p.trim().to_string());
        }
    }
    Ok(files)
}

fn detect_default_base_branch(git_root: &Path) -> AwoResult<String> {
    if let Some(remote_head) = run_git_allow_failure(
        git_root,
        [
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )? && let Some(branch) = remote_head.trim().strip_prefix("origin/")
        && !branch.is_empty()
    {
        return Ok(branch.to_string());
    }

    if let Some(current_branch) = run_git_allow_failure(git_root, ["branch", "--show-current"])? {
        let current_branch = current_branch.trim();
        if !current_branch.is_empty() {
            return Ok(current_branch.to_string());
        }
    }

    for candidate in ["main", "master"] {
        if git_ref_exists(git_root, &format!("refs/heads/{candidate}"))? {
            return Ok(candidate.to_string());
        }
    }

    Ok("main".to_string())
}

fn git_ref_exists(git_root: &Path, reference: &str) -> AwoResult<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(git_root)
        .args(["show-ref", "--verify", "--quiet", reference])
        .status()
        .map_err(|source| AwoError::git_invocation("show-ref", git_root, source))?;

    Ok(status.success())
}

fn resolve_base_ref(git_root: &Path, preferred: &str) -> AwoResult<String> {
    for candidate in [preferred, "HEAD"] {
        if git_ref_exists(git_root, &format!("refs/heads/{candidate}"))? {
            return Ok(candidate.to_string());
        }

        let status = Command::new("git")
            .arg("-C")
            .arg(git_root)
            .args(["rev-parse", "--verify", "--quiet", candidate])
            .status()
            .map_err(|source| AwoError::git_invocation("rev-parse", git_root, source))?;
        if status.success() {
            return Ok(candidate.to_string());
        }
    }

    Err(AwoError::invalid_state(format!(
        "repository at {} has no resolvable base ref yet; create an initial commit before acquiring slots",
        git_root.display()
    )))
}

fn run_git(
    path: &Path,
    operation: &'static str,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> AwoResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|source| AwoError::git_invocation(operation, path, source))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AwoError::git_command_failed(operation, path, stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_git_allow_failure(
    path: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> AwoResult<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|source| AwoError::git_invocation("optional git command", path, source))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(None)
        } else {
            Ok(Some(stdout))
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discover_repo_fails_on_non_git_dir() -> AwoResult<()> {
        let temp = tempdir().map_err(|e| AwoError::io("create temp dir", "temp", e))?;
        let result = discover_repo(temp.path());
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("discover repo root"));
        Ok(())
    }

    #[test]
    fn is_clean_fails_on_non_existent_path() -> AwoResult<()> {
        let temp = tempdir().map_err(|e| AwoError::io("create temp dir", "temp", e))?;
        let non_existent = temp.path().join("missing");
        let result = is_clean(&non_existent);
        assert!(result.is_err());
        Ok(())
    }
}
