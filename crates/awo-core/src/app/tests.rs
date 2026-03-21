use super::*;
use crate::commands::Command;
use crate::config::AppConfig;
use crate::runtime::{SessionLaunchMode, SessionRecord};
use crate::team::{TaskCard, TaskCardState, TeamExecutionMode, TeamMember, starter_team_manifest};
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

fn temp_core() -> Result<(tempfile::TempDir, AppCore)> {
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
    };
    let store = Store::open(&config.paths.state_db_path)?;
    store.initialize_schema()?;

    Ok((temp_dir, AppCore { config, store }))
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
            state: TaskCardState::Todo,
        },
    )?;

    let (manifest, slot_outcome, session_outcome, execution) =
        core.start_team_task(TeamTaskStartOptions {
            team_id: "team-beta".to_string(),
            task_id: "task-1".to_string(),
            strategy: "fresh".to_string(),
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot.as_str().to_string(),
            attach_context: false,
        })?;

    assert!(slot_outcome.is_some());
    assert_eq!(execution.runtime, "shell");
    assert_eq!(execution.session_status, "completed");
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
    slot.status = "released".to_string();
    core.store.upsert_slot(&slot)?;
    core.assign_team_member_slot("team-archive-session", "worker-a", &slot.id)?;
    core.bind_team_task_slot("team-archive-session", "task-1", &slot.id)?;
    core.store.upsert_session(&SessionRecord {
        id: "sess-archive-running".to_string(),
        repo_id: repo_id.clone(),
        slot_id: slot.id.clone(),
        runtime: "shell".to_string(),
        prompt: "sleep 30".to_string(),
        status: "running".to_string(),
        read_only: false,
        dry_run: false,
        command_line: "sh -lc 'sleep 30'".to_string(),
        stdout_path: Some("/tmp/archive-running.out.log".to_string()),
        stderr_path: Some("/tmp/archive-running.err.log".to_string()),
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let error = core
        .archive_team("team-archive-session")
        .expect_err("archive should block");
    assert!(error.to_string().contains("session `sess-archive-running`"));
    Ok(())
}
