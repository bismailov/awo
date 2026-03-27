#![allow(unused_crate_dependencies)]

use anyhow::{Context, Result, bail};
use awo_core::app::AppPaths;
use awo_core::config::{AppConfig, AppSettings};
use awo_core::runtime::{RuntimeKind, SessionLaunchMode, SessionStatus, detect_tmux};
use awo_core::{AppCore, Command, SlotStatus, SlotStrategy, TaskCardState};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use tempfile::TempDir;

struct TestHarness {
    _temp_dir: TempDir,
    config: AppConfig,
}

impl TestHarness {
    fn new() -> Result<Self> {
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

        Ok(Self {
            _temp_dir: temp_dir,
            config: AppConfig {
                paths: AppPaths {
                    config_dir,
                    data_dir: data_dir.clone(),
                    state_db_path: data_dir.join("state.sqlite3"),
                    logs_dir,
                    repos_dir,
                    clones_dir,
                    worktrees_dir: data_dir.join("worktrees"),
                    teams_dir,
                },
                settings: AppSettings::default(),
            },
        })
    }

    fn core(&self) -> Result<AppCore> {
        Ok(AppCore::from_config(self.config.clone())?)
    }

    fn create_repo(&self, name: &str) -> Result<PathBuf> {
        let repo_dir = self.config.paths.data_dir.join(name);
        fs::create_dir_all(&repo_dir)?;
        run_git(&repo_dir, ["init", "-b", "main"])?;
        fs::write(repo_dir.join("README.md"), "hello\n")?;
        run_git(&repo_dir, ["add", "README.md"])?;
        run_git_with_identity(&repo_dir, ["commit", "-m", "init"])?;
        Ok(repo_dir)
    }

    fn register_repo(&self, repo_dir: PathBuf) -> Result<String> {
        let expected_name = repo_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .context("repo dir missing final path component")?;
        let mut core = self.core()?;
        core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let snapshot = core.snapshot()?;
        let repo = snapshot
            .registered_repos
            .into_iter()
            .find(|repo| repo.name == expected_name)
            .context("registered repo not found")?;
        Ok(repo.id)
    }

    fn create_bare_remote(&self, name: &str) -> Result<PathBuf> {
        let bare_dir = self.config.paths.data_dir.join(format!("{name}.git"));
        let bare_dir_string = bare_dir.display().to_string();
        run_git_root(
            self.config.paths.data_dir.as_path(),
            [
                "init",
                "--bare",
                "--initial-branch=main",
                bare_dir_string.as_str(),
            ],
        )?;

        let seed_dir = self.config.paths.data_dir.join(format!("{name}-seed"));
        fs::create_dir_all(&seed_dir)?;
        run_git(&seed_dir, ["init", "-b", "main"])?;
        fs::write(seed_dir.join("README.md"), "seed\n")?;
        run_git(&seed_dir, ["add", "README.md"])?;
        run_git_with_identity(&seed_dir, ["commit", "-m", "seed"])?;
        run_git_root(
            &seed_dir,
            ["remote", "add", "origin", bare_dir_string.as_str()],
        )?;
        run_git_with_identity(&seed_dir, ["push", "-u", "origin", "main"])?;

        Ok(bare_dir)
    }
}

fn wait_for_repo_sessions(
    core: &AppCore,
    repo_id: &str,
    timeout: Duration,
) -> Result<Vec<awo_core::SessionSummary>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let sessions = core
            .snapshot()?
            .sessions
            .into_iter()
            .filter(|session| session.repo_id == repo_id)
            .collect::<Vec<_>>();
        if !sessions.is_empty() {
            return Ok(sessions);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(sessions);
        }
        sleep(Duration::from_millis(50));
    }
}

