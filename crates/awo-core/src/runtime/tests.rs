use super::*;
use crate::app::AppPaths;
use crate::platform::{default_shell_program, shell_command_args, shell_script_args};
use anyhow::Result;
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
        worktrees_dir: root.join("data/worktrees"),
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
        supervisor: None,
        prompt: "echo hi".to_string(),
        status: SessionStatus::Running,
        read_only: true,
        dry_run: false,
        command_line: "sh -lc 'echo hi'".to_string(),
        stdout_path: Some("/tmp/stdout.log".to_string()),
        stderr_path: Some("/tmp/stderr.log".to_string()),
        exit_code: None,
        end_reason: None,
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn running_supervised_session(session_id: &str) -> SessionRecord {
    SessionRecord {
        id: session_id.to_string(),
        repo_id: "repo-1".to_string(),
        slot_id: "slot-1".to_string(),
        runtime: "shell".to_string(),
        supervisor: Some("tmux".to_string()),
        prompt: "echo hi".to_string(),
        status: SessionStatus::Running,
        read_only: true,
        dry_run: false,
        command_line: "sh -lc 'echo hi'".to_string(),
        stdout_path: Some(format!("/tmp/{session_id}.pty.log")),
        stderr_path: None,
        exit_code: None,
        end_reason: None,
        timeout_secs: None,
        started_at: None,
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
        timeout_secs: None,
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
    assert_eq!(session.status, SessionStatus::Running);
    Ok(())
}

#[test]
fn sync_oneshot_marks_missing_process_as_failed() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    fs::create_dir_all(paths.logs_dir.join("sessions"))?;

    let mut session = running_oneshot_session("sess-2");
    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, SessionStatus::Failed);
    Ok(())
}

#[test]
fn sync_oneshot_timeout_sets_timeout_end_reason() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    fs::create_dir_all(paths.logs_dir.join("sessions"))?;

    let mut session = running_oneshot_session("sess-timeout");
    session.timeout_secs = Some(1);
    session.started_at = Some((Utc::now() - chrono::Duration::seconds(5)).to_rfc3339());

    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, SessionStatus::Failed);
    assert_eq!(session.end_reason, Some(SessionEndReason::Timeout));
    assert_eq!(session.capacity_status(), SessionCapacityStatus::TimedOut);
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
    assert_eq!(session.status, SessionStatus::Completed);
    assert_eq!(session.exit_code, Some(0));
    assert!(!pid_path(&sessions_dir, "sess-3").exists());
    Ok(())
}

#[test]
fn sync_oneshot_detects_token_exhaustion_from_logs() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-tokens"), "999999")?;
    fs::write(exit_path(&sessions_dir, "sess-tokens"), "1")?;

    let mut session = running_oneshot_session("sess-tokens");
    let stdout_path = sessions_dir.join("sess-tokens.out.log");
    fs::write(
        &stdout_path,
        "model stopped: out of tokens while generating response\n",
    )?;
    session.stdout_path = Some(stdout_path.display().to_string());

    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, SessionStatus::Failed);
    assert_eq!(session.end_reason, Some(SessionEndReason::TokenExhausted));
    assert_eq!(session.capacity_status(), SessionCapacityStatus::Exhausted);
    Ok(())
}

#[test]
fn sync_oneshot_detects_provider_limits_from_logs() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-provider-limit"), "999999")?;
    fs::write(exit_path(&sessions_dir, "sess-provider-limit"), "1")?;

    let mut session = running_oneshot_session("sess-provider-limit");
    let stderr_path = sessions_dir.join("sess-provider-limit.err.log");
    fs::write(
        &stderr_path,
        "request failed: rate limit exceeded for current quota\n",
    )?;
    session.stderr_path = Some(stderr_path.display().to_string());

    assert!(sync_session(&paths, &mut session)?);
    assert_eq!(session.status, SessionStatus::Failed);
    assert_eq!(session.end_reason, Some(SessionEndReason::ProviderLimited));
    assert_eq!(
        session.capacity_status(),
        SessionCapacityStatus::ProviderLimited
    );
    Ok(())
}

