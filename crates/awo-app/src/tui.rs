mod action_router;
mod forms;
mod keymap;
mod widgets;

use anyhow::Result;
use awo_core::{
    AppCore, AppSnapshot, Command, DomainEvent, MemberRoutingSummary, PlanItemState, RepoSummary,
    RoutingPreferencesSummary, RuntimeCapabilityDescriptor, SessionEndReason, SessionLaunchMode,
    SessionStatus, SessionSummary, SessionTerminalInput, SlotSummary, TaskCardState, TeamManifest,
    TeamSummary as CoreTeamSummary, TeamTaskStartOptions,
};
use crossbeam_channel::Sender;
use crossterm::event::{self, Event as CEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Wrap,
};
use std::io::{self, IsTerminal, Read};
use std::thread;
use std::time::Duration;
use tracing::info;

const SNAPSHOT_FALLBACK_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const EVENT_REFRESH_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const TERMINAL_CAPTURE_LINES: usize = 500;
const TERMINAL_SCROLL_PAGE: u16 = 12;

pub(crate) struct BackgroundResult {
    summary: String,
    events: Vec<DomainEvent>,
    error: Option<String>,
}

pub(crate) struct SnapshotRefreshResult {
    snapshot: Option<AppSnapshot>,
    teams: Vec<TeamManifest>,
    error: Option<String>,
}

struct EventRefreshTrigger {
    head_seq: u64,
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiFocus {
    Repos,
    Teams,
    Slots,
    Sessions,
    Terminal,
    TeamDashboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalLayoutMode {
    Docked,
    Workspace,
    Focus,
}

impl TerminalLayoutMode {
    fn next(self) -> Self {
        match self {
            Self::Docked => Self::Workspace,
            Self::Workspace => Self::Focus,
            Self::Focus => Self::Docked,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Docked => "Docked",
            Self::Workspace => "Workspace",
            Self::Focus => "Focus",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InputMode {
    Normal,
    TextInput {
        prompt_label: String,
        buffer: String,
        on_submit: InputAction,
    },
    Form(forms::FormState),
    Confirm(forms::ConfirmState),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InputAction {
    AcquireSlot,
    StartSession,
    SetFilter,
    SetTerminalSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamDashboardFocus {
    Team,
    Plan,
    Member,
    Task,
}

#[derive(Debug)]
pub(crate) struct TeamDashboardState {
    pub selected_team_idx: usize,
    pub selected_plan_idx: usize,
    pub selected_member_idx: usize,
    pub selected_task_idx: usize,
    pub teams: Vec<TeamManifest>,
    pub focus: TeamDashboardFocus,
}

#[derive(Debug)]
pub(crate) struct TuiState {
    status: String,
    messages: Vec<String>,
    focus: TuiFocus,
    selected_repo_index: usize,
    selected_team_index: usize,
    selected_slot_index: usize,
    selected_session_index: usize,
    log_content: Option<String>,
    log_session_id: Option<String>,
    log_path: Option<String>,
    show_log_panel: bool,
    terminal_content: Option<String>,
    terminal_session_id: Option<String>,
    show_terminal_panel: bool,
    terminal_input_mode: bool,
    terminal_scroll: u16,
    terminal_search_query: Option<String>,
    terminal_follow_output: bool,
    terminal_layout: TerminalLayoutMode,
    pending_ops: usize,
    input_mode: InputMode,
    show_help: bool,
    log_scroll: u16,
    filter_query: Option<String>,
    team_dashboard: TeamDashboardState,
    last_snapshot: Option<AppSnapshot>,
    last_snapshot_time: Option<std::time::Instant>,
    snapshot_refresh_in_flight: bool,
}

struct TerminalWorkspaceData<'a> {
    visible_slots: &'a [&'a SlotSummary],
    visible_sessions: &'a [&'a SessionSummary],
    visible_teams: &'a [&'a CoreTeamSummary],
}

pub fn run_tui() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        let mut input = String::new();
        let _ = io::stdin().read_to_string(&mut input);
        if input.contains('q') {
            return Ok(());
        }
        anyhow::bail!("TUI requires an interactive terminal");
    }

    let mut core = AppCore::bootstrap()?;
    let outcome = core
        .dispatch(Command::ReviewStatus { repo_id: None })
        .or_else(|_| {
            core.dispatch(Command::NoOp {
                label: "tui-startup".to_string(),
            })
        })?;

    let mut state = TuiState {
        status: outcome.summary.clone(),
        messages: outcome
            .events
            .into_iter()
            .map(|event| event.to_message())
            .collect(),
        focus: TuiFocus::Repos,
        selected_repo_index: 0,
        selected_team_index: 0,
        selected_slot_index: 0,
        selected_session_index: 0,
        log_content: None,
        log_session_id: None,
        log_path: None,
        show_log_panel: false,
        terminal_content: None,
        terminal_session_id: None,
        show_terminal_panel: false,
        terminal_input_mode: false,
        terminal_scroll: 0,
        terminal_search_query: None,
        terminal_follow_output: true,
        terminal_layout: TerminalLayoutMode::Docked,
        pending_ops: 0,
        input_mode: InputMode::Normal,
        show_help: false,
        log_scroll: 0,
        filter_query: None,
        team_dashboard: TeamDashboardState {
            selected_team_idx: 0,
            selected_plan_idx: 0,
            selected_member_idx: 0,
            selected_task_idx: 0,
            teams: Vec::new(),
            focus: TeamDashboardFocus::Team,
        },
        last_snapshot: None,
        last_snapshot_time: None,
        snapshot_refresh_in_flight: false,
    };

    let (tx, rx) = crossbeam_channel::unbounded::<BackgroundResult>();
    let (snapshot_tx, snapshot_rx) = crossbeam_channel::unbounded::<SnapshotRefreshResult>();
    let (event_refresh_tx, event_refresh_rx) =
        crossbeam_channel::unbounded::<EventRefreshTrigger>();

    state.last_snapshot = Some(core.snapshot()?);
    state.last_snapshot_time = Some(std::time::Instant::now());
    refresh_team_dashboard_data(core.paths(), &mut state);
    request_event_refresh_on_new_events(
        core.event_bus().clone(),
        core.event_bus().head_seq(),
        event_refresh_tx,
    );

    info!("TUI started");

    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        while let Ok(result) = rx.try_recv() {
            if state.pending_ops > 0 {
                state.pending_ops -= 1;
            }
            if let Some(error) = result.error {
                state.status = format!("Error: {error}");
            } else {
                state.status = result.summary;
                for event in &result.events {
                    state.messages.push(event.to_message());
                }
            }
            // Invalidate cache on background result
            state.last_snapshot_time = None;
        }

        while let Ok(result) = snapshot_rx.try_recv() {
            apply_snapshot_refresh_result(&mut state, result);
        }

        let mut event_refresh_requested = false;
        let mut latest_event_head_seq = None;
        while let Ok(trigger) = event_refresh_rx.try_recv() {
            latest_event_head_seq = Some(trigger.head_seq);
            event_refresh_requested = true;
        }

        if event_refresh_requested && !state.snapshot_refresh_in_flight {
            state.status = match latest_event_head_seq {
                Some(head_seq) => {
                    format!("Broker activity detected (event seq {head_seq}); refreshing snapshot.")
                }
                None => "Broker activity detected; refreshing snapshot.".to_string(),
            };
            state.snapshot_refresh_in_flight = true;
            request_snapshot_refresh(core.paths().clone(), snapshot_tx.clone());
        }

        let needs_refresh = state.last_snapshot.is_none()
            || state
                .last_snapshot_time
                .is_none_or(|t| t.elapsed() > SNAPSHOT_FALLBACK_REFRESH_INTERVAL);

        if needs_refresh && !state.snapshot_refresh_in_flight {
            state.snapshot_refresh_in_flight = true;
            request_snapshot_refresh(core.paths().clone(), snapshot_tx.clone());
        }

        let Some(snapshot) = state.last_snapshot.clone() else {
            continue;
        };
        clamp_selection(&mut state, &snapshot);

        if state.show_log_panel
            && let Some(session_id) = state.log_session_id.clone()
        {
            let session_running = visible_sessions(&snapshot, &state).iter().any(|s| {
                s.id == session_id && s.status == awo_core::runtime::SessionStatus::Running
            });
            if session_running {
                fetch_session_log(&mut core, &mut state, &session_id);
                state.last_snapshot_time = None; // Force refresh after log fetch
            }
        }

        if state.show_terminal_panel
            && let Some(session_id) = state.terminal_session_id.clone()
        {
            if let Some(session) = snapshot
                .sessions
                .iter()
                .find(|session| session.id == session_id)
            {
                if session.embedded_terminal_supported
                    && session.status == awo_core::runtime::SessionStatus::Running
                    && state.terminal_follow_output
                {
                    fetch_session_terminal(&mut core, &mut state, &session_id);
                } else if state.terminal_input_mode {
                    state.terminal_input_mode = false;
                    state.status = format!(
                        "Session `{session_id}` is no longer interactive; terminal switched to view mode."
                    );
                }
            } else {
                state.show_terminal_panel = false;
                state.terminal_input_mode = false;
                state.terminal_content = None;
                state.terminal_session_id = None;
                state.status = format!(
                    "Attached terminal session `{session_id}` is no longer present in the snapshot."
                );
            }
        }

        terminal.draw(|frame| render(frame, &snapshot, &state))?;

        if event::poll(Duration::from_millis(200))?
            && let CEvent::Key(key) = event::read()?
            && matches!(
                action_router::handle_key_event(
                    &mut core,
                    &mut state,
                    &snapshot,
                    key.code,
                    tx.clone()
                ),
                action_router::KeyOutcome::Quit
            )
        {
            break;
        }
    }

    let _ = terminal.show_cursor();
    Ok(())
}

