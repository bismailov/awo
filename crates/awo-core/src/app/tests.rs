use super::*;
use crate::commands::Command;
use crate::config::{AppConfig, AppSettings};
use crate::runtime::{SessionLaunchMode, SessionRecord, SessionStatus};
use crate::slot::SlotStatus;
use crate::team::{
    DelegationContext, TaskCard, TaskCardState, TeamExecutionMode, TeamMember,
    TeamTaskDelegateOptions, TeamTaskStartOptions, starter_team_manifest,
};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

fn temp_core() -> Result<(tempfile::TempDir, AppCore)> {
    let _ = tracing_subscriber::fmt::try_init();
    let temp_dir = tempfile::tempdir()?;
    let config_dir = temp_dir.path().join("config");
    let data_dir = temp_dir.path().join("data");
    let logs_dir = data_dir.join("logs");
    let clones_dir = data_dir.join("clones");
    let repos_dir = config_dir.join("repos");
    let teams_dir = config_dir.join("teams");
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&clones_dir)?;
    fs::create_dir_all(&repos_dir)?;
    fs::create_dir_all(&teams_dir)?;

    let config = AppConfig {
        paths: AppPaths {
            config_dir,
            data_dir: data_dir.clone(),
            state_db_path: data_dir.join("state.sqlite3"),
            logs_dir,
            repos_dir,
            clones_dir,
            teams_dir,
        },
        settings: AppSettings::default(),
    };
    let store = Store::open(&config.paths.state_db_path)?;
    store.initialize_schema()?;

    Ok((
        temp_dir,
        AppCore {
            config,
            store,
            dirty_cache: std::cell::RefCell::new(crate::snapshot::DirtyFileCache::new()),
            event_bus: crate::events::EventBus::new(),
        },
    ))
}

fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

