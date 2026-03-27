#[cfg(unix)]
use crate::cli::DaemonCommand;
use crate::cli::{
    AppCommand, ContextCommand, DebugCommand, RepoCommand, ReviewCommand, RuntimeCommand,
    RuntimePressureCommand, SessionCommand, SkillsCommand, SlotCommand, TeamCommand,
    TeamMemberCommand, TeamTaskCommand,
};
use crate::output::{
    OutputMode, merge_command_outcomes, print_context, print_context_doctor, print_json_response,
    print_outcome, print_registered_repos, print_review, print_routing_preview,
    print_routing_recommendation, print_runtime_capabilities, print_runtime_pressure_profile,
    print_session_log, print_sessions, print_skill_doctor, print_skills_catalog, print_slots,
    print_team_manifest, print_team_manifests, print_team_member, print_team_task_execution,
    print_team_teardown_plan, print_team_teardown_result,
};
use crate::tui::run_tui;
use anyhow::{Result, bail};
use awo_core::capabilities::CostTier;
use awo_core::commands::CommandOutcome;
#[cfg(unix)]
use awo_core::dispatch::Dispatcher;
use awo_core::error::AwoResult;
use awo_core::{
    AppCore, Command, DelegationContext, RoutingContext, RoutingPreferences, RoutingTarget,
    RuntimeKind, RuntimePressure, SessionLaunchMode, SkillLinkMode, SkillRuntime, SlotStrategy,
    TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamResetSummary,
    TeamTaskDelegateOptions, TeamTaskStartOptions, all_runtime_capabilities, route_runtime,
    runtime_capabilities,
};
#[cfg(unix)]
use awo_core::{DaemonOptions, DaemonServer, DaemonStatus, get_daemon_status, stop_daemon};
use serde::Serialize;
#[cfg(unix)]
use serde_json::json;
#[cfg(unix)]
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// CLI backend: auto-detects daemon and routes dispatch accordingly
// ---------------------------------------------------------------------------

/// Wraps `AppCore` with optional daemon dispatch.
///
/// When `awod` is running, mutations go through [`DaemonClient`] while
/// reads (snapshot, context, etc.) go directly to SQLite via `AppCore`
/// — safe because WAL mode supports concurrent readers.
struct CliBackend {
    core: AppCore,
    #[cfg(unix)]
    daemon: Option<awo_core::DaemonClient>,
    #[cfg(unix)]
    notice: Option<String>,
}

impl CliBackend {
    fn bootstrap() -> Result<Self> {
        let core = AppCore::bootstrap()?;

        #[cfg(unix)]
        let (daemon, notice) = bootstrap_daemon_transport(core.paths());

        Ok(Self {
            core,
            #[cfg(unix)]
            daemon,
            #[cfg(unix)]
            notice,
        })
    }

    fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        #[cfg(unix)]
        if let Some(client) = &mut self.daemon {
            return client.dispatch(command);
        }
        self.core.dispatch(command)
    }

    fn core(&self) -> &AppCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut AppCore {
        &mut self.core
    }

    #[cfg(unix)]
    fn emit_notice(&self, output: OutputMode) {
        if !output.json
            && let Some(notice) = &self.notice
        {
            eprintln!("Warning: {notice}");
        }
    }
}

#[cfg(unix)]
fn bootstrap_backend(output: OutputMode) -> Result<CliBackend> {
    let backend = CliBackend::bootstrap()?;
    backend.emit_notice(output);
    Ok(backend)
}

#[cfg(not(unix))]
fn bootstrap_backend(_output: OutputMode) -> Result<CliBackend> {
    CliBackend::bootstrap()
}

#[cfg(unix)]
fn bootstrap_daemon_transport(
    paths: &awo_core::AppPaths,
) -> (Option<awo_core::DaemonClient>, Option<String>) {
    match get_daemon_status(paths) {
        DaemonStatus::Healthy { pid } => match connect_daemon_client(paths) {
            Ok(client) => {
                tracing::info!(pid, "connected to healthy awod daemon");
                (Some(client), None)
            }
            Err(error) => {
                tracing::warn!(%error, pid, "daemon reported healthy but connection failed, using direct mode");
                (
                    None,
                    Some(format!(
                        "using direct mode because daemon reported healthy (pid {pid}) but connection failed: {error}"
                    )),
                )
            }
        },
        DaemonStatus::Starting { pid, .. } => {
            match wait_for_daemon_client(paths, Duration::from_secs(3)) {
                Ok(client) => {
                    tracing::info!(pid, "connected to awod daemon after startup wait");
                    (Some(client), None)
                }
                Err(error) => {
                    tracing::warn!(%error, pid, "daemon stayed in starting state, using direct mode");
                    (
                        None,
                        Some(format!(
                            "using direct mode because daemon is still starting (pid {pid}): {error}"
                        )),
                    )
                }
            }
        }
        DaemonStatus::Degraded { pid, issues } => {
            let issue_summary = format_daemon_issues(issues.as_slice());
            tracing::warn!(pid, issues = %issue_summary, "daemon is degraded, using direct mode");
            (
                None,
                Some(format!(
                    "using direct mode because daemon is degraded (pid {pid}, {issue_summary})"
                )),
            )
        }
        DaemonStatus::NotRunning => match awo_core::spawn_daemon(paths) {
            Ok(pid) => match connect_daemon_client(paths) {
                Ok(client) => {
                    tracing::info!(pid, "auto-started awod daemon");
                    (Some(client), None)
                }
                Err(error) => {
                    tracing::warn!(%error, pid, "auto-started daemon but connection failed, using direct mode");
                    (
                        None,
                        Some(format!(
                            "using direct mode because auto-started daemon (pid {pid}) was unreachable: {error}"
                        )),
                    )
                }
            },
            Err(error) => {
                tracing::debug!(%error, "could not auto-start daemon, using direct mode");
                (
                    None,
                    Some(format!(
                        "using direct mode because daemon auto-start failed: {error}"
                    )),
                )
            }
        },
    }
}