pub(crate) fn selected_repo<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Option<&'a RepoSummary> {
    let repos = visible_repos(snapshot, state);
    repos
        .get(state.selected_repo_index.min(repos.len().saturating_sub(1)))
        .copied()
}

fn matches_filter(query: Option<&str>, text: &str) -> bool {
    query.is_none_or(|q| text.to_lowercase().contains(&q.to_lowercase()))
}

fn visible_repos<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Vec<&'a RepoSummary> {
    let q = state.filter_query.as_deref();
    snapshot
        .registered_repos
        .iter()
        .filter(|r| matches_filter(q, &r.id) || matches_filter(q, &r.name))
        .collect()
}

pub(crate) fn visible_teams<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Vec<&'a CoreTeamSummary> {
    let selected_repo = selected_repo(snapshot, state);
    let q = state.filter_query.as_deref();
    snapshot
        .teams
        .iter()
        .filter(|team| {
            selected_repo.is_none_or(|repo| team.repo_id == repo.id)
                && (matches_filter(q, &team.team_id) || matches_filter(q, &team.objective))
        })
        .collect()
}

pub(crate) fn selected_team<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Option<&'a CoreTeamSummary> {
    let teams = visible_teams(snapshot, state);
    teams.get(state.selected_team_index).copied()
}

pub(crate) fn visible_slots<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Vec<&'a SlotSummary> {
    let selected_repo = selected_repo(snapshot, state);
    let q = state.filter_query.as_deref();
    snapshot
        .slots
        .iter()
        .filter(|slot| {
            selected_repo.is_none_or(|repo| repo.id == slot.repo_id)
                && (matches_filter(q, &slot.id) || matches_filter(q, &slot.task_name))
        })
        .collect()
}

pub(crate) fn selected_slot<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Option<&'a SlotSummary> {
    let slots = visible_slots(snapshot, state);
    slots.get(state.selected_slot_index).copied()
}

pub(crate) fn visible_sessions<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Vec<&'a SessionSummary> {
    let selected_repo = selected_repo(snapshot, state);
    let q = state.filter_query.as_deref();
    snapshot
        .sessions
        .iter()
        .filter(|session| {
            selected_repo.is_none_or(|repo| {
                snapshot
                    .slots
                    .iter()
                    .find(|slot| slot.id == session.slot_id)
                    .map(|slot| slot.repo_id == repo.id)
                    .unwrap_or(false)
            }) && (matches_filter(q, &session.id) || matches_filter(q, &session.runtime))
        })
        .collect()
}

pub(crate) fn selected_session<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Option<&'a SessionSummary> {
    let sessions = visible_sessions(snapshot, state);
    sessions.get(state.selected_session_index).copied()
}

fn terminal_attached_session<'a>(
    snapshot: &'a AppSnapshot,
    state: &TuiState,
) -> Option<&'a SessionSummary> {
    state.terminal_session_id.as_deref().and_then(|session_id| {
        snapshot
            .sessions
            .iter()
            .find(|session| session.id == session_id)
    })
}

pub(crate) fn clamp_selection(state: &mut TuiState, snapshot: &AppSnapshot) {
    let repo_count = visible_repos(snapshot, state).len();
    state.selected_repo_index = if repo_count > 0 {
        state.selected_repo_index.min(repo_count - 1)
    } else {
        0
    };

    let team_count = visible_teams(snapshot, state).len();
    state.selected_team_index = if team_count > 0 {
        state.selected_team_index.min(team_count - 1)
    } else {
        0
    };

    let slot_count = visible_slots(snapshot, state).len();
    state.selected_slot_index = if slot_count > 0 {
        state.selected_slot_index.min(slot_count - 1)
    } else {
        0
    };

    let session_count = visible_sessions(snapshot, state).len();
    state.selected_session_index = if session_count > 0 {
        state.selected_session_index.min(session_count - 1)
    } else {
        0
    };

    let dashboard_team_count = state.team_dashboard.teams.len();
    state.team_dashboard.selected_team_idx = if dashboard_team_count > 0 {
        state
            .team_dashboard
            .selected_team_idx
            .min(dashboard_team_count - 1)
    } else {
        0
    };

    let member_count = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
        .map(|team| team.members.len() + 1)
        .unwrap_or(0);
    let plan_count = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
        .map(|team| team.plan_items.len())
        .unwrap_or(0);
    state.team_dashboard.selected_plan_idx = if plan_count > 0 {
        state.team_dashboard.selected_plan_idx.min(plan_count - 1)
    } else {
        0
    };
    state.team_dashboard.selected_member_idx = if member_count > 0 {
        state
            .team_dashboard
            .selected_member_idx
            .min(member_count - 1)
    } else {
        0
    };

    let task_count = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
        .map(|team| team.tasks.len())
        .unwrap_or(0);
    state.team_dashboard.selected_task_idx = if task_count > 0 {
        state.team_dashboard.selected_task_idx.min(task_count - 1)
    } else {
        0
    };
}

pub(crate) fn apply_command(core: &mut AppCore, state: &mut TuiState, command: Command) {
    match core.dispatch(command) {
        Ok(outcome) => {
            state.status = outcome.summary;
            append_events(state, outcome.events);
            // Invalidate snapshot cache
            state.last_snapshot_time = None;
        }
        Err(error) => {
            let message = format!("Error: {error:#}");
            state.status = message.clone();
            state.messages.push(message);
        }
    }
}

pub(crate) fn append_events(state: &mut TuiState, events: Vec<DomainEvent>) {
    state
        .messages
        .extend(events.into_iter().map(|event| event.to_message()));
    if state.messages.len() > 30 {
        let overflow = state.messages.len() - 30;
        state.messages.drain(0..overflow);
    }
}

pub(crate) fn fetch_session_log(core: &mut AppCore, state: &mut TuiState, session_id: &str) {
    match core.dispatch(Command::SessionLog {
        session_id: session_id.to_string(),
        lines: Some(100),
        stream: None,
    }) {
        Ok(outcome) => {
            for event in &outcome.events {
                if let DomainEvent::SessionLogLoaded {
                    content,
                    log_path,
                    session_id,
                    ..
                } = event
                {
                    state.log_content = Some(content.clone());
                    state.log_session_id = Some(session_id.clone());
                    state.log_path = Some(log_path.clone());
                    state.show_log_panel = true;
                    state.show_terminal_panel = false;
                    state.terminal_input_mode = false;
                }
            }
            append_events(state, outcome.events);
        }
        Err(error) => {
            state.status = format!("Error: {error:#}");
        }
    }
}

pub(crate) fn fetch_slot_diff(core: &mut AppCore, state: &mut TuiState, slot_id: &str) {
    match core.dispatch(Command::ReviewDiff {
        slot_id: slot_id.to_string(),
    }) {
        Ok(outcome) => {
            if let Some(data) = &outcome.data {
                let content = data.get("content").and_then(|value| value.as_str());
                let slot_id = data.get("slot_id").and_then(|value| value.as_str());
                let slot_path = data.get("slot_path").and_then(|value| value.as_str());
                if let Some(content) = content {
                    state.log_content = Some(content.to_string());
                    state.log_session_id = Some(format!("slot-diff:{}", slot_id.unwrap_or("?")));
                    state.log_path = slot_path.map(ToString::to_string);
                    state.show_log_panel = true;
                    state.show_terminal_panel = false;
                    state.terminal_input_mode = false;
                }
            }
            append_events(state, outcome.events);
        }
        Err(error) => {
            state.status = format!("Error: {error:#}");
        }
    }
}

pub(crate) fn fetch_session_terminal(core: &mut AppCore, state: &mut TuiState, session_id: &str) {
    match core.dispatch(Command::SessionTerminalCapture {
        session_id: session_id.to_string(),
        max_lines: Some(TERMINAL_CAPTURE_LINES),
    }) {
        Ok(outcome) => {
            let session_switched = state.terminal_session_id.as_deref() != Some(session_id);
            if let Some(data) = &outcome.data
                && let Some(content) = data.get("content").and_then(|value| value.as_str())
            {
                state.terminal_content = Some(content.to_string());
                state.terminal_session_id = Some(session_id.to_string());
                state.show_terminal_panel = true;
                state.show_log_panel = false;
                if session_switched {
                    state.terminal_follow_output = true;
                    state.terminal_search_query = None;
                    state.terminal_scroll = u16::MAX;
                } else if state.terminal_follow_output {
                    state.terminal_scroll = u16::MAX;
                }
            }
        }
        Err(error) => {
            state.status = format!("Error: {error:#}");
            state.terminal_input_mode = false;
        }
    }
}

pub(crate) fn send_terminal_input(
    core: &mut AppCore,
    state: &mut TuiState,
    session_id: &str,
    input: SessionTerminalInput,
) {
    match core.dispatch(Command::SessionTerminalInput {
        session_id: session_id.to_string(),
        input,
    }) {
        Ok(_) => fetch_session_terminal(core, state, session_id),
        Err(error) => state.status = format!("Error: {error:#}"),
    }
}

fn terminal_display_content(state: &TuiState) -> String {
    let Some(content) = state.terminal_content.as_deref() else {
        return "(no terminal content yet)".to_string();
    };

    let Some(query) = state.terminal_search_query.as_deref() else {
        return content.to_string();
    };

    let lowered = query.to_lowercase();
    let filtered = content
        .lines()
        .filter(|line| line.to_lowercase().contains(&lowered))
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        format!("(no terminal lines match search `{query}`)")
    } else {
        filtered.join("\n")
    }
}

pub(crate) fn open_session_surface(
    core: &mut AppCore,
    state: &mut TuiState,
    session: &SessionSummary,
) {
    if session.embedded_terminal_supported {
        fetch_session_terminal(core, state, &session.id);
        state.focus = TuiFocus::Terminal;
        state.terminal_layout = TerminalLayoutMode::Workspace;
        state.terminal_follow_output = true;
        state.status = format!(
            "Attached terminal workspace to session `{}` on slot `{}`.",
            session.id, session.slot_id
        );
    } else {
        fetch_session_log(core, state, &session.id);
        state.log_scroll = u16::MAX;
        state.focus = TuiFocus::Sessions;
        state.status = format!(
            "Session `{}` does not expose an embedded terminal; opened log view instead.",
            session.id
        );
    }
}

