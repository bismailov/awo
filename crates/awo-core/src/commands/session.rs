use super::{CommandOutcome, CommandRunner, SessionStartOptions};
use crate::context::{discover_repo_context, plan_session_context, render_session_context_prompt};
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::runtime::{
    SessionLaunchMode, SessionRunRequest, SessionStatus, cancel_session, detect_runtime,
    detect_tmux, execute_prepared_session, prepare_session,
};
use crate::slot::{FingerprintStatus, SlotStatus};
use std::path::Path;

impl<'a> CommandRunner<'a> {
    pub(super) fn run_session_start(
        &mut self,
        options: SessionStartOptions,
    ) -> AwoResult<CommandOutcome> {
        let SessionStartOptions {
            slot_id,
            runtime,
            prompt,
            read_only,
            dry_run,
            launch_mode,
            attach_context,
            timeout_secs,
        } = options;
        if !dry_run {
            if !detect_runtime(runtime) {
                return Err(AwoError::runtime_launch(format!(
                    "runtime `{}` is not available on PATH; install it or check your PATH configuration",
                    runtime.as_str()
                )));
            }
            if launch_mode == SessionLaunchMode::Pty && !detect_tmux() {
                return Err(AwoError::runtime_launch(
                    "PTY launch is not available on this machine; use `--launch-mode oneshot` or install the configured supervisor backend",
                ));
            }
        }

        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .ok_or_else(|| self.slot_not_found_error(&slot_id))?;
        self.refresh_slot_state(&mut slot)?;
        self.store.upsert_slot(&slot)?;
        if slot.status != SlotStatus::Active {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` is in `{}` state, not `active`; acquire a new slot with `awo slot acquire`",
                slot.status
            )));
        }
        if !read_only && slot.dirty {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` is dirty; commit or stash changes, then run `awo slot refresh {slot_id}`, or use `--read-only`"
            )));
        }
        if !read_only && slot.fingerprint_status == FingerprintStatus::Stale {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` is stale; run `awo slot refresh {slot_id}` to update, or use `--read-only`"
            )));
        }

        let existing_sessions = self.store.list_sessions_for_slot(&slot.id)?;
        if !read_only
            && existing_sessions
                .iter()
                .any(|session| session.blocks_release() && session.is_write_capable())
        {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` already has a pending write-capable session; cancel it with `awo session cancel <session_id>` first, or use `--read-only`"
            )));
        }

        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(slot.repo_id.clone()))?;
        let (effective_prompt, context_files, selected_packs) =
            if attach_context && runtime.uses_agent_prompt() {
                let context = discover_repo_context(Path::new(&repo.repo_root))?;
                let plan = plan_session_context(&context, &prompt);
                let effective = render_session_context_prompt(&context, &plan, &prompt);
                (
                    effective,
                    plan.total_file_count(),
                    plan.selected_pack_names(),
                )
            } else {
                (prompt, 0, Vec::new())
            };

        let prepared = prepare_session(SessionRunRequest {
            paths: &self.config.paths,
            repo_id: &slot.repo_id,
            slot_id: &slot.id,
            slot_path: Path::new(&slot.slot_path),
            runtime,
            prompt: &effective_prompt,
            read_only,
            dry_run,
            launch_mode,
            timeout_secs,
        })?;
        self.store.upsert_session(&prepared.session)?;
        let session_id = prepared.session.id.clone();
        let slot_id_for_error = prepared.session.slot_id.clone();
        let session = match execute_prepared_session(prepared) {
            Ok(execution) => {
                self.store.upsert_session(&execution.session)?;
                execution.session
            }
            Err(error) => {
                let mut failed = self
                    .store
                    .get_session(&session_id)?
                    .ok_or_else(|| self.session_not_found_error(&session_id))?;
                failed.status = SessionStatus::Failed;
                self.store.upsert_session(&failed)?;
                return Err(AwoError::runtime_launch(format!(
                    "session `{session_id}` for slot `{slot_id_for_error}` failed to launch: {error}"
                )));
            }
        };
        self.store.insert_action(
            "session_start",
            &format!(
                "slot_id={} runtime={} status={} launch_mode={} context_files={} packs={}",
                slot.id,
                session.runtime,
                session.status,
                launch_mode.as_str(),
                context_files,
                if selected_packs.is_empty() {
                    "-".to_string()
                } else {
                    selected_packs.join(",")
                }
            ),
        )?;

        let mut events = vec![DomainEvent::CommandReceived {
            command: "session_start".to_string(),
        }];
        if context_files > 0 {
            events.push(DomainEvent::SessionContextPrepared {
                slot_id: slot.id.clone(),
                files: context_files,
                packs: selected_packs.clone(),
            });
        }
        events.push(DomainEvent::SessionStarted {
            session_id: session.id.clone(),
            slot_id: session.slot_id.clone(),
            runtime: session.runtime.clone(),
            supervisor: session.supervisor.clone(),
            status: session.status.as_str().to_string(),
        });

        Ok(CommandOutcome::with_events(
            format!(
                "Session `{}` for slot `{}` is {}.",
                session.id, session.slot_id, session.status
            ),
            events,
        ))
    }

    pub(super) fn run_session_list(
        &mut self,
        repo_id: Option<String>,
    ) -> AwoResult<CommandOutcome> {
        self.sync_runtime_state(repo_id.as_deref())?;
        let sessions = self.store.list_sessions(repo_id.as_deref())?;
        self.store
            .insert_action("session_list", &format!("count={}", sessions.len()))?;
        let events = vec![
            DomainEvent::CommandReceived {
                command: "session_list".to_string(),
            },
            DomainEvent::SessionListLoaded {
                count: sessions.len(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Found {} session(s).", sessions.len()),
            events,
        ))
    }

    pub(super) fn run_session_cancel(&mut self, session_id: String) -> AwoResult<CommandOutcome> {
        let mut session = self
            .store
            .get_session(&session_id)?
            .ok_or_else(|| self.session_not_found_error(&session_id))?;
        if session.is_terminal() {
            return Err(AwoError::invalid_state(format!(
                "session `{session_id}` is already terminal"
            )));
        }
        if session.status == SessionStatus::Running && !session.is_supervised() {
            return Err(AwoError::invalid_state(format!(
                "session `{session_id}` is a running one-shot launch; interruption is not supported yet"
            )));
        }

        cancel_session(&self.config.paths, &mut session)?;
        self.store.upsert_session(&session)?;
        self.store.insert_action(
            "session_cancel",
            &format!(
                "session_id={} slot_id={} status={}",
                session.id, session.slot_id, session.status
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "session_cancel".to_string(),
            },
            DomainEvent::SessionCancelled {
                session_id: session.id.clone(),
                slot_id: session.slot_id.clone(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Cancelled session `{}`.", session_id),
            events,
        ))
    }

    pub(super) fn run_session_log(
        &mut self,
        session_id: String,
        lines: Option<usize>,
        stream: Option<String>,
    ) -> AwoResult<CommandOutcome> {
        let session = self
            .store
            .get_session(&session_id)?
            .ok_or_else(|| self.session_not_found_error(&session_id))?;

        let stream_name = stream.as_deref().unwrap_or("stdout");
        let log_path = match stream_name {
            "stdout" => session.stdout_path.as_deref(),
            "stderr" => session.stderr_path.as_deref(),
            other => {
                return Err(AwoError::validation(format!(
                    "unknown log stream `{other}`; valid streams are `stdout` and `stderr`"
                )));
            }
        };

        let log_path = log_path.ok_or_else(|| {
            AwoError::invalid_state(format!(
                "session `{session_id}` has no {stream_name} log path"
            ))
        })?;

        let path = std::path::Path::new(log_path);
        if !path.exists() {
            return Err(AwoError::invalid_state(format!(
                "log file for session `{session_id}` not found at {log_path}"
            )));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|source| AwoError::io("read session log", path, source))?;

        let max_lines = lines.unwrap_or(50);
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(max_lines);
        let tail_content = all_lines[start..].join("\n");
        let lines_returned = all_lines.len() - start;

        self.store.insert_action(
            "session_log",
            &format!("session_id={session_id} stream={stream_name} lines={lines_returned}"),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "session_log".to_string(),
            },
            DomainEvent::SessionLogLoaded {
                session_id: session_id.clone(),
                stream: stream_name.to_string(),
                lines_returned,
                log_path: log_path.to_string(),
                content: tail_content,
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Loaded {lines_returned} line(s) of {stream_name} for session `{session_id}`."),
            events,
        ))
    }

    pub(super) fn run_session_delete(&mut self, session_id: String) -> AwoResult<CommandOutcome> {
        let session = self
            .store
            .get_session(&session_id)?
            .ok_or_else(|| self.session_not_found_error(&session_id))?;
        if !session.is_terminal() {
            return Err(AwoError::invalid_state(format!(
                "session `{session_id}` is not terminal; cancel it before deleting"
            )));
        }

        self.store.delete_session(&session_id)?;
        self.store
            .insert_action("session_delete", &format!("session_id={session_id}"))?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "session_delete".to_string(),
            },
            DomainEvent::SessionDeleted { session_id },
        ];

        Ok(CommandOutcome::with_events(
            "Deleted session from local state.".to_string(),
            events,
        ))
    }
}
