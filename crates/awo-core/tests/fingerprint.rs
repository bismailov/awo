#![allow(unused_crate_dependencies)]

use anyhow::{Context, Result};
use awo_core::app::AppPaths;
use awo_core::config::{AppConfig, AppSettings};
use awo_core::slot::FingerprintStatus;
use awo_core::{AppCore, Command, SlotStrategy};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
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

    fn init_repo(&self, repo_id: &str, with_lockfile: bool) -> Result<String> {
        let root = self._temp_dir.path().join("repo-source").join(repo_id);
        fs::create_dir_all(&root)?;

        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&root)
            .output()?;

        if with_lockfile {
            fs::write(root.join("Cargo.lock"), "original lock")?;
        } else {
            fs::write(root.join("README.md"), "hello")?;
        }

        ProcessCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&root)
            .output()?;
        ProcessCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&root)
            .output()?;
        ProcessCommand::new("git")
            .args(["add", "."])
            .current_dir(&root)
            .output()?;
        ProcessCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&root)
            .output()?;

        let mut core = self.core()?;
        core.dispatch(Command::RepoAdd { path: root.clone() })?;

        let snapshot = core.snapshot()?;
        let repo = snapshot
            .registered_repos
            .into_iter()
            .find(|repo| repo.name == repo_id)
            .context("missing repo")?;

        Ok(repo.id)
    }
}

#[test]
fn test_slot_acquire_fingerprint_ready() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.init_repo("repo-ready", true)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "test-slot".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let slots = core.snapshot()?.slots;
    assert_eq!(slots.len(), 1);
    let slot = &slots[0];

    assert_eq!(slot.fingerprint_status, FingerprintStatus::Ready);

    Ok(())
}

#[test]
fn test_slot_acquire_fingerprint_missing() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.init_repo("repo-missing", false)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "test-slot-missing".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let slots = core.snapshot()?.slots;
    assert_eq!(slots.len(), 1);
    let slot = &slots[0];

    assert_eq!(slot.fingerprint_status, FingerprintStatus::Missing);

    Ok(())
}

#[test]
fn test_slot_refresh_detects_stale_fingerprint() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.init_repo("repo-stale", true)?;
    let mut core = harness.core()?;

    core.dispatch(Command::SlotAcquire {
        repo_id,
        task_name: "test-slot-stale".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let slots = core.snapshot()?.slots;
    let slot = &slots[0];
    assert_eq!(slot.fingerprint_status, FingerprintStatus::Ready);

    let slot_path = Path::new(&slot.slot_path);
    fs::write(slot_path.join("Cargo.lock"), "modified lock")?;

    core.dispatch(Command::SlotRefresh {
        slot_id: slot.id.clone(),
    })?;

    let updated_slot = core
        .snapshot()?
        .slots
        .into_iter()
        .find(|candidate| candidate.id == slot.id)
        .context("missing refreshed slot")?;

    assert_eq!(updated_slot.fingerprint_status, FingerprintStatus::Stale);

    Ok(())
}