pub(crate) fn start_next_team_task(core: &mut AppCore, state: &mut TuiState, team_id: &str) {
    let manifest = match core.load_team_manifest(team_id) {
        Ok(m) => m,
        Err(err) => {
            state.status = format!("Error loading team: {err:#}");
            return;
        }
    };
    let next_task = manifest
        .tasks
        .iter()
        .find(|t| t.state == TaskCardState::Todo);
    let task = match next_task {
        Some(t) => t,
        None => {
            state.status = format!("Team `{team_id}` has no todo tasks.");
            return;
        }
    };
    let options = TeamTaskStartOptions {
        team_id: team_id.to_string(),
        task_id: task.task_id.clone(),
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::default_for_environment()
            .as_str()
            .to_string(),
        attach_context: true,
        routing_preferences: None,
    };
    apply_team_task_start(core, state, options);
}

fn load_team_dashboard_manifests(paths: &awo_core::app::AppPaths) -> Vec<TeamManifest> {
    let Ok(manifest_paths) = awo_core::team::list_team_manifest_paths(paths) else {
        return Vec::new();
    };

    manifest_paths
        .into_iter()
        .filter_map(|path| awo_core::team::load_team_manifest(&path).ok())
        .collect()
}

fn preserve_team_dashboard_selection(state: &mut TuiState, previous_team_id: Option<String>) {
    if let Some(team_id) = previous_team_id {
        state.team_dashboard.selected_team_idx = state
            .team_dashboard
            .teams
            .iter()
            .position(|team| team.team_id == team_id)
            .unwrap_or(0);
    }
}

fn apply_snapshot_refresh_result(state: &mut TuiState, result: SnapshotRefreshResult) {
    state.snapshot_refresh_in_flight = false;

    if let Some(error) = result.error {
        let message = format!("Error: {error}");
        state.status = message.clone();
        state.messages.push(message);
        return;
    }

    let previous_team_id = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
        .map(|team| team.team_id.clone());

    if let Some(snapshot) = result.snapshot {
        state.last_snapshot = Some(snapshot);
        state.last_snapshot_time = Some(std::time::Instant::now());
    }
    state.team_dashboard.teams = result.teams;
    preserve_team_dashboard_selection(state, previous_team_id);
}

fn request_snapshot_refresh(paths: awo_core::app::AppPaths, tx: Sender<SnapshotRefreshResult>) {
    thread::spawn(move || {
        let result = match AppCore::with_dirs(paths.config_dir.clone(), paths.data_dir.clone()) {
            Ok(bg_core) => match bg_core.snapshot() {
                Ok(snapshot) => SnapshotRefreshResult {
                    snapshot: Some(snapshot),
                    teams: load_team_dashboard_manifests(bg_core.paths()),
                    error: None,
                },
                Err(error) => SnapshotRefreshResult {
                    snapshot: None,
                    teams: Vec::new(),
                    error: Some(error.to_string()),
                },
            },
            Err(error) => SnapshotRefreshResult {
                snapshot: None,
                teams: Vec::new(),
                error: Some(format!("failed to open background core: {error}")),
            },
        };
        let _ = tx.send(result);
    });
}