#[test]
fn warm_slot_reuse_preserves_slot_identity() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("warm-reuse")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "first task".to_string(),
        strategy: SlotStrategy::Warm,
    })?;
    let first_slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing first slot")?;

    core.dispatch(Command::SlotRelease {
        slot_id: first_slot.id.clone(),
    })?;
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "second task".to_string(),
        strategy: SlotStrategy::Warm,
    })?;

    let slots = core
        .snapshot()?
        .slots
        .into_iter()
        .filter(|slot| slot.repo_id == repo_id)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), 1);
    let reused = &slots[0];
    assert_eq!(reused.id, first_slot.id);
    assert_eq!(reused.slot_path, first_slot.slot_path);
    assert_eq!(reused.task_name, "second task");
    assert_eq!(reused.status, SlotStatus::Active);

    Ok(())
}

#[test]
fn release_blocks_dirty_slot() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("dirty-slot")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "dirty task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.task_name == "dirty task")
        .context("missing dirty slot")?;
    fs::write(Path::new(&slot.slot_path).join("DIRTY.txt"), "dirty\n")?;

    let error = core
        .dispatch(Command::SlotRelease {
            slot_id: slot.id.clone(),
        })
        .expect_err("dirty slot release should fail");
    assert!(error.to_string().contains("dirty"));

    Ok(())
}

#[test]
fn release_blocks_pending_session() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("busy-slot")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "busy task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing busy slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf pending".to_string(),
        read_only: false,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;

    let error = core
        .dispatch(Command::SlotRelease {
            slot_id: slot.id.clone(),
        })
        .expect_err("busy slot release should fail");
    assert!(error.to_string().contains("pending session"));

    Ok(())
}

#[test]
fn cancelling_pending_session_unblocks_release() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("cancel-session")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "cancel task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing cancel slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf pending".to_string(),
        read_only: false,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .map(|session| session.id)
        .context("missing pending session")?;

    core.dispatch(Command::SessionCancel { session_id })?;
    core.dispatch(Command::SlotRelease {
        slot_id: slot.id.clone(),
    })?;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .context("missing cancelled session")?;
    assert_eq!(session.status, SessionStatus::Cancelled);

    Ok(())
}

#[test]
fn deleting_terminal_session_removes_it_from_state() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("delete-session")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "delete task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing delete slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf pending".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .map(|session| session.id)
        .context("missing prepared session")?;

    core.dispatch(Command::SessionCancel {
        session_id: session_id.clone(),
    })?;
    core.dispatch(Command::SessionDelete {
        session_id: session_id.clone(),
    })?;

    assert!(
        core.snapshot()?
            .sessions
            .into_iter()
            .all(|session| session.id != session_id)
    );

    Ok(())
}

#[test]
fn deleting_released_warm_slot_removes_worktree_and_state() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("delete-slot")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "cleanup me".to_string(),
        strategy: SlotStrategy::Warm,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.task_name == "cleanup me")
        .context("missing cleanup slot")?;

    core.dispatch(Command::SlotRelease {
        slot_id: slot.id.clone(),
    })?;
    assert!(Path::new(&slot.slot_path).exists());

    core.dispatch(Command::SlotDelete {
        slot_id: slot.id.clone(),
    })?;

    assert!(
        core.snapshot()?
            .slots
            .into_iter()
            .all(|candidate| candidate.id != slot.id)
    );
    assert!(!Path::new(&slot.slot_path).exists());
    Ok(())
}

#[test]
fn deleting_active_slot_requires_release_first() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("delete-active-slot")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "still active".to_string(),
        strategy: SlotStrategy::Warm,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.task_name == "still active")
        .context("missing slot")?;

    let error = core
        .dispatch(Command::SlotDelete {
            slot_id: slot.id.clone(),
        })
        .expect_err("active slot delete should fail");
    assert!(error.to_string().contains("release it first"));
    Ok(())
}

