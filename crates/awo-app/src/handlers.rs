use crate::cli::{
    AppCommand, ContextCommand, DebugCommand, RepoCommand, ReviewCommand, RuntimeCommand,
    SessionCommand, SkillsCommand, SlotCommand, TeamCommand, TeamMemberCommand, TeamTaskCommand,
};
use crate::output::{
    OutputMode, merge_command_outcomes, print_context, print_context_doctor, print_json_response,
    print_outcome, print_registered_repos, print_review, print_runtime_capabilities,
    print_sessions, print_skill_doctor, print_skills_catalog, print_slots, print_team_manifest,
    print_team_manifests, print_team_task_execution,
};
use crate::tui::run_tui;
use anyhow::{Result, bail};
use awo_core::{
    AppCore, Command, RuntimeKind, SessionLaunchMode, SkillLinkMode, SkillRuntime, SlotStrategy,
    TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamMember, TeamResetSummary,
    TeamTaskStartOptions, all_runtime_capabilities, default_team_manifest_path,
    runtime_capabilities, starter_team_manifest,
};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

pub fn initialize_tracing() -> Result<()> {
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

pub fn execute(command: AppCommand, output: OutputMode) -> Result<()> {
    match command {
        AppCommand::Tui => {
            if output.json {
                bail!("`--json` is not supported with the interactive TUI");
            }
            run_tui()
        }
        AppCommand::Repo { command } => run_repo(command, output),
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
    let mut core = AppCore::bootstrap()?;
    let outcome = match command {
        DebugCommand::Noop { label } => core.dispatch(Command::NoOp { label })?,
    };

    if output.json {
        print_json_response(&(), Some(&outcome));
    } else {
        print_outcome(&outcome);
    }
    Ok(())
}

fn run_repo(command: RepoCommand, output: OutputMode) -> Result<()> {
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

    let snapshot = core.snapshot()?;
    if output.json {
        print_json_response(&snapshot.registered_repos, Some(&outcome));
    } else {
        print_outcome(&outcome);
        print_registered_repos(&snapshot);
    }
    Ok(())
}

fn run_context(command: ContextCommand, output: OutputMode) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
    match command {
        ContextCommand::Pack { repo_id } => {
            let outcome = core.dispatch(Command::ContextPack {
                repo_id: repo_id.clone(),
            })?;
            let context = core.context_for_repo(&repo_id)?;
            if output.json {
                print_json_response(&context, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_context(&context);
            }
        }
        ContextCommand::Doctor { repo_id } => {
            let outcome = core.dispatch(Command::ContextDoctor {
                repo_id: repo_id.clone(),
            })?;
            let report = core.context_doctor_for_repo(&repo_id)?;
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
    let mut core = AppCore::bootstrap()?;
    match command {
        SkillsCommand::List { repo_id } => {
            let outcome = core.dispatch(Command::SkillsList {
                repo_id: repo_id.clone(),
            })?;
            let catalog = core.skills_for_repo(&repo_id)?;
            if output.json {
                print_json_response(&catalog, Some(&outcome));
            } else {
                print_outcome(&outcome);
                print_skills_catalog(&catalog);
            }
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
            let reports = core.skills_doctor_for_repo(&repo_id, &parsed_runtimes)?;
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
                let outcome = core.dispatch(Command::SkillsLink {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                outcomes.push(outcome);
                reports.extend(core.skills_doctor_for_repo(&repo_id, &[runtime])?);
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
                let outcome = core.dispatch(Command::SkillsSync {
                    repo_id: repo_id.clone(),
                    runtime,
                    mode,
                })?;
                outcomes.push(outcome);
                reports.extend(core.skills_doctor_for_repo(&repo_id, &[runtime])?);
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
    }

    Ok(())
}

fn run_team(command: TeamCommand, output: OutputMode) -> Result<()> {
    let mut core = AppCore::bootstrap()?;
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
                bail!("unknown repo id `{repo_id}`");
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
                bail!(
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
        TeamCommand::Member { command } => match command {
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
            } => {
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
                    },
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
                        depends_on,
                        state: TaskCardState::Todo,
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
            } => {
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
            if !force && (!summary.non_todo_tasks.is_empty() || !summary.bound_members.is_empty())
            {
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
    }

    Ok(())
}

fn run_slot(command: SlotCommand, output: OutputMode) -> Result<()> {
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

    let snapshot = core.snapshot()?;
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

    let snapshot = core.snapshot()?;
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
    let mut core = AppCore::bootstrap()?;
    let repo_filter = match &command {
        ReviewCommand::Status { repo_id } => repo_id.clone(),
    };
    let outcome = match command {
        ReviewCommand::Status { repo_id } => core.dispatch(Command::ReviewStatus { repo_id })?,
    };

    let snapshot = core.snapshot()?;
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