#[test]
fn running_supervised_session_is_detected_via_supervisor_metadata() {
    let session = running_supervised_session("sess-4");
    assert!(session.is_supervised());
    assert_eq!(
        SessionSupervisor::from_session(&session),
        Some(SessionSupervisor::Tmux)
    );
    #[cfg(unix)]
    assert!(session_supports_embedded_terminal(&session));
}

#[test]
fn oneshot_records_do_not_match_supervisor_metadata() {
    let session = running_oneshot_session("sess-5");
    assert!(!session.is_supervised());
    assert_eq!(SessionSupervisor::from_session(&session), None);
    assert!(!session_supports_embedded_terminal(&session));
}

#[test]
fn oneshot_layout_keeps_split_logs() {
    let logs_dir = Path::new("/tmp/awo-runtime-test");
    let layout = session_io_layout(logs_dir, "sess-6", None);
    assert_eq!(layout.stdout_path, logs_dir.join("sess-6.out.log"));
    assert_eq!(layout.stderr_path, Some(logs_dir.join("sess-6.err.log")));
}

#[test]
fn tmux_supervisor_layout_uses_single_pty_log() {
    let logs_dir = Path::new("/tmp/awo-runtime-test");
    let layout = session_io_layout(logs_dir, "sess-7", Some(SessionSupervisor::Tmux));
    assert_eq!(layout.stdout_path, logs_dir.join("sess-7.pty.log"));
    assert_eq!(layout.stderr_path, None);
}

#[test]
fn prepare_command_codex_write_mode() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Codex,
        slot,
        "implement feature",
        false,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert_eq!(prepared.program, "codex");
    assert!(prepared.args.contains(&"--full-auto".to_string()));
    assert!(prepared.args.contains(&"--skip-git-repo-check".to_string()));
    assert!(prepared.args.contains(&"implement feature".to_string()));
    assert!(!prepared.args.contains(&"-s".to_string()));
    assert_eq!(prepared.cwd, slot);
}

#[test]
fn prepare_command_codex_read_only() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Codex,
        slot,
        "review code",
        true,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert!(prepared.args.contains(&"-s".to_string()));
    assert!(prepared.args.contains(&"read-only".to_string()));
    assert!(!prepared.args.contains(&"--full-auto".to_string()));
}

#[test]
fn prepare_command_codex_pty_omits_output_last_message() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Codex,
        slot,
        "task",
        false,
        stdout,
        SessionLaunchMode::Pty,
    );
    assert!(!prepared.args.contains(&"--output-last-message".to_string()));
}

#[test]
fn prepare_command_claude_write_mode() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Claude,
        slot,
        "fix bug",
        false,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert_eq!(prepared.program, "claude");
    assert!(prepared.args.contains(&"-p".to_string()));
    assert!(prepared.args.contains(&"--permission-mode".to_string()));
    assert!(prepared.args.contains(&"acceptEdits".to_string()));
    assert!(!prepared.args.contains(&"plan".to_string()));
}

#[test]
fn prepare_command_claude_read_only() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Claude,
        slot,
        "explain code",
        true,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert!(prepared.args.contains(&"plan".to_string()));
    assert!(!prepared.args.contains(&"acceptEdits".to_string()));
}

#[test]
fn prepare_command_gemini_write_mode() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Gemini,
        slot,
        "refactor module",
        false,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert_eq!(prepared.program, "gemini");
    assert!(prepared.args.contains(&"--prompt".to_string()));
    assert!(prepared.args.contains(&"--approval-mode".to_string()));
    assert!(prepared.args.contains(&"auto_edit".to_string()));
}

#[test]
fn prepare_command_gemini_read_only() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Gemini,
        slot,
        "analyze code",
        true,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    assert!(prepared.args.contains(&"plan".to_string()));
    assert!(!prepared.args.contains(&"auto_edit".to_string()));
}

