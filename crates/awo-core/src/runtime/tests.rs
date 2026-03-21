use super::*;
use crate::app::AppPaths;
use std::fs;
use std::path::Path;

fn sample_paths(root: &Path) -> AppPaths {
    AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        state_db_path: root.join("data/state.sqlite3"),
        logs_dir: root.join("data/logs"),
        repos_dir: root.join("config/repos"),
        clones_dir: root.join("data/clones"),
        teams_dir: root.join("config/teams"),
    }
}

fn pid_path(sessions_dir: &Path, session_id: &str) -> std::path::PathBuf {
    sessions_dir.join(format!("{session_id}.pid"))
}

fn exit_path(sessions_dir: &Path, session_id: &str) -> std::path::PathBuf {
    sessions_dir.join(format!("{session_id}.exit"))
}

fn running_oneshot_session(session_id: &str) -> SessionRecord {
    SessionRecord {
        id: session_id.to_string(),
        repo_id: "repo-1".to_string(),
        slot_id: "slot-1".to_string(),
        runtime: "shell".to_string(),
        prompt: "echo hi".to_string(),
        status: "running".to_string(),
        read_only: true,
        dry_run: false,
        command_line: "sh -lc 'echo hi'".to_string(),
        stdout_path: Some("/tmp/stdout.log".to_string()),
        stderr_path: Some("/tmp/stderr.log".to_string()),
        exit_code: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn prepare_session_creates_pending_pid_sidecar_for_oneshot() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let slot_path = temp_dir.path().join("slot");
    fs::create_dir_all(&slot_path)?;

    let prepared = prepare_session(SessionRunRequest {
        paths: &paths,
        repo_id: "repo-1",
        slot_id: "slot-1",
        slot_path: &slot_path,
        runtime: RuntimeKind::Shell,
        prompt: "echo hi",
        read_only: true,
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot,
    })?;

    let pid_path = pid_path(&paths.logs_dir.join("sessions"), &prepared.session.id);
    assert!(pid_path.exists());
    assert_eq!(fs::read_to_string(pid_path)?.trim(), "pending");
    Ok(())
}

#[test]
fn sync_oneshot_keeps_pending_sidecar_running() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-1"), "pending")?;

    let mut session = running_oneshot_session("sess-1");
    assert!(!sync_session(&paths, &mut session)?);
    assert_eq!(session.status, "running");
    Ok(())
}

#[test]
fn sync_oneshot_marks_missing_process_as_failed() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    fs::create_dir_all(paths.logs_dir.join("sessions"))?;

    let mut session = running_oneshot_session("sess-2");
    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, "failed");
    Ok(())
}

#[test]
fn sync_oneshot_uses_exit_sidecar_when_process_is_gone() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-3"), "999999")?;
    fs::write(exit_path(&sessions_dir, "sess-3"), "0")?;

    let mut session = running_oneshot_session("sess-3");
    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, "completed");
    assert_eq!(session.exit_code, Some(0));
    assert!(!pid_path(&sessions_dir, "sess-3").exists());
    Ok(())
}