fn run_git_with_identity(dir: &Path, args: &[&str]) -> Result<()> {
    let output = ProcessCommand::new("git")
        .args([
            "-c",
            "user.name=AWO Tests",
            "-c",
            "user.email=awo-tests@example.com",
        ])
        .args(args)
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

fn create_repo(root: &Path, name: &str) -> Result<PathBuf> {
    let repo_dir = root.join(name);
    fs::create_dir_all(&repo_dir)?;
    run_git(&repo_dir, &["init", "-b", "main"])?;
    fs::write(repo_dir.join("README.md"), "hello\n")?;
    run_git(&repo_dir, &["add", "README.md"])?;
    run_git_with_identity(&repo_dir, &["commit", "-m", "init"])?;
    Ok(repo_dir)
}

fn team_task_start_options(team_id: &str, task_id: &str) -> TeamTaskStartOptions {
    TeamTaskStartOptions {
        team_id: team_id.to_string(),
        task_id: task_id.to_string(),
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
        attach_context: false,
        routing_preferences: None,
    }
}

fn create_routed_team_task(core: &mut AppCore, team_id: &str) -> Result<()> {
    let repo_dir = create_repo(&core.paths().data_dir, &format!("{team_id}-repo"))?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .find(|repo| repo.name == format!("{team_id}-repo"))
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        team_id,
        "Exercise routing preferences",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        team_id,
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("claude".to_string()),
            model: Some("sonnet".to_string()),
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: true,
            write_scope: Vec::new(),
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: Some("gemini".to_string()),
            fallback_model: Some("flash".to_string()),
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        team_id,
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Review task".to_string(),
            summary: "Review the routing behavior.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: None,
            slot_id: None,
            branch_name: None,
            read_only: true,
            write_scope: Vec::new(),
            deliverable: "A routing decision".to_string(),
            verification: vec!["true".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;
    Ok(())
}

fn create_routed_team_with_manifest_defaults(
    core: &mut AppCore,
    team_id: &str,
    routing_preferences: crate::routing::RoutingPreferences,
) -> Result<()> {
    create_routed_team_task(core, team_id)?;
    let mut manifest = core.load_team_manifest(team_id)?;
    manifest.routing_preferences = Some(routing_preferences);
    core.save_team_manifest(&manifest)?;
    Ok(())
}

fn set_member_routing_preferences(
    core: &mut AppCore,
    team_id: &str,
    member_id: &str,
    routing_preferences: crate::routing::RoutingPreferences,
) -> Result<()> {
    let mut manifest = core.load_team_manifest(team_id)?;
    let member = manifest
        .members
        .iter_mut()
        .find(|member| member.member_id == member_id)
        .context("missing team member")?;
    member.routing_preferences = Some(routing_preferences);
    core.save_team_manifest(&manifest)?;
    Ok(())
}

#[test]
fn team_member_and_task_mutations_persist() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-persist")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-alpha",
        "Ship the feature",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;

    core.add_team_member(
        "team-alpha",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    let manifest = core.add_team_task(
        "team-alpha",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Touch the repo".to_string(),
            summary: "printf ok > TEAM_TASK.txt".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["TEAM_TASK.txt".to_string()],
            deliverable: "A file".to_string(),
            verification: vec!["test -f TEAM_TASK.txt".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    assert_eq!(manifest.members.len(), 1);
    assert_eq!(manifest.tasks.len(), 1);
    Ok(())
}

#[test]
fn start_team_task_auto_acquires_slot_and_updates_state() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-start")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-beta",
        "Run a deterministic shell task",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-beta",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["TEAM_TASK.txt".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-beta",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Create task file".to_string(),
            summary: "printf ok > TEAM_TASK.txt".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["TEAM_TASK.txt".to_string()],
            deliverable: "A file".to_string(),
            verification: vec!["test -f TEAM_TASK.txt".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    let (manifest, slot_outcome, session_outcome, execution) =
        core.start_team_task(team_task_start_options("team-beta", "task-1"))?;

    assert!(slot_outcome.is_some());
    assert_eq!(execution.runtime, "shell");
    assert_eq!(execution.model, None);
    assert_eq!(
        execution.routing_source,
        crate::routing::RoutingSource::Primary
    );
    assert!(
        execution.routing_reason.contains("primary"),
        "expected primary-style reason, got: {}",
        execution.routing_reason
    );
    assert_eq!(execution.session_status, SessionStatus::Completed);
    assert!(session_outcome.summary.contains("Session"));
    assert_eq!(
        manifest.task("task-1").map(|task| task.state),
        Some(TaskCardState::Review)
    );
    let slot_path = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.id == execution.slot_id)
        .map(|slot| slot.slot_path)
        .context("missing slot summary")?;
    assert!(Path::new(&slot_path).join("TEAM_TASK.txt").exists());
    Ok(())
}

#[test]
fn start_team_task_missing_runtime_fails() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-start-missing")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-beta",
        "Run task without runtime",
        None,
        None,
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-beta",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: None,
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["TEAM_TASK.txt".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: Some("shell".to_string()),
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-beta",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Create task file".to_string(),
            summary: "printf ok > TEAM_TASK.txt".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["TEAM_TASK.txt".to_string()],
            deliverable: "A file".to_string(),
            verification: vec!["test -f TEAM_TASK.txt".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    let result = core.start_team_task(team_task_start_options("team-beta", "task-1"));

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("has no runtime"));
    Ok(())
}

#[test]
fn start_team_task_prefers_fallback_under_cost_ceiling() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-routing-fallback")?;

    let mut options = team_task_start_options("team-routing-fallback", "task-1");
    options.dry_run = true;
    options.routing_preferences = Some(crate::routing::RoutingPreferences {
        max_cost_tier: Some(crate::capabilities::CostTier::Standard),
        ..Default::default()
    });

    let (_manifest, _slot_outcome, _session_outcome, execution) = core.start_team_task(options)?;

    assert_eq!(execution.runtime, "gemini");
    assert_eq!(execution.model.as_deref(), Some("flash"));
    assert_eq!(
        execution.routing_source,
        crate::routing::RoutingSource::Fallback
    );
    assert!(
        execution.routing_reason.contains("fallback accepted"),
        "expected fallback reason, got: {}",
        execution.routing_reason
    );
    assert_eq!(execution.session_status, SessionStatus::Prepared);
    Ok(())
}

#[test]
fn start_team_task_no_fallback_preserves_primary_selection() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-routing-primary")?;

    let mut options = team_task_start_options("team-routing-primary", "task-1");
    options.dry_run = true;
    options.routing_preferences = Some(crate::routing::RoutingPreferences {
        allow_fallback: false,
        max_cost_tier: Some(crate::capabilities::CostTier::Standard),
        ..Default::default()
    });

    let (_manifest, _slot_outcome, _session_outcome, execution) = core.start_team_task(options)?;

    assert_eq!(execution.runtime, "claude");
    assert_eq!(execution.model.as_deref(), Some("sonnet"));
    assert_eq!(
        execution.routing_source,
        crate::routing::RoutingSource::Primary
    );
    assert!(
        execution
            .routing_reason
            .contains("fallback was not allowed"),
        "expected no-fallback reason, got: {}",
        execution.routing_reason
    );
    assert_eq!(execution.session_status, SessionStatus::Prepared);
    Ok(())
}

#[test]
fn team_task_routing_policy_persistence() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_with_manifest_defaults(
        &mut core,
        "team-routing-policy",
        crate::routing::RoutingPreferences {
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            ..Default::default()
        },
    )?;

    let mut inherited = team_task_start_options("team-routing-policy", "task-1");
    inherited.dry_run = true;
    let (_manifest, _slot_outcome, _session_outcome, inherited_execution) =
        core.start_team_task(inherited)?;
    assert_eq!(inherited_execution.runtime, "gemini");
    assert_eq!(
        inherited_execution.routing_source,
        crate::routing::RoutingSource::Fallback
    );

    let mut override_cli = team_task_start_options("team-routing-policy", "task-1");
    override_cli.dry_run = true;
    override_cli.routing_preferences = Some(crate::routing::RoutingPreferences {
        allow_fallback: false,
        max_cost_tier: Some(crate::capabilities::CostTier::Standard),
        ..Default::default()
    });
    let (_manifest, _slot_outcome, _session_outcome, override_execution) =
        core.start_team_task(override_cli)?;
    assert_eq!(override_execution.runtime, "claude");
    assert_eq!(
        override_execution.routing_source,
        crate::routing::RoutingSource::Primary
    );

    Ok(())
}

