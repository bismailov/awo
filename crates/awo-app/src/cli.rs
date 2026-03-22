use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "awo", version, about = "Agent workspace orchestrator")]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<AppCommand>,
}

#[derive(Debug, Subcommand)]
pub enum AppCommand {
    Tui,
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    Slot {
        #[command(subcommand)]
        command: SlotCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    Debug {
        #[command(subcommand)]
        command: DebugCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    Add {
        path: String,
    },
    Clone {
        remote_url: String,
        destination: Option<String>,
    },
    Fetch {
        repo_id: String,
    },
    List,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    Pack { repo_id: String },
    Doctor { repo_id: String },
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    List {
        repo_id: String,
    },
    Doctor {
        repo_id: String,
        #[arg(long)]
        runtime: Option<String>,
    },
    Link {
        repo_id: String,
        runtime: String,
        #[arg(long)]
        mode: Option<String>,
    },
    Sync {
        repo_id: String,
        runtime: String,
        #[arg(long)]
        mode: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum RuntimeCommand {
    List,
    Show {
        runtime: String,
    },
    RoutePreview {
        #[arg(long)]
        primary: String,
        #[arg(long)]
        primary_model: Option<String>,
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
    },
}

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    Init {
        repo_id: String,
        team_id: String,
        objective: String,
        #[arg(long)]
        lead_runtime: Option<String>,
        #[arg(long)]
        lead_model: Option<String>,
        #[arg(long, default_value = "external_slots")]
        execution_mode: String,
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
        #[arg(long)]
        force: bool,
    },
    List,
    Show {
        team_id: String,
    },
    Recommend {
        team_id: String,
        #[arg(long)]
        member: Option<String>,
        #[arg(long)]
        task: Option<String>,
    },
    Member {
        #[command(subcommand)]
        command: TeamMemberCommand,
    },
    Task {
        #[command(subcommand)]
        command: TeamTaskCommand,
    },
    /// Archive a team whose tasks have all reached terminal states.
    Archive {
        team_id: String,
    },
    /// Reset a team to planning state, clearing all task progress and slot bindings.
    Reset {
        team_id: String,
        /// Skip confirmation and proceed even if work will be discarded.
        #[arg(long)]
        force: bool,
    },
    /// Cancel cancellable sessions, release slots, and reset the team to planning.
    Teardown {
        team_id: String,
        /// Skip preview output and execute the teardown when no blockers remain.
        #[arg(long)]
        force: bool,
    },
    /// Delete a team manifest once it no longer references active workspace state.
    Delete {
        team_id: String,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum TeamMemberCommand {
    Add {
        team_id: String,
        member_id: String,
        role: String,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value = "external_slots")]
        execution_mode: String,
        #[arg(long)]
        read_only: bool,
        #[arg(long)]
        write_scope: Vec<String>,
        #[arg(long)]
        context_pack: Vec<String>,
        #[arg(long)]
        skill: Vec<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        fallback_runtime: Option<String>,
        #[arg(long)]
        fallback_model: Option<String>,
    },
    Remove {
        team_id: String,
        member_id: String,
    },
    AssignSlot {
        team_id: String,
        member_id: String,
        slot_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TeamTaskCommand {
    Add {
        team_id: String,
        task_id: String,
        owner_id: String,
        title: String,
        summary: String,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        read_only: bool,
        #[arg(long)]
        write_scope: Vec<String>,
        #[arg(long)]
        deliverable: String,
        #[arg(long)]
        verification: Vec<String>,
        #[arg(long)]
        depends_on: Vec<String>,
    },
    State {
        team_id: String,
        task_id: String,
        state: String,
    },
    BindSlot {
        team_id: String,
        task_id: String,
        slot_id: String,
    },
    Start {
        team_id: String,
        task_id: String,
        #[arg(long, default_value = "fresh")]
        strategy: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        launch_mode: Option<String>,
        #[arg(long)]
        no_auto_context: bool,
        #[arg(long)]
        prefer_local: bool,
        #[arg(long)]
        avoid_metered: bool,
        #[arg(long)]
        max_cost_tier: Option<String>,
        #[arg(long)]
        no_fallback: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum SlotCommand {
    Acquire {
        repo_id: String,
        task_name: String,
        #[arg(long, default_value = "fresh")]
        strategy: String,
    },
    List {
        #[arg(long)]
        repo_id: Option<String>,
    },
    Release {
        slot_id: String,
    },
    Refresh {
        slot_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    Start {
        slot_id: String,
        runtime: String,
        prompt: String,
        #[arg(long)]
        read_only: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        launch_mode: Option<String>,
        #[arg(long)]
        no_auto_context: bool,
    },
    List {
        #[arg(long)]
        repo_id: Option<String>,
    },
    Cancel {
        session_id: String,
    },
    Delete {
        session_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    Status {
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
