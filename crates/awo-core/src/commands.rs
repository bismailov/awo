use crate::config::AppConfig;
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::runtime::{RuntimeKind, SessionLaunchMode, sync_session};
use crate::skills::{SkillLinkMode, SkillRuntime};
use crate::slot::{SlotStatus, SlotStrategy};
use crate::store::Store;
use crate::team::{TaskCard, TeamMember, TeamTaskDelegateOptions, TeamTaskStartOptions};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod context;
mod repo;
mod review;
mod session;
mod skills;
mod slot;
mod team;

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
    pub timeout_secs: Option<i64>,
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
    #[serde(rename = "repo.remove")]
    RepoRemove { repo_id: String },
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
        timeout_secs: Option<i64>,
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
    #[serde(rename = "team.list")]
    TeamList { repo_id: Option<String> },
    #[serde(rename = "team.show")]
    TeamShow { team_id: String },
    #[serde(rename = "team.init")]
    TeamInit {
        team_id: String,
        repo_id: String,
        objective: String,
        lead_runtime: Option<String>,
        lead_model: Option<String>,
        execution_mode: String,
        fallback_runtime: Option<String>,
        fallback_model: Option<String>,
        routing_preferences: Option<crate::routing::RoutingPreferences>,
        force: bool,
    },
    #[serde(rename = "team.member.add")]
    TeamMemberAdd { team_id: String, member: TeamMember },
    #[serde(rename = "team.task.add")]
    TeamTaskAdd { team_id: String, task: TaskCard },
    #[serde(rename = "team.task.start")]
    TeamTaskStart { options: TeamTaskStartOptions },
    #[serde(rename = "team.task.delegate")]
    TeamTaskDelegate { options: TeamTaskDelegateOptions },
    #[serde(rename = "team.reset")]
    TeamReset { team_id: String, force: bool },
    #[serde(rename = "team.report")]
    TeamReport { team_id: String },
    #[serde(rename = "team.archive")]
    TeamArchive { team_id: String, force: bool },
    #[serde(rename = "team.teardown")]
    TeamTeardown { team_id: String, force: bool },
    #[serde(rename = "team.delete")]
    TeamDelete { team_id: String },
    #[serde(rename = "events.poll")]
    EventsPoll {
        since_seq: Option<u64>,
        limit: Option<usize>,
    },
}

impl Command {
    /// Returns the JSON-RPC method name for this command variant.
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::NoOp { .. } => "noop",
            Self::RepoAdd { .. } => "repo.add",
            Self::RepoClone { .. } => "repo.clone",
            Self::RepoRemove { .. } => "repo.remove",
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
            Self::TeamList { .. } => "team.list",
            Self::TeamShow { .. } => "team.show",
            Self::TeamInit { .. } => "team.init",
            Self::TeamMemberAdd { .. } => "team.member.add",
            Self::TeamTaskAdd { .. } => "team.task.add",
            Self::TeamTaskStart { .. } => "team.task.start",
            Self::TeamTaskDelegate { .. } => "team.task.delegate",
            Self::TeamReset { .. } => "team.reset",
            Self::TeamReport { .. } => "team.report",
            Self::TeamArchive { .. } => "team.archive",
            Self::TeamTeardown { .. } => "team.teardown",
            Self::TeamDelete { .. } => "team.delete",
            Self::EventsPoll { .. } => "events.poll",
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
    pub data: Option<serde_json::Value>,
}

impl CommandOutcome {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            events: vec![],
            data: None,
        }
    }

    pub fn with_events(summary: impl Into<String>, events: Vec<DomainEvent>) -> Self {
        Self {
            summary: summary.into(),
            events,
            data: None,
        }
    }

    pub fn with_data(summary: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            summary: summary.into(),
            events: vec![],
            data: Some(data),
        }
    }

    pub fn with_all(
        summary: impl Into<String>,
        events: Vec<DomainEvent>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            summary: summary.into(),
            events,
            data: Some(data),
        }
    }
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
            Command::RepoRemove { repo_id } => self.run_repo_remove(repo_id),
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
                timeout_secs,
            } => self.run_session_start(SessionStartOptions {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context,
                timeout_secs,
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
            Command::TeamList { repo_id } => self.run_team_list(repo_id),
            Command::TeamShow { team_id } => self.run_team_show(team_id),
            Command::TeamInit {
                team_id,
                repo_id,
                objective,
                lead_runtime,
                lead_model,
                execution_mode,
                fallback_runtime,
                fallback_model,
                routing_preferences,
                force,
            } => self.run_team_init(
                team_id,
                repo_id,
                objective,
                lead_runtime,
                lead_model,
                execution_mode,
                fallback_runtime,
                fallback_model,
                routing_preferences,
                force,
            ),
            Command::TeamMemberAdd { team_id, member } => self.run_team_member_add(team_id, member),
            Command::TeamTaskAdd { team_id, task } => self.run_team_task_add(team_id, task),
            Command::TeamTaskStart { options } => self.run_team_task_start(options),
            Command::TeamTaskDelegate { options } => self.run_team_task_delegate(options),
            Command::TeamReset { team_id, force } => self.run_team_reset(team_id, force),
            Command::TeamReport { team_id } => self.run_team_report(team_id),
            Command::TeamArchive { team_id, force } => self.run_team_archive(team_id, force),
            Command::TeamTeardown { team_id, force } => self.run_team_teardown(team_id, force),
            Command::TeamDelete { team_id } => self.run_team_delete(team_id),
            Command::EventsPoll { .. } => {
                // Handled at the AppCore level (requires EventBus access).
                Ok(CommandOutcome::new(
                    "events.poll requires event bus context",
                ))
            }
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

        Ok(CommandOutcome::with_events(
            format!("Executed no-op command for `{label}`."),
            events,
        ))
    }
}

#[cfg(test)]
mod tests;