#[test]
fn pruning_released_slots_removes_multiple_retained_worktrees() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("prune-slots")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "keep-one".to_string(),
        strategy: SlotStrategy::Warm,
    })?;
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "keep-two".to_string(),
        strategy: SlotStrategy::Warm,
    })?;

    let slots = core
        .snapshot()?
        .slots
        .into_iter()
        .filter(|slot| slot.repo_id == repo_id)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), 2);

    for slot in &slots {
        core.dispatch(Command::SlotRelease {
            slot_id: slot.id.clone(),
        })?;
        assert!(Path::new(&slot.slot_path).exists());
    }

    let outcome = core.dispatch(Command::SlotPrune {
        repo_id: Some(repo_id),
    })?;
    assert!(outcome.summary.contains("Pruned 2 released slot(s)"));
    assert!(core.snapshot()?.slots.is_empty());
    Ok(())
}

#[test]
fn repo_clone_registers_remote_repo() -> Result<()> {
    let harness = TestHarness::new()?;
    let remote = harness.create_bare_remote("remote-clone")?;
    let mut core = harness.core()?;

    core.dispatch(Command::RepoClone {
        remote_url: remote.display().to_string(),
        destination: None,
    })?;

    let repo = core
        .snapshot()?
        .registered_repos
        .into_iter()
        .next()
        .context("missing cloned repo")?;
    assert!(Path::new(&repo.repo_root).exists());
    let remote_url = git_stdout(Path::new(&repo.repo_root), ["remote", "get-url", "origin"])?;
    assert_eq!(remote_url.trim(), remote.display().to_string());

    Ok(())
}

#[test]
#[cfg_attr(windows, ignore = "ConPTY is currently a stub")]
fn pty_session_runs_and_syncs_to_completion() -> Result<()> {
    if !detect_tmux() {
        return Ok(());
    }

    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("pty-session")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "pty task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing PTY slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf pty-ok; sleep 1; printf done".to_string(),
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Pty,
        attach_context: false,
        timeout_secs: None,
    })?;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .context("missing PTY session")?;
    assert_eq!(session.status, SessionStatus::Running);

    let session_id = session.id.clone();

    let mut session = session;
    for _ in 0..20 {
        sleep(Duration::from_millis(500));
        session = core
            .snapshot()?
            .sessions
            .into_iter()
            .find(|session| session.id == session_id)
            .context("missing synced PTY session")?;
        if session.status == SessionStatus::Completed {
            break;
        }
    }
    assert_eq!(session.status, SessionStatus::Completed);
    assert_eq!(session.exit_code, Some(0));
    let log_path = session.log_path.context("missing PTY log path")?;
    let log = fs::read_to_string(&log_path)?;
    assert!(log.contains("pty-ok"));
    assert!(log.contains("done"));
    let _ = kill_tmux_session(&session.id);

    Ok(())
}

