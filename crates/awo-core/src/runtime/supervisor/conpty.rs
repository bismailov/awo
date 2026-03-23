//! Windows ConPTY session supervisor.
//!
//! Uses `portable-pty` to spawn processes in a Windows Pseudo Console,
//! providing PTY supervision on Windows that parallels the Unix tmux backend.
//!
//! A background thread captures PTY output to a log file and writes the
//! process exit code to a sidecar file when the session terminates.

use super::{PreparedCommand, exit_code_path_for, materialize_shell_script, read_exit_code};
use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Spawn a command inside a ConPTY pseudo-console.
///
/// A background thread captures PTY output to `combined_log_path` and
/// writes the process exit code to the sidecar file when done.
pub fn launch(
    session_id: &str,
    _slot_path: &Path,
    prepared: &PreparedCommand,
    combined_log_path: PathBuf,
) -> AwoResult<()> {
    // Ensure log directory exists and clean up stale files
    if let Some(parent) = combined_log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| AwoError::io("create conpty log directory", parent, source))?;
    }
    if combined_log_path.exists() {
        fs::remove_file(&combined_log_path).map_err(|source| {
            AwoError::io("remove previous PTY log", &combined_log_path, source)
        })?;
    }
    let logs_dir = combined_log_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let exit_path = exit_code_path_for(&logs_dir, session_id);
    if exit_path.exists() {
        fs::remove_file(&exit_path)
            .map_err(|source| AwoError::io("remove previous exit-code file", &exit_path, source))?;
    }
    materialize_shell_script(prepared)?;

    // Open a Windows pseudo-console
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AwoError::supervisor(format!("failed to open ConPTY: {e}")))?;

    // Build the command to run in the PTY
    let mut cmd = CommandBuilder::new(&prepared.program);
    cmd.args(&prepared.args);
    cmd.cwd(&prepared.cwd);

    // Spawn the child process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AwoError::supervisor(format!("failed to spawn in ConPTY: {e}")))?;
    // Close the slave so the reader will see EOF when the process exits
    drop(pair.slave);

    // Write PID sidecar
    let pid = child.process_id().unwrap_or(0);
    let pid_path = super::pid_path_for(&logs_dir, session_id);
    fs::write(&pid_path, pid.to_string())
        .map_err(|source| AwoError::io("write conpty pid sidecar", &pid_path, source))?;

    // Clone a reader from the PTY master for output capture
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AwoError::supervisor(format!("failed to clone ConPTY reader: {e}")))?;

    // Spawn a background thread that captures PTY output and waits for exit
    std::thread::spawn(move || {
        let mut log_file = match fs::File::create(&combined_log_path) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("failed to create ConPTY log file: {e}");
                return;
            }
        };

        // Stream PTY output to the log file
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = log_file.write_all(&buf[..n]) {
                        tracing::error!("failed to write ConPTY output: {e}");
                        break;
                    }
                }
                Err(e) => {
                    tracing::debug!("ConPTY reader closed: {e}");
                    break;
                }
            }
        }

        // Wait for the child process to finish and write exit code
        let exit_code = match child.wait() {
            Ok(status) => {
                if status.success() {
                    0
                } else {
                    1
                }
            }
            Err(e) => {
                tracing::warn!("failed to wait for ConPTY child: {e}");
                -1
            }
        };
        let _ = fs::write(&exit_path, exit_code.to_string());
    });

    Ok(())
}

/// Check whether the ConPTY session is still running.
///
/// Returns `None` if the process is alive, `Some(exit_code)` if it has
/// exited.  Falls back to -1 if the exit code cannot be determined.
pub fn sync(paths: &AppPaths, session_id: &str) -> AwoResult<Option<i64>> {
    if let Some(pid) = super::read_pid(paths, session_id)? {
        if pid > 0 && super::process_is_running(pid) {
            return Ok(None);
        }
    }
    Ok(read_exit_code(paths, session_id)?.or(Some(-1)))
}

/// Terminate a ConPTY-supervised session by killing the process via PID.
pub fn kill(paths: &AppPaths, session_id: &str) -> AwoResult<()> {
    if let Some(pid) = super::read_pid(paths, session_id)? {
        if pid > 0 && super::process_is_running(pid) {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }
    }
    Ok(())
}
