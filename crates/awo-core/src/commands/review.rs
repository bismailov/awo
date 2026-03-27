use super::{CommandOutcome, CommandRunner};
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::slot::{FingerprintStatus, SlotStatus};
use std::path::Path;
use std::process::Command as ProcessCommand;

impl<'a> CommandRunner<'a> {
    pub(super) fn run_review_status(
        &mut self,
        repo_id: Option<String>,
    ) -> AwoResult<CommandOutcome> {
        self.sync_runtime_state(repo_id.as_deref())?;
        let mut slots = self.store.list_slots(repo_id.as_deref())?;
        let mut dirty = 0usize;
        let mut stale = 0usize;

        for slot in &mut slots {
            if slot.status == SlotStatus::Active || slot.status == SlotStatus::Released {
                self.refresh_slot_state(slot)?;
                self.store.upsert_slot(slot)?;
            }
            if slot.dirty {
                dirty += 1;
            }
            if slot.fingerprint_status == FingerprintStatus::Stale {
                stale += 1;
            }
        }

        self.store
            .insert_action("review_status", &format!("dirty={} stale={}", dirty, stale))?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "review_status".to_string(),
            },
            DomainEvent::ReviewStatusLoaded { dirty, stale },
        ];

        Ok(CommandOutcome::with_events(
            format!("Review status updated: {dirty} dirty, {stale} stale."),
            events,
        ))
    }

    pub(super) fn run_review_diff(&mut self, slot_id: String) -> AwoResult<CommandOutcome> {
        self.sync_runtime_state(None)?;
        let slot = self
            .store
            .get_slot(&slot_id)?
            .ok_or_else(|| AwoError::validation(format!("unknown slot `{slot_id}`")))?;
        let slot_path = Path::new(&slot.slot_path);
        if !slot_path.exists() {
            return Err(AwoError::validation(format!(
                "slot `{slot_id}` worktree is missing at `{}`",
                slot.slot_path
            )));
        }

        let status = run_git_capture(slot_path, ["status", "--short"])?;
        let diff_stat = run_git_capture(slot_path, ["diff", "--stat", "HEAD"])?;
        let patch = run_git_capture(slot_path, ["diff", "--unified=3", "HEAD"])?;
        let changed_files = status
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        let patch_summary = truncate_lines(&patch, 220);
        let content = format!(
            "# Review Diff: {slot_id}\n\n## Slot\n- path: {}\n- branch: {}\n- strategy: {}\n- status: {}\n\n## Git Status\n{}\n\n## Diff Stat\n{}\n\n## Patch{}\n{}",
            slot.slot_path,
            slot.branch_name,
            slot.strategy.as_str(),
            slot.status.as_str(),
            section_or_none(&status),
            section_or_none(&diff_stat),
            if patch_summary.truncated {
                " (truncated to 220 lines)"
            } else {
                ""
            },
            section_or_none(&patch_summary.text),
        );

        Ok(CommandOutcome::with_all(
            format!("Loaded review diff for slot `{slot_id}`."),
            vec![DomainEvent::ReviewDiffLoaded {
                slot_id: slot_id.clone(),
                changed_files,
            }],
            serde_json::json!({
                "slot_id": slot_id,
                "slot_path": slot.slot_path,
                "content": content,
            }),
        ))
    }
}

struct TruncatedText {
    text: String,
    truncated: bool,
}

fn run_git_capture<const N: usize>(cwd: &Path, args: [&str; N]) -> AwoResult<String> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| AwoError::io("run git review command", cwd, err))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(AwoError::validation(format!(
        "git {} failed in {}: {}",
        args.join(" "),
        cwd.display(),
        stderr.trim()
    )))
}

fn truncate_lines(text: &str, max_lines: usize) -> TruncatedText {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return TruncatedText {
            text: text.trim().to_string(),
            truncated: false,
        };
    }

    TruncatedText {
        text: lines[..max_lines].join("\n"),
        truncated: true,
    }
}

fn section_or_none(text: &str) -> &str {
    if text.trim().is_empty() {
        "(clean)"
    } else {
        text
    }
}