fn request_event_refresh_on_new_events(
    event_bus: awo_core::EventBus,
    since_seq: u64,
    tx: Sender<EventRefreshTrigger>,
) {
    thread::spawn(move || {
        let mut last_seen_seq = since_seq;
        loop {
            let result = event_bus.wait(last_seen_seq, 1, EVENT_REFRESH_WAIT_TIMEOUT);
            if result.head_seq <= last_seen_seq || result.entries.is_empty() {
                continue;
            }

            last_seen_seq = result.head_seq;
            if tx
                .send(EventRefreshTrigger {
                    head_seq: last_seen_seq,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

pub(crate) fn refresh_team_dashboard_data(paths: &awo_core::app::AppPaths, state: &mut TuiState) {
    let previous_team_id = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
        .map(|team| team.team_id.clone());
    state.team_dashboard.teams = load_team_dashboard_manifests(paths);
    preserve_team_dashboard_selection(state, previous_team_id);
}

pub(crate) fn start_team_task_explicit(
    core: &mut AppCore,
    state: &mut TuiState,
    team_id: &str,
    task_id: &str,
) {
    let options = TeamTaskStartOptions {
        team_id: team_id.to_string(),
        task_id: task_id.to_string(),
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::default_for_environment()
            .as_str()
            .to_string(),
        attach_context: true,
        routing_preferences: None,
    };
    apply_team_task_start(core, state, options);
}

fn apply_team_task_start(core: &mut AppCore, state: &mut TuiState, options: TeamTaskStartOptions) {
    match core.dispatch(Command::TeamTaskStart { options }) {
        Ok(outcome) => {
            state.status = outcome.summary.clone();
            append_events(state, outcome.events);
            if let Some(data) = outcome.data
                && let Some(execution) = data.get("execution")
                && let Ok(execution) =
                    serde_json::from_value::<awo_core::TeamTaskExecution>(execution.clone())
            {
                state.messages.push(format!(
                    "Task `{}` started with {} on slot `{}`.",
                    execution.task_id, execution.runtime, execution.slot_id
                ));
                state.status = format!(
                    "Task `{}` started with {} on slot `{}`.",
                    execution.task_id, execution.runtime, execution.slot_id
                );
                if let Some(session_id) = execution.session_id.as_deref()
                    && let Ok(snapshot) = core.snapshot()
                    && let Some(session) = snapshot
                        .sessions
                        .iter()
                        .find(|session| session.id == session_id)
                {
                    open_session_surface(core, state, session);
                }
            }
        }
        Err(err) => {
            state.status = format!("Error: {err:#}");
            state.messages.push(state.status.clone());
        }
    }
}

fn render(frame: &mut Frame, snapshot: &AppSnapshot, state: &TuiState) {
    if state.focus == TuiFocus::TeamDashboard {
        render_team_dashboard(frame, state);
        return;
    }

    let selected_repo = selected_repo(snapshot, state);
    let visible_teams = visible_teams(snapshot, state);
    let selected_team = selected_team(snapshot, state);
    let visible_slots = visible_slots(snapshot, state);
    let visible_sessions = visible_sessions(snapshot, state);

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(11),
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ])
    .split(frame.area());

    let title = if state.pending_ops > 0 {
        format!("Status (Working: {} ops...)", state.pending_ops)
    } else {
        "Status".to_string()
    };

    let header = Paragraph::new(format!(
        "awo V1 | q quit | / search | Tab focus | s acquire | Enter start/log | e terminal | i interact | x cancel | X release | r refresh | Esc close | t next task | {}",
        state.status
    ))
    .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(header, vertical[0]);

    let paths = vec![
        Line::from(format!("Platform: {}", snapshot.platform_label)),
        Line::from(format!("Config: {}", snapshot.config_dir)),
        Line::from(format!("State DB: {}", snapshot.state_db_path)),
        Line::from(format!("Repo Overlays: {}", snapshot.repos_dir)),
        Line::from(format!("Managed Clones: {}", snapshot.clones_dir)),
        Line::from(format!("Default Worktrees: {}", snapshot.worktrees_dir)),
        Line::from(format!("Team Manifests: {}", snapshot.teams_dir)),
        Line::from(format!(
            "Review: active={} releasable={} dirty={} stale={} pending={} sessions_ok={} sessions_failed={}",
            snapshot.review.active_slots,
            snapshot.review.releasable_slots,
            snapshot.review.dirty_slots,
            snapshot.review.stale_slots,
            snapshot.review.pending_sessions,
            snapshot.review.completed_sessions,
            snapshot.review.failed_sessions
        )),
        Line::from(format!(
            "Runtimes: {}",
            snapshot
                .runtime_capabilities
                .iter()
                .map(|capability| format!(
                    "{}(launch={},subagents={},teams={})",
                    capability.runtime,
                    capability.default_launch_mode,
                    capability.inline_subagents.as_str(),
                    capability.multi_session_teams.as_str(),
                ))
                .collect::<Vec<_>>()
                .join(" ")
        )),
        Line::from(snapshot.runtime_pressure.clone()),
    ];
    let paths_widget =
        Paragraph::new(paths).block(Block::default().borders(Borders::ALL).title("Overview"));
    frame.render_widget(paths_widget, vertical[1]);

    if state.show_terminal_panel && state.terminal_layout != TerminalLayoutMode::Docked {
        render_terminal_workspace(
            frame,
            snapshot,
            state,
            vertical[2],
            vertical[3],
            TerminalWorkspaceData {
                visible_slots: &visible_slots,
                visible_sessions: &visible_sessions,
                visible_teams: &visible_teams,
            },
        );
        return;
    }

    let top = Layout::horizontal([
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(vertical[2]);
    let bottom = Layout::horizontal([
        Constraint::Percentage(24),
        Constraint::Percentage(20),
        Constraint::Percentage(26),
        Constraint::Percentage(30),
    ])
    .split(vertical[3]);

    let repo_items = if visible_repos(snapshot, state).is_empty() {
        vec![ListItem::new("(no repos - press 'a' to add)")]
    } else {
        visible_repos(snapshot, state)
            .iter()
            .enumerate()
            .map(|(index, repo)| render_repo_item(repo, index == state.selected_repo_index))
            .collect::<Vec<_>>()
    };
    let filter_suffix = state
        .filter_query
        .as_deref()
        .map_or("".to_string(), |q| format!(" (filter: {})", q));
    let repos_border_style = if state.focus == TuiFocus::Repos {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let repos = List::new(repo_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Repositories{}", filter_suffix))
            .border_style(repos_border_style),
    );
    frame.render_widget(repos, top[0]);

    let repo_detail_title = match selected_repo {
        Some(repo) => format!("Repo: {}", repo.name),
        None => "Repo (none)".to_string(),
    };
    let repo_details = render_repo_detail(
        selected_repo,
        snapshot.teams.as_slice(),
        snapshot.runtime_capabilities.as_slice(),
    );
    let repo_detail_widget = Paragraph::new(repo_details).block(
        Block::default()
            .borders(Borders::ALL)
            .title(repo_detail_title),
    );
    frame.render_widget(repo_detail_widget, top[1]);

    let team_detail_title = match selected_team {
        Some(team) => format!("Team: {}", team.team_id),
        None => "Team (none)".to_string(),
    };
    let team_detail_widget = Paragraph::new(render_team_detail(selected_team)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(team_detail_title),
    );
    frame.render_widget(team_detail_widget, top[2]);

    let slot_items: Vec<ListItem> = visible_slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let marker = if index == state.selected_slot_index {
                ">"
            } else {
                " "
            };
            ListItem::new(format!(
                "{} {} [{}] {} {} dirty={} fp={}",
                marker,
                slot.task_name,
                slot.id,
                slot.status,
                slot.strategy,
                slot.dirty,
                slot.fingerprint_status
            ))
        })
        .collect();
    let slot_items = if slot_items.is_empty() {
        vec![ListItem::new("(no slots)")]
    } else {
        slot_items
    };
    let slots_border_style = if state.focus == TuiFocus::Slots {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let slots = List::new(slot_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Slots{}", filter_suffix))
            .border_style(slots_border_style),
    );
    frame.render_widget(slots, top[3]);

    let session_items: Vec<ListItem> = visible_sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            let marker = if index == state.selected_session_index {
                ">"
            } else {
                " "
            };
            let terminal_badge = if session.embedded_terminal_supported {
                " tty"
            } else {
                ""
            };
            ListItem::new(format!(
                "{} {} [{}] {}{} read_only={} dry_run={} exit={} reason={} cap={}",
                marker,
                session.runtime,
                session.slot_id,
                session.status,
                terminal_badge,
                session.read_only,
                session.dry_run,
                session
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                session
                    .end_reason
                    .map(|reason| reason.as_str())
                    .unwrap_or("-"),
                session.capacity_status.as_str(),
            ))
        })
        .collect();
    let session_items = if session_items.is_empty() {
        vec![ListItem::new("(no sessions)")]
    } else {
        session_items
    };
    let sessions_border_style = if state.focus == TuiFocus::Sessions {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let sessions = List::new(session_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Sessions{}", filter_suffix))
            .border_style(sessions_border_style),
    );
    frame.render_widget(sessions, bottom[0]);

    let team_items: Vec<ListItem> = if visible_teams.is_empty() {
        vec![ListItem::new("(no teams)")]
    } else {
        visible_teams
            .iter()
            .enumerate()
            .map(|(index, team)| {
                let marker = if index == state.selected_team_index {
                    ">"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{} {} {} {}/{} w={}",
                    marker,
                    team.team_id,
                    team.status,
                    team.open_task_count,
                    team.task_count,
                    team.write_member_count
                ))
            })
            .collect()
    };
    let teams_border_style = if state.focus == TuiFocus::Teams {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let teams = List::new(team_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Teams{}", filter_suffix))
            .border_style(teams_border_style),
    );
    frame.render_widget(teams, bottom[1]);

    let warning_items: Vec<ListItem> = snapshot
        .review
        .warnings
        .iter()
        .rev()
        .take(10)
        .map(|warning| ListItem::new(warning.message.clone()))
        .collect();
    let warning_items = if warning_items.is_empty() {
        vec![ListItem::new("(no warnings)")]
    } else {
        warning_items
    };
    if state.show_terminal_panel {
        let terminal_area = Rect {
            x: bottom[2].x,
            y: bottom[2].y,
            width: bottom[2].width.saturating_add(bottom[3].width),
            height: bottom[2].height,
        };
        let terminal_border_style = if state.focus == TuiFocus::Terminal {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let session_id = state.terminal_session_id.as_deref().unwrap_or("?");
        let mode = if state.terminal_input_mode {
            "INTERACT"
        } else {
            "VIEW"
        };
        let search = state
            .terminal_search_query
            .as_deref()
            .map_or(String::new(), |query| format!(" | search={query}"));
        let follow = if state.terminal_follow_output {
            " | follow=on"
        } else {
            " | follow=off"
        };
        let title = format!(
            "Terminal: {session_id} [{mode}] [{}] (i interact, z layout, f follow, / search){follow}{search}",
            state.terminal_layout.label()
        );
        let terminal_widget = Paragraph::new(terminal_display_content(state))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(terminal_border_style),
            )
            .wrap(Wrap { trim: false })
            .scroll((state.terminal_scroll, 0));
        frame.render_widget(terminal_widget, terminal_area);
    } else {
        let warnings = List::new(warning_items)
            .block(Block::default().borders(Borders::ALL).title("Warnings"));
        frame.render_widget(warnings, bottom[2]);

        let message_items: Vec<ListItem> = if state.messages.is_empty() {
            vec![ListItem::new("(no events yet)")]
        } else {
            state
                .messages
                .iter()
                .rev()
                .take(12)
                .map(|message| ListItem::new(message.clone()))
                .collect()
        };
        let messages = List::new(message_items)
            .block(Block::default().borders(Borders::ALL).title("Event Log"));
        frame.render_widget(messages, bottom[3]);
    }

    if state.show_log_panel {
        let log_id = state.log_session_id.as_deref().unwrap_or("?");
        let title = if let Some(slot_id) = log_id.strip_prefix("slot-diff:") {
            format!("Review Diff: {slot_id} (Esc to close, r to refresh)")
        } else {
            let session_running = snapshot.sessions.iter().any(|s| {
                Some(&s.id) == state.log_session_id.as_ref()
                    && s.status == awo_core::runtime::SessionStatus::Running
            });
            let status_indicator = if session_running { " [running]" } else { "" };
            format!(
                "Log: {}{} (Esc to close, r to refresh)",
                log_id, status_indicator,
            )
        };
        let content = state.log_content.as_deref().unwrap_or("(empty)");
        let log_widget = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false })
            .scroll((state.log_scroll, 0));
        frame.render_widget(log_widget, vertical[3]);
    }

    // Input bar overlay
    if let InputMode::TextInput {
        prompt_label,
        buffer,
        ..
    } = &state.input_mode
    {
        let area = frame.area();
        let input_area = Rect {
            x: area.x,
            y: area.height.saturating_sub(3),
            width: area.width,
            height: 3,
        };
        let text = format!("{prompt_label}{buffer}_");
        let input_widget =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Input"));
        frame.render_widget(Clear, input_area);
        frame.render_widget(input_widget, input_area);
    }

    // Help overlay
    if let InputMode::Form(form) = &state.input_mode {
        widgets::render_form_overlay(frame, form);
    }

    if let InputMode::Confirm(confirm) = &state.input_mode {
        widgets::render_confirm_overlay(frame, confirm);
    }

    if state.show_help {
        let area = frame.area();
        let width = 50u16.min(area.width.saturating_sub(4));
        let height = 22u16.min(area.height.saturating_sub(4));
        let help_area = Rect {
            x: (area.width.saturating_sub(width)) / 2,
            y: (area.height.saturating_sub(height)) / 2,
            width,
            height,
        };
        let help_text = keymap::help_lines();
        let help_widget = Paragraph::new(help_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keybindings (press any key to close)"),
        );
        frame.render_widget(Clear, help_area);
        frame.render_widget(help_widget, help_area);
    }
}

fn render_terminal_workspace(
    frame: &mut Frame,
    snapshot: &AppSnapshot,
    state: &TuiState,
    top_area: Rect,
    bottom_area: Rect,
    data: TerminalWorkspaceData<'_>,
) {
    let workspace_area = Rect {
        x: top_area.x,
        y: top_area.y,
        width: top_area.width,
        height: top_area.height.saturating_add(bottom_area.height),
    };

    match state.terminal_layout {
        TerminalLayoutMode::Docked => {}
        TerminalLayoutMode::Workspace => {
            let layout =
                Layout::horizontal([Constraint::Percentage(26), Constraint::Percentage(74)])
                    .split(workspace_area);
            let sidebar =
                Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(layout[0]);
            let main = Layout::vertical([Constraint::Percentage(72), Constraint::Percentage(28)])
                .split(layout[1]);
            let inspector =
                Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                    .split(main[1]);

            render_workspace_session_list(frame, state, data.visible_sessions, sidebar[0]);
            render_workspace_slot_list(frame, state, data.visible_slots, sidebar[1]);
            render_terminal_panel(frame, snapshot, state, main[0], true);
            render_workspace_terminal_detail(frame, snapshot, state, inspector[0]);
            render_workspace_events(frame, snapshot, state, data.visible_teams, inspector[1]);
        }
        TerminalLayoutMode::Focus => {
            let layout = Layout::vertical([Constraint::Percentage(82), Constraint::Percentage(18)])
                .split(workspace_area);
            let footer =
                Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
                    .split(layout[1]);
            render_terminal_panel(frame, snapshot, state, layout[0], true);
            render_workspace_terminal_detail(frame, snapshot, state, footer[0]);
            render_workspace_events(frame, snapshot, state, data.visible_teams, footer[1]);
        }
    }
}

fn render_terminal_panel(
    frame: &mut Frame,
    snapshot: &AppSnapshot,
    state: &TuiState,
    area: Rect,
    show_layout_hint: bool,
) {
    let terminal_border_style = if state.focus == TuiFocus::Terminal {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let session = terminal_attached_session(snapshot, state);
    let session_id = state.terminal_session_id.as_deref().unwrap_or("?");
    let mode = if state.terminal_input_mode {
        "INTERACT"
    } else {
        "VIEW"
    };
    let search = state
        .terminal_search_query
        .as_deref()
        .map_or(String::new(), |query| format!(" | search={query}"));
    let follow = if state.terminal_follow_output {
        " | follow=on"
    } else {
        " | follow=off"
    };
    let session_status = session
        .map(|session| format!(" | status={}", session.status))
        .unwrap_or_default();
    let layout_hint = if show_layout_hint {
        format!(" [{}]", state.terminal_layout.label())
    } else {
        String::new()
    };
    let title =
        format!("Terminal: {session_id} [{mode}]{layout_hint}{session_status}{follow}{search}");
    let terminal_widget = Paragraph::new(terminal_display_content(state))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(terminal_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((state.terminal_scroll, 0));
    frame.render_widget(terminal_widget, area);
}

fn render_workspace_session_list(
    frame: &mut Frame,
    state: &TuiState,
    visible_sessions: &[&SessionSummary],
    area: Rect,
) {
    let session_items: Vec<ListItem> = if visible_sessions.is_empty() {
        vec![ListItem::new("(no sessions)")]
    } else {
        visible_sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                let marker = if index == state.selected_session_index {
                    ">"
                } else {
                    " "
                };
                let tty = if session.embedded_terminal_supported {
                    " tty"
                } else {
                    ""
                };
                ListItem::new(format!(
                    "{} {} [{}] {}{}",
                    marker, session.runtime, session.slot_id, session.status, tty
                ))
            })
            .collect()
    };
    let sessions_border_style = if state.focus == TuiFocus::Sessions {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let sessions = List::new(session_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Sessions")
            .border_style(sessions_border_style),
    );
    frame.render_widget(sessions, area);
}

fn render_workspace_slot_list(
    frame: &mut Frame,
    state: &TuiState,
    visible_slots: &[&SlotSummary],
    area: Rect,
) {
    let slot_items: Vec<ListItem> = if visible_slots.is_empty() {
        vec![ListItem::new("(no slots)")]
    } else {
        visible_slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let marker = if index == state.selected_slot_index {
                    ">"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{} {} [{}] {} {} dirty={}",
                    marker, slot.task_name, slot.id, slot.status, slot.strategy, slot.dirty
                ))
            })
            .collect()
    };
    let slots_border_style = if state.focus == TuiFocus::Slots {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let slots = List::new(slot_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Slots")
            .border_style(slots_border_style),
    );
    frame.render_widget(slots, area);
}

fn render_workspace_terminal_detail(
    frame: &mut Frame,
    snapshot: &AppSnapshot,
    state: &TuiState,
    area: Rect,
) {
    let mut lines = Vec::new();
    if let Some(session) = terminal_attached_session(snapshot, state) {
        lines.push(format!("Session: {} ({})", session.id, session.runtime));
        lines.push(format!(
            "Status: {} | slot={} | supervisor={}",
            session.status,
            session.slot_id,
            session.supervisor.as_deref().unwrap_or("-")
        ));
        lines.push(format!(
            "Mode: read_only={} dry_run={} terminal={}",
            session.read_only,
            session.dry_run,
            if session.embedded_terminal_supported {
                "supported"
            } else {
                "unsupported"
            }
        ));
        lines.push(format!(
            "Exit: {} | reason={} | capacity={}",
            session
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "-".to_string()),
            session
                .end_reason
                .map(SessionEndReason::as_str)
                .unwrap_or("-"),
            session.capacity_status.as_str(),
        ));
        if let Some(log_path) = &session.log_path {
            lines.push(format!("Log: {log_path}"));
        }
        if let Some(usage_note) = &session.usage_note {
            lines.push(format!("Usage: {usage_note}"));
        }
        if let Some(recovery_hint) = &session.recovery_hint {
            lines.push(format!("Recovery: {recovery_hint}"));
        } else if session.status.is_terminal() && session.status != SessionStatus::Completed {
            lines.push(
                "Recovery: inspect the terminal/log, then restart in a fresh slot or hand off with a focused follow-up."
                    .to_string(),
            );
        }
    } else {
        lines.push("No terminal session is attached.".to_string());
        lines.push("Press `e` on a running supported session to open the workspace.".to_string());
    }

    let detail = Paragraph::new(lines.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Terminal Detail"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, area);
}

fn render_workspace_events(
    frame: &mut Frame,
    snapshot: &AppSnapshot,
    state: &TuiState,
    visible_teams: &[&CoreTeamSummary],
    area: Rect,
) {
    let title = format!(
        "Operator Feed (teams={} warnings={})",
        visible_teams.len(),
        snapshot.review.warnings.len()
    );
    let lines = if !state.messages.is_empty() {
        state
            .messages
            .iter()
            .rev()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
    } else if !snapshot.review.warnings.is_empty() {
        snapshot
            .review
            .warnings
            .iter()
            .rev()
            .take(10)
            .map(|warning| warning.message.clone())
            .collect::<Vec<_>>()
    } else {
        vec!["(no warnings or broker events yet)".to_string()]
    };
    let feed = Paragraph::new(lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });
    frame.render_widget(feed, area);
}

fn render_team_dashboard(frame: &mut Frame, state: &TuiState) {
    let area = frame.area();
    let dashboard = state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx);

    let layout =
        Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)]).split(area);

    // Sidebar: Team List
    let team_items: Vec<ListItem> = state
        .team_dashboard
        .teams
        .iter()
        .enumerate()
        .map(|(idx, team)| {
            let style = if idx == state.team_dashboard.selected_team_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} [{}]", team.team_id, team.status)).style(style)
        })
        .collect();

    let sidebar_border_style = if state.team_dashboard.focus == TeamDashboardFocus::Team {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let sidebar = List::new(team_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Teams")
            .border_style(sidebar_border_style),
    );
    frame.render_widget(sidebar, layout[0]);

    // Main Area: Team Details
    if let Some(team) = dashboard {
        let details_layout = Layout::vertical([
            Constraint::Length(7),  // Mission / lead state
            Constraint::Length(8),  // Plan items
            Constraint::Length(8),  // Members
            Constraint::Min(8),     // Task cards
            Constraint::Length(10), // Selected detail
            Constraint::Length(3),  // Progress
        ])
        .split(layout[1]);

        // Mission
        let mission_lines = render_dashboard_mission_lines(team, state.last_snapshot.as_ref());
        let objective = Paragraph::new(mission_lines.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Mission"))
            .wrap(Wrap { trim: true });
        frame.render_widget(objective, details_layout[0]);

        // Plan Items
        let plan_border_style = if state.team_dashboard.focus == TeamDashboardFocus::Plan {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let plan_items: Vec<ListItem> = if team.plan_items.is_empty() {
            vec![ListItem::new("(no plan items)")]
        } else {
            team.plan_items
                .iter()
                .enumerate()
                .map(|(idx, plan)| {
                    let selected = state.team_dashboard.focus == TeamDashboardFocus::Plan
                        && state.team_dashboard.selected_plan_idx == idx;
                    let style = if selected {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!("{} [{}] {}", plan.plan_id, plan.state, plan.title))
                        .style(style)
                })
                .collect()
        };
        let plan_list = List::new(plan_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Plan Items")
                .border_style(plan_border_style),
        );
        frame.render_widget(plan_list, details_layout[1]);

        // Members
        let member_items: Vec<ListItem> = team
            .members
            .iter()
            .enumerate()
            .map(|(idx, m)| {
                let slot = m.slot_id.as_deref().unwrap_or("none");
                let current = if team.current_lead_member_id() == m.member_id {
                    " [current]"
                } else {
                    ""
                };
                let selected = state.team_dashboard.focus == TeamDashboardFocus::Member
                    && state.team_dashboard.selected_member_idx == idx + 1;
                let style = if selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                ListItem::new(format!(
                    "{}: {}{} (slot: {})",
                    m.member_id, m.role, current, slot
                ))
                .style(style)
            })
            .collect();
        let lead_item = ListItem::new(format!(
            "{} (lead): {}{}",
            team.lead.member_id,
            team.lead.role,
            if team.current_lead_member_id() == team.lead.member_id {
                " [current]"
            } else {
                ""
            }
        ))
        .style(
            if state.team_dashboard.focus == TeamDashboardFocus::Member
                && state.team_dashboard.selected_member_idx == 0
            {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            },
        );
        let mut all_members = vec![lead_item];
        all_members.extend(member_items);

        let members_border_style = if state.team_dashboard.focus == TeamDashboardFocus::Member {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let members_list = List::new(all_members).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Members")
                .border_style(members_border_style),
        );
        frame.render_widget(members_list, details_layout[2]);

        // Task Card Table
        let task_border_style = if state.team_dashboard.focus == TeamDashboardFocus::Task {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let header_cells = ["ID", "Owner", "Queue", "State", "Slot", "Deliverable"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells).height(1).bottom_margin(0);

        let rows = team.tasks.iter().enumerate().map(|(idx, task)| {
            let style = if idx == state.team_dashboard.selected_task_idx
                && state.team_dashboard.focus == TeamDashboardFocus::Task
            {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(task.task_id.as_str()),
                Cell::from(task.owner_id.as_str()),
                Cell::from(task_queue_role_label(task)),
                Cell::from(task.state.to_string()),
                Cell::from(task.slot_id.as_deref().unwrap_or("-")),
                Cell::from(task.deliverable.as_str()),
            ])
            .style(style)
        });

        let task_table = Table::new(
            rows,
            [
                Constraint::Percentage(14),
                Constraint::Percentage(14),
                Constraint::Percentage(18),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Task Cards")
                .border_style(task_border_style),
        );
        frame.render_widget(task_table, details_layout[3]);

        let (detail_title, detail_lines) = if state.team_dashboard.focus == TeamDashboardFocus::Plan
        {
            (
                "Plan Item Detail",
                render_dashboard_plan_detail_lines(team, state.team_dashboard.selected_plan_idx),
            )
        } else {
            (
                "Task Card Detail",
                render_dashboard_task_detail_lines(
                    team,
                    state.team_dashboard.selected_task_idx,
                    state.last_snapshot.as_ref(),
                ),
            )
        };
        let task_detail = Paragraph::new(detail_lines.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(detail_title))
            .wrap(Wrap { trim: true });
        frame.render_widget(task_detail, details_layout[4]);

        // Progress
        let total_tasks = team.tasks.len();
        let done_tasks = team.tasks.iter().filter(|t| t.state.is_closed()).count();
        let progress = if total_tasks > 0 {
            done_tasks as f64 / total_tasks as f64
        } else {
            0.0
        };

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Progress"))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(progress)
            .label(format!("{}/{} task cards closed", done_tasks, total_tasks));
        frame.render_widget(gauge, details_layout[5]);
    } else {
        let no_team = Paragraph::new("No team selected or no teams found.")
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(no_team, layout[1]);
    }
}

fn render_repo_item(repo: &RepoSummary, selected: bool) -> ListItem<'static> {
    let marker = if selected { ">" } else { " " };
    let mcp = if repo.mcp_config_present { "+" } else { "" };
    ListItem::new(format!(
        "{} {} {} {}{}",
        marker, repo.name, repo.remote_label, repo.default_base_branch, mcp,
    ))
}

fn render_repo_detail(
    repo: Option<&RepoSummary>,
    teams: &[CoreTeamSummary],
    runtime_capabilities: &[RuntimeCapabilityDescriptor],
) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![Line::from("Press 'a' to add a repository by path.")];
    };

    let mut lines = vec![
        Line::from(format!("Name: {}", repo.name)),
        Line::from(format!("Root: {}", repo.repo_root)),
        Line::from(format!("Base: {}", repo.default_base_branch)),
        Line::from(format!("Worktrees: {}", repo.worktree_root)),
        Line::from(format!("Remote: {}", repo.remote_label)),
    ];

    if repo.context_packs.is_empty() {
        lines.push(Line::from("Packs: none"));
    } else {
        lines.push(Line::from("Packs:"));
        for pack in &repo.context_packs {
            lines.push(Line::from(format!(
                "  - {} ({})",
                pack.name, pack.file_count
            )));
        }
    }

    if repo.skill_runtimes.is_empty() {
        lines.push(Line::from("Skills: none"));
    } else {
        lines.push(Line::from("Skill runtimes:"));
        for runtime in &repo.skill_runtimes {
            lines.push(Line::from(format!(
                "  - {} ready={}/{} warnings={} strategy={}",
                runtime.runtime, runtime.ready, runtime.total, runtime.warnings, runtime.strategy
            )));
        }
    }

    let repo_teams = teams
        .iter()
        .filter(|team| team.repo_id == repo.id)
        .collect::<Vec<_>>();
    if repo_teams.is_empty() {
        lines.push(Line::from("Teams: none"));
    } else {
        lines.push(Line::from("Teams:"));
        for team in repo_teams {
            lines.push(Line::from(format!(
                "  - {} status={} open={}/{}",
                team.team_id, team.status, team.open_task_count, team.task_count
            )));
        }
        lines.push(Line::from(
            "Select a team from the Teams panel for details.",
        ));
    }

    lines.push(Line::from("Runtime capabilities:"));
    for capability in runtime_capabilities {
        lines.push(Line::from(format!(
            "  - {} tier={} limit={} launch={} subagents={} teams={} skills={}",
            capability.runtime,
            capability.cost_tier.as_str(),
            capability.limit_profile.as_str(),
            capability.default_launch_mode,
            capability.inline_subagents.as_str(),
            capability.multi_session_teams.as_str(),
            capability.skill_preload.as_str()
        )));
        if let Some(note) = capability.notes.first() {
            lines.push(Line::from(format!("    {note}")));
        }
    }

    lines
}

