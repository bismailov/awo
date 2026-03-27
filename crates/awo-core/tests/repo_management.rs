#![allow(unused_extern_crates, unused_crate_dependencies)]

use anyhow::Result;
use awo_core::app::AppPaths;
use awo_core::config::{AppConfig, AppSettings};
use awo_core::{AppCore, Command, SlotStrategy};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

struct TestHarness {
    _temp_dir: tempfile::TempDir,
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
        self.run_git(&repo_dir, &["init", "-b", "main"])?;
        fs::write(repo_dir.join("README.md"), "hello\n")?;
        self.run_git(&repo_dir, &["add", "README.md"])?;
        self.run_git(
            &repo_dir,
            &["commit", "-m", "init", "--author", "AWO <awo@example.com>"],
        )?;
        Ok(repo_dir)
    }

    fn run_git(&self, dir: &std::path::Path, args: &[&str]) -> Result<()> {
        let output = ProcessCommand::new("git")
            .args(args)
            .current_dir(dir)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }
}

#[test]
fn repo_remove_leaves_orphaned_slots_and_sessions() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("orphaned-test")?;
    let mut core = harness.core()?;

    // Register repo
    core.dispatch(Command::RepoAdd { path: repo_dir })?;
    let snapshot = core.snapshot()?;
    let repo_id = snapshot.registered_repos[0].id.clone();

    // Acquire a slot
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "test task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let snapshot = core.snapshot()?;
    let slot_id = snapshot.slots[0].id.clone();

    // Start a dry-run session
    core.dispatch(Command::SessionStart {
        slot_id: slot_id.clone(),
        runtime: awo_core::runtime::RuntimeKind::Shell,
        prompt: "echo hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: awo_core::runtime::SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;
    let session_id = core.snapshot()?.sessions[0].id.clone();

    // Cancel the session to make it terminal
    core.dispatch(Command::SessionCancel {
        session_id: session_id.clone(),
    })?;

    // Release the slot so it's not "Active"
    core.dispatch(Command::SlotRelease {
        slot_id: slot_id.clone(),
    })?;

    // Verify we have 1 repo, 1 slot, 1 session in the snapshot
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.registered_repos.len(), 1);
    assert_eq!(snapshot.slots.len(), 1);
    assert_eq!(snapshot.sessions.len(), 1);

    // Remove the repo
    core.dispatch(Command::RepoRemove {
        repo_id: repo_id.clone(),
    })?;

    // Audit: Snapshot should show 0 repos, BUT what about slots and sessions?
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.registered_repos.len(), 0);

    // THESE FAIL if the implementation is buggy (orphaned data)
    assert_eq!(snapshot.slots.len(), 0, "Slots should have been removed");
    assert_eq!(
        snapshot.sessions.len(),
        0,
        "Sessions should have been removed"
    );

    Ok(())
}

#[test]
fn repo_remove_fails_when_team_references_it() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("team-ref-test")?;
    let mut core = harness.core()?;

    // Register repo
    core.dispatch(Command::RepoAdd { path: repo_dir })?;
    let snapshot = core.snapshot()?;
    let repo_id = snapshot.registered_repos[0].id.clone();

    // Create a team for this repo
    core.dispatch(Command::TeamInit {
        repo_id: repo_id.clone(),
        team_id: "test-team".to_string(),
        objective: "test objective".to_string(),
        lead_runtime: Some("claude".to_string()),
        lead_model: Some("sonnet".to_string()),
        execution_mode: "external_slots".to_string(),
        routing_preferences: None,
        force: false,
        fallback_runtime: None,
        fallback_model: None,
    })?;

    // Verify team exists
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.teams.len(), 1);
    assert_eq!(snapshot.teams[0].repo_id, repo_id);

    // Remove the repo -- THIS SHOULD FAIL
    let result = core.dispatch(Command::RepoRemove {
        repo_id: repo_id.clone(),
    });

    assert!(
        result.is_err(),
        "Repo removal should fail if a team references it"
    );
    assert!(
        result.unwrap_err().to_string().contains("team"),
        "Error message should mention the team"
    );

    Ok(())
}

#[test]
fn repo_remove_fails_when_session_is_running() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("running-session-test")?;
    let mut core = harness.core()?;

    // Register repo
    core.dispatch(Command::RepoAdd { path: repo_dir })?;
    let snapshot = core.snapshot()?;
    let repo_id = snapshot.registered_repos[0].id.clone();

    // Acquire a slot
    core.dispatch(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "test task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot_id = core.snapshot()?.slots[0].id.clone();

    // Start a dry-run session (it's non-terminal but "Prepared")
    core.dispatch(Command::SessionStart {
        slot_id: slot_id.clone(),
        runtime: awo_core::runtime::RuntimeKind::Shell,
        prompt: "echo hello".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: awo_core::runtime::SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    })?;

    // Remove the repo -- THIS SHOULD FAIL because of the active slot AND non-terminal session
    let result = core.dispatch(Command::RepoRemove {
        repo_id: repo_id.clone(),
    });

    assert!(
        result.is_err(),
        "Repo removal should fail if a session or slot is still active"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("active session") || err_msg.contains("active slot"),
        "Error message should mention active sessions or slots; got: {}",
        err_msg
    );

    Ok(())
}