#[test]
fn repo_scoped_review_summary_excludes_other_repos() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_a = harness.register_repo(harness.create_repo("review-a")?)?;
    let repo_b = harness.register_repo(harness.create_repo("review-b")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_a.clone(),
        task_name: "repo-a".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_b.clone(),
        task_name: "repo-b".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let snapshot = core.snapshot()?;
    let slot_a = snapshot
        .slots
        .iter()
        .find(|slot| slot.repo_id == repo_a)
        .cloned()
        .context("missing repo-a slot")?;
    let slot_b = snapshot
        .slots
        .iter()
        .find(|slot| slot.repo_id == repo_b)
        .cloned()
        .context("missing repo-b slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot_a.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "true".to_string(),
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;

    let config = harness.config.clone();
    let slot_b_id = slot_b.id.clone();
    let worker = std::thread::spawn(move || -> Result<()> {
        let mut core = AppCore::from_config(config)?;
        core.dispatch(Command::SessionStart {
            slot_id: slot_b_id,
            runtime: RuntimeKind::Shell,
            prompt: "sleep 1; printf visible".to_string(),
            read_only: true,
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot,
            attach_context: false,
            timeout_secs: None,
        })?;
        Ok(())
    });

    sleep(Duration::from_millis(200));
    let repo_a_review = core.snapshot()?.review_for_repo(Some(repo_a.as_str()));

    assert_eq!(repo_a_review.active_slots, 1);
    assert_eq!(repo_a_review.completed_sessions, 1);
    assert_eq!(repo_a_review.pending_sessions, 0);
    assert!(
        repo_a_review
            .warnings
            .iter()
            .all(|warning| warning.slot_id.as_deref() != Some(slot_b.id.as_str()))
    );

    worker
        .join()
        .map_err(|_| anyhow::anyhow!("worker thread panicked"))??;

    Ok(())
}

#[test]
fn team_task_cancel_and_supersede_preserve_history() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("task-recovery")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::TeamInit {
        team_id: "alpha".to_string(),
        repo_id,
        objective: "Recover immutable task cards".to_string(),
        lead_runtime: None,
        lead_model: None,
        execution_mode: "external_slots".to_string(),
        fallback_runtime: None,
        fallback_model: None,
        routing_preferences: None,
        force: false,
    })?;
    core.dispatch(Command::TeamTaskAdd {
        team_id: "alpha".to_string(),
        task: awo_core::TaskCard {
            task_id: "task-1".to_string(),
            title: "Original".to_string(),
            summary: "Original plan".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
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
        },
    })?;
    core.dispatch(Command::TeamTaskAdd {
        team_id: "alpha".to_string(),
        task: awo_core::TaskCard {
            task_id: "task-2".to_string(),
            title: "Replacement".to_string(),
            summary: "Better plan".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
            model: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "Replacement patch".to_string(),
            verification: Vec::new(),
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
            result_summary: None,
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
            superseded_by_task_id: None,
        },
    })?;

    let outcome = core.dispatch(Command::TeamTaskCancel {
        team_id: "alpha".to_string(),
        task_id: "task-1".to_string(),
    })?;
    assert!(outcome.summary.contains("Cancelled task `task-1`"));
    let manifest = core.load_team_manifest("alpha")?;
    assert_eq!(
        manifest.task("task-1").unwrap().state,
        TaskCardState::Cancelled
    );

    core.dispatch(Command::TeamTaskState {
        team_id: "alpha".to_string(),
        task_id: "task-1".to_string(),
        state: TaskCardState::Todo,
    })?;
    let outcome = core.dispatch(Command::TeamTaskSupersede {
        team_id: "alpha".to_string(),
        task_id: "task-1".to_string(),
        replacement_task_id: "task-2".to_string(),
    })?;
    assert!(outcome.summary.contains("Superseded task `task-1`"));
    let manifest = core.load_team_manifest("alpha")?;
    let task = manifest.task("task-1").unwrap();
    assert_eq!(task.state, TaskCardState::Superseded);
    assert_eq!(task.superseded_by_task_id.as_deref(), Some("task-2"));

    Ok(())
}

