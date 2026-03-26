use super::{CommandOutcome, CommandRunner};
use crate::error::AwoResult;
use crate::events::DomainEvent;
use crate::git;
use crate::repo::{default_clone_destination, register_repo};
use std::path::{Path, PathBuf};

impl<'a> CommandRunner<'a> {
    pub(super) fn run_repo_add(&mut self, path: PathBuf) -> AwoResult<CommandOutcome> {
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

        Ok(CommandOutcome::with_events(
            format!("Registered repo `{}` at {}.", repo.name, repo.repo_root),
            events,
        ))
    }

    pub(super) fn run_repo_remove(&mut self, repo_id: String) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;

        // Safety: refuse removal if active slots exist for this repo.
        let slots = self.store.list_slots(Some(&repo_id))?;
        let active_slots: Vec<_> = slots
            .iter()
            .filter(|s| s.status == crate::slot::SlotStatus::Active)
            .collect();
        if !active_slots.is_empty() {
            crate::awo_bail!(
                "cannot remove repo `{}`: {} active slot(s) still reference it — release them first",
                repo_id,
                active_slots.len()
            );
        }

        // Safety: refuse removal if any non-terminal sessions exist for this repo.
        let sessions = self.store.list_sessions(Some(&repo_id))?;
        let active_sessions: Vec<_> = sessions.iter().filter(|s| !s.is_terminal()).collect();
        if !active_sessions.is_empty() {
            crate::awo_bail!(
                "cannot remove repo `{}`: {} active session(s) still reference it — cancel them first",
                repo_id,
                active_sessions.len()
            );
        }

        // Safety: refuse removal if any teams reference this repo.
        let team_paths = crate::team::list_team_manifest_paths(&self.config.paths)?;
        let mut referencing_teams = Vec::new();
        for path in team_paths {
            if let Ok(manifest) = crate::team::load_team_manifest(&path)
                && manifest.repo_id == repo_id
            {
                referencing_teams.push(manifest.team_id);
            }
        }
        if !referencing_teams.is_empty() {
            crate::awo_bail!(
                "cannot remove repo `{}`: it is still referenced by team(s): {} — delete or reset those teams first",
                repo_id,
                referencing_teams.join(", ")
            );
        }

        self.store.delete_repository(&repo_id)?;
        self.store.insert_action(
            "repo_remove",
            &format!("repo_id={} name={}", repo.id, repo.name),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_remove".to_string(),
            },
            DomainEvent::RepoRemoved {
                id: repo.id.clone(),
                name: repo.name.clone(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Removed repo `{}` ({}).", repo.name, repo.id),
            events,
        ))
    }

    pub(super) fn run_repo_clone(
        &mut self,
        remote_url: String,
        destination: Option<PathBuf>,
    ) -> AwoResult<CommandOutcome> {
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

        Ok(CommandOutcome::with_events(
            format!(
                "Cloned `{remote_url}` into {} and registered repo `{}`.",
                destination.display(),
                repo.id
            ),
            events,
        ))
    }

    pub(super) fn run_repo_fetch(&mut self, repo_id: String) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;
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

        Ok(CommandOutcome::with_events(
            format!("Fetched and refreshed repo `{}`.", repo.id),
            events,
        ))
    }

    pub(super) fn run_repo_list(&mut self) -> AwoResult<CommandOutcome> {
        let repos = self.store.list_repositories()?;
        self.store
            .insert_action("repo_list", &format!("count={}", repos.len()))?;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "repo_list".to_string(),
            },
            DomainEvent::RepoListLoaded { count: repos.len() },
        ];

        Ok(CommandOutcome::with_events(
            format!("Loaded {} registered repo(s).", repos.len()),
            events,
        ))
    }
}
