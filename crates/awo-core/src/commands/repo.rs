use super::{CommandOutcome, CommandRunner};
use crate::events::DomainEvent;
use crate::git;
use crate::repo::{default_clone_destination, register_repo};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

impl<'a> CommandRunner<'a> {
    pub(super) fn run_repo_add(&mut self, path: PathBuf) -> Result<CommandOutcome> {
        let git = git::discover_repo(&path)?;
        let result = register_repo(&self.config.paths, path.clone(), git)?;
        self.store.upsert_repository(&result.registered_repo)?;
        self.store.insert_action(
            "repo_add",
            &format!(
                "path={} repo_id={} overlay={}",
                path.display(),
                result.registered_repo.id,
                result.overlay_path.display()
            ),
        )?;

        let repo = result.registered_repo;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_add".to_string(),
            },
            DomainEvent::RepoRegistered {
                id: repo.id.clone(),
                name: repo.name.clone(),
                repo_root: repo.repo_root.clone(),
                default_base_branch: repo.default_base_branch.clone(),
                worktree_root: repo.worktree_root.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Registered repo `{}` at {}.", repo.name, repo.repo_root),
            events,
        })
    }

    pub(super) fn run_repo_clone(
        &mut self,
        remote_url: String,
        destination: Option<PathBuf>,
    ) -> Result<CommandOutcome> {
        let destination = destination
            .unwrap_or_else(|| default_clone_destination(&self.config.paths, &remote_url));
        git::clone_repo(&remote_url, &destination)?;

        let git = git::discover_repo(&destination)?;
        let result = register_repo(&self.config.paths, destination.clone(), git)?;
        self.store.upsert_repository(&result.registered_repo)?;
        self.store.insert_action(
            "repo_clone",
            &format!(
                "remote_url={} destination={} repo_id={}",
                remote_url,
                destination.display(),
                result.registered_repo.id
            ),
        )?;

        let repo = result.registered_repo;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_clone".to_string(),
            },
            DomainEvent::RepoRegistered {
                id: repo.id.clone(),
                name: repo.name.clone(),
                repo_root: repo.repo_root.clone(),
                default_base_branch: repo.default_base_branch.clone(),
                worktree_root: repo.worktree_root.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!(
                "Cloned `{remote_url}` into {} and registered repo `{}`.",
                destination.display(),
                repo.id
            ),
            events,
        })
    }

    pub(super) fn run_repo_fetch(&mut self, repo_id: String) -> Result<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .with_context(|| format!("unknown repo id `{repo_id}`"))?;
        git::fetch_repo(Path::new(&repo.repo_root))?;
        let git = git::discover_repo(Path::new(&repo.repo_root))?;
        let result = register_repo(&self.config.paths, PathBuf::from(&repo.repo_root), git)?;
        self.store.upsert_repository(&result.registered_repo)?;
        self.store.insert_action(
            "repo_fetch",
            &format!("repo_id={} repo_root={}", repo.id, repo.repo_root),
        )?;

        let repo = result.registered_repo;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_fetch".to_string(),
            },
            DomainEvent::RepoRegistered {
                id: repo.id.clone(),
                name: repo.name.clone(),
                repo_root: repo.repo_root.clone(),
                default_base_branch: repo.default_base_branch.clone(),
                worktree_root: repo.worktree_root.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Fetched and refreshed repo `{}`.", repo.id),
            events,
        })
    }

    pub(super) fn run_repo_list(&mut self) -> Result<CommandOutcome> {
        let repos = self.store.list_repositories()?;
        self.store
            .insert_action("repo_list", &format!("count={}", repos.len()))?;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_list".to_string(),
            },
            DomainEvent::RepoListLoaded { count: repos.len() },
        ];

        Ok(CommandOutcome {
            summary: format!("Loaded {} registered repo(s).", repos.len()),
            events,
        })
    }
}