#[test]
fn team_task_start_cli_preferences_override_member_preferences() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-routing-member-override")?;
    set_member_routing_preferences(
        &mut core,
        "team-routing-member-override",
        "worker-a",
        crate::routing::RoutingPreferences {
            allow_fallback: false,
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            ..Default::default()
        },
    )?;

    let mut options = team_task_start_options("team-routing-member-override", "task-1");
    options.dry_run = true;
    options.routing_preferences = Some(crate::routing::RoutingPreferences {
        max_cost_tier: Some(crate::capabilities::CostTier::Standard),
        ..Default::default()
    });

    let (_manifest, _slot_outcome, _session_outcome, execution) = core.start_team_task(options)?;
    assert_eq!(execution.runtime, "gemini");
    assert_eq!(execution.model.as_deref(), Some("flash"));
    assert_eq!(
        execution.routing_source,
        crate::routing::RoutingSource::Fallback
    );
    Ok(())
}

#[test]
fn recommend_team_routing_for_member_uses_member_runtime() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-recommend-member")?;

    let recommendation = core.recommend_team_routing(
        "team-recommend-member",
        Some("worker-a"),
        None,
        &crate::routing::RoutingContext::default(),
    )?;

    assert_eq!(recommendation.member_id, "worker-a");
    assert_eq!(recommendation.task_id, None);
    assert_eq!(
        recommendation.decision.selected_runtime,
        crate::runtime::RuntimeKind::Claude
    );
    assert_eq!(
        recommendation.decision.selected_model.as_deref(),
        Some("sonnet")
    );
    assert_eq!(
        recommendation.decision.source,
        crate::routing::RoutingSource::Primary
    );
    Ok(())
}

#[test]
fn recommend_team_routing_for_task_respects_manifest_defaults() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_with_manifest_defaults(
        &mut core,
        "team-recommend-task",
        crate::routing::RoutingPreferences {
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            ..Default::default()
        },
    )?;

    let recommendation = core.recommend_team_routing(
        "team-recommend-task",
        None,
        Some("task-1"),
        &crate::routing::RoutingContext::default(),
    )?;

    assert_eq!(recommendation.member_id, "worker-a");
    assert_eq!(recommendation.task_id.as_deref(), Some("task-1"));
    assert_eq!(
        recommendation.preferences.max_cost_tier,
        Some(crate::capabilities::CostTier::Standard)
    );
    assert_eq!(
        recommendation.decision.selected_runtime,
        crate::runtime::RuntimeKind::Gemini
    );
    assert_eq!(
        recommendation.decision.source,
        crate::routing::RoutingSource::Fallback
    );
    Ok(())
}

#[test]
fn recommend_team_routing_member_preferences_override_team_defaults() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_with_manifest_defaults(
        &mut core,
        "team-recommend-member-policy",
        crate::routing::RoutingPreferences {
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            ..Default::default()
        },
    )?;
    set_member_routing_preferences(
        &mut core,
        "team-recommend-member-policy",
        "worker-a",
        crate::routing::RoutingPreferences {
            allow_fallback: false,
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            ..Default::default()
        },
    )?;

    let recommendation = core.recommend_team_routing(
        "team-recommend-member-policy",
        None,
        Some("task-1"),
        &crate::routing::RoutingContext::default(),
    )?;

    assert_eq!(
        recommendation.preferences.max_cost_tier,
        Some(crate::capabilities::CostTier::Standard)
    );
    assert!(!recommendation.preferences.allow_fallback);
    assert_eq!(
        recommendation.decision.selected_runtime,
        crate::runtime::RuntimeKind::Claude
    );
    assert_eq!(
        recommendation.decision.source,
        crate::routing::RoutingSource::Primary
    );
    Ok(())
}

#[test]
fn recommend_team_routing_rejects_invalid_selector_usage() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-recommend-invalid")?;

    let missing = core.recommend_team_routing(
        "team-recommend-invalid",
        None,
        None,
        &crate::routing::RoutingContext::default(),
    );
    assert!(missing.is_err());
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("choose one selector")
    );

    let both = core.recommend_team_routing(
        "team-recommend-invalid",
        Some("worker-a"),
        Some("task-1"),
        &crate::routing::RoutingContext::default(),
    );
    assert!(both.is_err());
    assert!(
        both.unwrap_err()
            .to_string()
            .contains("either `--member` or `--task`")
    );

    Ok(())
}