#[test]
fn prepare_command_shell_uses_platform_shell() {
    let slot = Path::new("/tmp/slot");
    let stdout = Path::new("/tmp/out.log");
    let prepared = prepare_command(
        RuntimeKind::Shell,
        slot,
        "echo hello",
        false,
        stdout,
        SessionLaunchMode::Oneshot,
    );
    let expected_shell = default_shell_program();
    assert_eq!(prepared.program, expected_shell);
    let expected_display = shell_join(expected_shell, &shell_command_args("echo hello"));
    assert_eq!(
        prepared.display_command_line.as_deref(),
        Some(expected_display.as_str())
    );

    let expected_script = if cfg!(windows) {
        stdout.with_extension("ps1")
    } else {
        stdout.with_extension("sh")
    };
    let expected_args = shell_script_args(&expected_script);
    assert_eq!(prepared.args, expected_args);
    assert_eq!(
        prepared.script_path.as_deref(),
        Some(expected_script.as_path())
    );
    assert_eq!(prepared.script_body.as_deref(), Some("echo hello"));
    assert_eq!(prepared.cwd, slot);
}

#[cfg(not(windows))]
#[test]
fn materialize_shell_script_writes_prompt_body() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let script_path = temp_dir.path().join("prompt.sh");
    let prepared = PreparedCommand {
        program: default_shell_program().to_string(),
        args: shell_script_args(&script_path),
        cwd: temp_dir.path().to_path_buf(),
        display_command_line: None,
        script_path: Some(script_path.clone()),
        script_body: Some("echo hello".to_string()),
    };

    materialize_shell_script(&prepared)?;

    let contents = fs::read_to_string(&script_path)?;
    assert_eq!(contents, "echo hello\n");
    Ok(())
}

#[test]
fn prepare_command_cwd_matches_slot_path() {
    let slot = Path::new("/some/custom/slot/path");
    let stdout = Path::new("/tmp/out.log");
    for runtime in [
        RuntimeKind::Codex,
        RuntimeKind::Claude,
        RuntimeKind::Gemini,
        RuntimeKind::Shell,
    ] {
        let prepared = prepare_command(
            runtime,
            slot,
            "task",
            false,
            stdout,
            SessionLaunchMode::Oneshot,
        );
        assert_eq!(prepared.cwd, slot, "cwd mismatch for runtime {:?}", runtime);
    }
}

#[test]
fn shell_quote_empty_string() {
    assert_eq!(shell_quote(""), "''");
}

#[test]
fn shell_quote_simple_value() {
    assert_eq!(shell_quote("hello"), "'hello'");
}

#[test]
fn shell_quote_escapes_single_quotes() {
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
}

#[test]
fn shell_quote_preserves_spaces() {
    assert_eq!(shell_quote("hello world"), "'hello world'");
}

#[test]
fn shell_quote_preserves_special_chars() {
    assert_eq!(shell_quote("a$b"), "'a$b'");
    assert_eq!(shell_quote("a;b"), "'a;b'");
    assert_eq!(shell_quote("a|b"), "'a|b'");
}

#[test]
fn shell_join_single_program() {
    let result = shell_join("echo", &[]);
    assert_eq!(result, "'echo'");
}

#[test]
fn shell_join_program_with_args() {
    let result = shell_join("ls", &["-la".to_string(), "/tmp".to_string()]);
    assert_eq!(result, "'ls' '-la' '/tmp'");
}

#[test]
fn shell_join_handles_args_with_spaces() {
    let result = shell_join("echo", &["hello world".to_string()]);
    assert_eq!(result, "'echo' 'hello world'");
}

#[test]
fn format_command_line_produces_shell_quoted_output() {
    let prepared = PreparedCommand {
        program: "codex".to_string(),
        args: vec!["exec".to_string(), "--full-auto".to_string()],
        cwd: Path::new("/tmp").to_path_buf(),
        display_command_line: None,
        script_path: None,
        script_body: None,
    };
    let line = format_command_line(&prepared);
    assert_eq!(line, "'codex' 'exec' '--full-auto'");
}

#[test]
fn supervisor_ref_is_deterministic() {
    let ref1 = supervisor_ref("sess-shell-slot-1-12345");
    let ref2 = supervisor_ref("sess-shell-slot-1-12345");
    assert_eq!(ref1, ref2);
}

#[test]
fn supervisor_ref_starts_with_awo_prefix() {
    let result = supervisor_ref("sess-codex-slot-1-99999");
    assert!(
        result.starts_with("awo-"),
        "expected awo- prefix, got {result}"
    );
}

#[test]
fn supervisor_ref_has_expected_length() {
    let result = supervisor_ref("any-session-id");
    assert_eq!(result.len(), 20, "unexpected length for {result}");
}

