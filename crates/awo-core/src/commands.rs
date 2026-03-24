use crate::config::AppConfig;
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::runtime::{RuntimeKind, SessionLaunchMode, sync_session};
use crate::skills::{SkillLinkMode, SkillRuntime};
use crate::slot::{SlotStatus, SlotStrategy};
use crate::store::Store;
use serde::{Deserialize, Serialize};
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

/// A command that can be dispatched to the orchestration core.
///
/// Each variant maps to a JSON-RPC method name (e.g. `"slot.acquire"`),
/// enabling transport-agnostic dispatch for both in-process CLI usage
/// and future daemon-based RPC execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Command {
    #[serde(rename = "noop")]
    NoOp { label: String },
    #[serde(rename = "repo.add")]
    RepoAdd { path: PathBuf },
    #[serde(rename = "repo.clone")]
    RepoClone {
        remote_url: String,
        destination: Option<PathBuf>,
    },
    #[serde(rename = "repo.fetch")]
    RepoFetch { repo_id: String },
    #[serde(rename = "repo.list")]
    RepoList,
    #[serde(rename = "context.pack")]
    ContextPack { repo_id: String },
    #[serde(rename = "context.doctor")]
    ContextDoctor { repo_id: String },
    #[serde(rename = "skills.list")]
    SkillsList { repo_id: String },
    #[serde(rename = "skills.doctor")]
    SkillsDoctor {
        repo_id: String,
        runtime: Option<SkillRuntime>,
    },
    #[serde(rename = "skills.link")]
    SkillsLink {
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    },
    #[serde(rename = "skills.sync")]
    SkillsSync {
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    },
    #[serde(rename = "slot.acquire")]
    SlotAcquire {
        repo_id: String,
        task_name: String,
        strategy: SlotStrategy,
    },
    #[serde(rename = "slot.list")]
    SlotList { repo_id: Option<String> },
    #[serde(rename = "slot.release")]
    SlotRelease { slot_id: String },
    #[serde(rename = "slot.refresh")]
    SlotRefresh { slot_id: String },
    #[serde(rename = "session.start")]
    SessionStart {
        slot_id: String,
        runtime: RuntimeKind,
        prompt: String,
        read_only: bool,
        dry_run: bool,
        launch_mode: SessionLaunchMode,
        attach_context: bool,
    },
    #[serde(rename = "session.list")]
    SessionList { repo_id: Option<String> },
    #[serde(rename = "session.cancel")]
    SessionCancel { session_id: String },
    #[serde(rename = "session.delete")]
    SessionDelete { session_id: String },
    #[serde(rename = "session.log")]
    SessionLog {
        session_id: String,
        lines: Option<usize>,
        stream: Option<String>,
    },
    #[serde(rename = "review.status")]
    ReviewStatus { repo_id: Option<String> },
}

impl Command {
    /// Returns the JSON-RPC method name for this command variant.
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::NoOp { .. } => "noop",
            Self::RepoAdd { .. } => "repo.add",
            Self::RepoClone { .. } => "repo.clone",
            Self::RepoFetch { .. } => "repo.fetch",
            Self::RepoList => "repo.list",
            Self::ContextPack { .. } => "context.pack",
            Self::ContextDoctor { .. } => "context.doctor",
            Self::SkillsList { .. } => "skills.list",
            Self::SkillsDoctor { .. } => "skills.doctor",
            Self::SkillsLink { .. } => "skills.link",
            Self::SkillsSync { .. } => "skills.sync",
            Self::SlotAcquire { .. } => "slot.acquire",
            Self::SlotList { .. } => "slot.list",
            Self::SlotRelease { .. } => "slot.release",
            Self::SlotRefresh { .. } => "slot.refresh",
            Self::SessionStart { .. } => "session.start",
            Self::SessionList { .. } => "session.list",
            Self::SessionCancel { .. } => "session.cancel",
            Self::SessionDelete { .. } => "session.delete",
            Self::SessionLog { .. } => "session.log",
            Self::ReviewStatus { .. } => "review.status",
        }
    }

    /// Reconstruct a `Command` from a JSON-RPC method name and params.
    ///
    /// This leverages the adjacently-tagged serde representation: the
    /// `method` and `params` fields from a JSON-RPC request map directly
    /// to the serde envelope for `Command`.
    pub fn from_rpc(method: &str, params: serde_json::Value) -> Result<Self, serde_json::Error> {
        let envelope = serde_json::json!({
            "method": method,
            "params": params,
        });
        serde_json::from_value(envelope)
    }
}

