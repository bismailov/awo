use super::*;
use crate::app::AppPaths;
use crate::config::AppConfig;
use crate::runtime::{RuntimeKind, SessionLaunchMode, SessionRecord};
use crate::snapshot::AppSnapshot;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use tempfile::TempDir;

struct TestHarness {
    _temp_dir: TempDir,
    config: AppConfig,
    store: Store,
}

impl TestHarness {
    fn new() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
        let clones_dir = data_dir.join("clones");
        let repos_dir = config_dir.join("repos");
        fs::create_dir_all(&logs_dir)?;
        fs::create_dir_all(&clones_dir)?;
        fs::create_dir_all(&repos_dir)?;

        let config = AppConfig {
            paths: AppPaths {
                config_dir,
                data_dir: data_dir.clone(),
                state_db_path: data_dir.join("state.sqlite3"),
                logs_dir,
                repos_dir,
                clones_dir,
                teams_dir: temp_dir.path().join("config/teams"),
            },
        };
        let store = Store::open(&config.paths.state_db_path)?;
        store.initialize_schema()?;

        Ok(Self {
            _temp_dir: temp_dir,
            config,
            store,
        })
    }

    fn runner(&self) -> CommandRunner<'_> {
        CommandRunner::new(&self.config, &self.store)
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
        let mut runner = self.runner();
        runner.run(Command::RepoAdd {
            path: repo_dir.clone(),
        })?;
        let expected_name = repo_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .context("repo dir missing final path component")?;
        let repos = self.store.list_repositories()?;
        let repo = repos
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
    let repo_dir = harness.create_repo("warm-reuse")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "first task".to_string(),
        strategy: SlotStrategy::Warm,
    })?;
    let first_slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing first slot")?;

    runner.run(Command::SlotRelease {
        slot_id: first_slot.id.clone(),
    })?;
    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "second task".to_string(),
        strategy: SlotStrategy::Warm,
    })?;

    let slots = harness.store.list_slots(Some(repo_id.as_str()))?;
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
    let repo_dir = harness.create_repo("dirty-slot")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id,
        task_name: "dirty task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(None)?
        .into_iter()
        .find(|slot| slot.task_name == "dirty task")
        .context("missing dirty slot")?;
    fs::write(Path::new(&slot.slot_path).join("DIRTY.txt"), "dirty\n")?;

    let error = runner
        .run(Command::SlotRelease {
            slot_id: slot.id.clone(),
        })
        .expect_err("dirty slot release should fail");
    assert!(error.to_string().contains("dirty"));

    Ok(())
}

