use super::{RuntimeKind, SessionLaunchMode};
use crate::app::AppPaths;
use crate::platform::{default_shell_program, shell_command_args, supports_tmux_supervision};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct PreparedCommand {
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) cwd: PathBuf,
}

pub(super) fn prepare_command(
    runtime: RuntimeKind,
    slot_path: &Path,
    prompt: &str,
    read_only: bool,
    stdout_path: &Path,
    launch_mode: SessionLaunchMode,
) -> PreparedCommand {
    match runtime {
        RuntimeKind::Codex => {
            let mut args = vec![
                "exec".to_string(),
                "--skip-git-repo-check".to_string(),
                "-C".to_string(),
                slot_path.display().to_string(),
            ];
            if launch_mode == SessionLaunchMode::Oneshot {
                args.push("--output-last-message".to_string());
                args.push(stdout_path.display().to_string());
            }
            if read_only {
                args.push("-s".to_string());
                args.push("read-only".to_string());
            } else {
                args.push("--full-auto".to_string());
            }
            args.push(prompt.to_string());
            PreparedCommand {
                program: "codex".to_string(),
                args,
                cwd: slot_path.to_path_buf(),
            }
        }
        RuntimeKind::Claude => {
            let mut args = vec!["-p".to_string()];
            if read_only {
                args.push("--permission-mode".to_string());
                args.push("plan".to_string());
            } else {
                args.push("--permission-mode".to_string());
                args.push("acceptEdits".to_string());
            }
            args.push(prompt.to_string());
            PreparedCommand {
                program: "claude".to_string(),
                args,
                cwd: slot_path.to_path_buf(),
            }
        }
        RuntimeKind::Gemini => {
            let mut args = vec![
                "--prompt".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "text".to_string(),
            ];
            if read_only {
                args.push("--approval-mode".to_string());
                args.push("plan".to_string());
            } else {
                args.push("--approval-mode".to_string());
                args.push("auto_edit".to_string());
            }
            PreparedCommand {
                program: "gemini".to_string(),
                args,
                cwd: slot_path.to_path_buf(),
            }
        }
        RuntimeKind::Shell => PreparedCommand {
            program: default_shell_program().to_string(),
            args: shell_command_args(prompt),
            cwd: slot_path.to_path_buf(),
        },
    }
}

pub(super) fn format_command_line(prepared: &PreparedCommand) -> String {
    shell_join(&prepared.program, &prepared.args)
}

pub(super) fn build_session_id(slot_id: &str, runtime: RuntimeKind) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("sess-{}-{}-{suffix}", runtime.as_str(), slot_id)
}

pub(super) fn execute_tmux_session(
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
    let wrapped_command = build_tmux_wrapper(prepared, &combined_log_path, &exit_path);
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

pub(super) fn sync_tmux_session(paths: &AppPaths, session_id: &str) -> Result<Option<i64>> {
    let supervisor_ref = supervisor_ref(session_id);
    match tmux_session_state(&supervisor_ref)? {
        Some(TmuxSessionState::Running) => Ok(None),
        Some(TmuxSessionState::Exited(exit_code)) => {
            tmux_kill_session(session_id)?;
            Ok(Some(exit_code))
        }
        None => Ok(read_exit_code(paths, session_id)?.or(Some(-1))),
    }
}

fn build_tmux_wrapper(
    prepared: &PreparedCommand,
    combined_log_path: &Path,
    exit_path: &Path,
) -> String {
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

fn tmux_session_state(supervisor_ref: &str) -> Result<Option<TmuxSessionState>> {
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

pub(super) fn tmux_kill_session(session_id: &str) -> Result<()> {
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

pub(super) fn read_exit_code(paths: &AppPaths, session_id: &str) -> Result<Option<i64>> {
    let path = exit_code_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read exit-code file at {}", path.display()))?;
    Ok(contents.trim().parse::<i64>().ok())
}

pub(super) fn read_pid(paths: &AppPaths, session_id: &str) -> Result<Option<u32>> {
    let path = pid_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read pid sidecar at {}", path.display()))?;
    Ok(contents.trim().parse::<u32>().ok())
}

pub(super) fn pid_sidecar_exists(paths: &AppPaths, session_id: &str) -> bool {
    pid_path_for(&paths.logs_dir.join("sessions"), session_id).exists()
}

pub(super) fn exit_code_path_for(logs_dir: &Path, session_id: &str) -> PathBuf {
    logs_dir.join(format!("{session_id}.exit"))
}

pub(super) fn pid_path_for(logs_dir: &Path, session_id: &str) -> PathBuf {
    logs_dir.join(format!("{session_id}.pid"))
}

pub(super) fn clear_sidecar_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).with_context(|| format!("failed to remove sidecar {}", path.display()))
}

#[cfg(unix)]
pub(super) fn process_is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub(super) fn process_is_running(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}")])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
        })
        .unwrap_or(false)
}

fn supervisor_ref(session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    let digest = hasher.finalize();
    let suffix = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("awo-{suffix}")
}

fn shell_join(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().cloned())
        .map(|part| shell_quote(&part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

enum TmuxSessionState {
    Running,
    Exited(i64),
}
