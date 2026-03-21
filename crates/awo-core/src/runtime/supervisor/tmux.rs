use super::{PreparedCommand, exit_code_path_for, read_exit_code, shell_join, shell_quote};
use crate::app::AppPaths;
use crate::platform::{default_shell_program, shell_command_args, supports_tmux_supervision};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn launch(
    session_id: &str,
    slot_path: &Path,
    prepared: &PreparedCommand,
    combined_log_path: PathBuf,
) -> Result<()> {
    if !supports_tmux_supervision() {
        bail!("tmux is not available on PATH");
    }

    if let Some(parent) = combined_log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log dir at {}", parent.display()))?;
    }
    if combined_log_path.exists() {
        fs::remove_file(&combined_log_path).with_context(|| {
            format!(
                "failed to remove previous PTY log at {}",
                combined_log_path.display()
            )
        })?;
    }
    let exit_path = exit_code_path_for(
        combined_log_path.parent().unwrap_or(Path::new(".")),
        session_id,
    );
    if exit_path.exists() {
        fs::remove_file(&exit_path).with_context(|| {
            format!(
                "failed to remove previous exit-code file at {}",
                exit_path.display()
            )
        })?;
    }

    let supervisor_ref = supervisor_ref(session_id);
    let wrapped_command = build_wrapper(prepared, &combined_log_path, &exit_path);
    let output = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &supervisor_ref,
            "-c",
            &slot_path.display().to_string(),
            &wrapped_command,
        ])
        .output()
        .with_context(|| format!("failed to start tmux session `{supervisor_ref}`"))?;

    if !output.status.success() {
        bail!(
            "failed to launch tmux-backed session: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let output = Command::new("tmux")
        .args(["set-option", "-t", &supervisor_ref, "remain-on-exit", "on"])
        .output()
        .with_context(|| format!("failed to configure tmux session `{supervisor_ref}`"))?;
    if !output.status.success() {
        bail!(
            "failed to configure tmux session: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

pub(super) fn sync(paths: &AppPaths, session_id: &str) -> Result<Option<i64>> {
    let supervisor_ref = supervisor_ref(session_id);
    match session_state(&supervisor_ref)? {
        Some(TmuxSessionState::Running) => Ok(None),
        Some(TmuxSessionState::Exited(exit_code)) => {
            kill(session_id)?;
            Ok(Some(exit_code))
        }
        None => Ok(read_exit_code(paths, session_id)?.or(Some(-1))),
    }
}

pub(super) fn kill(session_id: &str) -> Result<()> {
    let supervisor_ref = supervisor_ref(session_id);
    let output = Command::new("tmux")
        .args(["kill-session", "-t", &supervisor_ref])
        .output()
        .with_context(|| format!("failed to kill tmux session `{supervisor_ref}`"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("can't find session") || stderr.contains("no server running") {
            return Ok(());
        }
        bail!("failed to kill tmux session: {}", stderr.trim());
    }
    Ok(())
}

fn build_wrapper(prepared: &PreparedCommand, combined_log_path: &Path, exit_path: &Path) -> String {
    let command = shell_join(&prepared.program, &prepared.args);
    let snippet = format!(
        "set -o pipefail; {} 2>&1 | tee -a {}; exit_code=${{pipestatus[1]}}; printf '%s\\n' \"$exit_code\" > {}; exit \"$exit_code\"",
        command,
        shell_quote(&combined_log_path.display().to_string()),
        shell_quote(&exit_path.display().to_string()),
    );
    let shell = default_shell_program();
    let args = shell_command_args(&snippet)
        .into_iter()
        .map(|arg| shell_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{} {}", shell_quote(shell), args)
}

fn session_state(supervisor_ref: &str) -> Result<Option<TmuxSessionState>> {
    let has_session = Command::new("tmux")
        .args(["has-session", "-t", supervisor_ref])
        .status()
        .with_context(|| format!("failed to check tmux session `{supervisor_ref}`"))?;
    if !has_session.success() {
        return Ok(None);
    }

    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            supervisor_ref,
            "-F",
            "#{pane_dead} #{pane_dead_status}",
        ])
        .output()
        .with_context(|| format!("failed to inspect tmux session `{supervisor_ref}`"))?;
    if !output.status.success() {
        return Ok(None);
    }

    let line = String::from_utf8_lossy(&output.stdout);
    let mut parts = line.split_whitespace();
    let pane_dead = parts.next().unwrap_or("0");
    let pane_dead_status = parts.next().unwrap_or("0");
    if pane_dead == "1" {
        Ok(Some(TmuxSessionState::Exited(
            pane_dead_status.parse::<i64>().unwrap_or(1),
        )))
    } else {
        Ok(Some(TmuxSessionState::Running))
    }
}

pub(super) fn supervisor_ref(session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    let digest = hasher.finalize();
    let suffix = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("awo-{suffix}")
}

enum TmuxSessionState {
    Running,
    Exited(i64),
}