#[test]
fn team_report_includes_plan_review_cleanup_and_history_sections() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("team-report")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::TeamInit {
        team_id: "alpha".to_string(),
        repo_id,
        objective: "Produce a rich report".to_string(),
        lead_runtime: None,
        lead_model: None,
        execution_mode: "external_slots".to_string(),
        fallback_runtime: None,
        fallback_model: None,
        routing_preferences: None,
        force: false,
    })?;
    core.dispatch(Command::TeamPlanAdd {
        team_id: "alpha".to_string(),
        plan: awo_core::PlanItem {
            plan_id: "plan-1".to_string(),
            title: "Break out review work".to_string(),
            summary: "Turn the review into task cards".to_string(),
            owner_id: Some("lead".to_string()),
            runtime: None,
            model: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: Some("Task card".to_string()),
            verification: vec!["cargo test".to_string()],
            depends_on: Vec::new(),
            notes: None,
            state: awo_core::PlanItemState::Draft,
            generated_task_id: None,
        },
    })?;
    core.dispatch(Command::TeamPlanApprove {
        team_id: "alpha".to_string(),
        plan_id: "plan-1".to_string(),
    })?;
    core.dispatch(Command::TeamPlanGenerate {
        team_id: "alpha".to_string(),
        plan_id: "plan-1".to_string(),
        task: awo_core::TaskCard {
            task_id: "review-task".to_string(),
            title: "Review task".to_string(),
            summary: "Review".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
            model: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "Review output".to_string(),
            verification: Vec::new(),
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
            result_summary: None,
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
            superseded_by_task_id: None,
        },
    })?;
    core.dispatch(Command::TeamTaskAdd {
        team_id: "alpha".to_string(),
        task: awo_core::TaskCard {
            task_id: "cleanup-task".to_string(),
            title: "Cleanup task".to_string(),
            summary: "Done but still bound".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
            model: None,
            slot_id: Some("slot-1".to_string()),
            branch_name: Some("awo/cleanup".to_string()),
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "Cleanup output".to_string(),
            verification: Vec::new(),
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Done,
            result_summary: Some("Looks good".to_string()),
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
            superseded_by_task_id: None,
        },
    })?;
    core.dispatch(Command::TeamTaskAdd {
        team_id: "alpha".to_string(),
        task: awo_core::TaskCard {
            task_id: "history-task".to_string(),
            title: "Old plan".to_string(),
            summary: "Retired".to_string(),
            owner_id: "lead".to_string(),
            runtime: None,
            model: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: Vec::new(),
            deliverable: "Retired".to_string(),
            verification: Vec::new(),
            verification_command: None,
            depends_on: Vec::new(),
            state: TaskCardState::Cancelled,
            result_summary: None,
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
            superseded_by_task_id: None,
        },
    })?;
    core.dispatch(Command::TeamTaskState {
        team_id: "alpha".to_string(),
        task_id: "review-task".to_string(),
        state: TaskCardState::Review,
    })?;

    let outcome = core.dispatch(Command::TeamReport {
        team_id: "alpha".to_string(),
    })?;
    let report_path = outcome
        .events
        .iter()
        .find_map(|event| match event {
            awo_core::DomainEvent::TeamReportGenerated { report_path, .. } => {
                Some(PathBuf::from(report_path))
            }
            _ => None,
        })
        .context("report path event")?;
    let content = fs::read_to_string(report_path)?;
    assert!(content.contains("## Queues"));
    assert!(content.contains("## Plan Items"));
    assert!(content.contains("## Review Queue"));
    assert!(content.contains("## Cleanup Queue"));
    assert!(content.contains("## History Queue"));

    Ok(())
}

#[test]
fn review_diff_returns_bounded_slot_diff_content() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("review-diff")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "review-diff".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .next()
        .context("missing slot")?;
    fs::write(
        Path::new(&slot.slot_path).join("README.md"),
        "hello\nupdated\n",
    )?;

    let outcome = core.dispatch(Command::ReviewDiff {
        slot_id: slot.id.clone(),
    })?;
    let data = outcome.data.context("missing diff payload")?;
    let content = data
        .get("content")
        .and_then(|value| value.as_str())
        .context("missing diff content")?;
    assert!(content.contains("# Review Diff"));
    assert!(content.contains("README.md"));
    assert!(content.contains("Diff Stat"));

    Ok(())
}

#[test]
fn failed_session_is_reflected_in_review_summary() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("failed-review")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "failed task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing failed-review slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "exit 7".to_string(),
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .context("missing failed session")?;
    assert_eq!(session.status, SessionStatus::Failed);
    assert_eq!(session.exit_code, Some(7));

    let review = core.snapshot()?.review_for_repo(Some(repo_id.as_str()));
    assert_eq!(review.failed_sessions, 1);
    assert!(
        review
            .warnings
            .iter()
            .any(|warning| warning.kind == "failed-session")
    );

    Ok(())
}