#[test]
fn recommend_team_routing_respects_pressure_context() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    create_routed_team_task(&mut core, "team-recommend-pressure")?;

    let mut context = crate::routing::RoutingContext::default();
    context.pressure.insert(
        crate::runtime::RuntimeKind::Claude,
        crate::routing::RuntimePressure::HardLimit,
    );

    let recommendation =
        core.recommend_team_routing("team-recommend-pressure", Some("worker-a"), None, &context)?;

    assert_eq!(
        recommendation.decision.selected_runtime,
        crate::runtime::RuntimeKind::Gemini
    );
    assert_eq!(
        recommendation.decision.source,
        crate::routing::RoutingSource::Fallback
    );
    assert_eq!(
        recommendation
            .context
            .pressure
            .get(&crate::runtime::RuntimeKind::Claude),
        Some(&crate::routing::RuntimePressure::HardLimit)
    );
    Ok(())
}

fn create_team_with_bound_slot(
    core: &mut AppCore,
    repo_name: &str,
    team_id: &str,
) -> Result<(String, String)> {
    let repo_dir = create_repo(&core.paths().data_dir, repo_name)?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .find(|repo| repo.name == repo_name)
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        team_id,
        "Exercise team reconciliation",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        team_id,
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        team_id,
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Reconcile task".to_string(),
            summary: "Run reconciliation.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            deliverable: "A reconciled task".to_string(),
            verification: vec!["cargo test".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: format!("{team_id}-slot"),
        strategy: crate::slot::SlotStrategy::Fresh,
    })?;
    let slot = core
        .store
        .list_slots(Some(&repo_id))?
        .into_iter()
        .find(|slot| slot.task_name == format!("{team_id}-slot"))
        .context("missing acquired slot")?;
    core.assign_team_member_slot(team_id, "worker-a", &slot.id)?;
    core.bind_team_task_slot(team_id, "task-1", &slot.id)?;
    core.set_team_task_state(team_id, "task-1", TaskCardState::InProgress)?;
    Ok((repo_id, slot.id))
}

#[test]
fn load_team_manifest_reconciles_completed_session_to_review() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (repo_id, slot_id) = create_team_with_bound_slot(
        &mut core,
        "team-reconcile-complete",
        "team-reconcile-complete",
    )?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-reconcile-complete".to_string(),
        repo_id,
        slot_id: slot_id.clone(),
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "echo done".to_string(),
        status: SessionStatus::Completed,
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'echo done'".to_string(),
        stdout_path: Some("/tmp/reconcile-complete.out.log".to_string()),
        stderr_path: Some("/tmp/reconcile-complete.err.log".to_string()),
        exit_code: Some(0),
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let manifest = core.load_team_manifest("team-reconcile-complete")?;
    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.state, TaskCardState::Review);
    assert_eq!(manifest.status, crate::team::TeamStatus::Running);
    assert_eq!(task.slot_id.as_deref(), Some(slot_id.as_str()));
    Ok(())
}

#[test]
fn load_team_manifest_reconciles_failed_session_to_blocked() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (repo_id, slot_id) =
        create_team_with_bound_slot(&mut core, "team-reconcile-failed", "team-reconcile-failed")?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-reconcile-failed".to_string(),
        repo_id,
        slot_id,
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "false".to_string(),
        status: SessionStatus::Failed,
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'false'".to_string(),
        stdout_path: Some("/tmp/reconcile-failed.out.log".to_string()),
        stderr_path: Some("/tmp/reconcile-failed.err.log".to_string()),
        exit_code: Some(1),
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let manifest = core.load_team_manifest("team-reconcile-failed")?;
    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.state, TaskCardState::Blocked);
    assert_eq!(manifest.status, crate::team::TeamStatus::Blocked);
    Ok(())
}

#[test]
fn load_team_manifest_clears_released_slot_bindings() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (repo_id, slot_id) = create_team_with_bound_slot(
        &mut core,
        "team-reconcile-release",
        "team-reconcile-release",
    )?;
    let mut slot = core
        .store
        .get_slot(&slot_id)?
        .context("missing acquired slot")?;
    slot.status = SlotStatus::Released;
    core.store.upsert_slot(&slot)?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-reconcile-release".to_string(),
        repo_id,
        slot_id,
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "echo done".to_string(),
        status: SessionStatus::Completed,
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'echo done'".to_string(),
        stdout_path: Some("/tmp/reconcile-release.out.log".to_string()),
        stderr_path: Some("/tmp/reconcile-release.err.log".to_string()),
        exit_code: Some(0),
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let manifest = core.load_team_manifest("team-reconcile-release")?;
    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.state, TaskCardState::Review);
    assert!(task.slot_id.is_none());
    assert!(task.branch_name.is_none());
    let member = manifest.member("worker-a").context("missing member")?;
    assert!(member.slot_id.is_none());
    assert!(member.branch_name.is_none());
    Ok(())
}

