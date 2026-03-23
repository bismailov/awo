use anyhow::{Context, Result};
use awo_core::{
    AppCore, AppSnapshot, Command, DomainEvent, MemberRoutingSummary, RepoSummary,
    RoutingPreferencesSummary, RuntimeCapabilityDescriptor, RuntimeKind, SessionLaunchMode,
    SessionSummary, SlotStrategy, SlotSummary, TeamSummary,
};
use crossterm::event::{self, Event as CEvent, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::io;
use std::time::Duration;
use tracing::info;

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
}

#[derive(Debug)]
struct TuiState {
    status: String,
    messages: Vec<String>,
    focus: TuiFocus,
    selected_repo_index: usize,
    selected_team_index: usize,
    selected_slot_index: usize,
    selected_session_index: usize,
}

pub fn run_tui() -> Result<()> {
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
    };

    info!("TUI started");

    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        let snapshot = core.snapshot()?;
        clamp_selection(&mut state, &snapshot);
        terminal.draw(|frame| render(frame, &snapshot, &state))?;

        if event::poll(Duration::from_millis(200))?
            && let CEvent::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Tab => {
                    state.focus = match state.focus {
                        TuiFocus::Repos => TuiFocus::Teams,
                        TuiFocus::Teams => TuiFocus::Slots,
                        TuiFocus::Slots => TuiFocus::Sessions,
                        TuiFocus::Sessions => TuiFocus::Repos,
                    };
                }
                KeyCode::BackTab => {
                    state.focus = match state.focus {
                        TuiFocus::Repos => TuiFocus::Sessions,
                        TuiFocus::Teams => TuiFocus::Repos,
                        TuiFocus::Slots => TuiFocus::Teams,
                        TuiFocus::Sessions => TuiFocus::Slots,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') => match state.focus {
                    TuiFocus::Repos => {
                        if state.selected_repo_index > 0 {
                            state.selected_repo_index -= 1;
                        }
                    }
                    TuiFocus::Teams => {
                        if state.selected_team_index > 0 {
                            state.selected_team_index -= 1;
                        }
                    }
                    TuiFocus::Slots => {
                        if state.selected_slot_index > 0 {
                            state.selected_slot_index -= 1;
                        }
                    }
                    TuiFocus::Sessions => {
                        if state.selected_session_index > 0 {
                            state.selected_session_index -= 1;
                        }
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match state.focus {
                    TuiFocus::Repos => {
                        if state.selected_repo_index + 1 < snapshot.registered_repos.len() {
                            state.selected_repo_index += 1;
                        }
                    }
                    TuiFocus::Teams => {
                        if state.selected_team_index + 1 < visible_teams(&snapshot, &state).len() {
                            state.selected_team_index += 1;
                        }
                    }
                    TuiFocus::Slots => {
                        if state.selected_slot_index + 1 < visible_slots(&snapshot, &state).len() {
                            state.selected_slot_index += 1;
                        }
                    }
                    TuiFocus::Sessions => {
                        if state.selected_session_index + 1
                            < visible_sessions(&snapshot, &state).len()
                        {
                            state.selected_session_index += 1;
                        }
                    }
                },
                KeyCode::Char('s') => {
                    if let Some(repo) = selected_repo(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::SlotAcquire {
                                repo_id: repo.id.clone(),
                                task_name: "tui-task".to_string(),
                                strategy: SlotStrategy::Fresh,
                            },
                        );
                    }
                }
                KeyCode::Enter => {
                    if let Some(slot) = selected_slot(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::SessionStart {
                                slot_id: slot.id.clone(),
                                runtime: RuntimeKind::Shell,
                                prompt: "ls -R".to_string(),
                                read_only: false,
                                dry_run: false,
                                launch_mode: SessionLaunchMode::Oneshot,
                                attach_context: true,
                            },
                        );
                    }
                }
                KeyCode::Char('x') => {
                    if let Some(session) = selected_session(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::SessionCancel {
                                session_id: session.id.clone(),
                            },
                        );
                    }
                }
                KeyCode::Char('X') => {
                    if let Some(slot) = selected_slot(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::SlotRelease {
                                slot_id: slot.id.clone(),
                            },
                        );
                    }
                }
                KeyCode::Char('a') => {
                    let current_dir = std::env::current_dir()
                        .context("failed to resolve current working directory")?;
                    apply_command(
                        &mut core,
                        &mut state,
                        Command::RepoAdd { path: current_dir },
                    );
                }
                KeyCode::Char('n') => {
                    apply_command(
                        &mut core,
                        &mut state,
                        Command::NoOp {
                            label: "manual-noop".to_string(),
                        },
                    );
                }
                KeyCode::Char('r') => {
                    apply_command(
                        &mut core,
                        &mut state,
                        Command::ReviewStatus { repo_id: None },
                    );
                }
                KeyCode::Char('c') => {
                    if let Some(repo) = selected_repo(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::ContextDoctor {
                                repo_id: repo.id.clone(),
                            },
                        );
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(repo) = selected_repo(&snapshot, &state) {
                        apply_command(
                            &mut core,
                            &mut state,
                            Command::SkillsDoctor {
                                repo_id: repo.id.clone(),
                                runtime: None,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn selected_repo<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Option<&'a RepoSummary> {
    snapshot.registered_repos.get(
        state
            .selected_repo_index
            .min(snapshot.registered_repos.len().saturating_sub(1)),
    )
}

fn visible_teams<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Vec<&'a TeamSummary> {
    let selected_repo = selected_repo(snapshot, state);
    snapshot
        .teams
        .iter()
        .filter(|team| selected_repo.is_none_or(|repo| team.repo_id == repo.id))
        .collect()
}

fn selected_team<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Option<&'a TeamSummary> {
    let teams = visible_teams(snapshot, state);
    teams.get(state.selected_team_index).copied()
}

fn visible_slots<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Vec<&'a SlotSummary> {
    let selected_repo = selected_repo(snapshot, state);
    snapshot
        .slots
        .iter()
        .filter(|slot| selected_repo.is_none_or(|repo| repo.id == slot.repo_id))
        .collect()
}