fn render_team_detail(team: Option<&CoreTeamSummary>) -> Vec<Line<'static>> {
    let Some(team) = team else {
        return vec![Line::from(
            "Select a team from the Teams panel or press 'c' to create one.",
        )];
    };

    let mut lines = vec![
        Line::from(format!("ID: {}", team.team_id)),
        Line::from(format!("Objective: {}", team.objective)),
        Line::from(format!("Status: {}", team.status)),
        Line::from(format!(
            "Open task cards: {} of {}",
            team.open_task_count, team.task_count
        )),
    ];

    if let Some(preferences) = &team.routing_preferences {
        lines.push(Line::from(format!(
            "Team routing: {}",
            format_routing_preferences(preferences)
        )));
    }

    let has_lead = team.lead_runtime.is_some()
        || team.lead_model.is_some()
        || team.lead_fallback_runtime.is_some()
        || team.lead_fallback_model.is_some();
    if has_lead {
        lines.push(Line::from(format!(
            "Lead profile: runtime={} model={} fallback={}",
            team.lead_runtime.as_deref().unwrap_or("-"),
            team.lead_model.as_deref().unwrap_or("-"),
            format_fallback_target(
                team.lead_fallback_runtime.as_deref(),
                team.lead_fallback_model.as_deref()
            )
        )));
    }
    lines.push(Line::from(format!(
        "Current lead: {} runtime={} model={} session={} status={}",
        team.current_lead_member_id,
        team.current_lead_runtime.as_deref().unwrap_or("-"),
        team.current_lead_model.as_deref().unwrap_or("-"),
        team.current_lead_session_id.as_deref().unwrap_or("-"),
        team.current_lead_session_status
            .map(|status| status.as_str())
            .unwrap_or("-")
    )));
    if let Some(attention) = &team.current_lead_attention {
        lines.push(Line::from(format!("Lead attention: {attention}")));
    }

    if team.member_routing.is_empty() {
        lines.push(Line::from("Members: none"));
    } else {
        lines.push(Line::from("Members:"));
        for member in &team.member_routing {
            lines.push(Line::from(format!("  {}", format_member_routing(member))));
        }
    }

    lines
}

