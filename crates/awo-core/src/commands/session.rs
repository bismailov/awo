use super::{CommandOutcome, CommandRunner, SessionStartOptions};
use crate::context::{discover_repo_context, plan_session_context, render_session_context_prompt};
use crate::events::DomainEvent;
use crate::runtime::{
    SessionLaunchMode, SessionRunRequest, cancel_session, detect_runtime, detect_tmux,
    execute_prepared_session, prepare_session,
};
use anyhow::{Context, Result, bail};
use std::path::Path;

impl<'a> CommandRunner<'a> {
    pub(super) fn run_session_start(
        &mut self,
        options: SessionStartOptions,
    ) -> Result<CommandOutcome> {
        let SessionStartOptions {
            slot_id,
            runtime,
            prompt,
            read_only,
            dry_run,
            launch_mode,
            attach_context,
        } = options;
        if !detect_runtime(runtime) {
            bail!("runtime `{}` is not available on PATH", runtime.as_str());
        }
        if launch_mode == SessionLaunchMode::Pty && !detect_tmux() {
            bail!(
                "PTY launch is not available on this machine; use `--launch-mode oneshot` or install the configured supervisor backend"
            );
        }

        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .with_context(|| format!("unknown slot id `{slot_id}`"))?;
        self.refresh_slot_state(&mut slot)?;
        self.store.upsert_slot(&slot)?;
        if slot.status != "active" {
            bail!("slot `{slot_id}` is not active");
        }
        if !read_only && slot.dirty {
            bail!("slot `{slot_id}` is dirty; refusing to start another write-capable session");
        }
        if !read_only && slot.fingerprint_status == "stale" {
            bail!(
                "slot `{slot_id}` is stale; refresh or reacquire it before launching a write-capable session"
            );
        }

        let existing_sessions = self.store.list_sessions_for_slot(&slot.id)?;
        if !read_only
            && existing_sessions
                .iter()
                .any(|session| session.blocks_release() && session.is_write_capable())
        {
            bail!("slot `{slot_id}` already has a pending write-capable session");
        }

        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .with_context(|| format!("missing repo for slot `{slot_id}`"))?;
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
                    .with_context(|| format!("failed to reload session `{session_id}`"))?;
                failed.status = "failed".to_string();
                self.store.upsert_session(&failed)?;
                return Err(error).with_context(|| {
                    format!(
                        "session `{session_id}` for slot `{slot_id_for_error}` failed to launch"
                    )
                });
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
            status: session.status.clone(),
        });

        Ok(CommandOutcome {
            summary: format!(
                "Session `{}` for slot `{}` is {}.",
                session.id, session.slot_id, session.status
            ),
            events,
        })
    }

    pub(super) fn run_session_list(&mut self, repo_id: Option<String>) -> Result<CommandOutcome> {
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

        Ok(CommandOutcome {
            summary: format!("Loaded {} session(s).", sessions.len()),
            events,
        })
    }

    pub(super) fn run_session_cancel(&mut self, session_id: String) -> Result<CommandOutcome> {
        let mut session = self
            .store
            .get_session(&session_id)?
            .with_context(|| format!("unknown session id `{session_id}`"))?;
        if session.is_terminal() {
            bail!("session `{session_id}` is already terminal");
        }
        if session.status == "running" && !session.is_supervised() {
            bail!(
                "session `{session_id}` is a running one-shot launch; interruption is not supported yet"
            );
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

        Ok(CommandOutcome {
            summary: format!("Cancelled session `{}`.", session.id),
            events,
        })
    }

    pub(super) fn run_session_delete(&mut self, session_id: String) -> Result<CommandOutcome> {
        let session = self
            .store
            .get_session(&session_id)?
            .with_context(|| format!("unknown session id `{session_id}`"))?;
        if !session.is_terminal() {
            bail!("session `{session_id}` is not terminal; cancel it before deleting");
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

        Ok(CommandOutcome {
            summary: "Deleted session from local state.".to_string(),
            events,
        })
    }
}
