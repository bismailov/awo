#![allow(unused_crate_dependencies)]

use anyhow::{Context, Result, bail};
use awo_core::app::AppPaths;
use awo_core::config::AppConfig;
use awo_core::runtime::{RuntimeKind, SessionLaunchMode, detect_tmux};
use awo_core::{AppCore, Command, SlotStrategy};
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
                    teams_dir,
                },
            },
        })
    }

    fn core(&self) -> Result<AppCore> {
        AppCore::from_config(self.config.clone())
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
    assert_eq!(reused.status, "active");

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
    assert_eq!(session.status, "cancelled");

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
    })?;

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.slot_id == slot.id)
        .context("missing PTY session")?;
    assert_eq!(session.status, "running");

    sleep(Duration::from_secs(2));
    let session_id = session.id.clone();

    let session = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .context("missing synced PTY session")?;
    assert_eq!(session.status, "completed");
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
        })?;
        Ok(())
    });

    sleep(Duration::from_millis(200));
    let sessions = core
        .snapshot()?
        .sessions
        .into_iter()
        .filter(|session| session.repo_id == repo_id)
        .collect::<Vec<_>>();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].status, "running");
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
    assert_eq!(session.status, "completed");
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
        })?;
        Ok(())
    });

    sleep(Duration::from_millis(200));
    let session_id = core
        .snapshot()?
        .sessions
        .into_iter()
        .find(|session| session.repo_id == repo_id)
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