fn render_dashboard_mission_lines(
    team: &TeamManifest,
    snapshot: Option<&AppSnapshot>,
) -> Vec<String> {
    let mut lines = vec![format!("Objective: {}", team.objective)];
    let draft_plans = team
        .plan_items
        .iter()
        .filter(|plan| plan.state == PlanItemState::Draft)
        .count();
    let approved_plans = team
        .plan_items
        .iter()
        .filter(|plan| plan.state == PlanItemState::Approved)
        .count();
    let review_queue = team
        .tasks
        .iter()
        .filter(|task| task.state == TaskCardState::Review)
        .count();
    let consolidation_queue = team
        .tasks
        .iter()
        .filter(|task| task.state == TaskCardState::Done && task.slot_id.is_some())
        .count();
    let history_queue = team
        .tasks
        .iter()
        .filter(|task| {
            matches!(
                task.state,
                TaskCardState::Cancelled | TaskCardState::Superseded
            )
        })
        .count();
    let session = team.current_lead_session_id().and_then(|session_id| {
        snapshot.and_then(|snapshot| {
            snapshot
                .sessions
                .iter()
                .find(|session| session.id == session_id)
        })
    });
    lines.push(format!(
        "Current lead: {} session={} status={}",
        team.current_lead_member_id(),
        team.current_lead_session_id().unwrap_or("-"),
        session
            .map(|session| session.status.as_str())
            .unwrap_or("-")
    ));
    lines.push(format!(
        "Plan queue: {} draft | {} approved | {} generated",
        draft_plans,
        approved_plans,
        team.plan_items
            .len()
            .saturating_sub(draft_plans + approved_plans)
    ));
    lines.push(format!(
        "Review queue: {} | Consolidation cleanup: {} | History: {}",
        review_queue, consolidation_queue, history_queue
    ));
    if let Some(attention) =
        dashboard_current_lead_attention(team, session, team.current_lead_session_id().is_some())
    {
        lines.push(format!("Lead attention: {attention}"));
    }
    lines
}

fn render_dashboard_plan_detail_lines(
    team: &TeamManifest,
    selected_plan_idx: usize,
) -> Vec<String> {
    let Some(plan) = team.plan_items.get(selected_plan_idx) else {
        return vec!["Select a plan item to inspect or press p to add one.".to_string()];
    };

    let mut lines = vec![
        format!("Plan: {} ({})", plan.title, plan.plan_id),
        format!("State: {}", plan.state),
        format!("Owner intent: {}", plan.owner_id.as_deref().unwrap_or("-")),
        format!("Summary: {}", plan.summary),
    ];
    if plan.runtime.is_some() || plan.model.is_some() {
        lines.push(format!(
            "Requested runtime/model: {}/{}",
            plan.runtime.as_deref().unwrap_or("-"),
            plan.model.as_deref().unwrap_or("-"),
        ));
    }
    if let Some(deliverable) = &plan.deliverable {
        lines.push(format!("Deliverable: {deliverable}"));
    }
    if !plan.write_scope.is_empty() {
        lines.push(format!("Write scope: {}", plan.write_scope.join(", ")));
    }
    if !plan.verification.is_empty() {
        lines.push(format!("Verification: {}", plan.verification.join(", ")));
    }
    if !plan.depends_on.is_empty() {
        lines.push(format!("Depends on: {}", plan.depends_on.join(", ")));
    }
    if let Some(notes) = &plan.notes {
        lines.push(format!("Notes: {notes}"));
    }
    if let Some(task_id) = &plan.generated_task_id {
        lines.push(format!("Generated task: {task_id}"));
    } else {
        lines.push("Actions: P approve draft item, G generate task card.".to_string());
    }
    lines
}