#[test]
fn archive_team_blocks_active_bound_slot() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-archive-slot")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-archive-slot",
        "Archive with slot safety",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-archive-slot",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-archive-slot",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Ready for archive".to_string(),
            summary: "Task already finished.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            deliverable: "A finished task".to_string(),
            verification: vec!["cargo test".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Done,
        },
    )?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "archive-worker".to_string(),
        strategy: crate::slot::SlotStrategy::Fresh,
    })?;
    let slot = core
        .store
        .list_slots(Some(&repo_id))?
        .into_iter()
        .next()
        .context("missing acquired slot")?;
    core.assign_team_member_slot("team-archive-slot", "worker-a", &slot.id)?;
    core.bind_team_task_slot("team-archive-slot", "task-1", &slot.id)?;

    let error = core
        .archive_team("team-archive-slot")
        .expect_err("archive should block");
    assert!(error.to_string().contains("still active"));
    Ok(())
}

#[test]
fn archive_team_blocks_running_session_for_bound_slot() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-archive-session")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-archive-session",
        "Archive with session safety",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-archive-session",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-archive-session",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Ready for archive".to_string(),
            summary: "Task already finished.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            deliverable: "A finished task".to_string(),
            verification: vec!["cargo test".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Done,
        },
    )?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "archive-running".to_string(),
        strategy: crate::slot::SlotStrategy::Fresh,
    })?;
    let mut slot = core
        .store
        .list_slots(Some(&repo_id))?
        .into_iter()
        .next()
        .context("missing acquired slot")?;
    slot.status = SlotStatus::Released;
    core.store.upsert_slot(&slot)?;
    core.assign_team_member_slot("team-archive-session", "worker-a", &slot.id)?;
    core.bind_team_task_slot("team-archive-session", "task-1", &slot.id)?;
    let mut child = ProcessCommand::new("sleep").arg("30").spawn()?;
    let sessions_dir = core.paths().logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(
        sessions_dir.join("sess-archive-running.pid"),
        child.id().to_string(),
    )?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-archive-running".to_string(),
        repo_id: repo_id.clone(),
        slot_id: slot.id.clone(),
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "sleep 30".to_string(),
        status: SessionStatus::Running,
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'sleep 30'".to_string(),
        stdout_path: Some("/tmp/archive-running.out.log".to_string()),
        stderr_path: Some("/tmp/archive-running.err.log".to_string()),
        exit_code: None,
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let error = core
        .archive_team("team-archive-session")
        .expect_err("archive should block");
    let _ = child.kill();
    let _ = child.wait();
    assert!(error.to_string().contains("session `sess-archive-running`"));
    Ok(())
}

#[test]
fn teardown_team_cancels_prepared_sessions_releases_slots_and_resets() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (_repo_id, slot_id) =
        create_team_with_bound_slot(&mut core, "team-teardown", "team-teardown")?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-team-teardown".to_string(),
        repo_id: core
            .store
            .get_slot(&slot_id)?
            .context("missing slot")?
            .repo_id,
        slot_id: slot_id.clone(),
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "echo hi".to_string(),
        status: SessionStatus::Prepared,
        read_only: false,
        dry_run: true,
        command_line: "sh -lc 'echo hi'".to_string(),
        stdout_path: Some("/tmp/team-teardown.out.log".to_string()),
        stderr_path: Some("/tmp/team-teardown.err.log".to_string()),
        exit_code: None,
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let (manifest, result) = core.teardown_team("team-teardown")?;
    assert_eq!(manifest.status, crate::team::TeamStatus::Planning);
    assert_eq!(
        manifest.task("task-1").map(|task| task.state),
        Some(TaskCardState::Todo)
    );
    assert_eq!(
        result.cancelled_sessions,
        vec!["sess-team-teardown".to_string()]
    );
    assert_eq!(result.released_slots, vec![slot_id.clone()]);
    let slot = core
        .store
        .get_slot(&slot_id)?
        .context("missing slot after teardown")?;
    assert_eq!(slot.status, SlotStatus::Released);
    let session = core
        .store
        .get_session("sess-team-teardown")?
        .context("missing session after teardown")?;
    assert_eq!(session.status, SessionStatus::Cancelled);
    Ok(())
}

