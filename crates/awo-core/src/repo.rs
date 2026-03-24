use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use crate::git::GitDiscovery;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RegisteredRepo {
    pub id: String,
    pub name: String,
    pub repo_root: String,
    pub remote_url: Option<String>,
    pub default_base_branch: String,
    pub worktree_root: String,
    pub shared_manifest_path: Option<String>,
    pub shared_manifest_present: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RegisterRepoResult {
    pub registered_repo: RegisteredRepo,
    pub overlay_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub struct SharedRepoManifest {
    pub name: Option<String>,
    pub default_base_branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalRepoOverlay {
    pub repo_id: String,
    pub repo_root: String,
    pub worktree_root: String,
    pub terminal_app: Option<String>,
    pub preferred_machine: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDescriptor {
    pub provider: String,
    pub host: String,
    pub owner: Option<String>,
    pub repo_name: String,
}

pub fn register_repo(
    paths: &AppPaths,
    input_path: PathBuf,
    git: GitDiscovery,
) -> AwoResult<RegisterRepoResult> {
    let canonical_root = dunce::canonicalize(&git.git_root)
        .map_err(|source| AwoError::io("canonicalize repo root", &git.git_root, source))?;
    let shared_manifest_path = canonical_root.join(".awo").join("repo.toml");
    let shared_manifest = load_shared_manifest(&shared_manifest_path)?;

    let name = shared_manifest.name.unwrap_or_else(|| {
        canonical_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("repo")
            .to_string()
    });
    let default_base_branch = shared_manifest
        .default_base_branch
        .unwrap_or(git.default_base_branch);
    let id = build_repo_id(&canonical_root, git.remote_url.as_deref(), &name);
    let worktree_root = default_worktree_root(&canonical_root, &name);

    let overlay = LocalRepoOverlay {
        repo_id: id.clone(),
        repo_root: canonical_root.display().to_string(),
        worktree_root: worktree_root.display().to_string(),
        terminal_app: None,
        preferred_machine: "local".to_string(),
    };
    let overlay_path = write_local_overlay(paths, &overlay)?;

    let registered_repo = RegisteredRepo {
        id,
        name,
        repo_root: canonical_root.display().to_string(),
        remote_url: git.remote_url,
        default_base_branch,
        worktree_root: worktree_root.display().to_string(),
        shared_manifest_path: if shared_manifest_path.exists() {
            Some(shared_manifest_path.display().to_string())
        } else {
            None
        },
        shared_manifest_present: shared_manifest_path.exists(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let _ = input_path;

    Ok(RegisterRepoResult {
        registered_repo,
        overlay_path,
    })
}

pub fn describe_remote(input: &str) -> RemoteDescriptor {
    if let Some(path) = input.strip_prefix("file://") {
        return describe_local_remote(Path::new(path));
    }

    if input.starts_with('/')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
    {
        return describe_local_remote(Path::new(input));
    }

    if let Some(rest) = input.strip_prefix("git@")
        && let Some((host, path)) = rest.split_once(':')
    {
        return describe_hosted_remote(host, path);
    }

    if let Some((_, remainder)) = input.split_once("://") {
        let authority_and_path = remainder.split_once('/').unwrap_or((remainder, ""));
        let host = authority_and_path
            .0
            .split('@')
            .next_back()
            .unwrap_or(authority_and_path.0);
        return describe_hosted_remote(host, authority_and_path.1);
    }

    describe_local_remote(Path::new(input))
}

pub fn remote_label(remote_url: Option<&str>) -> String {
    remote_url
        .map(describe_remote)
        .map(|remote| remote.provider)
        .unwrap_or_else(|| "local".to_string())
}

pub fn default_clone_destination(paths: &AppPaths, remote_url: &str) -> PathBuf {
    let remote = describe_remote(remote_url);
    let mut destination = paths.clones_dir.join(slugify(&remote.provider));
    if let Some(owner) = remote.owner {
        destination = destination.join(slugify(&owner));
    }
    destination.join(slugify(&remote.repo_name))
}

fn load_shared_manifest(path: &Path) -> AwoResult<SharedRepoManifest> {
    if !path.exists() {
        return Ok(SharedRepoManifest::default());
    }

    let contents = fs::read_to_string(path)
        .map_err(|source| AwoError::io("read shared repo manifest", path, source))?;
    let manifest = toml::from_str::<SharedRepoManifest>(&contents)
        .map_err(|e| AwoError::team_manifest_parse(path, e))?;
    Ok(manifest)
}

fn describe_local_remote(path: &Path) -> RemoteDescriptor {
    let repo_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string();
    RemoteDescriptor {
        provider: "local".to_string(),
        host: "local".to_string(),
        owner: None,
        repo_name,
    }
}

fn describe_hosted_remote(host: &str, path: &str) -> RemoteDescriptor {
    let segments = path
        .trim_matches('/')
        .trim_end_matches(".git")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let owner = if segments.len() >= 2 {
        Some(segments[segments.len() - 2].to_string())
    } else {
        None
    };
    let repo_name = segments.last().copied().unwrap_or("repo").to_string();

    let provider = if host.contains("github") {
        "github"
    } else if host.contains("bitbucket") {
        "bitbucket"
    } else if host.contains("gitlab") {
        "gitlab"
    } else {
        "generic"
    };

    RemoteDescriptor {
        provider: provider.to_string(),
        host: host.to_string(),
        owner,
        repo_name,
    }
}

fn write_local_overlay(paths: &AppPaths, overlay: &LocalRepoOverlay) -> AwoResult<PathBuf> {
    fs::create_dir_all(&paths.repos_dir)
        .map_err(|source| AwoError::io("create repo overlay dir", &paths.repos_dir, source))?;
    let overlay_path = paths.repos_dir.join(format!("{}.toml", overlay.repo_id));
    let contents = toml::to_string_pretty(overlay).map_err(AwoError::team_manifest_serialize)?;
    fs::write(&overlay_path, contents)
        .map_err(|source| AwoError::io("write repo overlay", &overlay_path, source))?;
    Ok(overlay_path)
}

fn default_worktree_root(repo_root: &Path, repo_name: &str) -> PathBuf {
    let parent = repo_root.parent().unwrap_or(repo_root);
    parent.join(format!("{}.worktrees", slugify(repo_name)))
}

fn build_repo_id(repo_root: &Path, remote_url: Option<&str>, name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(repo_root.to_string_lossy().as_bytes());
    if let Some(remote_url) = remote_url {
        hasher.update(remote_url.as_bytes());
    }
    let digest = hasher.finalize();
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{}-{}", slugify(name), suffix)
}

fn slugify(input: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    output.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{default_clone_destination, describe_remote, remote_label};
    use crate::app::AppPaths;
    use std::path::PathBuf;

    #[test]
    fn describes_github_https_remote() {
        let remote = describe_remote("https://github.com/acme/awesome-repo.git");
        assert_eq!(remote.provider, "github");
        assert_eq!(remote.host, "github.com");
        assert_eq!(remote.owner.as_deref(), Some("acme"));
        assert_eq!(remote.repo_name, "awesome-repo");
    }

    #[test]
    fn describes_bitbucket_ssh_remote() {
        let remote = describe_remote("git@bitbucket.org:team/project.git");
        assert_eq!(remote.provider, "bitbucket");
        assert_eq!(remote.host, "bitbucket.org");
        assert_eq!(remote.owner.as_deref(), Some("team"));
        assert_eq!(remote.repo_name, "project");
    }

    #[test]
    fn clone_destination_uses_provider_and_owner() {
        let paths = AppPaths {
            config_dir: PathBuf::from("/tmp/config"),
            data_dir: PathBuf::from("/tmp/data"),
            state_db_path: PathBuf::from("/tmp/state.sqlite3"),
            logs_dir: PathBuf::from("/tmp/logs"),
            repos_dir: PathBuf::from("/tmp/repos"),
            clones_dir: PathBuf::from("/tmp/clones"),
            teams_dir: PathBuf::from("/tmp/teams"),
        };
        let destination =
            default_clone_destination(&paths, "https://github.com/acme/awesome-repo.git");
        assert_eq!(
            destination,
            PathBuf::from("/tmp/clones/github/acme/awesome-repo")
        );
        assert_eq!(
            remote_label(Some("https://github.com/acme/awesome-repo.git")),
            "github"
        );
    }
}
