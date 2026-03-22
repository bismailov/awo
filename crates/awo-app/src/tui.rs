use anyhow::{Context, Result};
use awo_core::{
    AppCore, AppSnapshot, Command, DomainEvent, MemberRoutingSummary, RepoSummary,
    RoutingPreferencesSummary, RuntimeCapabilityDescriptor, TeamSummary,
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

#[derive(Debug)]
struct TuiState {
    status: String,
    messages: Vec<String>,
    selected_repo_index: usize,
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
        selected_repo_index: 0,
    };

    info!("TUI started");

    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        let snapshot = core.snapshot()?;
        terminal.draw(|frame| render(frame, &snapshot, &state))?;

        if event::poll(Duration::from_millis(200))?
            && let CEvent::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.selected_repo_index > 0 {
                        state.selected_repo_index -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.selected_repo_index + 1 < snapshot.registered_repos.len() {
                        state.selected_repo_index += 1;
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
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(11),
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ])
    .split(frame.area());

    let header = Paragraph::new(format!(
        "awo V1 slice | q quit | j/k select repo | a add cwd repo | c context doctor | d skills doctor | n no-op | r review refresh | teams/runtimes visible below | {}",
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
        Constraint::Percentage(30),
        Constraint::Percentage(35),
        Constraint::Percentage(35),
    ])
    .split(vertical[2]);
    let bottom = Layout::horizontal([
        Constraint::Percentage(24),
        Constraint::Percentage(20),
        Constraint::Percentage(26),
        Constraint::Percentage(30),
    ])
    .split(vertical[3]);

    let repo_items = snapshot
        .registered_repos
        .iter()
        .enumerate()
        .map(|(index, repo)| {
            render_repo_item(
                repo,
                Some(index)
                    == selected_repo.map(|selected| {
                        snapshot
                            .registered_repos
                            .iter()
                            .position(|candidate| candidate.id == selected.id)
                            .unwrap_or_default()
                    }),
            )
        })
        .collect::<Vec<_>>();
    let repos =
        List::new(repo_items).block(Block::default().borders(Borders::ALL).title("Repositories"));
    frame.render_widget(repos, top[0]);

    let repo_details = render_repo_detail(
        selected_repo,
        snapshot.teams.as_slice(),
        snapshot.runtime_capabilities.as_slice(),
    );
    let repo_detail_widget = Paragraph::new(repo_details).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Selected Repo"),
    );
    frame.render_widget(repo_detail_widget, top[1]);

    let slot_items = snapshot
        .slots
        .iter()
        .filter(|slot| selected_repo.is_none_or(|repo| repo.id == slot.repo_id))
        .map(|slot| {
            ListItem::new(format!(
                "{} [{}] {} {} dirty={} fp={}",
                slot.task_name,
                slot.id,
                slot.status,
                slot.strategy,
                slot.dirty,
                slot.fingerprint_status
            ))
        })
        .collect::<Vec<_>>();
    let slots = List::new(slot_items).block(Block::default().borders(Borders::ALL).title("Slots"));
    frame.render_widget(slots, top[2]);

    let session_items = snapshot
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
        .map(|session| {
            ListItem::new(format!(
                "{} [{}] {} read_only={} dry_run={} exit={}",
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
        .collect::<Vec<_>>();
    let sessions =
        List::new(session_items).block(Block::default().borders(Borders::ALL).title("Sessions"));
    frame.render_widget(sessions, bottom[0]);

    let team_items = snapshot
        .teams
        .iter()
        .filter(|team| selected_repo.is_none_or(|repo| team.repo_id == repo.id))
        .map(|team| {
            ListItem::new(format!(
                "{} [{}] {} members={} open={}/{} write={}",
                team.team_id,
                team.repo_id,
                team.status,
                team.member_count,
                team.open_task_count,
                team.task_count,
                team.write_member_count
            ))
        })
        .collect::<Vec<_>>();
    let teams = List::new(team_items).block(Block::default().borders(Borders::ALL).title("Teams"));
    frame.render_widget(teams, bottom[1]);

    let warning_items = snapshot
        .review
        .warnings
        .iter()
        .rev()
        .take(10)
        .map(|warning| ListItem::new(warning.message.clone()))
        .collect::<Vec<_>>();
    let warnings =
        List::new(warning_items).block(Block::default().borders(Borders::ALL).title("Warnings"));
    frame.render_widget(warnings, bottom[2]);

    let message_items = state
        .messages
        .iter()
        .rev()
        .take(12)
        .map(|message| ListItem::new(message.clone()))
        .collect::<Vec<_>>();
    let messages =
        List::new(message_items).block(Block::default().borders(Borders::ALL).title("Event Log"));
    frame.render_widget(messages, bottom[3]);
}

fn render_repo_item(repo: &RepoSummary, selected: bool) -> ListItem<'static> {
    let manifest = if repo.shared_manifest_present {
        "shared"
    } else {
        "derived"
    };
    let mcp = if repo.mcp_config_present { "yes" } else { "no" };
    ListItem::new(format!(
        "{} {} [{}] base={} remote={} manifest={} ctx={} packs={} skills={} mcp={}",
        if selected { ">" } else { " " },
        repo.name,
        repo.id,
        repo.default_base_branch,
        repo.remote_label,
        manifest,
        repo.entrypoint_count,
        repo.context_pack_count,
        repo.shared_skill_count,
        mcp
    ))
}

fn render_repo_detail(
    repo: Option<&RepoSummary>,
    teams: &[TeamSummary],
    runtime_capabilities: &[RuntimeCapabilityDescriptor],
) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![Line::from("No repo selected.")];
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
            if let Some(preferences) = &team.routing_preferences {
                lines.push(Line::from(format!(
                    "    routing: {}",
                    format_routing_preferences(preferences)
                )));
            }
            if team.lead_fallback_runtime.is_some() || team.lead_fallback_model.is_some() {
                lines.push(Line::from(format!(
                    "    lead fallback: {}",
                    format_fallback_target(
                        team.lead_fallback_runtime.as_deref(),
                        team.lead_fallback_model.as_deref()
                    )
                )));
            }
            for member in &team.member_routing {
                lines.push(Line::from(format!(
                    "    member {}",
                    format_member_routing(member)
                )));
            }
        }
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