#[test]
fn teardown_team_blocks_running_oneshot_sessions() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (_repo_id, slot_id) =
        create_team_with_bound_slot(&mut core, "team-teardown-blocked", "team-teardown-blocked")?;
    let mut child = ProcessCommand::new("sleep").arg("30").spawn()?;
    let sessions_dir = core.paths().logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(
        sessions_dir.join("sess-team-teardown-blocked.pid"),
        child.id().to_string(),
    )?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-team-teardown-blocked".to_string(),
        repo_id: core
            .store
            .get_slot(&slot_id)?
            .context("missing slot")?
            .repo_id,
        slot_id: slot_id.clone(),
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "sleep 30".to_string(),
        status: SessionStatus::Running,
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'sleep 30'".to_string(),
        stdout_path: Some("/tmp/team-teardown-blocked.out.log".to_string()),
        stderr_path: Some("/tmp/team-teardown-blocked.err.log".to_string()),
        exit_code: None,
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let error = core
        .teardown_team("team-teardown-blocked")
        .expect_err("teardown should block on running oneshot");
    let _ = child.kill();
    let _ = child.wait();
    assert!(error.to_string().contains("cannot be interrupted yet"));
    Ok(())
}

#[test]
fn delete_team_removes_manifest_once_bindings_are_gone() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-delete")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .find(|repo| repo.name == "team-delete")
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-delete",
        "Delete a dormant team",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;

    let manifest_path = crate::team::default_team_manifest_path(core.paths(), "team-delete");
    assert!(manifest_path.exists());
    core.delete_team("team-delete")?;
    assert!(!manifest_path.exists());
    Ok(())
}

#[test]
fn update_team_member_policy_persists_through_load() -> Result<()> {
    let (_temp_dir, core) = temp_core()?;
    let manifest = starter_team_manifest(
        "repo-1",
        "team-update",
        "Test member update",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-update",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("claude".to_string()),
            model: Some("sonnet".to_string()),
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;

    let manifest = core.update_team_member_policy(
        "team-update",
        "worker-a",
        None,
        None,
        Some(Some("gemini".to_string())),
        Some(Some("flash".to_string())),
        Some(Some(crate::routing::RoutingPreferences {
            prefer_local: true,
            avoid_metered: false,
            max_cost_tier: Some(crate::capabilities::CostTier::Standard),
            allow_fallback: true,
        })),
    )?;

    let member = manifest.member("worker-a").expect("member should exist");
    assert_eq!(member.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(member.fallback_model.as_deref(), Some("flash"));
    assert!(member.routing_preferences.as_ref().unwrap().prefer_local);

    let loaded = core.load_team_manifest("team-update")?;
    let member = loaded.member("worker-a").expect("member should exist");
    assert_eq!(member.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(member.fallback_model.as_deref(), Some("flash"));
    assert!(member.routing_preferences.as_ref().unwrap().prefer_local);
    assert_eq!(
        member.routing_preferences.as_ref().unwrap().max_cost_tier,
        Some(crate::capabilities::CostTier::Standard)
    );
    assert_eq!(member.runtime.as_deref(), Some("claude"));
    assert_eq!(member.model.as_deref(), Some("sonnet"));
    assert_eq!(member.role, "implementer");
    Ok(())
}

#[test]
fn delegate_team_task_updates_owner_state_and_prompt() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-delegate")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-delegate",
        "Test delegation",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-delegate",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-delegate",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Delegate me".to_string(),
            summary: "printf ok > DELEGATED.txt".to_string(),
            owner_id: "lead".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["DELEGATED.txt".to_string()],
            deliverable: "A file".to_string(),
            verification: vec!["test -f DELEGATED.txt".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    let (manifest, _, _, execution) = core.delegate_team_task(TeamTaskDelegateOptions {
        team_id: "team-delegate".to_string(),
        task_id: "task-1".to_string(),
        delegation: DelegationContext {
            target_member_id: "worker-a".to_string(),
            lead_notes: Some("Please do this carefully.".to_string()),
            focus_files: vec!["README.md".to_string()],
            auto_start: true,
        },
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
        attach_context: false,
    })?;

    assert_eq!(execution.owner_id, "worker-a");
    assert!(execution.prompt.contains("Please do this carefully."));
    assert!(execution.prompt.contains("### Focus Files\n- README.md"));

    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.owner_id, "worker-a");
    assert_ne!(task.state, TaskCardState::Todo);

    Ok(())
}

#[test]
fn delegate_team_task_with_auto_start_false_only_updates_manifest() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let repo_dir = create_repo(&core.paths().data_dir, "team-delegate-no-start")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        "team-delegate-no-start",
        "Test delegation without start",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-delegate-no-start",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["README.md".to_string()],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-delegate-no-start",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Delegate me".to_string(),
            summary: "printf ok > DELEGATED.txt".to_string(),
            owner_id: "lead".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["DELEGATED.txt".to_string()],
            deliverable: "A file".to_string(),
            verification: vec!["test -f DELEGATED.txt".to_string()],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    let (manifest, slot_outcome, session_outcome, execution) =
        core.delegate_team_task(TeamTaskDelegateOptions {
            team_id: "team-delegate-no-start".to_string(),
            task_id: "task-1".to_string(),
            delegation: DelegationContext {
                target_member_id: "worker-a".to_string(),
                lead_notes: None,
                focus_files: vec![],
                auto_start: false,
            },
            strategy: "fresh".to_string(),
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            attach_context: false,
        })?;

    assert!(slot_outcome.is_none());
    assert!(session_outcome.summary.contains("Delegated"));
    assert_eq!(execution.owner_id, "worker-a");
    assert_eq!(execution.session_status, SessionStatus::Prepared);
    assert!(!execution.acquired_slot);

    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.owner_id, "worker-a");
    assert_eq!(task.state, TaskCardState::Todo);

    Ok(())
}

