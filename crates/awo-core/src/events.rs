use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    CommandReceived {
        command: String,
    },
    NoOpCompleted {
        label: String,
        config_dir: String,
        state_db_path: String,
    },
    RepoRegistered {
        id: String,
        name: String,
        repo_root: String,
        default_base_branch: String,
        worktree_root: String,
    },
    RepoListLoaded {
        count: usize,
    },
    ContextLoaded {
        repo_id: String,
        entrypoints: usize,
        packs: usize,
    },
    ContextDoctorCompleted {
        repo_id: String,
        errors: usize,
        warnings: usize,
    },
    SkillsCatalogLoaded {
        repo_id: String,
        skills: usize,
    },
    SkillsDoctorCompleted {
        repo_id: String,
        runtimes: usize,
        warnings: usize,
    },
    SkillsLinked {
        repo_id: String,
        runtime: String,
        linked: usize,
    },
    SkillsSynced {
        repo_id: String,
        runtime: String,
        linked: usize,
    },
    SlotAcquired {
        slot_id: String,
        repo_id: String,
        branch_name: String,
        slot_path: String,
        strategy: String,
    },
    SlotListLoaded {
        count: usize,
    },
    SlotReleased {
        slot_id: String,
        strategy: String,
    },
    SlotRefreshed {
        slot_id: String,
        dirty: bool,
        fingerprint_status: String,
    },
    SessionContextPrepared {
        slot_id: String,
        files: usize,
        packs: Vec<String>,
    },
    SessionStarted {
        session_id: String,
        slot_id: String,
        runtime: String,
        supervisor: Option<String>,
        status: String,
    },
    SessionCancelled {
        session_id: String,
        slot_id: String,
    },
    SessionDeleted {
        session_id: String,
    },
    SessionListLoaded {
        count: usize,
    },
    ReviewStatusLoaded {
        dirty: usize,
        stale: usize,
    },
    SessionLogLoaded {
        session_id: String,
        stream: String,
        lines_returned: usize,
        log_path: String,
        content: String,
    },
    TeamArchived {
        team_id: String,
    },
    TeamReset {
        team_id: String,
        tasks_reset: usize,
        slots_unbound: usize,
    },
    TeamTaskStarted {
        team_id: String,
        task_id: String,
        routing_reason: String,
    },
    TeamListLoaded {
        repo_id: Option<String>,
        count: usize,
    },
    TeamLoaded {
        team_id: String,
    },
    TeamCreated {
        team_id: String,
        repo_id: String,
    },
    TeamMemberAdded {
        team_id: String,
        member_id: String,
    },
    TeamTaskAdded {
        team_id: String,
        task_id: String,
    },
    TeamReportGenerated {
        team_id: String,
        report_path: String,
    },
    TeamDeleted {
        team_id: String,
    },
}

