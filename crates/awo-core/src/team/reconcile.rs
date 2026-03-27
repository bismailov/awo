use crate::error::AwoResult;
use crate::runtime::{SessionEndReason, SessionStatus};
use crate::slot::SlotStatus;
use crate::store::Store;
use crate::team::{TaskCardState, TeamManifest, TeamMember, TeamStatus, TeamTeardownPlan};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn collect_bound_slot_ids(manifest: &TeamManifest) -> Vec<String> {
    std::iter::once(manifest.lead.slot_id.as_deref())
        .chain(
            manifest
                .members
                .iter()
                .map(|member| member.slot_id.as_deref()),
        )
        .chain(manifest.tasks.iter().map(|task| task.slot_id.as_deref()))
        .flatten()
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn build_team_teardown_plan(
    store: &Store,
    manifest: &TeamManifest,
) -> AwoResult<TeamTeardownPlan> {
    let bound_slots = collect_bound_slot_ids(manifest);
    let mut active_slots = BTreeSet::new();
    let mut dirty_slots = BTreeSet::new();
    let mut cancellable_sessions = BTreeSet::new();
    let mut blocking_sessions = BTreeSet::new();

    for slot_id in &bound_slots {
        if let Some(slot) = store.get_slot(slot_id)? {
            if slot.status != SlotStatus::Released {
                active_slots.insert(slot_id.clone());
            }
            if slot.dirty {
                dirty_slots.insert(slot_id.clone());
            }
        }

        for session in store.list_sessions_for_slot(slot_id)? {
            if session.is_terminal() {
                continue;
            }

            if session.status == SessionStatus::Running && !session.is_supervised() {
                blocking_sessions.insert(format!(
                    "session `{}` on slot `{}` is a running one-shot launch and cannot be interrupted yet",
                    session.id, slot_id
                ));
            } else {
                cancellable_sessions.insert(session.id);
            }
        }
    }

    Ok(TeamTeardownPlan {
        reset_summary: manifest.reset_summary(),
        bound_slots,
        active_slots: active_slots.into_iter().collect(),
        dirty_slots: dirty_slots.into_iter().collect(),
        cancellable_sessions: cancellable_sessions.into_iter().collect(),
        blocking_sessions: blocking_sessions.into_iter().collect(),
    })
}

pub fn reconcile_team_manifest_state(
    store: &Store,
    manifest: &mut TeamManifest,
) -> AwoResult<bool> {
    if manifest.status == TeamStatus::Archived {
        return Ok(false);
    }

    let mut changed = false;

    for task in &mut manifest.tasks {
        if task.slot_id.is_none() {
            if task.branch_name.take().is_some() {
                changed = true;
            }
            continue;
        }

        let slot_id = task.slot_id.clone().unwrap_or_default();
        let slot = store.get_slot(&slot_id)?;
        let sessions = store.list_sessions_for_slot(&slot_id)?;
        let has_active_session = sessions
            .iter()
            .any(|session| session.status == SessionStatus::Running);
        let has_non_terminal_session = sessions.iter().any(|session| !session.is_terminal());
        let slot_missing_or_released = slot
            .as_ref()
            .is_none_or(|slot| slot.status == SlotStatus::Released);

        if has_active_session {
            if task.state != TaskCardState::InProgress {
                task.state = TaskCardState::InProgress;
                changed = true;
            }
            continue;
        }

        if let Some(session) = sessions.iter().find(|session| session.is_terminal()) {
            match session.status {
                SessionStatus::Completed => {
                    if !matches!(task.state, TaskCardState::Done | TaskCardState::Review) {
                        task.state = TaskCardState::Review;
                        changed = true;
                    }
                    task.result_session_id = Some(session.id.clone());
                    task.output_log_path = session.stdout_path.clone();
                    task.handoff_note = extract_handoff_note(session);
                    task.result_summary =
                        Some("Session completed successfully. Ready for review.".to_string());

                    if let Some(cmd) = &task.verification_command
                        && let Some(ref s) = slot
                    {
                        let path = Path::new(&s.slot_path);
                        match Command::new("sh")
                            .arg("-c")
                            .arg(cmd)
                            .current_dir(path)
                            .output()
                        {
                            Ok(output) if output.status.success() => {
                                task.result_summary =
                                    Some("Verification passed. Ready for review.".to_string());
                            }
                            Ok(output) => {
                                task.state = TaskCardState::Blocked;
                                changed = true;
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                task.result_summary =
                                    Some(format!("Verification failed: {}", stderr.trim()));
                            }
                            Err(e) => {
                                task.state = TaskCardState::Blocked;
                                changed = true;
                                task.result_summary =
                                    Some(format!("Verification failed to run: {}", e));
                            }
                        }
                    }
                }
                SessionStatus::Failed | SessionStatus::Cancelled => {
                    if task.state != TaskCardState::Blocked {
                        task.state = TaskCardState::Blocked;
                        changed = true;
                    }
                    task.result_session_id = Some(session.id.clone());
                    task.output_log_path = session.stdout_path.clone();
                    task.handoff_note = extract_handoff_note(session);
                    task.result_summary = Some(describe_terminal_failure(session));
                }
                _ => {}
            }
        } else if task.state == TaskCardState::InProgress && slot_missing_or_released {
            task.state = TaskCardState::Blocked;
            changed = true;
        }

        if slot_missing_or_released && !has_non_terminal_session {
            if task.slot_id.take().is_some() {
                changed = true;
            }
            if task.branch_name.take().is_some() {
                changed = true;
            }
        }
    }

    let task_bound_slot_ids = manifest
        .tasks
        .iter()
        .filter_map(|task| task.slot_id.as_deref())
        .collect::<BTreeSet<_>>();

    if should_clear_member_slot_binding(store, &task_bound_slot_ids, &manifest.lead)? {
        manifest.lead.slot_id = None;
        manifest.lead.branch_name = None;
        changed = true;
    }

    for member in &mut manifest.members {
        if should_clear_member_slot_binding(store, &task_bound_slot_ids, member)? {
            member.slot_id = None;
            member.branch_name = None;
            changed = true;
        }
    }

    if let Some(session_id) = manifest.current_lead_session_id().map(str::to_string) {
        let clear = match store.get_session(&session_id)? {
            Some(session) => session.is_terminal(),
            None => true,
        };
        if clear {
            manifest.clear_current_lead_session();
            changed = true;
        }
    }

    if changed {
        manifest.refresh_status();
        manifest.validate()?;
    }

    Ok(changed)
}

fn describe_terminal_failure(session: &crate::runtime::SessionRecord) -> String {
    match session.end_reason {
        Some(SessionEndReason::Timeout) => match session.timeout_secs {
            Some(timeout_secs) => format!("Session timed out after {timeout_secs}s."),
            None => "Session timed out.".to_string(),
        },
        Some(SessionEndReason::TokenExhausted) => {
            "Session appears to have run out of tokens or context budget.".to_string()
        }
        Some(SessionEndReason::OperatorCancelled) | Some(SessionEndReason::Completed) => {
            "Session was cancelled by the operator.".to_string()
        }
        Some(SessionEndReason::RuntimeFailure) | None => format!(
            "Session failed: exit={}",
            session
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
    }
}

fn extract_handoff_note(session: &crate::runtime::SessionRecord) -> Option<String> {
    [
        session.stdout_path.as_deref(),
        session.stderr_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(read_last_meaningful_line)
    .find(|line| !line.is_empty())
}

fn read_last_meaningful_line(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let line = content
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty() && *line != "```")?;
    let truncated = if line.chars().count() > 240 {
        let prefix = line.chars().take(237).collect::<String>();
        format!("{prefix}...")
    } else {
        line.to_string()
    };
    Some(truncated)
}

fn should_clear_member_slot_binding(
    store: &Store,
    task_bound_slot_ids: &BTreeSet<&str>,
    member: &TeamMember,
) -> AwoResult<bool> {
    let Some(slot_id) = member.slot_id.as_deref() else {
        return Ok(member.branch_name.is_some());
    };

    if !task_bound_slot_ids.contains(slot_id) {
        return Ok(true);
    }

    let has_running_session = store
        .list_sessions_for_slot(slot_id)?
        .iter()
        .any(|session| !session.is_terminal());
    if has_running_session {
        return Ok(false);
    }

    Ok(store
        .get_slot(slot_id)?
        .is_none_or(|slot| slot.status == SlotStatus::Released))
}
