use crate::app::AppPaths;
use crate::platform::{
    default_shell_program, executable_exists, shell_command_args, supports_tmux_supervision,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecord {
    pub id: String,
    pub repo_id: String,
    pub slot_id: String,
    pub runtime: String,
    pub prompt: String,
    pub status: String,
    pub read_only: bool,
    pub dry_run: bool,
    pub command_line: String,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub exit_code: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl SessionRecord {
    pub fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "cancelled")
    }

    pub fn blocks_release(&self) -> bool {
        !self.is_terminal()
    }

    pub fn is_write_capable(&self) -> bool {
        !self.read_only
    }

    pub fn is_supervised(&self) -> bool {
        self.status == "running" && self.stderr_path.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, EnumString, IntoStaticStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RuntimeKind {
    Codex,
    Claude,
    Gemini,
    Shell,
}

impl RuntimeKind {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    pub fn uses_agent_prompt(self) -> bool {
        !matches!(self, Self::Shell)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, EnumString, IntoStaticStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SessionLaunchMode {
    Pty,
    Oneshot,
}

impl SessionLaunchMode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    pub fn default_for_environment() -> Self {
        if supports_tmux_supervision() {
            Self::Pty
        } else {
            Self::Oneshot
        }
    }
}

pub struct SessionRunRequest<'a> {
    pub paths: &'a AppPaths,
    pub repo_id: &'a str,
    pub slot_id: &'a str,
    pub slot_path: &'a Path,
    pub runtime: RuntimeKind,
    pub prompt: &'a str,
    pub read_only: bool,
    pub dry_run: bool,
    pub launch_mode: SessionLaunchMode,
}

pub struct SessionExecutionResult {
    pub session: SessionRecord,
}

pub struct PreparedSession {
    pub session: SessionRecord,
    prepared: PreparedCommand,
    launch_mode: SessionLaunchMode,
    stdout_path: PathBuf,
    stderr_path: Option<PathBuf>,
}

pub fn prepare_session(request: SessionRunRequest<'_>) -> Result<PreparedSession> {
    let session_id = build_session_id(request.slot_id, request.runtime);
    let logs_dir = request.paths.logs_dir.join("sessions");
    fs::create_dir_all(&logs_dir)
        .with_context(|| format!("failed to create session log dir at {}", logs_dir.display()))?;
    if request.launch_mode == SessionLaunchMode::Oneshot && !request.dry_run {
        clear_sidecar_if_exists(&exit_code_path_for(&logs_dir, &session_id))?;
        fs::write(pid_path_for(&logs_dir, &session_id), "pending").with_context(|| {
            format!(
                "failed to prepare pid sidecar at {}",
                pid_path_for(&logs_dir, &session_id).display()
            )
        })?;
    }

    let stdout_path = match request.launch_mode {
        SessionLaunchMode::Pty => logs_dir.join(format!("{session_id}.pty.log")),
        SessionLaunchMode::Oneshot => logs_dir.join(format!("{session_id}.out.log")),
    };
    let stderr_path = match request.launch_mode {
        SessionLaunchMode::Pty => None,
        SessionLaunchMode::Oneshot => Some(logs_dir.join(format!("{session_id}.err.log"))),
    };
    let prepared = prepare_command(
        request.runtime,
        request.slot_path,
        request.prompt,
        request.read_only,
        &stdout_path,
        request.launch_mode,
    );
    let command_line = format_command_line(&prepared);
    let status = if request.dry_run {
        "prepared"
    } else {
        "running"
    };

    Ok(PreparedSession {
        session: SessionRecord {
            id: session_id,
            repo_id: request.repo_id.to_string(),
            slot_id: request.slot_id.to_string(),
            runtime: request.runtime.as_str().to_string(),
            prompt: request.prompt.to_string(),
            status: status.to_string(),
            read_only: request.read_only,
            dry_run: request.dry_run,
            command_line,
            stdout_path: Some(stdout_path.display().to_string()),
            stderr_path: stderr_path.as_ref().map(|path| path.display().to_string()),
            exit_code: None,
            created_at: String::new(),
            updated_at: String::new(),
        },
        prepared,
        launch_mode: request.launch_mode,
        stdout_path,
        stderr_path,
    })
}

pub fn execute_prepared_session(
    mut prepared_session: PreparedSession,
) -> Result<SessionExecutionResult> {
    if prepared_session.session.dry_run {
        return Ok(SessionExecutionResult {
            session: prepared_session.session,
        });
    }

    if prepared_session.launch_mode == SessionLaunchMode::Pty {
        start_tmux_session(
            &prepared_session.session.id,
            &prepared_session.prepared.cwd,
            &prepared_session.prepared,
            prepared_session.stdout_path.clone(),
        )?;
        return Ok(SessionExecutionResult {
            session: prepared_session.session,
        });
    }

    let logs_dir = prepared_session
        .stdout_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let exit_path = exit_code_path_for(&logs_dir, &prepared_session.session.id);
    let pid_path = pid_path_for(&logs_dir, &prepared_session.session.id);
    clear_sidecar_if_exists(&exit_path)?;

    let child = Command::new(&prepared_session.prepared.program)
        .args(&prepared_session.prepared.args)
        .current_dir(&prepared_session.prepared.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .inspect_err(|_error| {
            let _ = clear_sidecar_if_exists(&pid_path);
        })
        .with_context(|| {
            format!(
                "failed to launch runtime `{}`",
                prepared_session.session.runtime
            )
        })?;
    fs::write(&pid_path, child.id().to_string())
        .with_context(|| format!("failed to write pid sidecar at {}", pid_path.display()))?;

    let output = child
        .wait_with_output()
        .inspect_err(|_error| {
            let _ = clear_sidecar_if_exists(&pid_path);
        })
        .with_context(|| {
            format!(
                "failed while waiting for runtime `{}`",
                prepared_session.session.runtime
            )
        })?;

    fs::write(&prepared_session.stdout_path, &output.stdout).with_context(|| {
        format!(
            "failed to write stdout log at {}",
            prepared_session.stdout_path.display()
        )
    })?;
    if let Some(stderr_path) = &prepared_session.stderr_path {
        fs::write(stderr_path, &output.stderr)
            .with_context(|| format!("failed to write stderr log at {}", stderr_path.display()))?;
    }

    prepared_session.session.status = if output.status.success() {
        "completed"
    } else {
        "failed"
    }
    .to_string();
    prepared_session.session.exit_code = output.status.code().map(i64::from);
    let exit_code = prepared_session.session.exit_code.unwrap_or(-1);
    fs::write(&exit_path, exit_code.to_string()).with_context(|| {
        format!(
            "failed to write exit-code sidecar at {}",
            exit_path.display()
        )
    })?;
    clear_sidecar_if_exists(&pid_path)?;

    Ok(SessionExecutionResult {
        session: prepared_session.session,
    })
}

pub fn sync_session(paths: &AppPaths, session: &mut SessionRecord) -> Result<bool> {
    if session.is_terminal() {
        return Ok(false);
    }
    if session.status != "running" {
        return Ok(false);
    }

    if !session.is_supervised() {
        return sync_oneshot_session(paths, session);
    }

    let supervisor_ref = supervisor_ref(&session.id);
    match tmux_session_state(&supervisor_ref)? {
        Some(TmuxSessionState::Running) => Ok(false),
        Some(TmuxSessionState::Exited(exit_code)) => {
            session.exit_code = Some(exit_code);
            session.status = if exit_code == 0 {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
            tmux_kill_session(&supervisor_ref)?;
            Ok(true)
        }
        None => {
            if let Some(exit_code) = read_exit_code(paths, &session.id)? {
                session.exit_code = Some(exit_code);
                session.status = if exit_code == 0 {
                    "completed".to_string()
                } else {
                    "failed".to_string()
                };
            } else {
                session.status = "failed".to_string();
            }
            Ok(true)
        }
    }
}

pub fn cancel_session(paths: &AppPaths, session: &mut SessionRecord) -> Result<bool> {
    if session.is_terminal() {
        return Ok(false);
    }

    if session.is_supervised() {
        let supervisor_ref = supervisor_ref(&session.id);
        let _ = tmux_kill_session(&supervisor_ref);
        if session.exit_code.is_none() {
            session.exit_code = read_exit_code(paths, &session.id)?;
        }
    }

    session.status = "cancelled".to_string();
    Ok(true)
}

fn sync_oneshot_session(paths: &AppPaths, session: &mut SessionRecord) -> Result<bool> {
    let pid = read_pid(paths, &session.id)?;
    match pid {
        Some(pid) if process_is_running(pid) => return Ok(false),
        None if pid_sidecar_exists(paths, &session.id) => return Ok(false),
        _ => {}
    }

    if let Some(exit_code) = read_exit_code(paths, &session.id)? {
        session.exit_code = Some(exit_code);
        session.status = if exit_code == 0 {
            "completed".to_string()
        } else {
            "failed".to_string()
        };
    } else {
        session.status = "failed".to_string();
    }

    clear_sidecar_if_exists(&pid_path_for(&paths.logs_dir.join("sessions"), &session.id))?;
    Ok(true)
}

pub fn detect_runtime(runtime: RuntimeKind) -> bool {
    let executable = match runtime {
        RuntimeKind::Shell => default_shell_program(),
        _ => runtime.as_str(),
    };
    executable_exists(executable)
}

pub fn detect_tmux() -> bool {
    supports_tmux_supervision()
}

struct PreparedCommand {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
}

fn prepare_command(
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

fn format_command_line(prepared: &PreparedCommand) -> String {
    shell_join(&prepared.program, &prepared.args)
}

fn build_session_id(slot_id: &str, runtime: RuntimeKind) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("sess-{}-{}-{suffix}", runtime.as_str(), slot_id)
}

fn start_tmux_session(
    session_id: &str,
    slot_path: &Path,
    prepared: &PreparedCommand,
    combined_log_path: PathBuf,
) -> Result<()> {
    if !detect_tmux() {
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

fn tmux_kill_session(supervisor_ref: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", supervisor_ref])
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

fn read_exit_code(paths: &AppPaths, session_id: &str) -> Result<Option<i64>> {
    let path = exit_code_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read exit-code file at {}", path.display()))?;
    Ok(contents.trim().parse::<i64>().ok())
}

fn read_pid(paths: &AppPaths, session_id: &str) -> Result<Option<u32>> {
    let path = pid_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read pid sidecar at {}", path.display()))?;
    Ok(contents.trim().parse::<u32>().ok())
}

fn pid_sidecar_exists(paths: &AppPaths, session_id: &str) -> bool {
    pid_path_for(&paths.logs_dir.join("sessions"), session_id).exists()
}

fn exit_code_path_for(logs_dir: &Path, session_id: &str) -> PathBuf {
    logs_dir.join(format!("{session_id}.exit"))
}

fn pid_path_for(logs_dir: &Path, session_id: &str) -> PathBuf {
    logs_dir.join(format!("{session_id}.pid"))
}

fn clear_sidecar_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).with_context(|| format!("failed to remove sidecar {}", path.display()))
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppPaths;

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

        let pid_path = pid_path_for(&paths.logs_dir.join("sessions"), &prepared.session.id);
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
        fs::write(pid_path_for(&sessions_dir, "sess-1"), "pending")?;

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
        fs::write(pid_path_for(&sessions_dir, "sess-3"), "999999")?;
        fs::write(exit_code_path_for(&sessions_dir, "sess-3"), "0")?;

        let mut session = running_oneshot_session("sess-3");
        assert!(sync_session(&paths, &mut session)?);
        assert_eq!(session.status, "completed");
        assert_eq!(session.exit_code, Some(0));
        assert!(!pid_path_for(&sessions_dir, "sess-3").exists());
        Ok(())
    }
}
