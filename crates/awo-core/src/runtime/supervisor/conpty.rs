use super::{exit_code_path_for, materialize_shell_script, read_exit_code, PreparedCommand};

use crate::app::AppPaths;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn launch(
    session_id: &str,
    slot_path: &Path,
    prepared: &PreparedCommand,
    combined_log_path: PathBuf,
) -> Result<()> {
    if let Some(parent) = combined_log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log dir at {}", parent.display()))?;
    }
    materialize_shell_script(prepared)
        .with_context(|| format!("failed to materialize shell prompt script for `{session_id}`"))?;

    let pid_path = super::pid_path_for(
        combined_log_path.parent().unwrap_or(Path::new(".")),
        session_id,
    );
    fs::write(&pid_path, "0").with_context(|| "failed to write pid")?;

    Ok(())
}

pub fn sync(paths: &AppPaths, session_id: &str) -> Result<Option<i64>> {
    if let Some(pid) = super::read_pid(paths, session_id)? {
        if super::process_is_running(pid) {
            return Ok(None);
        }
    }
    Ok(read_exit_code(paths, session_id)?.or(Some(-1)))
}

pub fn kill(session_id: &str) -> Result<()> {
    Ok(())
}