#[test]
fn supervisor_ref_differs_for_different_sessions() {
    let ref1 = supervisor_ref("sess-a");
    let ref2 = supervisor_ref("sess-b");
    assert_ne!(ref1, ref2);
}

#[test]
fn exit_code_path_uses_exit_extension() {
    let path = exit_code_path_for(Path::new("/logs"), "sess-1");
    assert_eq!(path, Path::new("/logs/sess-1.exit"));
}

#[test]
fn pid_path_uses_pid_extension() {
    let path = pid_path_for(Path::new("/logs"), "sess-1");
    assert_eq!(path, Path::new("/logs/sess-1.pid"));
}

#[test]
fn sidecar_paths_handle_complex_session_ids() {
    let id = "sess-codex-slot-repo-abc123-1711000000000";
    let exit = exit_code_path_for(Path::new("/var/awo/logs/sessions"), id);
    let pid = pid_path_for(Path::new("/var/awo/logs/sessions"), id);
    assert!(exit.to_string_lossy().ends_with(".exit"));
    assert!(pid.to_string_lossy().ends_with(".pid"));
    assert!(exit.to_string_lossy().contains(id));
    assert!(pid.to_string_lossy().contains(id));
}

#[test]
fn build_session_id_contains_runtime_and_slot() {
    let id = build_session_id("slot-abc", RuntimeKind::Claude);
    assert!(id.starts_with("sess-claude-slot-abc-"), "got: {id}");
}

#[test]
fn build_session_id_contains_shell_runtime() {
    let id = build_session_id("slot-1", RuntimeKind::Shell);
    assert!(id.starts_with("sess-shell-slot-1-"), "got: {id}");
}

#[cfg(unix)]
#[test]
fn prepare_session_pty_log_path_naming() -> Result<()> {
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
        dry_run: true,
        launch_mode: SessionLaunchMode::Pty,
        timeout_secs: None,
    })?;

    let stdout = prepared
        .session
        .stdout_path
        .expect("pty session should have stdout log");
    assert!(stdout.ends_with(".pty.log"), "got: {stdout}");
    assert!(
        prepared.session.stderr_path.is_none(),
        "PTY mode should not have separate stderr"
    );
    Ok(())
}

#[cfg(not(any(unix, windows)))]
#[test]
fn prepare_session_pty_is_unavailable_on_non_unix() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let slot_path = temp_dir.path().join("slot");
    fs::create_dir_all(&slot_path)?;

    let error = prepare_session(SessionRunRequest {
        paths: &paths,
        repo_id: "repo-1",
        slot_id: "slot-1",
        slot_path: &slot_path,
        runtime: RuntimeKind::Shell,
        prompt: "echo hi",
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Pty,
        timeout_secs: None,
    })
    .expect_err("PTY sessions should be unavailable on non-Unix platforms");

    assert!(error.to_string().contains("PTY launch is not implemented"));
    Ok(())
}

#[test]
fn prepare_session_oneshot_log_path_naming() -> Result<()> {
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
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        timeout_secs: None,
    })?;

    let stdout = prepared
        .session
        .stdout_path
        .expect("oneshot session should have stdout log");
    let stderr = prepared
        .session
        .stderr_path
        .expect("oneshot session should have stderr log");
    assert!(stdout.ends_with(".out.log"), "got stdout: {stdout}");
    assert!(stderr.ends_with(".err.log"), "got stderr: {stderr}");
    Ok(())
}

#[test]
fn session_record_terminal_states() {
    for status in [
        SessionStatus::Completed,
        SessionStatus::Failed,
        SessionStatus::Cancelled,
    ] {
        let mut session = running_oneshot_session("s1");
        session.status = status;
        assert!(session.is_terminal(), "{:?} should be terminal", status);
        assert!(
            !session.blocks_release(),
            "{:?} should not block release",
            status
        );
    }
}

#[test]
fn session_record_non_terminal_states() {
    for status in [SessionStatus::Running, SessionStatus::Prepared] {
        let mut session = running_oneshot_session("s1");
        session.status = status;
        assert!(
            !session.is_terminal(),
            "{:?} should not be terminal",
            status
        );
        assert!(
            session.blocks_release(),
            "{:?} should block release",
            status
        );
    }
}

