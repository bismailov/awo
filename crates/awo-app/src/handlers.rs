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
    AppCore, Command, RoutingContext, RoutingPreferences, RoutingTarget, RuntimeKind,
    RuntimePressure, SessionLaunchMode, SkillLinkMode, SkillRuntime, SlotStrategy, TaskCard,
    TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamResetSummary,
    TeamTaskStartOptions, all_runtime_capabilities, default_team_manifest_path, route_runtime,
    runtime_capabilities, starter_team_manifest,
};
#[cfg(unix)]
use awo_core::{DaemonOptions, DaemonServer, DaemonStatus, get_daemon_status, stop_daemon};
use serde::Serialize;
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
}

impl CliBackend {
    fn bootstrap() -> Result<Self> {
        let core = AppCore::bootstrap()?;

        #[cfg(unix)]
        let daemon = {
            if awo_core::daemon_is_running(core.paths()) {
                match awo_core::DaemonClient::connect(&core.paths().daemon_socket_path()) {
                    Ok(client) => {
                        tracing::info!("connected to awod daemon");
                        Some(client)
                    }
                    Err(error) => {
                        tracing::warn!(%error, "daemon appears running but connection failed, using direct mode");
                        None
                    }
                }
            } else {
                None
            }
        };

        Ok(Self {
            core,
            #[cfg(unix)]
            daemon,
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
        AppCommand::Debug { command } => run_debug(command, output),
    }
}

fn run_debug(command: DebugCommand, output: OutputMode) -> Result<()> {
    let mut backend = CliBackend::bootstrap()?;
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
    let mut backend = CliBackend::bootstrap()?;
    let outcome = match command {
        RepoCommand::Add { path } => backend.dispatch(Command::RepoAdd { path: path.into() })?,
        RepoCommand::Clone {
            remote_url,
            destination,
        } => backend.dispatch(Command::RepoClone {
            remote_url,
            destination: destination.map(Into::into),
        })?,
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
                let json = match &status {
                    DaemonStatus::Running { pid, socket_ok } => serde_json::json!({
                        "status": "running",
                        "pid": pid,
                        "socket_ok": socket_ok
                    }),
                    DaemonStatus::NotRunning => serde_json::json!({
                        "status": "not_running"
                    }),
                };
                print_json_response(&json, None);
            } else {
                match status {
                    DaemonStatus::Running { pid, socket_ok } => {
                        let socket_msg = if socket_ok {
                            "socket ok"
                        } else {
                            "socket not responding"
                        };
                        println!("running (pid {}, {})", pid, socket_msg);
                    }
                    DaemonStatus::NotRunning => {
                        println!("not running");
                    }
                }
            }
            if !status.is_running() {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn run_context(command: ContextCommand, output: OutputMode) -> Result<()> {
    let mut backend = CliBackend::bootstrap()?;
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
    let mut backend = CliBackend::bootstrap()?;
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
    let mut backend = CliBackend::bootstrap()?;
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
    let mut backend = CliBackend::bootstrap()?;
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
            let snapshot = core.snapshot()?;
            if !snapshot
                .registered_repos
                .iter()
                .any(|repo| repo.id == repo_id)
            {
                bail!("unknown repo id `{repo_id}`");
            }

            let execution_mode = execution_mode
                .parse::<TeamExecutionMode>()
                .map_err(anyhow::Error::msg)?;
            let lead_runtime = parse_optional_runtime(lead_runtime.as_deref())?;
            let fallback_runtime = parse_optional_runtime(fallback_runtime.as_deref())?;
            let routing_preferences = parse_routing_preferences(
                prefer_local,
                avoid_metered,
                max_cost_tier.as_deref(),
                no_fallback,
            )?;
            let manifest_path = default_team_manifest_path(core.paths(), &team_id);
            if manifest_path.exists() && !force {
                bail!(
                    "team manifest `{team_id}` already exists at {} (pass --force to overwrite)",
                    manifest_path.display()
                );
            }

            let mut manifest = starter_team_manifest(
                &repo_id,
                &team_id,
                &objective,
                lead_runtime.as_deref(),
                lead_model.as_deref(),
                execution_mode,
                fallback_runtime.as_deref(),
                fallback_model.as_deref(),
            );
            manifest.routing_preferences = routing_preferences;
            let path = core.save_team_manifest(&manifest)?;
            if output.json {
                #[derive(Serialize)]
                struct TeamInitResult<'a> {
                    manifest_path: String,
                    manifest: &'a TeamManifest,
                }

                print_json_response(
                    &TeamInitResult {
                        manifest_path: path.display().to_string(),
                        manifest: &manifest,
                    },
                    None,
                );
            } else {
                println!("Saved starter team manifest to {}", path.display());
                print_team_manifest(&manifest);
            }
        }
        TeamCommand::List => {
            let manifests = core.list_team_manifests()?;
            if output.json {
                print_json_response(&manifests, None);
            } else {
                print_team_manifests(&manifests);
            }
        }
        TeamCommand::Show { team_id } => {
            let manifest = core.load_team_manifest(&team_id)?;
            if output.json {
                print_json_response(&manifest, None);
            } else {
                print_team_manifest(&manifest);
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
                let manifest = core.add_team_member(
                    &team_id,
                    TeamMember {
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
                )?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
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
                read_only,
                write_scope,
                deliverable,
                verification,
                depends_on,
            } => {
                let manifest = core.add_team_task(
                    &team_id,
                    TaskCard {
                        task_id,
                        title,
                        summary,
                        owner_id,
                        runtime: parse_optional_runtime(runtime.as_deref())?,
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
                        output_log_path: None,
                    },
                )?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
                    print_team_manifest(&manifest);
                }
            }
            TeamTaskCommand::State {
                team_id,
                task_id,
                state,
            } => {
                let manifest = core.set_team_task_state(
                    &team_id,
                    &task_id,
                    state.parse::<TaskCardState>().map_err(anyhow::Error::msg)?,
                )?;
                if output.json {
                    print_json_response(&manifest, None);
                } else {
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
                let (manifest, slot_outcome, session_outcome, execution) =
                    core.start_team_task(TeamTaskStartOptions {
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
                    })?;
                if output.json {
                    #[derive(Serialize)]
                    struct TeamTaskStartResult<'a> {
                        manifest: &'a TeamManifest,
                        execution: &'a awo_core::TeamTaskExecution,
                        slot_outcome: &'a Option<awo_core::CommandOutcome>,
                        session_outcome: &'a awo_core::CommandOutcome,
                    }

                    print_json_response(
                        &TeamTaskStartResult {
                            manifest: &manifest,
                            execution: &execution,
                            slot_outcome: &slot_outcome,
                            session_outcome: &session_outcome,
                        },
                        None,
                    );
                } else {
                    if let Some(outcome) = &slot_outcome {
                        print_outcome(outcome);
                    }
                    print_outcome(&session_outcome);
                    print_team_task_execution(&execution);
                    print_team_manifest(&manifest);
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
    let mut backend = CliBackend::bootstrap()?;
    let repo_filter = match &command {
        SlotCommand::List { repo_id } => repo_id.clone(),
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
    let mut backend = CliBackend::bootstrap()?;
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
    let mut backend = CliBackend::bootstrap()?;
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