#[test]
fn oneshot_session_is_visible_while_running() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("oneshot-visible")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "oneshot".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing oneshot slot")?;

    let config = harness.config.clone();
    let slot_id = slot.id.clone();
    let worker = std::thread::spawn(move || -> Result<()> {
        let mut core = AppCore::from_config(config)?;
        core.dispatch(Command::SessionStart {
            slot_id,
            runtime: RuntimeKind::Shell,
            prompt: "sleep 1; printf visible".to_string(),
            read_only: true,
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot,
            attach_context: false,
            timeout_secs: None,
        })?;
        Ok(())
    });

    let sessions = wait_for_repo_sessions(&core, &repo_id, Duration::from_secs(2))?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].status, SessionStatus::Running);
    assert!(sessions[0].log_path.is_some());

    worker
        .join()
        .map_err(|_| anyhow::anyhow!("worker thread panicked"))??;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.repo_id == repo_id)
        .context("missing finished oneshot session")?;
    assert_eq!(session.status, SessionStatus::Completed);
    assert_eq!(session.exit_code, Some(0));

    Ok(())
}

#[test]
fn cancelling_running_oneshot_session_is_rejected() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("cancel-running-oneshot")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "cancel".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing cancel slot")?;

    let config = harness.config.clone();
    let slot_id = slot.id.clone();
    let worker = std::thread::spawn(move || -> Result<()> {
        let mut core = AppCore::from_config(config)?;
        core.dispatch(Command::SessionStart {
            slot_id,
            runtime: RuntimeKind::Shell,
            prompt: "sleep 1".to_string(),
            read_only: true,
            dry_run: false,
            launch_mode: SessionLaunchMode::Oneshot,
            attach_context: false,
            timeout_secs: None,
        })?;
        Ok(())
    });

    let session_id = wait_for_repo_sessions(&core, &repo_id, Duration::from_secs(2))?
        .into_iter()
        .find(|session| session.status == SessionStatus::Running)
        .map(|session| session.id)
        .context("missing running oneshot session")?;

    let error = core
        .dispatch(Command::SessionCancel { session_id })
        .expect_err("running oneshot cancellation should be rejected");
    assert!(error.to_string().contains("interruption is not supported"));

    worker
        .join()
        .map_err(|_| anyhow::anyhow!("worker thread panicked"))??;

    Ok(())
}

#[test]
fn actionable_error_for_unknown_repo_with_registered_repos() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("known-repo")?)?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotAcquire {
            repo_id: "nonexistent".to_string(),
            task_name: "task".to_string(),
            strategy: SlotStrategy::Fresh,
        })
        .expect_err("should fail for unknown repo");
    let msg = error.to_string();
    assert!(
        msg.contains(&repo_id),
        "error should list the registered repo id; got: {msg}"
    );
    assert!(
        msg.contains("nonexistent"),
        "error should mention the bad id; got: {msg}"
    );

    Ok(())
}

#[test]
fn actionable_error_for_unknown_repo_with_empty_store() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotAcquire {
            repo_id: "nonexistent".to_string(),
            task_name: "task".to_string(),
            strategy: SlotStrategy::Fresh,
        })
        .expect_err("should fail for unknown repo");
    let msg = error.to_string();
    assert!(
        msg.contains("awo repo add"),
        "error should suggest `awo repo add`; got: {msg}"
    );

    Ok(())
}

#[test]
fn actionable_error_for_unknown_slot() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotRelease {
            slot_id: "nonexistent".to_string(),
        })
        .expect_err("should fail for unknown slot");
    let msg = error.to_string();
    assert!(
        msg.contains("awo slot acquire"),
        "error should suggest `awo slot acquire`; got: {msg}"
    );

    Ok(())
}

#[test]
fn actionable_error_for_unknown_session() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SessionCancel {
            session_id: "nonexistent".to_string(),
        })
        .expect_err("should fail for unknown session");
    let msg = error.to_string();
    assert!(
        msg.contains("awo session start"),
        "error should suggest `awo session start`; got: {msg}"
    );

    Ok(())
}

