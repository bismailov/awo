use crate::config::AppConfig;
use crate::error::AwoResult;
use crate::events::DomainEvent;
use crate::runtime::{RuntimeKind, SessionLaunchMode, sync_session};
use crate::skills::{SkillLinkMode, SkillRuntime};
use crate::slot::SlotStrategy;
use crate::store::Store;
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

mod context;
mod repo;
mod review;
mod session;
mod skills;
mod slot;

pub(super) struct FreshSlotOptions<'a> {
    pub repo_id: &'a str,
    pub repo_root: &'a Path,
    pub worktree_root: &'a str,
    pub base_branch: &'a str,
    pub task_name: &'a str,
    pub strategy: SlotStrategy,
    pub fingerprint_hash: Option<String>,
}

pub(super) struct SessionStartOptions {
    pub slot_id: String,
    pub runtime: RuntimeKind,
    pub prompt: String,
    pub read_only: bool,
    pub dry_run: bool,
    pub launch_mode: SessionLaunchMode,
    pub attach_context: bool,
}

#[derive(Debug, Clone)]
pub enum Command {
    NoOp {
        label: String,
    },
    RepoAdd {
        path: PathBuf,
    },
    RepoClone {
        remote_url: String,
        destination: Option<PathBuf>,
    },
    RepoFetch {
        repo_id: String,
    },
    RepoList,
    ContextPack {
        repo_id: String,
    },
    ContextDoctor {
        repo_id: String,
    },
    SkillsList {
        repo_id: String,
    },
    SkillsDoctor {
        repo_id: String,
        runtime: Option<SkillRuntime>,
    },
    SkillsLink {
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    },
    SkillsSync {
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    },
    SlotAcquire {
        repo_id: String,
        task_name: String,
        strategy: SlotStrategy,
    },
    SlotList {
        repo_id: Option<String>,
    },
    SlotRelease {
        slot_id: String,
    },
    SlotRefresh {
        slot_id: String,
    },
    SessionStart {
        slot_id: String,
        runtime: RuntimeKind,
        prompt: String,
        read_only: bool,
        dry_run: bool,
        launch_mode: SessionLaunchMode,
        attach_context: bool,
    },
    SessionList {
        repo_id: Option<String>,
    },
    SessionCancel {
        session_id: String,
    },
    SessionDelete {
        session_id: String,
    },
    ReviewStatus {
        repo_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandOutcome {
    pub summary: String,
    pub events: Vec<DomainEvent>,
}

pub struct CommandRunner<'a> {
    pub(super) config: &'a AppConfig,
    pub(super) store: &'a Store,
}

impl<'a> CommandRunner<'a> {
    pub fn new(config: &'a AppConfig, store: &'a Store) -> Self {
        Self { config, store }
    }

    pub fn sync_runtime_state(&self, repo_id: Option<&str>) -> AwoResult<()> {
        let mut sessions = self.store.list_sessions(repo_id)?;
        for session in &mut sessions {
            if sync_session(&self.config.paths, session)? {
                self.store.upsert_session(session)?;
            }
        }
        Ok(())
    }

    pub fn run(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        match command {
            Command::NoOp { label } => self.run_noop(label).map_err(Into::into),
            Command::RepoAdd { path } => self.run_repo_add(path).map_err(Into::into),
            Command::RepoClone {
                remote_url,
                destination,
            } => self
                .run_repo_clone(remote_url, destination)
                .map_err(Into::into),
            Command::RepoFetch { repo_id } => self.run_repo_fetch(repo_id).map_err(Into::into),
            Command::RepoList => self.run_repo_list().map_err(Into::into),
            Command::ContextPack { repo_id } => self.run_context_pack(repo_id).map_err(Into::into),
            Command::ContextDoctor { repo_id } => {
                self.run_context_doctor(repo_id).map_err(Into::into)
            }
            Command::SkillsList { repo_id } => self.run_skills_list(repo_id).map_err(Into::into),
            Command::SkillsDoctor { repo_id, runtime } => {
                self.run_skills_doctor(repo_id, runtime).map_err(Into::into)
            }
            Command::SkillsLink {
                repo_id,
                runtime,
                mode,
            } => self
                .run_skills_link(repo_id, runtime, mode)
                .map_err(Into::into),
            Command::SkillsSync {
                repo_id,
                runtime,
                mode,
            } => self
                .run_skills_sync(repo_id, runtime, mode)
                .map_err(Into::into),
            Command::SlotAcquire {
                repo_id,
                task_name,
                strategy,
            } => self
                .run_slot_acquire(repo_id, task_name, strategy)
                .map_err(Into::into),
            Command::SlotList { repo_id } => self.run_slot_list(repo_id).map_err(Into::into),
            Command::SlotRelease { slot_id } => self.run_slot_release(slot_id).map_err(Into::into),
            Command::SlotRefresh { slot_id } => self.run_slot_refresh(slot_id).map_err(Into::into),
            Command::SessionStart {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context,
            } => self
                .run_session_start(SessionStartOptions {
                    slot_id,
                    runtime,
                    prompt,
                    read_only,
                    dry_run,
                    launch_mode,
                    attach_context,
                })
                .map_err(Into::into),
            Command::SessionList { repo_id } => self.run_session_list(repo_id).map_err(Into::into),
            Command::SessionCancel { session_id } => {
                self.run_session_cancel(session_id).map_err(Into::into)
            }
            Command::SessionDelete { session_id } => {
                self.run_session_delete(session_id).map_err(Into::into)
            }
            Command::ReviewStatus { repo_id } => {
                self.run_review_status(repo_id).map_err(Into::into)
            }
        }
    }

    fn run_noop(&mut self, label: String) -> Result<CommandOutcome> {
        let command_name = "noop";
        let payload = format!("label={label}");
        self.store.insert_action(command_name, &payload)?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: command_name.to_string(),
            },
            DomainEvent::NoOpCompleted {
                label: label.clone(),
                config_dir: self.config.paths.config_dir.display().to_string(),
                state_db_path: self.config.paths.state_db_path.display().to_string(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Executed no-op command for `{label}`."),
            events,
        })
    }
}