#[cfg(unix)]
fn connect_daemon_client(paths: &awo_core::AppPaths) -> AwoResult<awo_core::DaemonClient> {
    awo_core::DaemonClient::connect(&paths.daemon_socket_path())
}

#[cfg(unix)]
fn wait_for_daemon_client(
    paths: &awo_core::AppPaths,
    timeout: Duration,
) -> AwoResult<awo_core::DaemonClient> {
    let start = Instant::now();
    let interval = Duration::from_millis(100);

    while start.elapsed() < timeout {
        match get_daemon_status(paths) {
            DaemonStatus::Healthy { .. } => return connect_daemon_client(paths),
            DaemonStatus::Starting { .. } => std::thread::sleep(interval),
            DaemonStatus::Degraded { pid, issues } => {
                return Err(awo_core::AwoError::supervisor(format!(
                    "daemon became degraded (pid {pid}, {})",
                    format_daemon_issues(issues.as_slice())
                )));
            }
            DaemonStatus::NotRunning => {
                return Err(awo_core::AwoError::supervisor(
                    "daemon stopped before becoming healthy",
                ));
            }
        }
    }

    Err(awo_core::AwoError::supervisor(format!(
        "daemon did not become healthy within {}s",
        timeout.as_secs()
    )))
}

#[cfg(unix)]
fn format_daemon_issues(issues: &[awo_core::daemon::DaemonHealthIssue]) -> String {
    issues
        .iter()
        .map(|issue| issue.description())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(unix)]
fn daemon_status_payload(status: &DaemonStatus) -> serde_json::Value {
    json!({
        "status": status.state_label(),
        "running": status.is_running(),
        "healthy": status.is_healthy(),
        "pid": status.pid(),
        "issues": status
            .issues()
            .iter()
            .map(|issue| issue.description())
            .collect::<Vec<_>>(),
    })
}

#[cfg(unix)]
fn daemon_status_text(status: &DaemonStatus) -> String {
    match status {
        DaemonStatus::NotRunning => "not running".to_string(),
        DaemonStatus::Healthy { pid } => {
            format!("healthy (pid {pid}, socket accepting connections)")
        }
        DaemonStatus::Starting { pid, issues } => {
            format!(
                "starting (pid {pid}, {})",
                format_daemon_issues(issues.as_slice())
            )
        }
        DaemonStatus::Degraded { pid, issues } => {
            format!(
                "degraded (pid {pid}, {})",
                format_daemon_issues(issues.as_slice())
            )
        }
    }
}

pub fn initialize_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .map_err(anyhow::Error::from)?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(())
}

pub fn execute(command: AppCommand, output: OutputMode) -> Result<()> {
    match command {
        AppCommand::Tui => {
            if output.json {
                bail!("`--json` is not supported with the interactive TUI");
            }
            run_tui()
        }
        AppCommand::Repo { command } => run_repo(command, output),
        #[cfg(unix)]
        AppCommand::Daemon { command } => run_daemon(command, output),
        AppCommand::Context { command } => run_context(command, output),
        AppCommand::Skills { command } => run_skills(command, output),
        AppCommand::Runtime { command } => run_runtime(command, output),
        AppCommand::Team { command } => run_team(command, output),
        AppCommand::Slot { command } => run_slot(command, output),
        AppCommand::Session { command } => run_session(command, output),
        AppCommand::Review { command } => run_review(command, output),
        AppCommand::Help { manual } => run_help(manual),
        AppCommand::Debug { command } => run_debug(command, output),
    }
}

