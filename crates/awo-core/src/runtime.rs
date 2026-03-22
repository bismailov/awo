use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use crate::platform::{default_shell_program, executable_exists};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use strum_macros::{Display, EnumString, IntoStaticStr};

mod supervisor;

use supervisor::{
    PreparedCommand, SessionSupervisor, build_session_id, clear_sidecar_if_exists,
    exit_code_path_for, format_command_line, materialize_shell_script, pid_path_for,
    pid_sidecar_exists, prepare_command, process_is_running, pty_supervision_available,
    read_exit_code, read_pid, session_io_layout,
};
#[cfg(test)]
use supervisor::{shell_join, shell_quote, supervisor_ref};

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecord {
    pub id: String,
    pub repo_id: String,
    pub slot_id: String,
    pub runtime: String,
    pub supervisor: Option<String>,
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
        self.status == "running" && SessionSupervisor::from_session(self).is_some()
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    IntoStaticStr,
)]
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
        if pty_supervision_available() {
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
    supervisor: Option<SessionSupervisor>,
    stdout_path: PathBuf,
    stderr_path: Option<PathBuf>,
}

pub fn prepare_session(request: SessionRunRequest<'_>) -> AwoResult<PreparedSession> {
    let session_id = build_session_id(request.slot_id, request.runtime);
    let logs_dir = request.paths.logs_dir.join("sessions");
    fs::create_dir_all(&logs_dir)
        .map_err(|source| AwoError::io("create session log directory", &logs_dir, source))?;
    let supervisor = SessionSupervisor::from_launch_mode(request.launch_mode)?;

    if supervisor.is_none() && !request.dry_run {
        clear_sidecar_if_exists(&exit_code_path_for(&logs_dir, &session_id))?;
        let pid_path = pid_path_for(&logs_dir, &session_id);
        fs::write(&pid_path, "pending")
            .map_err(|source| AwoError::io("prepare pid sidecar", &pid_path, source))?;
    }

    let io_layout = session_io_layout(&logs_dir, &session_id, supervisor);
    let stdout_path = io_layout.stdout_path;
    let stderr_path = io_layout.stderr_path;
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
            supervisor: supervisor.map(|supervisor| supervisor.as_str().to_string()),
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
        supervisor,
        stdout_path,
        stderr_path,
    })
}

pub fn execute_prepared_session(
    mut prepared_session: PreparedSession,
) -> AwoResult<SessionExecutionResult> {
    if prepared_session.session.dry_run {
        return Ok(SessionExecutionResult {
            session: prepared_session.session,
        });
    }

    if let Some(supervisor) = prepared_session.supervisor {
        supervisor.launch(
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
    materialize_shell_script(&prepared_session.prepared)?;

    let child = Command::new(&prepared_session.prepared.program)
        .args(&prepared_session.prepared.args)
        .current_dir(&prepared_session.prepared.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .inspect_err(|_error| {
            let _ = clear_sidecar_if_exists(&pid_path);
        })
        .map_err(|error| {
            AwoError::runtime_launch(format!(
                "failed to launch runtime `{}`: {error}",
                prepared_session.session.runtime
            ))
        })?;
    fs::write(&pid_path, child.id().to_string())
        .map_err(|source| AwoError::io("write pid sidecar", &pid_path, source))?;

    let output = child
        .wait_with_output()
        .inspect_err(|_error| {
            let _ = clear_sidecar_if_exists(&pid_path);
        })
        .map_err(|error| {
            AwoError::runtime_launch(format!(
                "failed while waiting for runtime `{}`: {error}",
                prepared_session.session.runtime
            ))
        })?;

    fs::write(&prepared_session.stdout_path, &output.stdout).map_err(|source| {
        AwoError::io("write stdout log", &prepared_session.stdout_path, source)
    })?;
    if let Some(stderr_path) = &prepared_session.stderr_path {
        fs::write(stderr_path, &output.stderr)
            .map_err(|source| AwoError::io("write stderr log", stderr_path, source))?;
    }

    prepared_session.session.status = if output.status.success() {
        "completed"
    } else {
        "failed"
    }
    .to_string();
    prepared_session.session.exit_code = output.status.code().map(i64::from);
    let exit_code = prepared_session.session.exit_code.unwrap_or(-1);
    fs::write(&exit_path, exit_code.to_string())
        .map_err(|source| AwoError::io("write exit-code sidecar", &exit_path, source))?;
    clear_sidecar_if_exists(&pid_path)?;

    Ok(SessionExecutionResult {
        session: prepared_session.session,
    })
}

pub fn sync_session(paths: &AppPaths, session: &mut SessionRecord) -> AwoResult<bool> {
    if session.is_terminal() {
        return Ok(false);
    }
    if session.status != "running" {
        return Ok(false);
    }

    if !session.is_supervised() {
        return sync_oneshot_session(paths, session);
    }

    let supervisor = SessionSupervisor::from_session(session).ok_or_else(|| {
        AwoError::invalid_state("running supervised session is missing supervisor metadata")
    })?;
    match supervisor.sync(paths, &session.id)? {
        Some(exit_code) => {
            session.exit_code = Some(exit_code);
            session.status = if exit_code == 0 {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
            Ok(true)
        }
        None => Ok(false),
    }
}

pub fn cancel_session(paths: &AppPaths, session: &mut SessionRecord) -> AwoResult<bool> {
    if session.is_terminal() {
        return Ok(false);
    }

    if session.is_supervised() {
        let supervisor = SessionSupervisor::from_session(session).ok_or_else(|| {
            AwoError::invalid_state("running supervised session is missing supervisor metadata")
        })?;
        supervisor.cancel(paths, session)?;
    }

    session.status = "cancelled".to_string();
    Ok(true)
}

fn sync_oneshot_session(paths: &AppPaths, session: &mut SessionRecord) -> AwoResult<bool> {
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
    pty_supervision_available()
}
#[cfg(test)]
mod tests;