fn run_git(
    repo_dir: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed in {}: {}",
            repo_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn run_git_with_identity(
    repo_dir: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args([
            "-c",
            "user.name=AWO Tests",
            "-c",
            "user.email=awo-tests@example.com",
        ])
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed in {}: {}",
            repo_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn run_git_root(
    repo_dir: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed in {}: {}",
            repo_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn git_stdout(
    repo_dir: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed in {}: {}",
            repo_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ---------------------------------------------------------------------------
// Negative-path: store edge cases
// ---------------------------------------------------------------------------

#[test]
fn get_slot_nonexistent_returns_none() -> Result<()> {
    let harness = TestHarness::new()?;
    let core = harness.core()?;
    let snapshot = core.snapshot()?;
    assert!(
        snapshot
            .slots
            .iter()
            .all(|slot| slot.id != "nonexistent-slot-id"),
        "snapshot should contain no slot with a fabricated id"
    );
    Ok(())
}

#[test]
fn get_session_nonexistent_returns_none() -> Result<()> {
    let harness = TestHarness::new()?;
    let core = harness.core()?;
    let snapshot = core.snapshot()?;
    assert!(
        snapshot
            .sessions
            .iter()
            .all(|session| session.id != "nonexistent-session-id"),
        "snapshot should contain no session with a fabricated id"
    );
    Ok(())
}

#[test]
fn get_repository_nonexistent_returns_none() -> Result<()> {
    let harness = TestHarness::new()?;
    let core = harness.core()?;
    let snapshot = core.snapshot()?;
    assert!(
        snapshot
            .registered_repos
            .iter()
            .all(|repo| repo.id != "nonexistent-repo-id"),
        "snapshot should contain no repo with a fabricated id"
    );
    Ok(())
}

#[test]
fn list_slots_nonexistent_repo_filter_returns_empty() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("filter-slots")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "some task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let snapshot = core.snapshot()?;
    let filtered: Vec<_> = snapshot
        .slots
        .iter()
        .filter(|slot| slot.repo_id == "nonexistent-repo-id")
        .collect();
    assert!(
        filtered.is_empty(),
        "filtering slots by nonexistent repo_id should yield empty vec"
    );
    Ok(())
}

#[test]
fn list_sessions_nonexistent_repo_filter_returns_empty() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("filter-sessions")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "session task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;

    let snapshot = core.snapshot()?;
    let filtered: Vec<_> = snapshot
        .sessions
        .iter()
        .filter(|session| session.repo_id == "nonexistent-repo-id")
        .collect();
    assert!(
        filtered.is_empty(),
        "filtering sessions by nonexistent repo_id should yield empty vec"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Negative-path: session log edge cases
// ---------------------------------------------------------------------------

#[test]
fn session_log_nonexistent_session_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SessionLog {
            session_id: "nonexistent-session-id".to_string(),
            lines: None,
            stream: None,
        })
        .expect_err("session log for nonexistent session should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("nonexistent-session-id") || msg.contains("unknown session"),
        "error should reference the bad session id; got: {msg}"
    );
    Ok(())
}

#[test]
fn session_log_invalid_stream_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("log-stream")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "log task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .map(|session| session.id)
        .context("missing session")?;

    let error = core
        .dispatch(Command::SessionLog {
            session_id,
            lines: None,
            stream: Some("invalid-stream".to_string()),
        })
        .expect_err("session log with invalid stream should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("unknown log stream") && msg.contains("invalid-stream"),
        "error should mention the invalid stream name; got: {msg}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Negative-path: slot lifecycle edge cases
// ---------------------------------------------------------------------------

#[test]
fn slot_acquire_nonexistent_repo_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotAcquire {
            repo_id: "ghost-repo".to_string(),
            task_name: "task".to_string(),
            strategy: SlotStrategy::Fresh,
        })
        .expect_err("slot acquire for nonexistent repo should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("ghost-repo"),
        "error should mention the bad repo id; got: {msg}"
    );
    Ok(())
}

#[test]
fn slot_release_nonexistent_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotRelease {
            slot_id: "ghost-slot".to_string(),
        })
        .expect_err("slot release for nonexistent slot should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("ghost-slot") || msg.contains("awo slot acquire"),
        "error should reference the bad slot id or suggest acquiring; got: {msg}"
    );
    Ok(())
}

