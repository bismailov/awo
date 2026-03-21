use anyhow::{Context, Result};
use awo_core::{
    AppCore, AppSnapshot, Command, ContextDoctorReport, Diagnostic, DomainEvent, RepoContext,
    RepoSkillCatalog, RepoSummary, ReviewSummary, RuntimeCapabilityDescriptor, RuntimeKind,
    SessionLaunchMode, SkillDoctorReport, SkillLinkMode, SkillRuntime, SlotStrategy,
    TeamExecutionMode, TeamManifest, TeamSummary, all_runtime_capabilities,
    default_team_manifest_path, runtime_capabilities, starter_team_manifest,
};
use clap::{Parser, Subcommand};
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
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "awo", version, about = "Agent workspace orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Option<AppCommand>,
}

#[derive(Debug, Subcommand)]
enum AppCommand {
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
enum RepoCommand {
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
enum ContextCommand {
    Pack { repo_id: String },
    Doctor { repo_id: String },
}

#[derive(Debug, Subcommand)]
enum SkillsCommand {
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
enum RuntimeCommand {
    List,
    Show { runtime: String },
}

#[derive(Debug, Subcommand)]
enum TeamCommand {
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
        force: bool,
    },
    List,
    Show {
        team_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum SlotCommand {
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
enum SessionCommand {
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
enum ReviewCommand {
    Status {
        #[arg(long)]
        repo_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum DebugCommand {
    Noop {
        #[arg(long, default_value = "cli-debug")]
        label: String,
    },
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

#[derive(Debug)]
struct TuiState {
    status: String,
    messages: Vec<String>,
    selected_repo_index: usize,
}

fn main() -> Result<()> {
    initialize_tracing()?;

    let cli = Cli::parse();
    match cli.command.unwrap_or(AppCommand::Tui) {
        AppCommand::Tui => run_tui(),
        AppCommand::Repo { command } => run_repo(command),
        AppCommand::Context { command } => run_context(command),
        AppCommand::Skills { command } => run_skills(command),
        AppCommand::Runtime { command } => run_runtime(command),
        AppCommand::Team { command } => run_team(command),
        AppCommand::Slot { command } => run_slot(command),
        AppCommand::Session { command } => run_session(command),
        AppCommand::Review { command } => run_review(command),
        AppCommand::Debug { command } => run_debug(command),
    }
}

fn initialize_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .map_err(anyhow::Error::from)?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(())
}

fn run_debug(command: DebugCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    let outcome = match command {
        DebugCommand::Noop { label } => core.dispatch(Command::NoOp { label })?,
    };

    print_outcome(&outcome);
    Ok(())
}

fn run_repo(command: RepoCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    let outcome = match command {
        RepoCommand::Add { path } => core.dispatch(Command::RepoAdd { path: path.into() })?,
        RepoCommand::Clone {
            remote_url,
            destination,
        } => core.dispatch(Command::RepoClone {
            remote_url,
            destination: destination.map(Into::into),
        })?,
        RepoCommand::Fetch { repo_id } => core.dispatch(Command::RepoFetch { repo_id })?,
        RepoCommand::List => core.dispatch(Command::RepoList)?,
    };

    print_outcome(&outcome);
    let snapshot = core.snapshot()?;
    print_registered_repos(&snapshot);
    Ok(())
}

fn run_context(command: ContextCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    match command {
        ContextCommand::Pack { repo_id } => {
            let outcome = core.dispatch(Command::ContextPack {
                repo_id: repo_id.clone(),
            })?;
            print_outcome(&outcome);
            let context = core.context_for_repo(&repo_id)?;
            print_context(&context);
        }
        ContextCommand::Doctor { repo_id } => {
            let outcome = core.dispatch(Command::ContextDoctor {
                repo_id: repo_id.clone(),
            })?;
            print_outcome(&outcome);
            let report = core.context_doctor_for_repo(&repo_id)?;
            print_context_doctor(&report);
        }
    }

    Ok(())
}

fn run_skills(command: SkillsCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    match command {
        SkillsCommand::List { repo_id } => {
            let outcome = core.dispatch(Command::SkillsList {
                repo_id: repo_id.clone(),
            })?;
            print_outcome(&outcome);
            let catalog = core.skills_for_repo(&repo_id)?;
            print_skills_catalog(&catalog);
        }
        SkillsCommand::Doctor { repo_id, runtime } => {
            let parsed_runtimes = parse_skill_runtimes(runtime.as_deref())?;
            let outcome = core.dispatch(Command::SkillsDoctor {
                repo_id: repo_id.clone(),
                runtime: parsed_runtimes
                    .first()
                    .copied()
                    .filter(|_| parsed_runtimes.len() == 1),
            })?;
            print_outcome(&outcome);
            let reports = core.skills_doctor_for_repo(&repo_id, &parsed_runtimes)?;
            print_skill_doctor(&reports);
        }
        SkillsCommand::Link {
            repo_id,
            runtime,
            mode,
        } => {
            let runtimes = parse_skill_runtimes(Some(&runtime))?;
            let mode = match mode {
                Some(mode) => mode.parse::<SkillLinkMode>().map_err(anyhow::Error::msg)?,
                None => SkillLinkMode::default_for_platform(),
            };
            for runtime in runtimes {
                let outcome = core.dispatch(Command::SkillsLink {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                print_outcome(&outcome);
                let reports = core.skills_doctor_for_repo(&repo_id, &[runtime])?;
                print_skill_doctor(&reports);
            }
        }
        SkillsCommand::Sync {
            repo_id,
            runtime,
            mode,
        } => {
            let runtimes = parse_skill_runtimes(Some(&runtime))?;
            let mode = match mode {
                Some(mode) => mode.parse::<SkillLinkMode>().map_err(anyhow::Error::msg)?,
                None => SkillLinkMode::default_for_platform(),
            };
            for runtime in runtimes {
                let outcome = core.dispatch(Command::SkillsSync {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                print_outcome(&outcome);
                let reports = core.skills_doctor_for_repo(&repo_id, &[runtime])?;
                print_skill_doctor(&reports);
            }
        }
    }

    Ok(())
}

fn run_runtime(command: RuntimeCommand) -> Result<()> {
    match command {
        RuntimeCommand::List => print_runtime_capabilities(&all_runtime_capabilities()),
        RuntimeCommand::Show { runtime } => {
            let runtime = runtime.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
            let capability = runtime_capabilities(runtime);
            print_runtime_capabilities(&[capability]);
        }
    }

    Ok(())
}

fn run_team(command: TeamCommand) -> Result<()> {
    let core = AppCore::bootstrap()?;
    match command {
        TeamCommand::Init {
            repo_id,
            team_id,
            objective,
            lead_runtime,
            lead_model,
            execution_mode,
            force,
        } => {
            let snapshot = core.snapshot()?;
            if !snapshot
                .registered_repos
                .iter()
                .any(|repo| repo.id == repo_id)
            {
                anyhow::bail!("unknown repo id `{repo_id}`");
            }

            let execution_mode = execution_mode
                .parse::<TeamExecutionMode>()
                .map_err(anyhow::Error::msg)?;
            let lead_runtime = lead_runtime
                .as_deref()
                .map(|runtime| {
                    runtime
                        .parse::<RuntimeKind>()
                        .map(|runtime| runtime.as_str().to_string())
                        .map_err(anyhow::Error::msg)
                })
                .transpose()?;
            let manifest_path = default_team_manifest_path(core.paths(), &team_id);
            if manifest_path.exists() && !force {
                anyhow::bail!(
                    "team manifest `{team_id}` already exists at {} (pass --force to overwrite)",
                    manifest_path.display()
                );
            }

            let manifest = starter_team_manifest(
                &repo_id,
                &team_id,
                &objective,
                lead_runtime.as_deref(),
                lead_model.as_deref(),
                execution_mode,
            );
            let path = core.save_team_manifest(&manifest)?;
            println!("Saved starter team manifest to {}", path.display());
            print_team_manifest(&manifest);
        }
        TeamCommand::List => {
            let manifests = core.list_team_manifests()?;
            print_team_manifests(&manifests);
        }
        TeamCommand::Show { team_id } => {
            let manifest = core.load_team_manifest(&team_id)?;
            print_team_manifest(&manifest);
        }
    }

    Ok(())
}

fn run_slot(command: SlotCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    let repo_filter = match &command {
        SlotCommand::List { repo_id } => repo_id.clone(),
        _ => None,
    };
    let outcome = match command {
        SlotCommand::Acquire {
            repo_id,
            task_name,
            strategy,
        } => core.dispatch(Command::SlotAcquire {
            repo_id,
            task_name,
            strategy: strategy
                .parse::<SlotStrategy>()
                .map_err(anyhow::Error::msg)?,
        })?,
        SlotCommand::List { repo_id } => core.dispatch(Command::SlotList { repo_id })?,
        SlotCommand::Release { slot_id } => core.dispatch(Command::SlotRelease { slot_id })?,
        SlotCommand::Refresh { slot_id } => core.dispatch(Command::SlotRefresh { slot_id })?,
    };

    print_outcome(&outcome);
    let snapshot = core.snapshot()?;
    print_slots(&snapshot, repo_filter.as_deref());
    Ok(())
}

fn run_session(command: SessionCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    let repo_filter = match &command {
        SessionCommand::List { repo_id } => repo_id.clone(),
        _ => None,
    };
    let outcome = match command {
        SessionCommand::Start {
            slot_id,
            runtime,
            prompt,
            read_only,
            dry_run,
            launch_mode,
            no_auto_context,
        } => {
            let runtime = runtime.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
            let launch_mode = match launch_mode {
                Some(mode) => mode
                    .parse::<SessionLaunchMode>()
                    .map_err(anyhow::Error::msg)?,
                None => SessionLaunchMode::default_for_environment(),
            };
            core.dispatch(Command::SessionStart {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context: !no_auto_context,
            })?
        }
        SessionCommand::List { repo_id } => core.dispatch(Command::SessionList { repo_id })?,
        SessionCommand::Cancel { session_id } => {
            core.dispatch(Command::SessionCancel { session_id })?
        }
        SessionCommand::Delete { session_id } => {
            core.dispatch(Command::SessionDelete { session_id })?
        }
    };

    print_outcome(&outcome);
    let snapshot = core.snapshot()?;
    print_sessions(&snapshot, repo_filter.as_deref());
    Ok(())
}

fn run_review(command: ReviewCommand) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    let repo_filter = match &command {
        ReviewCommand::Status { repo_id } => repo_id.clone(),
    };
    let outcome = match command {
        ReviewCommand::Status { repo_id } => core.dispatch(Command::ReviewStatus { repo_id })?,
    };

    print_outcome(&outcome);
    let snapshot = core.snapshot()?;
    print_review(&snapshot.review_for_repo(repo_filter.as_deref()));
    Ok(())
}

fn parse_skill_runtimes(runtime: Option<&str>) -> Result<Vec<SkillRuntime>> {
    match runtime {
        None => Ok(SkillRuntime::all().to_vec()),
        Some("all") => Ok(SkillRuntime::all().to_vec()),
        Some(runtime) => Ok(vec![
            runtime
                .parse::<SkillRuntime>()
                .map_err(anyhow::Error::msg)?,
        ]),
    }
}

fn print_outcome(outcome: &awo_core::CommandOutcome) {
    println!("{}", outcome.summary);
    for event in &outcome.events {
        println!("- {}", event.to_message());
    }
}

fn run_tui() -> Result<()> {
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
        Constraint::Length(7),
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
                    "{}(subagents={},teams={},skills={})",
                    capability.runtime,
                    capability.inline_subagents.as_str(),
                    capability.multi_session_teams.as_str(),
                    capability.skill_preload.as_str()
                ))
                .collect::<Vec<_>>()
                .join(" ")
        )),
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
        .filter(|team| selected_repo.is_none_or(|repo| repo.id == team.repo_id))
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
        }
    }

    lines.push(Line::from("Runtime capabilities:"));
    for capability in runtime_capabilities {
        lines.push(Line::from(format!(
            "  - {} launch={} subagents={} teams={}",
            capability.runtime,
            capability.default_launch_mode,
            capability.inline_subagents.as_str(),
            capability.multi_session_teams.as_str()
        )));
    }

    lines
}

fn print_registered_repos(snapshot: &AppSnapshot) {
    if snapshot.registered_repos.is_empty() {
        println!("No registered repos.");
        return;
    }

    println!();
    println!("Registered repos:");
    for repo in &snapshot.registered_repos {
        println!(
            "- {} [{}] base={} remote={} worktrees={} ctx={} packs={} skills={} mcp={}",
            repo.name,
            repo.id,
            repo.default_base_branch,
            repo.remote_label,
            repo.worktree_root,
            repo.entrypoint_count,
            repo.context_pack_count,
            repo.shared_skill_count,
            repo.mcp_config_present
        );
        println!("  root={}", repo.repo_root);
    }
}

fn print_context(context: &RepoContext) {
    println!();
    println!("Context library:");
    println!("- repo root: {}", context.repo_root);
    println!(
        "- entrypoints: {}",
        format_context_files(&context.entrypoints)
    );
    println!("- standards: {}", format_context_files(&context.standards));
    println!("- docs: {}", context.docs.len());
    if let Some(mcp_config) = &context.mcp_config_path {
        println!("- mcp: {}", mcp_config);
    } else {
        println!("- mcp: none");
    }
    if context.packs.is_empty() {
        println!("- packs: none");
    } else {
        println!("- packs:");
        for pack in &context.packs {
            println!("  - {} ({} files)", pack.name, pack.files.len());
            for file in &pack.files {
                println!("    - {}", file);
            }
        }
    }
}

fn print_context_doctor(report: &ContextDoctorReport) {
    println!();
    println!("Context doctor:");
    for diagnostic in &report.diagnostics {
        println!(
            "- [{}] {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
}

fn print_skills_catalog(catalog: &RepoSkillCatalog) {
    println!();
    println!("Skills catalog:");
    println!("- repo root: {}", catalog.repo_root);
    println!(
        "- shared root: {}",
        catalog.shared_root.as_deref().unwrap_or("none")
    );
    println!(
        "- lockfile: {}",
        catalog.lockfile_path.as_deref().unwrap_or("none")
    );
    if catalog.skills.is_empty() {
        println!("- skills: none");
    } else {
        println!("- skills:");
        for skill in &catalog.skills {
            println!(
                "  - {} name={} desc={}",
                skill.directory_name,
                skill.name.as_deref().unwrap_or("-"),
                skill.description.as_deref().unwrap_or("-")
            );
            println!("    source={}", skill.source_path);
        }
    }
    print_diagnostics(&catalog.diagnostics);
}

fn print_skill_doctor(reports: &[SkillDoctorReport]) {
    println!();
    println!("Skills doctor:");
    for report in reports {
        println!(
            "- runtime={} target={} strategy={} recommended_mode={}",
            report.runtime,
            report.target_dir.as_deref().unwrap_or("unresolved"),
            report.policy.discovery.as_str(),
            report.policy.recommended_mode.as_str()
        );
        println!("  note={}", report.policy.note);
        for entry in &report.entries {
            println!(
                "  - {} state={} target={}",
                entry.name,
                entry.state.as_str(),
                entry.target_path
            );
        }
        print_diagnostics(&report.diagnostics);
    }
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    println!("- diagnostics:");
    for diagnostic in diagnostics {
        println!(
            "  - [{}] {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
}

fn format_context_files(files: &[awo_core::context::ContextFile]) -> String {
    if files.is_empty() {
        return "none".to_string();
    }

    files
        .iter()
        .map(|file| file.label.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_slots(snapshot: &AppSnapshot, repo_filter: Option<&str>) {
    let slots = snapshot
        .slots
        .iter()
        .filter(|slot| repo_filter.is_none_or(|repo_id| slot.repo_id == repo_id))
        .collect::<Vec<_>>();
    if slots.is_empty() {
        println!("No slots.");
        return;
    }

    println!();
    println!("Slots:");
    for slot in slots {
        println!(
            "- {} [{}] repo={} branch={} status={} strategy={} dirty={} fp={}",
            slot.task_name,
            slot.id,
            slot.repo_id,
            slot.branch_name,
            slot.status,
            slot.strategy,
            slot.dirty,
            slot.fingerprint_status
        );
        println!("  path={}", slot.slot_path);
    }
}

fn print_sessions(snapshot: &AppSnapshot, repo_filter: Option<&str>) {
    let sessions = snapshot
        .sessions
        .iter()
        .filter(|session| {
            repo_filter.is_none_or(|repo_id| {
                snapshot
                    .slots
                    .iter()
                    .find(|slot| slot.id == session.slot_id)
                    .map(|slot| slot.repo_id == repo_id)
                    .unwrap_or(false)
            })
        })
        .collect::<Vec<_>>();
    if sessions.is_empty() {
        println!("No sessions.");
        return;
    }

    println!();
    println!("Sessions:");
    for session in sessions {
        println!(
            "- {} [{}] slot={} status={} read_only={} dry_run={} exit={}",
            session.runtime,
            session.id,
            session.slot_id,
            session.status,
            session.read_only,
            session.dry_run,
            session
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
        if let Some(log_path) = &session.log_path {
            println!("  log={log_path}");
        }
    }
}

fn print_review(review: &ReviewSummary) {
    println!();
    println!("Review summary:");
    println!("- active slots: {}", review.active_slots);
    println!("- releasable slots: {}", review.releasable_slots);
    println!("- dirty slots: {}", review.dirty_slots);
    println!("- stale slots: {}", review.stale_slots);
    println!("- pending sessions: {}", review.pending_sessions);
    println!("- completed sessions: {}", review.completed_sessions);
    println!("- failed sessions: {}", review.failed_sessions);
    if review.warnings.is_empty() {
        println!("- warnings: none");
        return;
    }

    println!("- warnings:");
    for warning in &review.warnings {
        println!("  - {}", warning.message);
    }
}

fn print_runtime_capabilities(capabilities: &[RuntimeCapabilityDescriptor]) {
    if capabilities.is_empty() {
        println!("No runtime capabilities found.");
        return;
    }

    println!("Runtime capabilities:");
    for capability in capabilities {
        println!(
            "- {} ({}) launch={} subagents={} teams={} skills={} mcp_reasoning={} interrupt={} resume={} structured={} read_only_hint={}",
            capability.display_name,
            capability.runtime,
            capability.default_launch_mode,
            capability.inline_subagents.as_str(),
            capability.multi_session_teams.as_str(),
            capability.skill_preload.as_str(),
            capability.reasoning_mcp_tools.as_str(),
            capability.interrupt.as_str(),
            capability.resume.as_str(),
            capability.structured_output.as_str(),
            capability.read_only_hint.as_str()
        );
        for note in &capability.notes {
            println!("  - note: {note}");
        }
    }
}

fn print_team_manifests(manifests: &[TeamManifest]) {
    if manifests.is_empty() {
        println!("No team manifests.");
        return;
    }

    println!("Team manifests:");
    for manifest in manifests {
        println!(
            "- {} repo={} status={} members={} tasks={}",
            manifest.team_id,
            manifest.repo_id,
            manifest.status,
            1 + manifest.members.len(),
            manifest.tasks.len()
        );
        println!("  objective={}", manifest.objective);
    }
}

fn print_team_manifest(manifest: &TeamManifest) {
    println!("Team manifest:");
    println!("- team id: {}", manifest.team_id);
    println!("- repo id: {}", manifest.repo_id);
    println!("- objective: {}", manifest.objective);
    println!("- status: {}", manifest.status);
    println!(
        "- lead: {} role={} runtime={} model={} mode={} read_only={}",
        manifest.lead.member_id,
        manifest.lead.role,
        manifest.lead.runtime.as_deref().unwrap_or("-"),
        manifest.lead.model.as_deref().unwrap_or("-"),
        manifest.lead.execution_mode,
        manifest.lead.read_only
    );
    if manifest.lead.context_packs.is_empty() {
        println!("- lead context packs: none");
    } else {
        println!(
            "- lead context packs: {}",
            manifest.lead.context_packs.join(", ")
        );
    }
    if manifest.lead.skills.is_empty() {
        println!("- lead skills: none");
    } else {
        println!("- lead skills: {}", manifest.lead.skills.join(", "));
    }

    if manifest.members.is_empty() {
        println!("- members: none");
    } else {
        println!("- members:");
        for member in &manifest.members {
            println!(
                "  - {} role={} runtime={} mode={} read_only={} scope={}",
                member.member_id,
                member.role,
                member.runtime.as_deref().unwrap_or("-"),
                member.execution_mode,
                member.read_only,
                if member.write_scope.is_empty() {
                    "-".to_string()
                } else {
                    member.write_scope.join(", ")
                }
            );
        }
    }

    if manifest.tasks.is_empty() {
        println!("- tasks: none");
    } else {
        println!("- tasks:");
        for task in &manifest.tasks {
            println!(
                "  - {} owner={} state={} deliverable={}",
                task.title, task.owner_id, task.state, task.deliverable
            );
            println!(
                "    scope={} verify={}",
                if task.write_scope.is_empty() {
                    "-".to_string()
                } else {
                    task.write_scope.join(", ")
                },
                if task.verification.is_empty() {
                    "-".to_string()
                } else {
                    task.verification.join(", ")
                }
            );
        }
    }
}