fn dashboard_current_lead_attention(
    team: &TeamManifest,
    session: Option<&SessionSummary>,
    has_session_id: bool,
) -> Option<String> {
    match session.map(|session| session.status) {
        Some(SessionStatus::Running) => None,
        Some(SessionStatus::Prepared) => {
            Some("Current lead session is prepared but not yet running.".to_string())
        }
        Some(SessionStatus::Completed) => Some(
            "Current lead session completed. Start another lead task or promote a replacement if coordination should continue."
                .to_string(),
        ),
        Some(SessionStatus::Failed) => Some(match session.and_then(|session| session.end_reason) {
            Some(SessionEndReason::Timeout) => {
                "Current lead session timed out. Inspect logs and hand off or restart the lead."
                    .to_string()
            }
            Some(SessionEndReason::TokenExhausted) => {
                "Current lead session appears to have exhausted tokens or context budget. Inspect logs and hand off or restart the lead."
                    .to_string()
            }
            Some(SessionEndReason::ProviderLimited) => {
                "Current lead session hit a provider quota or rate limit. Inspect logs, adjust spend or concurrency, and hand off or restart the lead."
                    .to_string()
            }
            _ => "Current lead session failed. This can happen after runtime errors, token exhaustion, or timeout; inspect logs and hand off or restart the lead."
                .to_string(),
        }),
        Some(SessionStatus::Cancelled) => Some(
            "Current lead session was cancelled. Restart the lead or promote another member before continuing."
                .to_string(),
        ),
        None if has_session_id => Some(
            "Tracked current lead session is missing. Inspect logs and replace or restart the lead session."
                .to_string(),
        ),
        None if matches!(
            team.status,
            awo_core::TeamStatus::Running | awo_core::TeamStatus::Blocked
        ) => Some(
            "No active current lead session is tracked. Promote another member or start a new lead session if coordination should continue."
                .to_string(),
        ),
        None => None,
    }
}

fn render_dashboard_task_detail_lines(
    team: &TeamManifest,
    selected_task_idx: usize,
    snapshot: Option<&AppSnapshot>,
) -> Vec<String> {
    let Some(task) = team.tasks.get(selected_task_idx) else {
        return vec!["Select a task card to inspect its review state.".to_string()];
    };

    let session = task.result_session_id.as_deref().and_then(|session_id| {
        snapshot.and_then(|snapshot| {
            snapshot
                .sessions
                .iter()
                .find(|session| session.id == session_id)
        })
    });
    let slot = task.slot_id.as_deref().and_then(|slot_id| {
        snapshot.and_then(|snapshot| snapshot.slots.iter().find(|slot| slot.id == slot_id))
    });

    let mut lines = vec![
        format!("Task: {} ({})", task.title, task.task_id),
        format!("Owner: {} state={}", task.owner_id, task.state),
    ];
    if task.runtime.is_some() || task.model.is_some() {
        lines.push(format!(
            "Requested runtime/model: {}/{}",
            task.runtime.as_deref().unwrap_or("-"),
            task.model.as_deref().unwrap_or("-"),
        ));
    }
    let queue_label = task_queue_role_label(task);
    lines.push(format!("Queue role: {queue_label}"));
    if let Some(result_summary) = &task.result_summary {
        lines.push(format!("Result: {result_summary}"));
    }
    if let Some(handoff_note) = &task.handoff_note {
        lines.push(format!("Handoff: {handoff_note}"));
    }
    if let Some(replacement_task_id) = &task.superseded_by_task_id {
        lines.push(format!("Superseded by: {replacement_task_id}"));
    }
    if let Some(session_id) = &task.result_session_id {
        lines.push(format!(
            "Session: {} status={} reason={} cap={}",
            session_id,
            session
                .map(|session| session.status.as_str())
                .unwrap_or("-"),
            session
                .and_then(|session| session.end_reason)
                .map(SessionEndReason::as_str)
                .unwrap_or("-"),
            session
                .map(|session| session.capacity_status.as_str())
                .unwrap_or("-"),
        ));
        if let Some(session) = session {
            if let Some(usage_note) = &session.usage_note {
                lines.push(format!("Usage: {usage_note}"));
            }
            if let Some(recovery_hint) = &session.recovery_hint {
                lines.push(format!("Recovery: {recovery_hint}"));
            }
        }
    }
    if let Some(slot_id) = &task.slot_id {
        lines.push(format!(
            "Slot: {} status={} strategy={} branch={}",
            slot_id,
            slot.map(|slot| slot.status.as_str()).unwrap_or("-"),
            slot.map(|slot| slot.strategy.as_str()).unwrap_or("-"),
            task.branch_name.as_deref().unwrap_or("-"),
        ));
        if let Some(slot) = slot {
            lines.push(format!("Path: {}", slot.slot_path));
            lines.push("Inspect: press v for diff or o for log.".to_string());
            if matches!(
                task.state,
                TaskCardState::Done | TaskCardState::Cancelled | TaskCardState::Superseded
            ) {
                lines.push(
                    "Cleanup: press X to release/retain-for-reuse, or K to delete now.".to_string(),
                );
            }
        }
    }
    if let Some(log_path) = &task.output_log_path {
        lines.push(format!("Log: {log_path}"));
    }
    lines
}

fn task_queue_role_label(task: &awo_core::TaskCard) -> &'static str {
    match task.state {
        TaskCardState::Review => "review queue",
        TaskCardState::Done if task.slot_id.is_some() => "cleanup queue",
        TaskCardState::Done => "done",
        TaskCardState::InProgress => "running",
        TaskCardState::Blocked => "blocked",
        TaskCardState::Cancelled => "cancelled history",
        TaskCardState::Superseded => "superseded history",
        TaskCardState::Todo => "todo",
    }
}

pub(crate) fn dispatch_in_background(
    paths: awo_core::app::AppPaths,
    command: Command,
    tx: Sender<BackgroundResult>,
) {
    thread::spawn(move || {
        let result = match AppCore::with_dirs(paths.config_dir.clone(), paths.data_dir.clone()) {
            Ok(mut bg_core) => match bg_core.dispatch(command) {
                Ok(outcome) => BackgroundResult {
                    summary: outcome.summary,
                    events: outcome.events,
                    error: None,
                },
                Err(e) => BackgroundResult {
                    summary: String::new(),
                    events: vec![],
                    error: Some(e.to_string()),
                },
            },
            Err(e) => BackgroundResult {
                summary: String::new(),
                events: vec![],
                error: Some(format!("failed to open background core: {e}")),
            },
        };
        let _ = tx.send(result);
    });
}

fn format_routing_preferences(preferences: &RoutingPreferencesSummary) -> String {
    let mut parts = vec![
        format!(
            "fallback={}",
            if preferences.allow_fallback {
                "on"
            } else {
                "off"
            }
        ),
        format!(
            "local={}",
            if preferences.prefer_local {
                "prefer"
            } else {
                "neutral"
            }
        ),
        format!(
            "metered={}",
            if preferences.avoid_metered {
                "avoid"
            } else {
                "ok"
            }
        ),
    ];
    if let Some(max_cost_tier) = preferences.max_cost_tier {
        parts.push(format!("max={}", max_cost_tier.as_str()));
    }
    parts.join(" ")
}

fn format_fallback_target(runtime: Option<&str>, model: Option<&str>) -> String {
    match (runtime, model) {
        (Some(runtime), Some(model)) => format!("{runtime}/{model}"),
        (Some(runtime), None) => runtime.to_string(),
        (None, Some(model)) => format!("model={model}"),
        (None, None) => "-".to_string(),
    }
}