fn run_help(manual: bool) -> Result<()> {
    if manual {
        println!("{}", include_str!("../../../README.md"));
        return Ok(());
    }

    println!(
        r#"
AWO — Agent Workspace Orchestrator
==================================

AWO is a high-performance orchestration engine designed for complex multi-agent
software engineering missions. It manages isolated work environments, enforces
cost and performance policies via runtime routing, and tracks team progress
through structured task lifecycles.

CORE CONCEPTS:
  - Repositories: Git repositories registered with AWO. All agent work happens
    in isolated worktree "slots" derived from these repositories.
  - Slots: Ephemeral, isolated Git worktrees. Agents never work directly in
    your main repository, preventing "dirty" workspace issues.
  - Sessions: A single execution of an agent (e.g., Claude) or a script (Shell)
    within a slot. Sessions are logged and can be cancelled.
  - Teams: Logical groups of agents with defined roles, capabilities, and
    routing policies. Teams execute "missions" consisting of multiple tasks.
  - Runtimes: Different ways to execute work. AWO supports AI runtimes (Claude)
    and local execution (Shell). Routing logic decides which to use based on
    cost, availability, and task requirements.

COMMON WORKFLOWS:

  1. Solo Task Execution:
     $ awo repo add .
     $ awo slot acquire <REPO_ID> refactor-auth
     $ awo session start <SLOT_ID> claude "Improve error handling in auth.rs"
     $ awo session log <SESSION_ID> --stream combined
     $ awo slot release <SLOT_ID>

  2. Managed Team Mission:
     $ awo team init <REPO_ID> web-migration "Port frontend to Leptos 0.7"
     $ awo team member add web-migration lead lead --runtime claude --model opus
     $ awo team member add web-migration dev worker --runtime claude --model sonnet
     $ awo team task add web-migration port-components dev "Port UI" "..."
     $ awo team task start web-migration port-components
     $ awo team report web-migration

RUNTIME ROUTING & POLICIES:
  AWO uses a sophisticated routing engine. You can configure policies like:
  - --prefer-local: Always try Shell or local models first.
  - --avoid-metered: Do not use runtimes with per-token or per-minute costs.
  - --max-cost-tier: Limit execution to 'standard', 'premium', etc.
  - Runtime Pressure: Use 'awo runtime pressure set' to signal that a runtime
    is overloaded, triggering automatic fallback to other options.

COMMAND GROUPS:
  repo      - Manage registered repositories (add, clone, list, remove, fetch).
  slot      - Manage isolated worktrees (acquire, release, refresh, list).
  session   - Manage execution instances (start, cancel, log, delete, list).
  team      - Manage agent teams, members, tasks, and reports.
  runtime   - Inspect runtimes, capabilities, and routing policies.
  context   - Inspect repository context packed for agents.
  skills    - Manage and link external skills into runtimes.
  review    - View workspace health and dirty file warnings.
  daemon    - Control the background orchestration process.

Use 'awo <command> --help' for details on specific commands.
Use 'awo help --manual' to view the full project README and manual.
"#
    );

    Ok(())
}

fn run_debug(command: DebugCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    let outcome = match command {
        DebugCommand::Noop { label } => backend.dispatch(Command::NoOp { label })?,
    };

    if output.json {
        print_json_response(&(), Some(&outcome));
    } else {
        print_outcome(&outcome);
    }
    Ok(())
}

fn run_repo(command: RepoCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    let outcome = match command {
        RepoCommand::Add { path } => backend.dispatch(Command::RepoAdd { path: path.into() })?,
        RepoCommand::Clone {
            remote_url,
            destination,
        } => backend.dispatch(Command::RepoClone {
            remote_url,
            destination: destination.map(Into::into),
        })?,
        RepoCommand::Remove { repo_id } => backend.dispatch(Command::RepoRemove { repo_id })?,
        RepoCommand::Fetch { repo_id } => backend.dispatch(Command::RepoFetch { repo_id })?,
        RepoCommand::List => backend.dispatch(Command::RepoList)?,
    };

    let snapshot = backend.core().snapshot()?;
    if output.json {
        print_json_response(&snapshot.registered_repos, Some(&outcome));
    } else {
        print_outcome(&outcome);
        print_registered_repos(&snapshot);
    }
    Ok(())
}