#[test]
fn delegate_team_task_fails_for_unknown_member() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let manifest = starter_team_manifest(
        "repo-1",
        "team-delegate-err",
        "Test delegation errors",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_task(
        "team-delegate-err",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Delegate me".to_string(),
            summary: "summary".to_string(),
            owner_id: "lead".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec![],
            deliverable: "A file".to_string(),
            verification: vec![],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::Todo,
        },
    )?;

    let result = core.delegate_team_task(TeamTaskDelegateOptions {
        team_id: "team-delegate-err".to_string(),
        task_id: "task-1".to_string(),
        delegation: DelegationContext {
            target_member_id: "unknown-worker".to_string(),
            lead_notes: None,
            focus_files: vec![],
            auto_start: true,
        },
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
        attach_context: false,
    });

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("unknown target member")
    );
    Ok(())
}

#[test]
fn delegate_team_task_fails_for_non_todo_task() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let manifest = starter_team_manifest(
        "repo-1",
        "team-delegate-state-err",
        "Test delegation errors",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;
    core.add_team_member(
        "team-delegate-state-err",
        TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec![],
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_task(
        "team-delegate-state-err",
        TaskCard {
            task_id: "task-1".to_string(),
            title: "Delegate me".to_string(),
            summary: "summary".to_string(),
            owner_id: "lead".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec![],
            deliverable: "A file".to_string(),
            verification: vec![],
            depends_on: Vec::new(),
            verification_command: None,
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::InProgress,
        },
    )?;

    let result = core.delegate_team_task(TeamTaskDelegateOptions {
        team_id: "team-delegate-state-err".to_string(),
        task_id: "task-1".to_string(),
        delegation: DelegationContext {
            target_member_id: "worker-a".to_string(),
            lead_notes: None,
            focus_files: vec![],
            auto_start: true,
        },
        strategy: "fresh".to_string(),
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
        attach_context: false,
    });

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("expected `todo`"));
    Ok(())
}