fn format_member_routing(member: &MemberRoutingSummary) -> String {
    let mut parts = vec![member.member_id.clone()];
    if member.fallback_runtime.is_some() || member.fallback_model.is_some() {
        parts.push(format!(
            "fallback={}",
            format_fallback_target(
                member.fallback_runtime.as_deref(),
                member.fallback_model.as_deref()
            )
        ));
    }
    if let Some(preferences) = &member.routing_preferences {
        parts.push(format!(
            "routing={}",
            format_routing_preferences(preferences)
        ));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use awo_core::team::{TaskCard, TeamExecutionMode, TeamMember};
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_member(
        member_id: &str,
        role: &str,
        runtime: Option<&str>,
        read_only: bool,
    ) -> TeamMember {
        TeamMember {
            member_id: member_id.to_string(),
            role: role.to_string(),
            runtime: runtime.map(str::to_string),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only,
            write_scope: Vec::new(),
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        }
    }

    fn test_task(task_id: &str, owner_id: &str) -> TaskCard {
        TaskCard {
            task_id: task_id.to_string(),
            title: format!("Title for {task_id}"),
            summary: "Summary".to_string(),
            owner_id: owner_id.to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "Patch".to_string(),
            verification: Vec::new(),
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
            result_summary: None,
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
            superseded_by_task_id: None,
        }
    }

    fn base_state() -> TuiState {
        TuiState {
            status: String::new(),
            messages: Vec::new(),
            focus: TuiFocus::Repos,
            selected_repo_index: 0,
            selected_team_index: 0,
            selected_slot_index: 0,
            selected_session_index: 0,
            log_content: None,
            log_session_id: None,
            log_path: None,
            show_log_panel: false,
            terminal_content: None,
            terminal_session_id: None,
            show_terminal_panel: false,
            terminal_input_mode: false,
            terminal_scroll: 0,
            terminal_search_query: None,
            terminal_follow_output: true,
            terminal_layout: TerminalLayoutMode::Docked,
            pending_ops: 0,
            input_mode: InputMode::Normal,
            show_help: false,
            log_scroll: 0,
            filter_query: None,
            team_dashboard: TeamDashboardState {
                selected_team_idx: 0,
                selected_plan_idx: 0,
                selected_member_idx: 0,
                selected_task_idx: 0,
                teams: Vec::new(),
                focus: TeamDashboardFocus::Team,
            },
            last_snapshot: None,
            last_snapshot_time: None,
            snapshot_refresh_in_flight: false,
        }
    }

    fn empty_snapshot() -> AppSnapshot {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("awo-tui-tests-{}-{id}", std::process::id()));
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let snapshot = AppCore::with_dirs(config_dir, data_dir)
            .expect("app core")
            .snapshot()
            .expect("snapshot");
        let _ = std::fs::remove_dir_all(root);
        snapshot
    }

    #[test]
    fn clamp_selection_clamps_dashboard_indexes_after_team_data_changes() {
        let mut state = base_state();
        state.team_dashboard.selected_team_idx = 3;
        state.team_dashboard.selected_member_idx = 4;
        state.team_dashboard.selected_task_idx = 5;
        state.team_dashboard.teams = vec![TeamManifest {
            version: 1,
            team_id: "alpha".to_string(),
            repo_id: "repo-1".to_string(),
            objective: "Ship".to_string(),
            status: awo_core::TeamStatus::Planning,
            routing_preferences: None,
            lead: test_member("lead", "lead", None, true),
            current_lead_member_id: Some("lead".to_string()),
            current_lead_session_id: None,
            plan_items: Vec::new(),
            members: vec![test_member("worker-a", "worker", Some("shell"), false)],
            tasks: vec![test_task("task-1", "worker-a")],
        }];

        clamp_selection(&mut state, &empty_snapshot());

        assert_eq!(state.team_dashboard.selected_team_idx, 0);
        assert_eq!(state.team_dashboard.selected_member_idx, 1);
        assert_eq!(state.team_dashboard.selected_task_idx, 0);
    }

    #[test]
    fn apply_snapshot_refresh_result_preserves_selected_team_by_id() {
        let mut state = base_state();
        state.snapshot_refresh_in_flight = true;
        state.team_dashboard.teams = vec![
            TeamManifest {
                version: 1,
                team_id: "alpha".to_string(),
                repo_id: "repo-1".to_string(),
                objective: "Alpha".to_string(),
                status: awo_core::TeamStatus::Planning,
                routing_preferences: None,
                lead: test_member("lead", "lead", None, true),
                current_lead_member_id: Some("lead".to_string()),
                current_lead_session_id: None,
                plan_items: Vec::new(),
                members: Vec::new(),
                tasks: Vec::new(),
            },
            TeamManifest {
                version: 1,
                team_id: "beta".to_string(),
                repo_id: "repo-2".to_string(),
                objective: "Beta".to_string(),
                status: awo_core::TeamStatus::Planning,
                routing_preferences: None,
                lead: test_member("lead", "lead", None, true),
                current_lead_member_id: Some("lead".to_string()),
                current_lead_session_id: None,
                plan_items: Vec::new(),
                members: Vec::new(),
                tasks: Vec::new(),
            },
        ];
        state.team_dashboard.selected_team_idx = 1;

        apply_snapshot_refresh_result(
            &mut state,
            SnapshotRefreshResult {
                snapshot: Some(empty_snapshot()),
                teams: vec![
                    TeamManifest {
                        version: 1,
                        team_id: "gamma".to_string(),
                        repo_id: "repo-3".to_string(),
                        objective: "Gamma".to_string(),
                        status: awo_core::TeamStatus::Planning,
                        routing_preferences: None,
                        lead: test_member("lead", "lead", None, true),
                        current_lead_member_id: Some("lead".to_string()),
                        current_lead_session_id: None,
                        plan_items: Vec::new(),
                        members: Vec::new(),
                        tasks: Vec::new(),
                    },
                    TeamManifest {
                        version: 1,
                        team_id: "beta".to_string(),
                        repo_id: "repo-2".to_string(),
                        objective: "Beta refreshed".to_string(),
                        status: awo_core::TeamStatus::Planning,
                        routing_preferences: None,
                        lead: test_member("lead", "lead", None, true),
                        current_lead_member_id: Some("lead".to_string()),
                        current_lead_session_id: None,
                        plan_items: Vec::new(),
                        members: Vec::new(),
                        tasks: Vec::new(),
                    },
                ],
                error: None,
            },
        );

        assert!(!state.snapshot_refresh_in_flight);
        assert_eq!(
            state.team_dashboard.teams[state.team_dashboard.selected_team_idx].team_id,
            "beta"
        );
        assert!(state.last_snapshot.is_some());
        assert!(state.last_snapshot_time.is_some());
    }

    #[test]
    fn event_refresh_watcher_emits_on_new_events() {
        let bus = awo_core::EventBus::new();
        let (tx, rx) = crossbeam_channel::unbounded();

        request_event_refresh_on_new_events(bus.clone(), 0, tx);
        bus.publish(&[DomainEvent::CommandReceived {
            command: "refresh".to_string(),
        }]);

        let trigger = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("event refresh trigger");
        assert_eq!(trigger.head_seq, 1);
    }

    #[test]
    fn repo_empty_state_describes_path_based_add() {
        let lines = render_repo_detail(None, &[], &[]);
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("repository by path"))
        );
    }

    #[test]
    fn task_detail_includes_usage_and_recovery_guidance() {
        let mut snapshot = empty_snapshot();
        snapshot.sessions.push(SessionSummary {
            id: "session-1".to_string(),
            repo_id: "repo-1".to_string(),
            slot_id: "slot-1".to_string(),
            runtime: "codex".to_string(),
            supervisor: None,
            status: SessionStatus::Failed,
            read_only: false,
            dry_run: false,
            log_path: Some("/tmp/session.log".to_string()),
            exit_code: Some(1),
            end_reason: Some(SessionEndReason::TokenExhausted),
            capacity_status: awo_core::runtime::SessionCapacityStatus::Exhausted,
            usage_note: Some(
                "Structured usage stats are not available through the current CLI adapter; inspect logs or provider dashboards for exact spend."
                    .to_string(),
            ),
            recovery_hint: Some(
                "Session likely exhausted context or token budget. Hand off to another agent, reduce scope, or choose a different model."
                    .to_string(),
            ),
            embedded_terminal_supported: false,
        });
        let lines = render_dashboard_task_detail_lines(
            &TeamManifest {
                version: 1,
                team_id: "alpha".to_string(),
                repo_id: "repo-1".to_string(),
                objective: "Ship".to_string(),
                status: awo_core::TeamStatus::Running,
                routing_preferences: None,
                lead: test_member("lead", "lead", None, true),
                current_lead_member_id: Some("lead".to_string()),
                current_lead_session_id: None,
                plan_items: Vec::new(),
                members: vec![test_member("worker-a", "worker", Some("codex"), false)],
                tasks: vec![TaskCard {
                    task_id: "task-1".to_string(),
                    title: "Title".to_string(),
                    summary: "Summary".to_string(),
                    owner_id: "worker-a".to_string(),
                    runtime: Some("codex".to_string()),
                    model: None,
                    slot_id: Some("slot-1".to_string()),
                    branch_name: Some("awo/task-1".to_string()),
                    read_only: false,
                    write_scope: Vec::new(),
                    deliverable: "Patch".to_string(),
                    verification: Vec::new(),
                    verification_command: None,
                    depends_on: Vec::new(),
                    state: TaskCardState::Review,
                    result_summary: Some("Needs another pass".to_string()),
                    result_session_id: Some("session-1".to_string()),
                    handoff_note: None,
                    output_log_path: Some("/tmp/session.log".to_string()),
                    superseded_by_task_id: None,
                }],
            },
            0,
            Some(&snapshot),
        );

        assert!(lines.iter().any(|line| line.contains("Usage:")));
        assert!(lines.iter().any(|line| line.contains("Recovery:")));
    }

    #[test]
    fn lead_attention_mentions_provider_limits() {
        let team = TeamManifest {
            version: 1,
            team_id: "alpha".to_string(),
            repo_id: "repo-1".to_string(),
            objective: "Ship".to_string(),
            status: awo_core::TeamStatus::Running,
            routing_preferences: None,
            lead: test_member("lead", "lead", Some("claude"), true),
            current_lead_member_id: Some("lead".to_string()),
            current_lead_session_id: Some("session-1".to_string()),
            plan_items: Vec::new(),
            members: Vec::new(),
            tasks: Vec::new(),
        };
        let session = SessionSummary {
            id: "session-1".to_string(),
            repo_id: "repo-1".to_string(),
            slot_id: "slot-1".to_string(),
            runtime: "claude".to_string(),
            supervisor: None,
            status: SessionStatus::Failed,
            read_only: false,
            dry_run: false,
            log_path: Some("/tmp/session.log".to_string()),
            exit_code: Some(1),
            end_reason: Some(SessionEndReason::ProviderLimited),
            capacity_status: awo_core::runtime::SessionCapacityStatus::ProviderLimited,
            usage_note: None,
            recovery_hint: None,
            embedded_terminal_supported: false,
        };

        let attention =
            dashboard_current_lead_attention(&team, Some(&session), true).expect("lead attention");
        assert!(attention.contains("provider quota or rate limit"));
    }

    #[test]
    fn team_empty_state_describes_current_team_controls() {
        let lines = render_team_detail(None);
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Teams panel"))
        );
    }

    #[test]
    fn team_detail_includes_current_lead_summary() {
        let team = CoreTeamSummary {
            team_id: "alpha".to_string(),
            repo_id: "repo-1".to_string(),
            status: awo_core::TeamStatus::Running,
            objective: "Ship".to_string(),
            member_count: 2,
            write_member_count: 1,
            task_count: 2,
            open_task_count: 1,
            routing_preferences: None,
            lead_fallback_runtime: None,
            lead_fallback_model: None,
            lead_runtime: Some("claude".to_string()),
            lead_model: Some("sonnet".to_string()),
            current_lead_member_id: "worker-a".to_string(),
            current_lead_runtime: Some("codex".to_string()),
            current_lead_model: None,
            current_lead_session_id: Some("session-1".to_string()),
            current_lead_session_status: Some(SessionStatus::Failed),
            current_lead_attention: Some(
                "Current lead session failed. This can happen after runtime errors, token exhaustion, or timeout; inspect logs and hand off or restart the lead.".to_string(),
            ),
            member_routing: Vec::new(),
        };

        let lines = render_team_detail(Some(&team));
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Current lead: worker-a"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Lead attention"))
        );
    }
}
