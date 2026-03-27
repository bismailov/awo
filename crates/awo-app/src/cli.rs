use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "awo",
    version,
    about = "Agent workspace orchestrator",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<AppCommand>,
}

#[derive(Debug, Subcommand)]
pub enum AppCommand {
    /// Launch the interactive TUI dashboard (default).
    Tui,
    /// Manage the background daemon (awod).
    #[cfg(unix)]
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    /// Manage registered repositories.
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Inspect and verify repository context for agents.
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Manage agent skills and their runtime integration.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    /// Inspect runtimes and manage routing policies.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Manage agent teams, members, and multi-step missions.
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Manage worktree slots for isolated task execution.
    Slot {
        #[command(subcommand)]
        command: SlotCommand,
    },
    /// Manage AI agent sessions and logs.
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// View workspace review summary and warnings.
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    /// Print extended manual and help information.
    Help {
        /// Print the full extended manual.
        #[arg(long)]
        manual: bool,
    },
    /// Internal debug utilities.
    Debug {
        #[command(subcommand)]
        command: DebugCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    /// Register a local Git repository.
    Add {
        /// Local filesystem path to the repository.
        path: String,
    },
    /// Clone a remote repository and register it.
    Clone {
        /// Remote Git URL.
        remote_url: String,
        /// Optional local directory name.
        destination: Option<String>,
    },
    /// Unregister a repository and clean up its associated data.
    Remove {
        /// ID of the repository to remove.
        repo_id: String,
    },
    /// Fetch latest changes from the remote for a registered repo.
    Fetch {
        /// ID of the repository.
        repo_id: String,
    },
    /// List all registered repositories.
    List,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// Preview the context files an agent would see for this repo.
    Pack { repo_id: String },
    /// Run diagnostic checks on repository context health.
    Doctor { repo_id: String },
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// List skills discovered in the repository.
    List { repo_id: String },
    /// Check if skills are correctly linked into runtimes.
    Doctor {
        repo_id: String,
        /// Filter by specific runtime.
        #[arg(long)]
        runtime: Option<String>,
    },
    /// Link repo skills into a runtime's library path.
    Link {
        repo_id: String,
        /// Target runtime (e.g. "claude").
        runtime: String,
        /// Linking mode (link or copy).
        #[arg(long)]
        mode: Option<String>,
    },
    /// Synchronize and repair skill links for a runtime.
    Sync {
        repo_id: String,
        /// Target runtime.
        runtime: String,
        /// Linking mode.
        #[arg(long)]
        mode: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum RuntimeCommand {
    /// List all supported runtimes and their capabilities.
    List,
    /// Show detailed capabilities for a specific runtime.
    Show { runtime: String },
    /// Preview routing decisions based on policies and pressure.
    RoutePreview {
        /// Preferred runtime kind.
        #[arg(long)]
        primary: String,
        /// Preferred model name.
        #[arg(long)]
        primary_model: Option<String>,
        /// Fallback runtime kind.
        #[arg(long)]
        fallback_runtime: Option<String>,
        /// Fallback model name.
        #[arg(long)]
        fallback_model: Option<String>,
        /// Prefer local (non-metered) runtimes.
        #[arg(long)]
        prefer_local: bool,
        /// Avoid runtimes with usage-based costs.
        #[arg(long)]
        avoid_metered: bool,
        /// Maximum allowed cost tier.
        #[arg(long)]
        max_cost_tier: Option<String>,
        /// Disable all fallback attempts.
        #[arg(long)]
        no_fallback: bool,
        /// Simulated runtime pressure (e.g. "claude=overloaded").
        #[arg(long = "pressure")]
        pressure: Vec<String>,
    },
    /// Manage runtime pressure profiles (used for automatic routing).
    Pressure {
        #[command(subcommand)]
        command: RuntimePressureCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum RuntimePressureCommand {
    /// Set a pressure level for a specific runtime.
    Set {
        runtime_kind: String,
        pressure_level: String,
    },
    /// Clear pressure level for a runtime.
    Clear { runtime_kind: String },
    /// List all active pressure levels.
    List,
}

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    /// Initialize a new agent team mission.
    Init {
        /// Target repository ID.
        repo_id: String,
        /// Unique ID for this team.
        team_id: String,
        /// Core objective or mission for the team.
        objective: String,
        /// Runtime for the team lead.
        #[arg(long)]
        lead_runtime: Option<String>,
        /// Model for the team lead.
        #[arg(long)]
        lead_model: Option<String>,
        /// How worktrees are managed (e.g. external_slots).
        #[arg(long, default_value = "external_slots")]
        execution_mode: String,
        /// Fallback runtime for all members.
        #[arg(long)]
        fallback_runtime: Option<String>,
        /// Fallback model for all members.
        #[arg(long)]
        fallback_model: Option<String>,
        /// Global 'prefer local' policy.
        #[arg(long)]
        prefer_local: bool,
        /// Global 'avoid metered' policy.
        #[arg(long)]
        avoid_metered: bool,
        /// Global max cost tier.
        #[arg(long)]
        max_cost_tier: Option<String>,
        /// Global no-fallback policy.
        #[arg(long)]
        no_fallback: bool,
        /// Overwrite existing team manifest.
        #[arg(long)]
        force: bool,
    },
    /// List all active teams.
    List {
        /// Filter by specific repository.
        #[arg(long)]
        repo_id: Option<String>,
    },
    /// Show detailed status and tasks for a team.
    Show { team_id: String },
    /// Get routing recommendations for team members.
    Recommend {
        team_id: String,
        /// Filter by specific member.
        #[arg(long)]
        member: Option<String>,
        /// Filter by specific task.
        #[arg(long)]
        task: Option<String>,
        /// Simulated pressure levels.
        #[arg(long = "pressure")]
        pressure: Vec<String>,
    },
    /// Manage team members.
    Member {
        #[command(subcommand)]
        command: TeamMemberCommand,
    },
    /// Manage team tasks.
    Task {
        #[command(subcommand)]
        command: TeamTaskCommand,
    },
    /// Archive a team whose tasks have all reached terminal states.
    Archive { team_id: String },
    /// Reset a team to planning state, clearing all task progress and slot bindings.
    Reset {
        team_id: String,
        /// Skip confirmation and proceed even if work will be discarded.
        #[arg(long)]
        force: bool,
    },
    /// Generate a comprehensive report of team activity and outcomes.
    Report { team_id: String },
    /// Cancel cancellable sessions, release slots, and reset the team to planning.
    Teardown {
        team_id: String,
        /// Skip preview output and execute the teardown when no blockers remain.
        #[arg(long)]
        force: bool,
    },
    /// Delete a team manifest once it no longer references active workspace state.
    Delete { team_id: String },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum TeamMemberCommand {
    /// Show detailed configuration for a team member.
    Show { team_id: String, member_id: String },
    /// Add a new member to the team.
    Add {
        team_id: String,
        member_id: String,
        /// Role of the member (e.g. worker, lead, reviewer).
        role: String,
        /// Preferred runtime kind.
        #[arg(long)]
        runtime: Option<String>,
        /// Preferred model name.
        #[arg(long)]
        model: Option<String>,
        /// Worktree management mode.
        #[arg(long, default_value = "external_slots")]
        execution_mode: String,
        /// Disable write access for this member.
        #[arg(long)]
        read_only: bool,
        /// Files or directories this member is allowed to modify.
        #[arg(long)]
        write_scope: Vec<String>,
        /// Context packs to always include for this member.
        #[arg(long)]
        context_pack: Vec<String>,
        /// Skills to prioritize for this member.
        #[arg(long)]
        skill: Vec<String>,
        /// Personal instructions or context for this member.
        #[arg(long)]
        notes: Option<String>,
        /// Member-specific fallback runtime.
        #[arg(long)]
        fallback_runtime: Option<String>,
        /// Member-specific fallback model.
        #[arg(long)]
        fallback_model: Option<String>,
        /// Member-specific 'prefer local' policy.
        #[arg(long)]
        prefer_local: bool,
        /// Member-specific 'avoid metered' policy.
        #[arg(long)]
        avoid_metered: bool,
        /// Member-specific max cost tier.
        #[arg(long)]
        max_cost_tier: Option<String>,
        /// Member-specific no-fallback policy.
        #[arg(long)]
        no_fallback: bool,
    },
    /// Update policies or runtimes for an existing member.
    Update {
        team_id: String,
        member_id: String,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        fallback_runtime: Option<String>,
        #[arg(long)]
        fallback_model: Option<String>,
        #[arg(long)]
        prefer_local: bool,
        #[arg(long)]
        avoid_metered: bool,
        #[arg(long)]
        max_cost_tier: Option<String>,
        #[arg(long)]
        no_fallback: bool,
        /// Remove the fallback runtime/model from this member.
        #[arg(long)]
        clear_fallback: bool,
        /// Remove all member-specific routing preferences.
        #[arg(long)]
        clear_routing_defaults: bool,
    },
    /// Remove a member from the team.
    Remove { team_id: String, member_id: String },
    /// Manually assign a worktree slot to a member.
    AssignSlot {
        team_id: String,
        member_id: String,
        slot_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TeamTaskCommand {
    /// Add a new task to the team manifest.
    Add {
        team_id: String,
        task_id: String,
        /// ID of the member who will own this task.
        owner_id: String,
        /// Short title of the task.
        title: String,
        /// Detailed summary of work to be performed.
        summary: String,
        /// Task-specific runtime override.
        #[arg(long)]
        runtime: Option<String>,
        /// Ensure no modifications are made during this task.
        #[arg(long)]
        read_only: bool,
        /// Files or directories relevant to this task.
        #[arg(long)]
        write_scope: Vec<String>,
        /// Description of the expected output.
        #[arg(long)]
        deliverable: String,
        /// How to verify the work (tests, logs, etc).
        #[arg(long)]
        verification: Vec<String>,
        /// IDs of tasks that must complete first.
        #[arg(long)]
        depends_on: Vec<String>,
    },
    /// Manually override the state of a task.
    State {
        team_id: String,
        task_id: String,
        /// Target state (e.g. todo, in_progress, review, done, blocked).
        state: String,
    },
    /// Manually bind a worktree slot to a task.
    BindSlot {
        team_id: String,
        task_id: String,
        slot_id: String,
    },
    /// Start execution of a team task (acquires slot and starts session).
    Start {
        team_id: String,
        task_id: String,
        /// Worktree reuse strategy (fresh or warm).
        #[arg(long, default_value = "fresh")]
        strategy: String,
        /// Prepare but do not actually launch the AI session.
        #[arg(long)]
        dry_run: bool,
        /// Execution environment (oneshot or pty).
        #[arg(long)]
        launch_mode: Option<String>,
        /// Do not automatically attach repository context.
        #[arg(long)]
        no_auto_context: bool,
        /// Task-specific 'prefer local' policy.
        #[arg(long)]
        prefer_local: bool,
        /// Task-specific 'avoid metered' policy.
        #[arg(long)]
        avoid_metered: bool,
        /// Task-specific max cost tier.
        #[arg(long)]
        max_cost_tier: Option<String>,
        /// Task-specific no-fallback policy.
        #[arg(long)]
        no_fallback: bool,
    },
    /// Delegate an existing task to a new owner with specific notes.
    Delegate {
        team_id: String,
        task_id: String,
        target_member_id: String,
        /// Instructions for the new owner.
        #[arg(long)]
        notes: Option<String>,
        /// Specific files the new owner should focus on.
        #[arg(long)]
        focus_file: Vec<String>,
        /// Automatically start the task after delegation.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        auto_start: bool,
        /// Keep the delegated task in planning state without starting a session.
        #[arg(long, conflicts_with = "auto_start")]
        no_auto_start: bool,
        #[arg(long, default_value = "fresh")]
        strategy: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        launch_mode: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SlotCommand {
    /// Acquire a worktree slot for a specific task.
    Acquire {
        repo_id: String,
        /// Unique name for the task (used for worktree branch).
        task_name: String,
        /// Reuse strategy (fresh or warm).
        #[arg(long, default_value = "fresh")]
        strategy: String,
    },
    /// List all worktree slots.
    List {
        /// Filter by specific repository.
        #[arg(long)]
        repo_id: Option<String>,
    },
    /// Release a slot and remove its associated worktree.
    Release { slot_id: String },
    /// Re-evaluate slot state (dirty checks, fingerprints).
    Refresh { slot_id: String },
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    /// Start a new AI or shell session in a slot.
    Start {
        slot_id: String,
        /// Runtime kind (e.g. claude, shell).
        runtime: String,
        /// The instruction or command to run.
        prompt: String,
        /// Prevent modifications to the filesystem.
        #[arg(long)]
        read_only: bool,
        /// Record the intent but do not launch the process.
        #[arg(long)]
        dry_run: bool,
        /// Launch mode (oneshot or pty).
        #[arg(long)]
        launch_mode: Option<String>,
        /// Skip automatic context attachment.
        #[arg(long)]
        no_auto_context: bool,
        /// Maximum execution time in seconds.
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// List all active and terminal sessions.
    List {
        /// Filter by specific repository.
        #[arg(long)]
        repo_id: Option<String>,
    },
    /// Attempt to cancel a running session.
    Cancel { session_id: String },
    /// Delete a terminal session record and its logs.
    Delete { session_id: String },
    /// View or stream logs for a session.
    Log {
        session_id: String,
        /// Number of lines to show.
        #[arg(long, default_value = "50")]
        lines: usize,
        /// Log stream to show (stdout, stderr, combined).
        #[arg(long, default_value = "stdout")]
        stream: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Show a summary of workspace health, dirty files, and warnings.
    Status {
        /// Filter by repository.
        #[arg(long)]
        repo_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum DebugCommand {
    Noop {
        #[arg(long, default_value = "cli-debug")]
        label: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    /// Start the daemon in the foreground.
    Start,
    /// Stop a running daemon.
    Stop,
    /// Check the status of the daemon.
    Status,
}