#[test]
fn team_task_delegation_flow() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let team_id = "delegate-team";
    let task_id = "task-1";
    let member_a = "worker-a";
    let member_b = "worker-b";

    let repo_dir = create_repo(&core.paths().data_dir, "delegate-repo")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        team_id,
        "Delegation test",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;

    core.add_team_member(
        team_id,
        TeamMember {
            member_id: member_a.to_string(),
            role: "lead".to_string(),
            runtime: Some("claude".to_string()),
            model: Some("sonnet".to_string()),
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;
    core.add_team_member(
        team_id,
        TeamMember {
            member_id: member_b.to_string(),
            role: "worker".to_string(),
            runtime: Some("shell".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            context_packs: Vec::new(),
            skills: Vec::new(),
            notes: None,
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
    )?;

    core.add_team_task(
        team_id,
        TaskCard {
            task_id: task_id.to_string(),
            title: "Original task".to_string(),
            summary: "Work on it".to_string(),
            owner_id: member_a.to_string(),
            runtime: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "code".to_string(),
            verification: vec!["test".to_string()],
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
            result_summary: None,
            output_log_path: None,
        },
    )?;

    // Delegate task-1 from worker-a to worker-b
    let (manifest, _slot, _session, execution) =
        core.delegate_team_task(TeamTaskDelegateOptions {
            team_id: team_id.to_string(),
            task_id: task_id.to_string(),
            delegation: DelegationContext {
                target_member_id: member_b.to_string(),
                lead_notes: Some("Please handle this".to_string()),
                focus_files: Vec::new(),
                auto_start: false,
            },
            strategy: "fresh".to_string(),
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            attach_context: false,
        })?;

    assert_eq!(execution.owner_id, member_b);
    assert_eq!(execution.session_status, SessionStatus::Prepared);
    assert!(execution.prompt.contains("Please handle this"));

    let updated_task = manifest.task(task_id).context("task missing")?;
    assert_eq!(updated_task.owner_id, member_b);

    Ok(())
}

#[test]
fn team_task_state_transitions() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let team_id = "state-team";
    let task_id = "task-1";

    let repo_dir = create_repo(&core.paths().data_dir, "state-repo")?;
    core.dispatch(Command::RepoAdd {
        path: repo_dir.clone(),
    })?;
    let repo_id = core
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .map(|repo| repo.id)
        .context("missing registered repo")?;

    let manifest = starter_team_manifest(
        &repo_id,
        team_id,
        "State test",
        Some("claude"),
        Some("sonnet"),
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    core.save_team_manifest(&manifest)?;

    core.add_team_task(
        team_id,
        TaskCard {
            task_id: task_id.to_string(),
            title: "Task".to_string(),
            summary: "Work".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "code".to_string(),
            verification: vec!["test".to_string()],
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
            result_summary: None,
            output_log_path: None,
        },
    )?;

    // Transition to InProgress
    let manifest = core.set_team_task_state(team_id, task_id, TaskCardState::InProgress)?;
    assert_eq!(
        manifest.task(task_id).unwrap().state,
        TaskCardState::InProgress
    );

    // Transition to Review
    let manifest = core.set_team_task_state(team_id, task_id, TaskCardState::Review)?;
    assert_eq!(manifest.task(task_id).unwrap().state, TaskCardState::Review);

    // Transition to Done
    let manifest = core.set_team_task_state(team_id, task_id, TaskCardState::Done)?;
    assert_eq!(manifest.task(task_id).unwrap().state, TaskCardState::Done);

    Ok(())
}

#[test]
fn test_reconcile_released_slot_clears_bindings() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (_repo_id, slot_id) = create_team_with_bound_slot(&mut core, "team-rel", "team-rel")?;

    core.dispatch(Command::SlotRelease { slot_id })?;

    let manifest = core.load_team_manifest("team-rel")?;
    let task = manifest.task("task-1").context("missing task")?;
    assert_eq!(task.state, TaskCardState::Blocked);
    assert!(task.slot_id.is_none());

    let member = manifest.member("worker-a").context("missing worker")?;
    assert!(member.slot_id.is_none());

    Ok(())
}

#[test]
fn test_reconcile_successful_verification_review() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (repo_id, slot_id) =
        create_team_with_bound_slot(&mut core, "team-ver-pass", "team-ver-pass")?;

    core.add_team_task(
        "team-ver-pass",
        TaskCard {
            task_id: "task-2".to_string(),
            title: "Task 2".to_string(),
            summary: "summary".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: Some(slot_id.clone()),
            branch_name: None,
            read_only: false,
            write_scope: vec![],
            deliverable: "del".to_string(),
            verification: vec![],
            depends_on: vec![],
            verification_command: Some("true".to_string()),
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::InProgress,
        },
    )?;

    core.store.upsert_session(&SessionRecord {
        id: "sess-ver-pass".to_string(),
        repo_id,
        slot_id,
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "echo done".to_string(),
        status: SessionStatus::Completed,
        read_only: false,
        dry_run: false,
        command_line: "echo done".to_string(),
        stdout_path: None,
        stderr_path: None,
        exit_code: Some(0),
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let updated_manifest = core.load_team_manifest("team-ver-pass")?;
    let updated_task = updated_manifest.task("task-2").context("missing task")?;
    assert_eq!(updated_task.state, TaskCardState::Review);
    assert_eq!(
        updated_task.result_summary.as_deref(),
        Some("Verification passed.")
    );

    Ok(())
}

#[test]
fn test_reconcile_failed_verification_blocks() -> Result<()> {
    let (_temp_dir, mut core) = temp_core()?;
    let (repo_id, slot_id) =
        create_team_with_bound_slot(&mut core, "team-ver-fail", "team-ver-fail")?;

    core.add_team_task(
        "team-ver-fail",
        TaskCard {
            task_id: "task-2".to_string(),
            title: "Task 2".to_string(),
            summary: "summary".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("shell".to_string()),
            slot_id: Some(slot_id.clone()),
            branch_name: None,
            read_only: false,
            write_scope: vec![],
            deliverable: "del".to_string(),
            verification: vec![],
            depends_on: vec![],
            verification_command: Some("false".to_string()),
            result_summary: None,
            output_log_path: None,
            state: TaskCardState::InProgress,
        },
    )?;

    core.store.upsert_session(&SessionRecord {
        id: "sess-ver-fail".to_string(),
        repo_id,
        slot_id,
        runtime: "shell".to_string(),
        supervisor: None,
        prompt: "echo done".to_string(),
        status: SessionStatus::Completed,
        read_only: false,
        dry_run: false,
        command_line: "echo done".to_string(),
        stdout_path: None,
        stderr_path: None,
        exit_code: Some(0),
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let updated_manifest = core.load_team_manifest("team-ver-fail")?;
    let updated_task = updated_manifest.task("task-2").context("missing task")?;
    assert_eq!(updated_task.state, TaskCardState::Blocked);
    assert!(
        updated_task
            .result_summary
            .as_deref()
            .unwrap_or_default()
            .contains("Verification failed")
    );

    Ok(())
}
