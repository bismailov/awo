use super::*;
use crate::app::AppPaths;
use crate::config::{AppConfig, AppSettings};
use crate::error::AwoError;
use crate::runtime::{RuntimeKind, SessionLaunchMode};
use crate::skills::SkillRuntime;
use crate::slot::SlotStrategy;
use crate::store::Store;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_runner() -> Result<(TempDir, AppConfig, Store)> {
    let temp_dir = tempfile::tempdir()?;
    let root = temp_dir.path();
    let config_dir = root.join("config");
    let data_dir = root.join("data");
    let logs_dir = root.join("logs");
    let clones_dir = root.join("clones");
    let repos_dir = root.join("repos");
    let teams_dir = root.join("teams");

    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&data_dir)?;
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
    Ok((temp_dir, config, store))
}

#[test]
fn slot_acquire_with_nonexistent_repo_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SlotAcquire {
        repo_id: "nonexistent".to_string(),
        task_name: "test".to_string(),
        strategy: SlotStrategy::Fresh,
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. }) || matches!(err, AwoError::UnknownRepoId { .. })
    );
    assert!(err.to_string().contains("unknown repo"));
    Ok(())
}

#[test]
fn slot_release_with_nonexistent_slot_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SlotRelease {
        slot_id: "nonexistent".to_string(),
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. }) || matches!(err, AwoError::UnknownSlotId { .. })
    );
    assert!(err.to_string().contains("unknown slot"));
    Ok(())
}

#[test]
fn session_start_with_nonexistent_slot_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SessionStart {
        slot_id: "nonexistent".to_string(),
        runtime: RuntimeKind::Shell,
        prompt: "echo hello".to_string(),
        read_only: false,
        dry_run: false,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false,
        timeout_secs: None,
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. }) || matches!(err, AwoError::UnknownSlotId { .. })
    );
    assert!(err.to_string().contains("unknown slot"));
    Ok(())
}

#[test]
fn session_cancel_with_nonexistent_session_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SessionCancel {
        session_id: "nonexistent".to_string(),
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. })
            || matches!(err, AwoError::UnknownSessionId { .. })
    );
    assert!(err.to_string().contains("unknown session"));
    Ok(())
}

#[test]
fn session_log_with_nonexistent_session_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SessionLog {
        session_id: "nonexistent".to_string(),
        lines: Some(10),
        stream: None,
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. })
            || matches!(err, AwoError::UnknownSessionId { .. })
    );
    assert!(err.to_string().contains("unknown session"));
    Ok(())
}

#[test]
fn repo_add_with_nonexistent_path_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::RepoAdd {
        path: PathBuf::from("/this/path/does/not/exist/anywhere"),
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Io { .. })
            || matches!(err, AwoError::GitInvocation { .. })
            || matches!(err, AwoError::GitCommandFailed { .. })
    );
    Ok(())
}

#[test]
fn context_doctor_with_nonexistent_repo_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::ContextDoctor {
        repo_id: "nonexistent".to_string(),
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. }) || matches!(err, AwoError::UnknownRepoId { .. })
    );
    assert!(err.to_string().contains("unknown repo"));
    Ok(())
}

#[test]
fn skills_doctor_with_nonexistent_repo_returns_error() -> Result<()> {
    let (_dir, config, store) = setup_runner()?;
    let mut runner = CommandRunner::new(&config, &store);

    let result = runner.run(Command::SkillsDoctor {
        repo_id: "nonexistent".to_string(),
        runtime: Some(SkillRuntime::Claude),
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AwoError::Validation { .. }) || matches!(err, AwoError::UnknownRepoId { .. })
    );
    assert!(err.to_string().contains("unknown repo"));
    Ok(())
}