/// The result of executing a command, including a human-readable summary
/// and structured domain events suitable for JSON-RPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub(super) fn repo_not_found_error(&self, repo_id: &str) -> AwoError {
        let hint = self
            .store
            .list_repositories()
            .ok()
            .map(|repos| {
                if repos.is_empty() {
                    "no repos registered; run `awo repo add <path>` first".to_string()
                } else {
                    let ids: Vec<_> = repos.iter().map(|r| r.id.as_str()).collect();
                    format!("registered repos: {}", ids.join(", "))
                }
            })
            .unwrap_or_default();
        if hint.is_empty() {
            AwoError::unknown_repo(repo_id)
        } else {
            AwoError::validation(format!("unknown repo id `{repo_id}`; {hint}"))
        }
    }

    pub(super) fn slot_not_found_error(&self, slot_id: &str) -> AwoError {
        let hint = self
            .store
            .list_slots(None)
            .ok()
            .map(|slots| {
                let active: Vec<_> = slots
                    .iter()
                    .filter(|s| s.status == SlotStatus::Active)
                    .map(|s| s.id.as_str())
                    .collect();
                if active.is_empty() {
                    "no active slots; run `awo slot acquire <repo_id> <task>` first".to_string()
                } else {
                    format!("active slots: {}", active.join(", "))
                }
            })
            .unwrap_or_default();
        if hint.is_empty() {
            AwoError::unknown_slot(slot_id)
        } else {
            AwoError::validation(format!("unknown slot id `{slot_id}`; {hint}"))
        }
    }

    pub(super) fn session_not_found_error(&self, session_id: &str) -> AwoError {
        let hint = self
            .store
            .list_sessions(None)
            .ok()
            .map(|sessions| {
                if sessions.is_empty() {
                    "no sessions exist; start one with `awo session start <slot_id> <runtime> <prompt>`".to_string()
                } else {
                    let ids: Vec<_> = sessions.iter().take(5).map(|s| s.id.as_str()).collect();
                    format!("recent sessions: {}", ids.join(", "))
                }
            })
            .unwrap_or_default();
        if hint.is_empty() {
            AwoError::unknown_session(session_id)
        } else {
            AwoError::validation(format!("unknown session id `{session_id}`; {hint}"))
        }
    }

    pub fn run(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        match command {
            Command::NoOp { label } => self.run_noop(label),
            Command::RepoAdd { path } => self.run_repo_add(path),
            Command::RepoClone {
                remote_url,
                destination,
            } => self.run_repo_clone(remote_url, destination),
            Command::RepoFetch { repo_id } => self.run_repo_fetch(repo_id),
            Command::RepoList => self.run_repo_list(),
            Command::ContextPack { repo_id } => self.run_context_pack(repo_id),
            Command::ContextDoctor { repo_id } => self.run_context_doctor(repo_id),
            Command::SkillsList { repo_id } => self.run_skills_list(repo_id),
            Command::SkillsDoctor { repo_id, runtime } => self.run_skills_doctor(repo_id, runtime),
            Command::SkillsLink {
                repo_id,
                runtime,
                mode,
            } => self.run_skills_link(repo_id, runtime, mode),
            Command::SkillsSync {
                repo_id,
                runtime,
                mode,
            } => self.run_skills_sync(repo_id, runtime, mode),
            Command::SlotAcquire {
                repo_id,
                task_name,
                strategy,
            } => self.run_slot_acquire(repo_id, task_name, strategy),
            Command::SlotList { repo_id } => self.run_slot_list(repo_id),
            Command::SlotRelease { slot_id } => self.run_slot_release(slot_id),
            Command::SlotRefresh { slot_id } => self.run_slot_refresh(slot_id),
            Command::SessionStart {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context,
            } => self.run_session_start(SessionStartOptions {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context,
            }),
            Command::SessionList { repo_id } => self.run_session_list(repo_id),
            Command::SessionCancel { session_id } => self.run_session_cancel(session_id),
            Command::SessionDelete { session_id } => self.run_session_delete(session_id),
            Command::SessionLog {
                session_id,
                lines,
                stream,
            } => self.run_session_log(session_id, lines, stream),
            Command::ReviewStatus { repo_id } => self.run_review_status(repo_id),
        }
    }

    fn run_noop(&mut self, label: String) -> AwoResult<CommandOutcome> {
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

#[cfg(test)]
mod tests;