#[test]
fn session_record_status_helpers_classify_known_states() {
    let mut session = running_oneshot_session("s1");
    session.status = SessionStatus::Running;
    assert!(session.is_running());
    assert!(!session.is_supervised());
    assert!(!session.is_prepared());
    assert!(!session.is_terminal());

    session.status = SessionStatus::Prepared;
    assert!(session.is_prepared());
    assert!(!session.is_running());
    assert!(!session.is_terminal());

    session.status = SessionStatus::Completed;
    assert!(session.is_completed());
    assert!(session.is_terminal());

    session.status = SessionStatus::Failed;
    assert!(session.is_failed());
    assert!(session.is_terminal());

    session.status = SessionStatus::Cancelled;
    assert!(session.is_cancelled());
    assert!(session.is_terminal());
}

#[test]
fn session_record_write_capable() {
    let mut session = running_oneshot_session("s1");
    session.read_only = false;
    assert!(session.is_write_capable());
    session.read_only = true;
    assert!(!session.is_write_capable());
}

#[test]
fn session_record_is_supervised_requires_running_and_no_stderr() {
    let mut session = running_oneshot_session("s1");
    assert!(!session.is_supervised());

    session.supervisor = Some("tmux".to_string());
    session.stderr_path = None;
    assert!(session.is_supervised());

    session.status = SessionStatus::Completed;
    assert!(!session.is_supervised());
}

#[test]
fn runtime_kind_parse_and_display_roundtrip() {
    for (input, expected) in [
        ("codex", RuntimeKind::Codex),
        ("claude", RuntimeKind::Claude),
        ("gemini", RuntimeKind::Gemini),
        ("shell", RuntimeKind::Shell),
    ] {
        let parsed: RuntimeKind = input.parse().expect("runtime should parse");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), input);
    }
}

#[test]
fn runtime_kind_uses_agent_prompt() {
    assert!(RuntimeKind::Codex.uses_agent_prompt());
    assert!(RuntimeKind::Claude.uses_agent_prompt());
    assert!(RuntimeKind::Gemini.uses_agent_prompt());
    assert!(!RuntimeKind::Shell.uses_agent_prompt());
}

#[test]
fn session_launch_mode_roundtrip() {
    for (input, expected) in [
        ("pty", SessionLaunchMode::Pty),
        ("oneshot", SessionLaunchMode::Oneshot),
    ] {
        let parsed: SessionLaunchMode = input.parse().expect("launch mode should parse");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), input);
    }
}

#[test]
fn default_shell_program_returns_known_shell() {
    let shell = default_shell_program();
    #[cfg(windows)]
    let shell = std::path::Path::new(shell)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(shell);
    let known_shells = ["zsh", "bash", "sh", "pwsh", "powershell"];
    assert!(
        known_shells.contains(&shell),
        "unexpected default shell: {shell}"
    );
}

#[test]
fn shell_command_args_includes_command_text() {
    let args = shell_command_args("echo hello");
    assert!(
        args.iter().any(|arg| arg == "echo hello"),
        "command text not found in args: {args:?}"
    );
}

#[cfg(not(windows))]
#[test]
fn shell_command_args_unix_uses_login_flag() {
    let args = shell_command_args("ls");
    assert_eq!(args[0], "-lc", "unix should use -lc flag, got: {args:?}");
}

#[cfg(windows)]
#[test]
fn shell_command_args_windows_uses_command_flag() {
    let args = shell_command_args("dir");
    assert!(
        args.contains(&"-Command".to_string()),
        "windows should use -Command flag, got: {args:?}"
    );
    assert!(
        args.contains(&"-NoProfile".to_string()),
        "windows should use -NoProfile, got: {args:?}"
    );
}

#[test]
fn clear_sidecar_removes_existing_file() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test.pid");
    fs::write(&path, "12345")?;
    assert!(path.exists());
    clear_sidecar_if_exists(&path)?;
    assert!(!path.exists());
    Ok(())
}

#[test]
fn clear_sidecar_noop_for_missing_file() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("nonexistent.pid");
    clear_sidecar_if_exists(&path)?;
    Ok(())
}