#[test]
fn slot_refresh_nonexistent_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;

    let error = core
        .dispatch(Command::SlotRefresh {
            slot_id: "ghost-slot".to_string(),
        })
        .expect_err("slot refresh for nonexistent slot should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("ghost-slot") || msg.contains("awo slot acquire"),
        "error should reference the bad slot id or suggest acquiring; got: {msg}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Negative-path: session lifecycle edge cases
// ---------------------------------------------------------------------------

#[test]
fn session_start_on_released_slot_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("released-slot-session")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "release me".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    core.dispatch(Command::SlotRelease {
        slot_id: slot.id.clone(),
    })?;

    let error = core
        .dispatch(Command::SessionStart {
            slot_id: slot.id.clone(),
            runtime: RuntimeKind::Shell,
            prompt: "printf nope".to_string(),
            read_only: true,
            dry_run: true,
            launch_mode: SessionLaunchMode::Oneshot,
            attach_context: false,
            timeout_secs: None,
        })
        .expect_err("session start on released slot should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("not `active`") || msg.contains("released"),
        "error should indicate slot is not active; got: {msg}"
    );
    Ok(())
}

#[test]
fn session_delete_on_non_terminal_session_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("delete-pending")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "delete pending".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id && session.status == SessionStatus::Prepared)
        .map(|session| session.id)
        .context("missing pending session")?;

    let error = core
        .dispatch(Command::SessionDelete {
            session_id: session_id.clone(),
        })
        .expect_err("deleting a non-terminal session should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("not terminal"),
        "error should indicate session is not terminal; got: {msg}"
    );
    Ok(())
}

#[test]
fn session_cancel_on_terminal_session_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("cancel-terminal")?)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "cancel terminal".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .map(|session| session.id)
        .context("missing session")?;

    // Cancel the session to make it terminal
    core.dispatch(Command::SessionCancel {
        session_id: session_id.clone(),
    })?;

    // Try to cancel again -- should fail because session is already terminal
    let error = core
        .dispatch(Command::SessionCancel {
            session_id: session_id.clone(),
        })
        .expect_err("cancelling a terminal session should fail");
    let msg = error.to_string();
    assert!(
        msg.contains("already terminal"),
        "error should indicate session is already terminal; got: {msg}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn kill_tmux_session(session_id: &str) -> Result<()> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    let digest = hasher.finalize();
    let suffix = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let session = format!("awo-{suffix}");
    let _ = std::process::Command::new("tmux")
        .args(["kill-session", "-t", &session])
        .output();
    Ok(())
}

#[test]
fn session_timeout_is_enforced() -> Result<()> {
    if cfg!(not(unix)) || !detect_tmux() {
        return Ok(());
    }
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("timeout-test")?)?;
    let mut core = harness.core()?;

    // Acquire a slot
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "timeout task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|slot| slot.repo_id == repo_id)
        .context("missing slot")?;

    // Start a session with a 1-second timeout that runs for 10 seconds
    core.dispatch(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "sleep 10".to_string(),
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Pty,
        attach_context: false,
        timeout_secs: Some(1),
    })?;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .context("missing session")?;

    assert_eq!(session.status, SessionStatus::Running);

    // Wait for timeout to expire
    sleep(Duration::from_secs(2));

    // snapshot() calls sync_session which should enforce the timeout
    let snapshot = core.snapshot()?;
    let session = snapshot
        .sessions
        .into_iter()
        .find(|s| s.id == session.id)
        .context("missing session after sync")?;

    assert_eq!(
        session.status,
        SessionStatus::Failed,
        "Session should have failed due to timeout"
    );

    Ok(())
}
