#[cfg(windows)]
mod conpty;
mod tmux;

use super::{RuntimeKind, SessionLaunchMode, SessionRecord};
use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use crate::platform::{
    default_shell_program, shell_command_args, shell_script_args, supports_tmux_supervision,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionSupervisor {
    Tmux,
    #[cfg(windows)]
    Conpty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionIoLayout {
    pub(super) stdout_path: PathBuf,
    pub(super) stderr_path: Option<PathBuf>,
}

pub(super) struct PreparedCommand {
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) cwd: PathBuf,
    pub(super) display_command_line: Option<String>,
    pub(super) script_path: Option<PathBuf>,
    pub(super) script_body: Option<String>,
}

impl SessionSupervisor {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            #[cfg(windows)]
            Self::Conpty => "conpty",
        }
    }

    pub(super) fn from_launch_mode(launch_mode: SessionLaunchMode) -> AwoResult<Option<Self>> {
        match launch_mode {
            SessionLaunchMode::Oneshot => Ok(None),
            SessionLaunchMode::Pty => configured_pty_supervisor().map(Some).ok_or_else(|| {
                AwoError::supervisor("PTY launch is not implemented on this platform")
            }),
        }
    }

    pub(super) fn from_session(session: &SessionRecord) -> Option<Self> {
        if let Some(supervisor) = session
            .supervisor
            .as_deref()
            .and_then(SessionSupervisor::from_persisted_name)
        {
            return Some(supervisor);
        }

        known_supervisors()
            .iter()
            .copied()
            .find(|supervisor| supervisor.matches_session(session))
    }

    pub(super) fn is_available(self) -> bool {
        match self {
            Self::Tmux => supports_tmux_supervision(),
            #[cfg(windows)]
            Self::Conpty => true,
        }
    }

    pub(super) fn io_layout(self, logs_dir: &Path, session_id: &str) -> SessionIoLayout {
        match self {
            Self::Tmux => SessionIoLayout {
                stdout_path: logs_dir.join(format!("{session_id}.pty.log")),
                stderr_path: None,
            },
            #[cfg(windows)]
            Self::Conpty => SessionIoLayout {
                stdout_path: logs_dir.join(format!("{session_id}.pty.log")),
                stderr_path: None,
            },
        }
    }

    pub(super) fn launch(
        self,
        session_id: &str,
        slot_path: &Path,
        prepared: &PreparedCommand,
        combined_log_path: PathBuf,
    ) -> AwoResult<()> {
        match self {
            Self::Tmux => tmux::launch(session_id, slot_path, prepared, combined_log_path),
            #[cfg(windows)]
            Self::Conpty => conpty::launch(session_id, slot_path, prepared, combined_log_path),
        }
    }

    pub(super) fn sync(self, paths: &AppPaths, session_id: &str) -> AwoResult<Option<i64>> {
        match self {
            Self::Tmux => tmux::sync(paths, session_id),
            #[cfg(windows)]
            Self::Conpty => conpty::sync(paths, session_id),
        }
    }

    pub(super) fn cancel(self, paths: &AppPaths, session: &mut SessionRecord) -> AwoResult<()> {
        match self {
            Self::Tmux => {
                let _ = tmux::kill(&session.id);
                if session.exit_code.is_none() {
                    session.exit_code = read_exit_code(paths, &session.id)?;
                }
                Ok(())
            }
            #[cfg(windows)]
            Self::Conpty => {
                let _ = conpty::kill(paths, &session.id);
                if session.exit_code.is_none() {
                    session.exit_code = read_exit_code(paths, &session.id)?;
                }
                Ok(())
            }
        }
    }

    fn matches_session(self, session: &SessionRecord) -> bool {
        match self {
            Self::Tmux => {
                session.stderr_path.is_none()
                    && session
                        .stdout_path
                        .as_deref()
                        .is_some_and(|path| path.ends_with(".pty.log"))
            }
            #[cfg(windows)]
            Self::Conpty => {
                session.stderr_path.is_none()
                    && session
                        .stdout_path
                        .as_deref()
                        .is_some_and(|path| path.ends_with(".pty.log"))
            }
        }
    }

    fn from_persisted_name(value: &str) -> Option<Self> {
        match value {
            "tmux" => Some(Self::Tmux),
            #[cfg(windows)]
            "conpty" => Some(Self::Conpty),
            _ => None,
        }
    }
}