#[test]
fn prepare_session_dry_run_does_not_create_pid_sidecar() -> Result<()> {
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
        read_only: false,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        timeout_secs: None,
    })?;

    assert_eq!(prepared.session.status, SessionStatus::Prepared);
    let pid = pid_path(&paths.logs_dir.join("sessions"), &prepared.session.id);
    assert!(!pid.exists(), "dry-run should not create pid sidecar");
    Ok(())
}

#[test]
fn prepare_session_command_line_is_populated() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let slot_path = temp_dir.path().join("slot");
    fs::create_dir_all(&slot_path)?;

    let prepared = prepare_session(SessionRunRequest {
        paths: &paths,
        repo_id: "repo-1",
        slot_id: "slot-1",
        slot_path: &slot_path,
        runtime: RuntimeKind::Codex,
        prompt: "implement feature",
        read_only: false,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        timeout_secs: None,
    })?;

    assert!(
        prepared.session.command_line.contains("codex"),
        "command_line should contain runtime program: {}",
        prepared.session.command_line
    );
    assert!(
        prepared.session.command_line.contains("implement feature"),
        "command_line should contain prompt"
    );
    Ok(())
}

#[test]
fn unknown_runtime_kind_parse_fails() {
    let result = "unknown".parse::<RuntimeKind>();
    assert!(
        result.is_err(),
        "expected error parsing unknown RuntimeKind"
    );
}

#[test]
fn unknown_session_status_parse_fails() {
    let result = "bogus".parse::<SessionStatus>();
    assert!(
        result.is_err(),
        "expected error parsing unknown SessionStatus"
    );
}

#[test]
fn unknown_launch_mode_parse_fails() {
    let result = "invalid".parse::<SessionLaunchMode>();
    assert!(
        result.is_err(),
        "expected error parsing unknown SessionLaunchMode"
    );
}

#[test]
fn read_exit_code_returns_none_for_missing_file() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let result = supervisor::read_exit_code(&paths, "sess-nonexistent")?;
    assert_eq!(result, None, "missing exit-code file should return None");
    Ok(())
}

#[test]
fn read_exit_code_returns_none_for_malformed_content() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(exit_path(&sessions_dir, "sess-bad"), "not-a-number\n")?;
    let result = supervisor::read_exit_code(&paths, "sess-bad")?;
    assert_eq!(
        result, None,
        "malformed exit-code content should parse to None"
    );
    Ok(())
}

#[test]
fn read_exit_code_parses_valid_code() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(exit_path(&sessions_dir, "sess-ok"), "42\n")?;
    let result = supervisor::read_exit_code(&paths, "sess-ok")?;
    assert_eq!(result, Some(42), "valid exit code should parse correctly");
    Ok(())
}

#[test]
fn read_pid_returns_none_for_missing_file() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let result = supervisor::read_pid(&paths, "sess-nonexistent")?;
    assert_eq!(result, None, "missing PID file should return None");
    Ok(())
}

#[test]
fn read_pid_returns_none_for_malformed_content() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-bad"), "garbage")?;
    let result = supervisor::read_pid(&paths, "sess-bad")?;
    assert_eq!(result, None, "malformed PID content should parse to None");
    Ok(())
}

#[test]
fn read_pid_parses_valid_pid() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-ok"), "12345\n")?;
    let result = supervisor::read_pid(&paths, "sess-ok")?;
    assert_eq!(result, Some(12345), "valid PID should parse correctly");
    Ok(())
}

#[test]
fn read_exit_code_negative_value() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(exit_path(&sessions_dir, "sess-neg"), "-1\n")?;
    let result = supervisor::read_exit_code(&paths, "sess-neg")?;
    assert_eq!(
        result,
        Some(-1),
        "negative exit code should parse correctly"
    );
    Ok(())
}

#[test]
fn pid_sidecar_exists_false_when_missing() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let paths = sample_paths(tmp.path());
    assert!(
        !supervisor::pid_sidecar_exists(&paths, "sess-missing"),
        "should return false when PID sidecar does not exist"
    );
}

#[test]
fn pid_sidecar_exists_true_when_present() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let paths = sample_paths(tmp.path());
    let sessions_dir = paths.logs_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    fs::write(pid_path(&sessions_dir, "sess-exists"), "999")?;
    assert!(
        supervisor::pid_sidecar_exists(&paths, "sess-exists"),
        "should return true when PID sidecar exists"
    );
    Ok(())
}
