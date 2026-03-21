use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitDiscovery {
    pub git_root: PathBuf,
    pub remote_url: Option<String>,
    pub default_base_branch: String,
}

pub fn discover_repo(path: &Path) -> Result<GitDiscovery> {
    let git_root =
        run_git(path, ["rev-parse", "--show-toplevel"]).context("failed to discover git root")?;
    let git_root = PathBuf::from(git_root.trim());

    let remote_url = run_git_allow_failure(&git_root, ["config", "--get", "remote.origin.url"])?;
    let default_base_branch = detect_default_base_branch(&git_root)?;

    Ok(GitDiscovery {
        git_root,
        remote_url,
        default_base_branch,
    })
}

pub fn clone_repo(remote_url: &str, destination: &Path) -> Result<()> {
    let parent = destination.parent().unwrap_or(destination);
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create clone parent directory at {}",
            parent.display()
        )
    })?;

    let output = Command::new("git")
        .args(["clone", remote_url, &destination.display().to_string()])
        .output()
        .with_context(|| {
            format!(
                "failed to clone remote `{remote_url}` into {}",
                destination.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git clone failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn fetch_repo(repo_root: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["fetch", "--all", "--prune", "--tags"])
        .output()
        .with_context(|| format!("failed to fetch repo at {}", repo_root.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git fetch failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn create_worktree(
    repo_root: &Path,
    slot_path: &Path,
    branch_name: &str,
    base_branch: &str,
) -> Result<()> {
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
        .with_context(|| format!("failed to create worktree at {}", slot_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree add failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn reuse_worktree(slot_path: &Path, branch_name: &str, base_branch: &str) -> Result<()> {
    let base_ref = resolve_base_ref(slot_path, base_branch)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(slot_path)
        .args(["checkout", "-B", branch_name, &base_ref])
        .output()
        .with_context(|| format!("failed to reuse worktree at {}", slot_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git checkout for reused worktree failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn detach_worktree(slot_path: &Path, base_branch: &str) -> Result<()> {
    let base_ref = resolve_base_ref(slot_path, base_branch)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(slot_path)
        .args(["checkout", "--detach", &base_ref])
        .output()
        .with_context(|| format!("failed to detach worktree at {}", slot_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git detach checkout failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn remove_worktree(repo_root: &Path, slot_path: &Path) -> Result<()> {
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
        .with_context(|| format!("failed to remove worktree at {}", slot_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree remove failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn is_clean(path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .with_context(|| format!("failed to check git status in {}", path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git status failed in {}: {}", path.display(), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn detect_default_base_branch(git_root: &Path) -> Result<String> {
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

fn git_ref_exists(git_root: &Path, reference: &str) -> Result<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(git_root)
        .args(["show-ref", "--verify", "--quiet", reference])
        .status()
        .with_context(|| format!("failed to check git ref `{reference}`"))?;

    Ok(status.success())
}

fn resolve_base_ref(git_root: &Path, preferred: &str) -> Result<String> {
    for candidate in [preferred, "HEAD"] {
        if git_ref_exists(git_root, &format!("refs/heads/{candidate}"))? {
            return Ok(candidate.to_string());
        }

        let status = Command::new("git")
            .arg("-C")
            .arg(git_root)
            .args(["rev-parse", "--verify", "--quiet", candidate])
            .status()
            .with_context(|| format!("failed to resolve git ref `{candidate}`"))?;
        if status.success() {
            return Ok(candidate.to_string());
        }
    }

    bail!(
        "repository at {} has no resolvable base ref yet; create an initial commit before acquiring slots",
        git_root.display()
    )
}

fn run_git(
    path: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git command failed in {}: {}",
            path.display(),
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_git_allow_failure(
    path: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", path.display()))?;

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