#[test]
fn release_blocks_pending_session() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("busy-slot")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "busy task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing busy slot")?;
    harness.store.upsert_session(&SessionRecord {
        id: "sess-test".to_string(),
        repo_id,
        slot_id: slot.id.clone(),
        runtime: "codex".to_string(),
        prompt: "test".to_string(),
        status: "prepared".to_string(),
        read_only: false,
        dry_run: true,
        command_line: "codex exec ...".to_string(),
        stdout_path: None,
        stderr_path: None,
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let error = runner
        .run(Command::SlotRelease {
            slot_id: slot.id.clone(),
        })
        .expect_err("busy slot release should fail");
    assert!(error.to_string().contains("pending session"));

    Ok(())
}

#[test]
fn cancelling_pending_session_unblocks_release() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("cancel-session")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "cancel task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing cancel slot")?;
    harness.store.upsert_session(&SessionRecord {
        id: "sess-cancel".to_string(),
        repo_id,
        slot_id: slot.id.clone(),
        runtime: "codex".to_string(),
        prompt: "test".to_string(),
        status: "prepared".to_string(),
        read_only: false,
        dry_run: true,
        command_line: "codex exec ...".to_string(),
        stdout_path: None,
        stderr_path: None,
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    runner.run(Command::SessionCancel {
        session_id: "sess-cancel".to_string(),
    })?;
    runner.run(Command::SlotRelease {
        slot_id: slot.id.clone(),
    })?;

    let session = harness
        .store
        .get_session("sess-cancel")?
        .context("missing cancelled session")?;
    assert_eq!(session.status, "cancelled");

    Ok(())
}

#[test]
fn deleting_terminal_session_removes_it_from_state() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("delete-session")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "delete task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing delete slot")?;
    harness.store.upsert_session(&SessionRecord {
        id: "sess-delete".to_string(),
        repo_id,
        slot_id: slot.id,
        runtime: "codex".to_string(),
        prompt: "test".to_string(),
        status: "cancelled".to_string(),
        read_only: true,
        dry_run: true,
        command_line: "codex exec ...".to_string(),
        stdout_path: None,
        stderr_path: None,
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    runner.run(Command::SessionDelete {
        session_id: "sess-delete".to_string(),
    })?;
    assert!(harness.store.get_session("sess-delete")?.is_none());

    Ok(())
}

#[test]
fn repo_clone_registers_remote_repo() -> Result<()> {
    let harness = TestHarness::new()?;
    let remote = harness.create_bare_remote("remote-clone")?;
    let mut runner = harness.runner();

    runner.run(Command::RepoClone {
        remote_url: remote.display().to_string(),
        destination: None,
    })?;

    let repo = harness
        .store
        .list_repositories()?
        .into_iter()
        .next()
        .context("missing cloned repo")?;
    assert!(Path::new(&repo.repo_root).exists());
    let remote_string = remote.display().to_string();
    assert_eq!(repo.remote_url.as_deref(), Some(remote_string.as_str()));

    Ok(())
}

#[test]
fn pty_session_runs_and_syncs_to_completion() -> Result<()> {
    if !crate::runtime::detect_tmux() {
        return Ok(());
    }

    let harness = TestHarness::new()?;
    let repo_dir = harness.create_repo("pty-session")?;
    let repo_id = harness.register_repo(repo_dir)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "pty task".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing PTY slot")?;

    runner.run(Command::SessionStart {
        slot_id: slot.id.clone(),
        runtime: RuntimeKind::Shell,
        prompt: "printf pty-ok; sleep 1; printf done".to_string(),
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Pty,
        attach_context: false,
    })?;

    let session = harness
        .store
        .list_sessions(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing PTY session")?;
    assert_eq!(session.status, "running");

    sleep(Duration::from_secs(2));
    runner.sync_runtime_state(Some(repo_id.as_str()))?;

    let session = harness
        .store
        .get_session(&session.id)?
        .context("missing synced PTY session")?;
    assert_eq!(session.status, "completed");
    assert_eq!(session.exit_code, Some(0));
    let log_path = session.stdout_path.context("missing PTY log path")?;
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
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_a.clone(),
        task_name: "repo-a".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    runner.run(Command::SlotAcquire {
        repo_id: repo_b.clone(),
        task_name: "repo-b".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;

    let slot_a = harness
        .store
        .list_slots(Some(repo_a.as_str()))?
        .into_iter()
        .next()
        .context("missing repo-a slot")?;
    let slot_b = harness
        .store
        .list_slots(Some(repo_b.as_str()))?
        .into_iter()
        .next()
        .context("missing repo-b slot")?;

    harness.store.upsert_session(&SessionRecord {
        id: "sess-a".to_string(),
        repo_id: repo_a.clone(),
        slot_id: slot_a.id.clone(),
        runtime: "shell".to_string(),
        prompt: "test".to_string(),
        status: "completed".to_string(),
        read_only: true,
        dry_run: false,
        command_line: "sh -c true".to_string(),
        stdout_path: None,
        stderr_path: Some("stderr.log".to_string()),
        exit_code: Some(0),
        created_at: String::new(),
        updated_at: String::new(),
    })?;
    harness.store.upsert_session(&SessionRecord {
        id: "sess-b".to_string(),
        repo_id: repo_b.clone(),
        slot_id: slot_b.id.clone(),
        runtime: "shell".to_string(),
        prompt: "test".to_string(),
        status: "running".to_string(),
        read_only: true,
        dry_run: false,
        command_line: "sh -c sleep 5".to_string(),
        stdout_path: Some("stdout.log".to_string()),
        stderr_path: Some("stderr.log".to_string()),
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let snapshot = AppSnapshot::load(&harness.config, &harness.store)?;
    let repo_a_review = snapshot.review_for_repo(Some(repo_a.as_str()));

    assert_eq!(repo_a_review.active_slots, 1);
    assert_eq!(repo_a_review.completed_sessions, 1);
    assert_eq!(repo_a_review.pending_sessions, 0);
    assert!(
        repo_a_review
            .warnings
            .iter()
            .all(|warning| warning.slot_id.as_deref() != Some(slot_b.id.as_str()))
    );

    Ok(())
}

#[test]
fn oneshot_session_is_visible_while_running() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("oneshot-visible")?)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "oneshot".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing oneshot slot")?;

    let config = harness.config.clone();
    let state_db_path = config.paths.state_db_path.clone();
    let slot_id = slot.id.clone();
    let worker = std::thread::spawn(move || -> Result<()> {
        let store = Store::open(&state_db_path)?;
        let mut runner = CommandRunner::new(&config, &store);
        runner.run(Command::SessionStart {
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
    let sessions = harness.store.list_sessions(Some(repo_id.as_str()))?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].status, "running");
    assert!(sessions[0].stderr_path.is_some());

    worker
        .join()
        .map_err(|_| anyhow::anyhow!("worker thread panicked"))??;

    let session = harness
        .store
        .list_sessions(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing finished oneshot session")?;
    assert_eq!(session.status, "completed");
    assert_eq!(session.exit_code, Some(0));

    Ok(())
}

#[test]
fn cancelling_running_oneshot_session_is_rejected() -> Result<()> {
    let harness = TestHarness::new()?;
    let repo_id = harness.register_repo(harness.create_repo("cancel-running-oneshot")?)?;
    let mut runner = harness.runner();

    runner.run(Command::SlotAcquire {
        repo_id: repo_id.clone(),
        task_name: "cancel".to_string(),
        strategy: SlotStrategy::Fresh,
    })?;
    let slot = harness
        .store
        .list_slots(Some(repo_id.as_str()))?
        .into_iter()
        .next()
        .context("missing cancel slot")?;

    harness.store.upsert_session(&SessionRecord {
        id: "sess-running-oneshot".to_string(),
        repo_id,
        slot_id: slot.id,
        runtime: "shell".to_string(),
        prompt: "sleep 5".to_string(),
        status: "running".to_string(),
        read_only: true,
        dry_run: false,
        command_line: "sh -c 'sleep 5'".to_string(),
        stdout_path: Some("stdout.log".to_string()),
        stderr_path: Some("stderr.log".to_string()),
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    })?;

    let error = runner
        .run(Command::SessionCancel {
            session_id: "sess-running-oneshot".to_string(),
        })
        .expect_err("running oneshot cancellation should be rejected");
    assert!(error.to_string().contains("interruption is not supported"));

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
