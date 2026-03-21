#[derive(Debug, Clone)]
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
                status,
            } => {
                format!("Session `{session_id}` for slot `{slot_id}` using `{runtime}` is {status}")
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
        }
    }
}