#[cfg(unix)]
fn run_daemon(command: DaemonCommand, output: OutputMode) -> Result<()> {
    let core = AppCore::bootstrap()?;
    let paths = core.paths();

    match command {
        DaemonCommand::Start => {
            if output.json {
                bail!("`--json` is not supported with `daemon start` (runs in foreground)");
            }
            println!("Starting awod daemon in foreground...");
            let options = DaemonOptions::from_paths(paths);
            let server = DaemonServer::acquire(options)?;
            let mut dispatcher = core;
            server.run(&mut dispatcher)?;
        }
        DaemonCommand::Stop => {
            let message = stop_daemon(paths)?;
            if output.json {
                print_json_response(&message, None);
            } else {
                println!("{}", message);
            }
        }
        DaemonCommand::Status => {
            let status = get_daemon_status(paths);
            if output.json {
                print_json_response(&daemon_status_payload(&status), None);
            } else {
                println!("{}", daemon_status_text(&status));
            }
            if !status.is_running() {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn run_context(command: ContextCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    match command {
        ContextCommand::Pack { repo_id } => {
            let outcome = backend.dispatch(Command::ContextPack {
                repo_id: repo_id.clone(),
            })?;
            let context = backend.core().context_for_repo(&repo_id)?;
            if output.json {
                print_json_response(&context, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_context(&context);
            }
        }
        ContextCommand::Doctor { repo_id } => {
            let outcome = backend.dispatch(Command::ContextDoctor {
                repo_id: repo_id.clone(),
            })?;
            let report = backend.core().context_doctor_for_repo(&repo_id)?;
            if output.json {
                print_json_response(&report, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_context_doctor(&report);
            }
        }
    }

    Ok(())
}

fn run_skills(command: SkillsCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    match command {
        SkillsCommand::List { repo_id } => {
            let outcome = backend.dispatch(Command::SkillsList {
                repo_id: repo_id.clone(),
            })?;
            let catalog = backend.core().skills_for_repo(&repo_id)?;
            if output.json {
                print_json_response(&catalog, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_skills_catalog(&catalog);
            }
        }
        SkillsCommand::Doctor { repo_id, runtime } => {
            let parsed_runtimes = parse_skill_runtimes(runtime.as_deref())?;
            let outcome = backend.dispatch(Command::SkillsDoctor {
                repo_id: repo_id.clone(),
                runtime: parsed_runtimes
                    .first()
                    .copied()
                    .filter(|_| parsed_runtimes.len() == 1),
            })?;
            let reports = backend
                .core()
                .skills_doctor_for_repo(&repo_id, &parsed_runtimes)?;
            if output.json {
                print_json_response(&reports, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_skill_doctor(&reports);
            }
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
            let mut reports = Vec::new();
            let mut outcomes = Vec::new();
            for runtime in runtimes {
                let outcome = backend.dispatch(Command::SkillsLink {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                outcomes.push(outcome);
                reports.extend(
                    backend
                        .core()
                        .skills_doctor_for_repo(&repo_id, &[runtime])?,
                );
            }
            if output.json {
                let merged = merge_command_outcomes(outcomes);
                print_json_response(&reports, Some(&merged));
            } else {
                for outcome in &outcomes {
                    print_outcome(outcome);
                }
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
            let mut reports = Vec::new();
            let mut outcomes = Vec::new();
            for runtime in runtimes {
                let outcome = backend.dispatch(Command::SkillsSync {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                outcomes.push(outcome);
                reports.extend(
                    backend
                        .core()
                        .skills_doctor_for_repo(&repo_id, &[runtime])?,
                );
            }
            if output.json {
                let merged = merge_command_outcomes(outcomes);
                print_json_response(&reports, Some(&merged));
            } else {
                for outcome in &outcomes {
                    print_outcome(outcome);
                }
                print_skill_doctor(&reports);
            }
        }
    }

    Ok(())
}

fn run_runtime(command: RuntimeCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    match command {
        RuntimeCommand::List => {
            let capabilities = all_runtime_capabilities();
            if output.json {
                print_json_response(&capabilities, None);
            } else {
                print_runtime_capabilities(&capabilities);
            }
        }
        RuntimeCommand::Show { runtime } => {
            let runtime = runtime.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
            let capability = runtime_capabilities(runtime);
            if output.json {
                print_json_response(&vec![capability], None);
            } else {
                print_runtime_capabilities(&[capability]);
            }
        }
        RuntimeCommand::RoutePreview {
            primary,
            primary_model,
            fallback_runtime,
            fallback_model,
            prefer_local,
            avoid_metered,
            max_cost_tier,
            no_fallback,
            pressure,
        } => {
            let primary_kind = primary.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
            let primary_target = RoutingTarget::new(primary_kind, primary_model);

            let fallback_target = if let Some(fallback_runtime) = fallback_runtime {
                let fallback_kind = fallback_runtime
                    .parse::<RuntimeKind>()
                    .map_err(anyhow::Error::msg)?;
                Some(RoutingTarget::new(fallback_kind, fallback_model))
            } else {
                None
            };

            let max_cost_tier = if let Some(tier) = max_cost_tier {
                Some(tier.parse::<CostTier>().map_err(anyhow::Error::msg)?)
            } else {
                None
            };

            let preferences = RoutingPreferences {
                allow_fallback: !no_fallback,
                prefer_local,
                avoid_metered,
                max_cost_tier,
            };
            let context = resolve_routing_context(backend.core(), &pressure)?;

            let decision = route_runtime(primary_target, fallback_target, &preferences, &context);

            if output.json {
                print_json_response(&decision, None);
            } else {
                print_routing_preview(&decision, &preferences, &context);
            }
        }
        RuntimeCommand::Pressure { command } => {
            handle_runtime_pressure_command(backend.core_mut(), command, output)?
        }
    }

    Ok(())
}

fn handle_runtime_pressure_command(
    core: &mut AppCore,
    command: RuntimePressureCommand,
    output: OutputMode,
) -> Result<()> {
    match command {
        RuntimePressureCommand::Set {
            runtime_kind,
            pressure_level,
        } => {
            let runtime_kind = runtime_kind
                .parse::<RuntimeKind>()
                .map_err(anyhow::Error::msg)?;
            let pressure_level = pressure_level
                .parse::<RuntimePressure>()
                .map_err(anyhow::Error::msg)?;
            core.config_mut()
                .settings
                .runtime_pressure_profile
                .insert(runtime_kind, pressure_level);
            core.config().save_settings()?;
            if output.json {
                print_json_response(&core.config().settings.runtime_pressure_profile, None);
            } else {
                print_runtime_pressure_profile(&core.config().settings.runtime_pressure_profile);
            }
        }
        RuntimePressureCommand::Clear { runtime_kind } => {
            let runtime_kind = runtime_kind
                .parse::<RuntimeKind>()
                .map_err(anyhow::Error::msg)?;
            core.config_mut()
                .settings
                .runtime_pressure_profile
                .remove(&runtime_kind);
            core.config().save_settings()?;
            if output.json {
                print_json_response(&core.config().settings.runtime_pressure_profile, None);
            } else {
                print_runtime_pressure_profile(&core.config().settings.runtime_pressure_profile);
            }
        }
        RuntimePressureCommand::List => {
            if output.json {
                print_json_response(&core.config().settings.runtime_pressure_profile, None);
            } else {
                print_runtime_pressure_profile(&core.config().settings.runtime_pressure_profile);
            }
        }
    }
    Ok(())
}

fn run_team(command: TeamCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    let core = backend.core_mut();
    match command {
        TeamCommand::Init {
            repo_id,
            team_id,
            objective,
            lead_runtime,
            lead_model,
            execution_mode,
            fallback_runtime,
            fallback_model,
            prefer_local,
            avoid_metered,
            max_cost_tier,
            no_fallback,
            force,
        } => {
            let routing_preferences = parse_routing_preferences(
                prefer_local,
                avoid_metered,
                max_cost_tier.as_deref(),
                no_fallback,
            )?;

            let outcome = backend.dispatch(Command::TeamInit {
                team_id: team_id.clone(),
                repo_id,
                objective,
                lead_runtime,
                lead_model,
                execution_mode,
                fallback_runtime,
                fallback_model,
                routing_preferences,
                force,
            })?;

            if output.json {
                print_json_response(&outcome.data, Some(&outcome));
            } else {
                print_outcome(&outcome);
                // Also show the manifest
                let _manifest_outcome = backend.dispatch(Command::TeamShow { team_id })?;
                // TeamShow event contains the manifest in data if we are lucky, but wait
                // Actually CommandOutcome doesn't have data.
                // We'll just print the outcome for now.
            }
        }
        TeamCommand::List { repo_id } => {
            let outcome = backend.dispatch(Command::TeamList { repo_id })?;
            if output.json {
                print_json_response(&outcome.data, Some(&outcome));
            } else if let Some(data) = &outcome.data {
                let manifests: Vec<TeamManifest> = serde_json::from_value(data.clone())?;
                print_team_manifests(&manifests);
            } else {
                print_outcome(&outcome);
            }
        }
        TeamCommand::Show { team_id } => {
            let outcome = backend.dispatch(Command::TeamShow { team_id })?;
            if output.json {
                print_json_response(&outcome.data, Some(&outcome));
            } else if let Some(data) = &outcome.data {
                let manifest: TeamManifest = serde_json::from_value(data.clone())?;
                print_team_manifest(&manifest);
            } else {
                print_outcome(&outcome);
            }
        }
        TeamCommand::Recommend {
            team_id,
            member,
            task,
            pressure,
        } => {
            let context = resolve_routing_context(core, &pressure)?;
            let recommendation = core.recommend_team_routing(
                &team_id,
                member.as_deref(),
                task.as_deref(),
                &context,
            )?;
            if output.json {
                print_json_response(&recommendation, None);
            } else {
                print_routing_recommendation(&recommendation);
            }
        }
        TeamCommand::Member { command } => match command {
            TeamMemberCommand::Show { team_id, member_id } => {
                let manifest = core.load_team_manifest(&team_id)?;
                let member = if manifest.lead.member_id == member_id {
                    manifest.lead.clone()
                } else {
                    manifest
                        .members
                        .into_iter()
                        .find(|candidate| candidate.member_id == member_id)
                        .ok_or_else(|| anyhow::anyhow!("member not found: {}", member_id))?
                };
                if output.json {
                    print_json_response(&member, None);
                } else {
                    print_team_member(&member);
                }
            }
            TeamMemberCommand::Add {
                team_id,
                member_id,
                role,
                runtime,
                model,
                execution_mode,
                read_only,
                write_scope,
                context_pack,
                skill,
                notes,
                fallback_runtime,
                fallback_model,
                prefer_local,
                avoid_metered,
                max_cost_tier,
                no_fallback,
            } => {
                let routing_preferences = parse_routing_preferences(
                    prefer_local,
                    avoid_metered,
                    max_cost_tier.as_deref(),
                    no_fallback,
                )?;
                let outcome = backend.dispatch(Command::TeamMemberAdd {
                    team_id,
                    member: TeamMember {
                        member_id,
                        role,
                        runtime: parse_optional_runtime(runtime.as_deref())?,
                        model,
                        execution_mode: execution_mode
                            .parse::<TeamExecutionMode>()
                            .map_err(anyhow::Error::msg)?,
                        slot_id: None,
                        branch_name: None,
                        read_only,
                        write_scope,
                        context_packs: context_pack,
                        skills: skill,
                        notes,
                        fallback_runtime: parse_optional_runtime(fallback_runtime.as_deref())?,
                        fallback_model,
                        routing_preferences,
                    },
                })?;
                if output.json {
                    print_json_response(&outcome.data, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                }
            }
            TeamMemberCommand::Update {
                team_id,
                member_id,
                runtime,
                model,
                fallback_runtime,
                fallback_model,
                prefer_local,
                avoid_metered,
                max_cost_tier,
                no_fallback,
                clear_fallback,
                clear_routing_defaults,
            } => {
                let runtime_update = match runtime {
                    Some(value) => Some(parse_optional_runtime(Some(&value))?),
                    None => None,
                };
                let model_update = model.map(Some);
                let fallback_runtime_update = if clear_fallback {
                    Some(None)
                } else {
                    match fallback_runtime {
                        Some(value) => Some(parse_optional_runtime(Some(&value))?),
                        None => None,
                    }
                };
                let fallback_model_update = if clear_fallback {
                    Some(None)
                } else {
                    fallback_model.map(Some)
                };
                let routing_preferences_update = if clear_routing_defaults {
                    Some(None)
                } else {
                    parse_routing_preferences(
                        prefer_local,
                        avoid_metered,
                        max_cost_tier.as_deref(),
                        no_fallback,
                    )?
                    .map(Some)
                };
                let manifest = core.update_team_member_policy(
                    &team_id,
                    &member_id,
                    runtime_update,
                    model_update,
                    fallback_runtime_update,
                    fallback_model_update,
                    routing_preferences_update,
                )?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
                }
            }
            TeamMemberCommand::Remove { team_id, member_id } => {
                let manifest = core.remove_team_member(&team_id, &member_id)?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
                }
            }
            TeamMemberCommand::PromoteLead { team_id, member_id } => {
                let outcome = backend.dispatch(Command::TeamLeadReplace { team_id, member_id })?;
                if output.json {
                    print_json_response(&outcome.data, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                }
            }
            TeamMemberCommand::AssignSlot {
                team_id,
                member_id,
                slot_id,
            } => {
                let manifest = core.assign_team_member_slot(&team_id, &member_id, &slot_id)?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
                }
            }
        },
        TeamCommand::Task { command } => match command {
            TeamTaskCommand::Add {
                team_id,
                task_id,
                owner_id,
                title,
                summary,
                runtime,
                model,
                read_only,
                write_scope,
                deliverable,
                verification,
                depends_on,
            } => {
                let outcome = backend.dispatch(Command::TeamTaskAdd {
                    team_id,
                    task: TaskCard {
                        task_id,
                        title,
                        summary,
                        owner_id,
                        runtime: parse_optional_runtime(runtime.as_deref())?,
                        model,
                        slot_id: None,
                        branch_name: None,
                        read_only,
                        write_scope,
                        deliverable,
                        verification,
                        verification_command: None,
                        depends_on,
                        state: TaskCardState::Todo,
                        result_summary: None,
                        result_session_id: None,
                        handoff_note: None,
                        output_log_path: None,
                    },
                })?;
                if output.json {
                    print_json_response(&outcome.data, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                }
            }
            TeamTaskCommand::State {
                team_id,
                task_id,
                state,
            } => {
                let outcome = backend.dispatch(Command::TeamTaskState {
                    team_id,
                    task_id,
                    state: state.parse::<TaskCardState>().map_err(anyhow::Error::msg)?,
                })?;
                let manifest: TeamManifest = serde_json::from_value(
                    outcome
                        .data
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing manifest data"))?,
                )?;
                if output.json {
                    print_json_response(&manifest, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                    print_team_manifest(&manifest);
                }
            }
            TeamTaskCommand::Accept { team_id, task_id } => {
                let outcome = backend.dispatch(Command::TeamTaskAccept { team_id, task_id })?;
                let manifest: TeamManifest = serde_json::from_value(
                    outcome
                        .data
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing manifest data"))?,
                )?;
                if output.json {
                    print_json_response(&manifest, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                    print_team_manifest(&manifest);
                }
            }
            TeamTaskCommand::Rework { team_id, task_id } => {
                let outcome = backend.dispatch(Command::TeamTaskRework { team_id, task_id })?;
                let manifest: TeamManifest = serde_json::from_value(
                    outcome
                        .data
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing manifest data"))?,
                )?;
                if output.json {
                    print_json_response(&manifest, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                    print_team_manifest(&manifest);
                }
            }
            TeamTaskCommand::BindSlot {
                team_id,
                task_id,
                slot_id,
            } => {
                let manifest = core.bind_team_task_slot(&team_id, &task_id, &slot_id)?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
                }
            }
            TeamTaskCommand::Start {
                team_id,
                task_id,
                strategy,
                dry_run,
                launch_mode,
                no_auto_context,
                prefer_local,
                avoid_metered,
                max_cost_tier,
                no_fallback,
            } => {
                let routing_preferences = parse_routing_preferences(
                    prefer_local,
                    avoid_metered,
                    max_cost_tier.as_deref(),
                    no_fallback,
                )?;
                let outcome = backend.dispatch(Command::TeamTaskStart {
                    options: TeamTaskStartOptions {
                        team_id,
                        task_id,
                        strategy,
                        dry_run,
                        launch_mode: launch_mode.unwrap_or_else(|| {
                            SessionLaunchMode::default_for_environment()
                                .as_str()
                                .to_string()
                        }),
                        attach_context: !no_auto_context,
                        routing_preferences,
                    },
                })?;
                if output.json {
                    print_json_response(&outcome.data, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                    if let Some(data) = &outcome.data {
                        if let Ok(manifest) =
                            serde_json::from_value::<TeamManifest>(data["manifest"].clone())
                        {
                            print_team_manifest(&manifest);
                        }
                        if let Ok(execution) = serde_json::from_value::<awo_core::TeamTaskExecution>(
                            data["execution"].clone(),
                        ) {
                            print_team_task_execution(&execution);
                        }
                    }
                }
            }
            TeamTaskCommand::Delegate {
                team_id,
                task_id,
                target_member_id,
                notes,
                focus_file,
                auto_start,
                no_auto_start,
                strategy,
                dry_run,
                launch_mode,
            } => {
                let outcome = backend.dispatch(Command::TeamTaskDelegate {
                    options: TeamTaskDelegateOptions {
                        team_id,
                        task_id,
                        delegation: DelegationContext {
                            target_member_id,
                            lead_notes: notes,
                            focus_files: focus_file,
                            auto_start: auto_start && !no_auto_start,
                        },
                        strategy,
                        dry_run,
                        launch_mode: launch_mode.unwrap_or_else(|| {
                            SessionLaunchMode::default_for_environment()
                                .as_str()
                                .to_string()
                        }),
                        attach_context: true,
                    },
                })?;
                if output.json {
                    print_json_response(&outcome.data, Some(&outcome));
                } else {
                    print_outcome(&outcome);
                    if let Some(data) = &outcome.data {
                        if let Ok(manifest) =
                            serde_json::from_value::<TeamManifest>(data["manifest"].clone())
                        {
                            print_team_manifest(&manifest);
                        }
                        if let Ok(execution) = serde_json::from_value::<awo_core::TeamTaskExecution>(
                            data["execution"].clone(),
                        ) {
                            print_team_task_execution(&execution);
                        }
                    }
                }
            }
        },
        TeamCommand::Archive { team_id } => {
            let manifest = core.archive_team(&team_id)?;
            if output.json {
                print_json_response(&manifest, None);
            } else {
                println!("Team `{}` archived.", manifest.team_id);
                print_team_manifest(&manifest);
            }
        }
        TeamCommand::Reset { team_id, force } => {
            let preview = core.load_team_manifest(&team_id)?;
            let summary = preview.reset_summary();
            if !force && (!summary.non_todo_tasks.is_empty() || !summary.bound_members.is_empty()) {
                #[derive(Serialize)]
                struct ResetPreview<'a> {
                    team_id: &'a str,
                    summary: &'a TeamResetSummary,
                }

                if output.json {
                    print_json_response(
                        &ResetPreview {
                            team_id: &team_id,
                            summary: &summary,
                        },
                        None,
                    );
                } else {
                    println!("Reset would discard the following state:");
                    if !summary.non_todo_tasks.is_empty() {
                        println!("- tasks not in todo:");
                        for t in &summary.non_todo_tasks {
                            println!("  - {t}");
                        }
                    }
                    if !summary.bound_members.is_empty() {
                        println!(
                            "- members with slot bindings: {}",
                            summary.bound_members.join(", ")
                        );
                    }
                    println!("Pass --force to confirm.");
                }
                return Ok(());
            }
            let (manifest, _summary) = core.reset_team(&team_id)?;
            if output.json {
                print_json_response(&manifest, None);
            } else {
                println!("Team `{}` reset to planning.", manifest.team_id);
                print_team_manifest(&manifest);
            }
        }
        TeamCommand::Report { team_id } => {
            let outcome = backend.dispatch(Command::TeamReport { team_id })?;
            if output.json {
                print_json_response(&outcome.summary, Some(&outcome));
            } else {
                println!("{}", outcome.summary);
            }
        }
        TeamCommand::Teardown { team_id, force } => {
            let plan = core.plan_team_teardown(&team_id)?;
            if !force && plan.requires_confirmation() {
                if output.json {
                    print_json_response(&plan, None);
                } else {
                    print_team_teardown_plan(&team_id, &plan);
                    println!("Pass --force to confirm.");
                }
                return Ok(());
            }

            let (manifest, result) = core.teardown_team(&team_id)?;
            if output.json {
                #[derive(Serialize)]
                struct TeamTeardownResponse<'a> {
                    manifest: &'a TeamManifest,
                    result: &'a awo_core::TeamTeardownResult,
                }

                print_json_response(
                    &TeamTeardownResponse {
                        manifest: &manifest,
                        result: &result,
                    },
                    None,
                );
            } else {
                print_team_teardown_result(&team_id, &result);
                print_team_manifest(&manifest);
            }
        }
        TeamCommand::Delete { team_id } => {
            core.delete_team(&team_id)?;
            if output.json {
                #[derive(Serialize)]
                struct TeamDeleteResponse<'a> {
                    team_id: &'a str,
                    deleted: bool,
                }

                print_json_response(
                    &TeamDeleteResponse {
                        team_id: &team_id,
                        deleted: true,
                    },
                    None,
                );
            } else {
                println!("Deleted team manifest `{team_id}`.");
            }
        }
    }

    Ok(())
}

fn run_slot(command: SlotCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    let repo_filter = match &command {
        SlotCommand::List { repo_id } | SlotCommand::Prune { repo_id } => repo_id.clone(),
        _ => None,
    };
    let outcome = match command {
        SlotCommand::Acquire {
            repo_id,
            task_name,
            strategy,
        } => backend.dispatch(Command::SlotAcquire {
            repo_id,
            task_name,
            strategy: strategy
                .parse::<SlotStrategy>()
                .map_err(anyhow::Error::msg)?,
        })?,
        SlotCommand::List { repo_id } => backend.dispatch(Command::SlotList { repo_id })?,
        SlotCommand::Release { slot_id } => backend.dispatch(Command::SlotRelease { slot_id })?,
        SlotCommand::Delete { slot_id } => backend.dispatch(Command::SlotDelete { slot_id })?,
        SlotCommand::Prune { repo_id } => backend.dispatch(Command::SlotPrune { repo_id })?,
        SlotCommand::Refresh { slot_id } => backend.dispatch(Command::SlotRefresh { slot_id })?,
    };

    let snapshot = backend.core().snapshot()?;
    let slots = snapshot
        .slots
        .iter()
        .filter(|slot| {
            repo_filter
                .as_deref()
                .is_none_or(|repo_id| slot.repo_id == repo_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if output.json {
        print_json_response(&slots, Some(&outcome));
    } else {
        print_outcome(&outcome);
        print_slots(&snapshot, repo_filter.as_deref());
    }
    Ok(())
}

fn run_session(command: SessionCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
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
            timeout,
        } => {
            let runtime = runtime.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
            let launch_mode = match launch_mode {
                Some(mode) => mode
                    .parse::<SessionLaunchMode>()
                    .map_err(anyhow::Error::msg)?,
                None => SessionLaunchMode::default_for_environment(),
            };
            backend.dispatch(Command::SessionStart {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context: !no_auto_context,
                timeout_secs: timeout.map(|v| v as i64),
            })?
        }
        SessionCommand::List { repo_id } => backend.dispatch(Command::SessionList { repo_id })?,
        SessionCommand::Cancel { session_id } => {
            backend.dispatch(Command::SessionCancel { session_id })?
        }
        SessionCommand::Delete { session_id } => {
            backend.dispatch(Command::SessionDelete { session_id })?
        }
        SessionCommand::Log {
            session_id,
            lines,
            stream,
        } => {
            let outcome = backend.dispatch(Command::SessionLog {
                session_id,
                lines: Some(lines),
                stream: Some(stream),
            })?;
            if output.json {
                print_json_response(&(), Some(&outcome));
            } else {
                print_session_log(&outcome);
            }
            return Ok(());
        }
    };

    let snapshot = backend.core().snapshot()?;
    let sessions = snapshot
        .sessions
        .iter()
        .filter(|session| {
            repo_filter
                .as_deref()
                .is_none_or(|repo_id| session.repo_id == repo_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if output.json {
        print_json_response(&sessions, Some(&outcome));
    } else {
        print_outcome(&outcome);
        print_sessions(&snapshot, repo_filter.as_deref());
    }
    Ok(())
}

fn run_review(command: ReviewCommand, output: OutputMode) -> Result<()> {
    let mut backend = bootstrap_backend(output)?;
    let repo_filter = match &command {
        ReviewCommand::Status { repo_id } => repo_id.clone(),
    };
    let outcome = match command {
        ReviewCommand::Status { repo_id } => backend.dispatch(Command::ReviewStatus { repo_id })?,
    };

    let snapshot = backend.core().snapshot()?;
    let review = snapshot.review_for_repo(repo_filter.as_deref());
    if output.json {
        print_json_response(&review, Some(&outcome));
    } else {
        print_outcome(&outcome);
        print_review(&review);
    }
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

fn parse_optional_runtime(runtime: Option<&str>) -> Result<Option<String>> {
    runtime
        .map(|value| {
            value
                .parse::<RuntimeKind>()
                .map(|runtime| runtime.as_str().to_string())
                .map_err(anyhow::Error::msg)
        })
        .transpose()
}

fn parse_optional_cost_tier(cost_tier: Option<&str>) -> Result<Option<CostTier>> {
    cost_tier
        .map(|value| value.parse::<CostTier>().map_err(anyhow::Error::msg))
        .transpose()
}

fn parse_routing_preferences(
    prefer_local: bool,
    avoid_metered: bool,
    max_cost_tier: Option<&str>,
    no_fallback: bool,
) -> Result<Option<RoutingPreferences>> {
    let max_cost_tier = parse_optional_cost_tier(max_cost_tier)?;
    let has_override = prefer_local || avoid_metered || max_cost_tier.is_some() || no_fallback;
    if !has_override {
        return Ok(None);
    }

    Ok(Some(RoutingPreferences {
        allow_fallback: !no_fallback,
        prefer_local,
        avoid_metered,
        max_cost_tier,
    }))
}

fn parse_routing_context(pressure_entries: &[String]) -> Result<RoutingContext> {
    let mut context = RoutingContext::default();
    for entry in pressure_entries {
        let (runtime, pressure) = entry.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("invalid `--pressure` value `{entry}`; expected runtime=level")
        })?;
        let runtime = runtime.parse::<RuntimeKind>().map_err(anyhow::Error::msg)?;
        let pressure = pressure
            .parse::<RuntimePressure>()
            .map_err(anyhow::Error::msg)?;
        context.pressure.insert(runtime, pressure);
    }
    Ok(context)
}

fn resolve_routing_context(core: &AppCore, pressure_entries: &[String]) -> Result<RoutingContext> {
    if pressure_entries.is_empty() {
        return Ok(RoutingContext {
            pressure: core.config().settings.runtime_pressure_profile.clone(),
        });
    }

    parse_routing_context(pressure_entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn daemon_status_payload_reports_health_fields() {
        let payload = daemon_status_payload(&DaemonStatus::Degraded {
            pid: 4242,
            issues: vec![awo_core::daemon::DaemonHealthIssue::SocketUnreachable],
        });

        assert_eq!(payload["status"], "degraded");
        assert_eq!(payload["running"], true);
        assert_eq!(payload["healthy"], false);
        assert_eq!(payload["pid"], 4242);
        assert_eq!(
            payload["issues"].as_array().unwrap(),
            &vec![serde_json::Value::String(
                "socket not accepting connections".to_string()
            )]
        );
    }

    #[cfg(unix)]
    #[test]
    fn daemon_status_text_distinguishes_starting_and_healthy() {
        let starting = daemon_status_text(&DaemonStatus::Starting {
            pid: 100,
            issues: vec![awo_core::daemon::DaemonHealthIssue::SocketMissing],
        });
        assert!(starting.contains("starting"));
        assert!(starting.contains("socket file missing"));

        let healthy = daemon_status_text(&DaemonStatus::Healthy { pid: 200 });
        assert!(healthy.contains("healthy"));
        assert!(healthy.contains("socket accepting connections"));
    }
}