impl DomainEvent {
    pub fn to_message(&self) -> String {
        match self {
            Self::CommandReceived { command } => format!("Command received: {command}"),
            Self::NoOpCompleted {
                label,
                config_dir,
                state_db_path,
            } => format!(
                "No-op finished for `{label}`. Config: {config_dir}. State DB: {state_db_path}"
            ),
            Self::RepoRegistered {
                id,
                name,
                repo_root,
                default_base_branch,
                worktree_root,
            } => format!(
                "Registered repo `{name}` ({id}) at {repo_root}. Base branch: {default_base_branch}. Worktrees: {worktree_root}"
            ),
            Self::RepoListLoaded { count } => format!("Loaded {count} registered repo(s)."),
            Self::ContextLoaded {
                repo_id,
                entrypoints,
                packs,
            } => format!(
                "Loaded context for repo `{repo_id}`: {entrypoints} entrypoint(s), {packs} pack(s)"
            ),
            Self::ContextDoctorCompleted {
                repo_id,
                errors,
                warnings,
            } => format!(
                "Context doctor for repo `{repo_id}` finished with {errors} error(s) and {warnings} warning(s)"
            ),
            Self::SkillsCatalogLoaded { repo_id, skills } => {
                format!("Loaded {skills} shared skill(s) for repo `{repo_id}`")
            }
            Self::SkillsDoctorCompleted {
                repo_id,
                runtimes,
                warnings,
            } => format!(
                "Skills doctor for repo `{repo_id}` finished across {runtimes} runtime(s) with {warnings} warning(s)"
            ),
            Self::SkillsLinked {
                repo_id,
                runtime,
                linked,
            } => format!("Linked {linked} skill(s) for repo `{repo_id}` into `{runtime}`"),
            Self::SkillsSynced {
                repo_id,
                runtime,
                linked,
            } => format!("Synced {linked} skill(s) for repo `{repo_id}` into `{runtime}`"),
            Self::SlotAcquired {
                slot_id,
                repo_id,
                branch_name,
                slot_path,
                strategy,
            } => format!(
                "Acquired {strategy} slot `{slot_id}` for repo `{repo_id}` on branch `{branch_name}` at {slot_path}"
            ),
            Self::SlotListLoaded { count } => format!("Loaded {count} slot(s)."),
            Self::SlotReleased { slot_id, strategy } => {
                format!("Released {strategy} slot `{slot_id}`")
            }
            Self::SlotRefreshed {
                slot_id,
                dirty,
                fingerprint_status,
            } => format!(
                "Refreshed slot `{slot_id}`. dirty={dirty} fingerprint={fingerprint_status}"
            ),
            Self::SessionContextPrepared {
                slot_id,
                files,
                packs,
            } => format!(
                "Prepared launch context for slot `{slot_id}` with {files} file(s) and packs [{}]",
                if packs.is_empty() {
                    "-".to_string()
                } else {
                    packs.join(", ")
                }
            ),
            Self::SessionStarted {
                session_id,
                slot_id,
                runtime,
                supervisor,
                status,
            } => {
                format!(
                    "Session `{session_id}` for slot `{slot_id}` using `{runtime}`{} is {status}",
                    supervisor
                        .as_deref()
                        .map(|value| format!(" via `{value}`"))
                        .unwrap_or_default()
                )
            }
            Self::SessionCancelled {
                session_id,
                slot_id,
            } => format!("Session `{session_id}` for slot `{slot_id}` was cancelled"),
            Self::SessionDeleted { session_id } => {
                format!("Session `{session_id}` was deleted from local state")
            }
            Self::SessionListLoaded { count } => format!("Loaded {count} session(s)."),
            Self::ReviewStatusLoaded { dirty, stale } => {
                format!("Review status: {dirty} dirty slot(s), {stale} stale slot(s)")
            }
            Self::SessionLogLoaded {
                session_id,
                stream,
                lines_returned,
                ..
            } => {
                format!("Loaded {lines_returned} line(s) of {stream} for session `{session_id}`")
            }
            Self::TeamArchived { team_id } => {
                format!("Team `{team_id}` archived")
            }
            Self::TeamReset {
                team_id,
                tasks_reset,
                slots_unbound,
            } => {
                format!(
                    "Team `{team_id}` reset to planning: {tasks_reset} task(s) reset, {slots_unbound} slot binding(s) cleared"
                )
            }
            Self::TeamTaskStarted {
                team_id,
                task_id,
                routing_reason,
            } => {
                format!("Team task `{task_id}` started on team `{team_id}`: {routing_reason}")
            }
            Self::TeamListLoaded { repo_id, count } => {
                format!(
                    "Loaded {count} team manifest(s){}",
                    repo_id
                        .as_ref()
                        .map(|id| format!(" for repo `{id}`"))
                        .unwrap_or_default()
                )
            }
            Self::TeamLoaded { team_id } => {
                format!("Loaded team `{team_id}`")
            }
            Self::TeamCreated { team_id, repo_id } => {
                format!("Created team `{team_id}` for repo `{repo_id}`")
            }
            Self::TeamMemberAdded { team_id, member_id } => {
                format!("Added member `{member_id}` to team `{team_id}`")
            }
            Self::TeamTaskAdded { team_id, task_id } => {
                format!("Added task `{task_id}` to team `{team_id}`")
            }
            Self::TeamReportGenerated {
                team_id,
                report_path,
            } => {
                format!("Generated report for team `{team_id}` at `{report_path}`")
            }
            Self::TeamDeleted { team_id } => {
                format!("Deleted team `{team_id}`")
            }
        }
    }
}