fn selected_slot<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Option<&'a SlotSummary> {
    let slots = visible_slots(snapshot, state);
    slots.get(state.selected_slot_index).copied()
}

fn visible_sessions<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Vec<&'a SessionSummary> {
    let selected_repo = selected_repo(snapshot, state);
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
            })
        })
        .collect()
}

fn selected_session<'a>(snapshot: &'a AppSnapshot, state: &TuiState) -> Option<&'a SessionSummary> {
    let sessions = visible_sessions(snapshot, state);
    sessions.get(state.selected_session_index).copied()
}

fn clamp_selection(state: &mut TuiState, snapshot: &AppSnapshot) {
    let repo_count = snapshot.registered_repos.len();
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
}

fn apply_command(core: &mut AppCore, state: &mut TuiState, command: Command) {
    match core.dispatch(command) {
        Ok(outcome) => {
            state.status = outcome.summary;
            append_events(state, outcome.events);
        }
        Err(error) => {
            let message = format!("Error: {error:#}");
            state.status = message.clone();
            state.messages.push(message);
        }
    }
}

fn append_events(state: &mut TuiState, events: Vec<DomainEvent>) {
    state
        .messages
        .extend(events.into_iter().map(|event| event.to_message()));
    if state.messages.len() > 30 {
        let overflow = state.messages.len() - 30;
        state.messages.drain(0..overflow);
    }
}

fn render(frame: &mut Frame, snapshot: &AppSnapshot, state: &TuiState) {
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

    let header = Paragraph::new(format!(
        "awo V1 | q quit | Tab focus | s acquire | Enter start | x cancel | X release | r refresh | {}",
        state.status
    ))
    .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(header, vertical[0]);

    let paths = vec![
        Line::from(format!("Platform: {}", snapshot.platform_label)),
        Line::from(format!("Config: {}", snapshot.config_dir)),
        Line::from(format!("State DB: {}", snapshot.state_db_path)),
        Line::from(format!("Repo Overlays: {}", snapshot.repos_dir)),
        Line::from(format!("Managed Clones: {}", snapshot.clones_dir)),
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

    let repo_items = if snapshot.registered_repos.is_empty() {
        vec![ListItem::new("(no repos - press 'a' to add)")]
    } else {
        snapshot
            .registered_repos
            .iter()
            .enumerate()
            .map(|(index, repo)| render_repo_item(repo, index == state.selected_repo_index))
            .collect::<Vec<_>>()
    };
    let repos_border_style = if state.focus == TuiFocus::Repos {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let repos = List::new(repo_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Repositories")
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
            .title("Slots")
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
            ListItem::new(format!(
                "{} {} [{}] {} read_only={} dry_run={} exit={}",
                marker,
                session.runtime,
                session.slot_id,
                session.status,
                session.read_only,
                session.dry_run,
                session
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "-".to_string())
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
            .title("Sessions")
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
            .title("Teams")
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
    let warnings =
        List::new(warning_items).block(Block::default().borders(Borders::ALL).title("Warnings"));
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
    let messages =
        List::new(message_items).block(Block::default().borders(Borders::ALL).title("Event Log"));
    frame.render_widget(messages, bottom[3]);
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
    teams: &[TeamSummary],
    runtime_capabilities: &[RuntimeCapabilityDescriptor],
) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![Line::from("Press 'a' to add the current directory.")];
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
        lines.push(Line::from("Select a team with h/l for details."));
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

fn render_team_detail(team: Option<&TeamSummary>) -> Vec<Line<'static>> {
    let Some(team) = team else {
        return vec![Line::from("Use h/l to select a team.")];
    };

    let mut lines = vec![
        Line::from(format!("ID: {}", team.team_id)),
        Line::from(format!("Objective: {}", team.objective)),
        Line::from(format!("Status: {}", team.status)),
        Line::from(format!(
            "Open tasks: {} of {}",
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
            "Lead: runtime={} model={} fallback={}",
            team.lead_runtime.as_deref().unwrap_or("-"),
            team.lead_model.as_deref().unwrap_or("-"),
            format_fallback_target(
                team.lead_fallback_runtime.as_deref(),
                team.lead_fallback_model.as_deref()
            )
        )));
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