impl SessionIoLayout {
    fn oneshot(logs_dir: &Path, session_id: &str) -> Self {
        Self {
            stdout_path: logs_dir.join(format!("{session_id}.out.log")),
            stderr_path: Some(logs_dir.join(format!("{session_id}.err.log"))),
        }
    }
}

pub(super) fn pty_supervision_available() -> bool {
    configured_pty_supervisor().is_some_and(SessionSupervisor::is_available)
}

pub(super) fn session_io_layout(
    logs_dir: &Path,
    session_id: &str,
    supervisor: Option<SessionSupervisor>,
) -> SessionIoLayout {
    supervisor.map_or_else(
        || SessionIoLayout::oneshot(logs_dir, session_id),
        |supervisor| supervisor.io_layout(logs_dir, session_id),
    )
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
                display_command_line: None,
                script_path: None,
                script_body: None,
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
                display_command_line: None,
                script_path: None,
                script_body: None,
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
                display_command_line: None,
                script_path: None,
                script_body: None,
            }
        }
        RuntimeKind::Shell => {
            let display_command_line =
                shell_join(default_shell_program(), &shell_command_args(prompt));
            let script_path = if cfg!(windows) {
                stdout_path.with_extension("ps1")
            } else {
                stdout_path.with_extension("sh")
            };
            PreparedCommand {
                program: default_shell_program().to_string(),
                args: shell_script_args(&script_path),
                cwd: slot_path.to_path_buf(),
                display_command_line: Some(display_command_line),
                script_path: Some(script_path),
                script_body: Some(prompt.to_string()),
            }
        }
    }
}

pub(super) fn format_command_line(prepared: &PreparedCommand) -> String {
    if let Some(display_command_line) = &prepared.display_command_line {
        return display_command_line.clone();
    }
    shell_join(&prepared.program, &prepared.args)
}

pub(super) fn materialize_shell_script(prepared: &PreparedCommand) -> AwoResult<()> {
    let (Some(script_path), Some(script_body)) = (&prepared.script_path, &prepared.script_body)
    else {
        return Ok(());
    };

    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| AwoError::io("create shell script directory", parent, source))?;
    }

    let mut contents = script_body.clone();
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    fs::write(script_path, contents)
        .map_err(|source| AwoError::io("write shell script", script_path, source))?;
    Ok(())
}

pub(super) fn build_session_id(slot_id: &str, runtime: RuntimeKind) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("sess-{}-{}-{suffix}", runtime.as_str(), slot_id)
}

pub(super) fn read_exit_code(paths: &AppPaths, session_id: &str) -> AwoResult<Option<i64>> {
    let path = exit_code_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .map_err(|source| AwoError::io("read exit-code sidecar", &path, source))?;
    Ok(contents.trim().parse::<i64>().ok())
}

pub(super) fn read_pid(paths: &AppPaths, session_id: &str) -> AwoResult<Option<u32>> {
    let path = pid_path_for(&paths.logs_dir.join("sessions"), session_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .map_err(|source| AwoError::io("read pid sidecar", &path, source))?;
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

#[cfg(test)]
pub(super) fn supervisor_ref(session_id: &str) -> String {
    tmux::supervisor_ref(session_id)
}

pub(super) fn clear_sidecar_if_exists(path: &Path) -> AwoResult<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|source| AwoError::io("remove sidecar", path, source))
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

pub(super) fn shell_join(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().cloned())
        .map(|part| shell_quote(&part))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn configured_pty_supervisor() -> Option<SessionSupervisor> {
    #[cfg(unix)]
    {
        Some(SessionSupervisor::Tmux)
    }

    #[cfg(windows)]
    {
        Some(SessionSupervisor::Conpty)
    }

    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

fn known_supervisors() -> &'static [SessionSupervisor] {
    #[cfg(unix)]
    {
        &[SessionSupervisor::Tmux]
    }

    #[cfg(windows)]
    {
        &[SessionSupervisor::Conpty]
    }

    #[cfg(not(any(unix, windows)))]
    {
        &[]
    }
}
