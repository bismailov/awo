use super::{CommandOutcome, CommandRunner, FreshSlotOptions};
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::fingerprint::fingerprint_for_dir;
use crate::git;
use crate::slot::{
    FingerprintStatus, SlotRecord, SlotStatus, SlotStrategy, build_branch_name, build_slot_id,
    build_slot_path,
};
use std::fs;
use std::path::{Path, PathBuf};

impl<'a> CommandRunner<'a> {
    pub(super) fn run_slot_acquire(
        &mut self,
        repo_id: String,
        task_name: String,
        strategy: SlotStrategy,
    ) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;
        let repo_root = PathBuf::from(&repo.repo_root);
        let repo_fingerprint = fingerprint_for_dir(&repo_root)?;

        let slot = if strategy == SlotStrategy::Warm {
            if let Some(mut existing) = self.store.find_reusable_warm_slot(&repo.id)? {
                let slot_path = PathBuf::from(&existing.slot_path);
                if slot_path.exists() {
                    let branch_name = build_branch_name(&task_name, &existing.id);
                    git::reuse_worktree(&slot_path, &branch_name, &repo.default_base_branch)?;
                    existing.task_name = task_name.clone();
                    existing.branch_name = branch_name;
                    existing.base_branch = repo.default_base_branch.clone();
                    existing.status = SlotStatus::Active;
                    existing.dirty = false;
                    existing.fingerprint_status = if repo_fingerprint.hash.is_some() {
                        FingerprintStatus::Ready
                    } else {
                        FingerprintStatus::Missing
                    };
                    existing.fingerprint_hash = repo_fingerprint.hash.clone();
                    existing
                } else {
                    self.create_fresh_slot(FreshSlotOptions {
                        repo_id: &repo.id,
                        repo_root: &repo_root,
                        worktree_root: &repo.worktree_root,
                        base_branch: &repo.default_base_branch,
                        task_name: &task_name,
                        strategy,
                        fingerprint_hash: repo_fingerprint.hash.clone(),
                    })?
                }
            } else {
                self.create_fresh_slot(FreshSlotOptions {
                    repo_id: &repo.id,
                    repo_root: &repo_root,
                    worktree_root: &repo.worktree_root,
                    base_branch: &repo.default_base_branch,
                    task_name: &task_name,
                    strategy,
                    fingerprint_hash: repo_fingerprint.hash.clone(),
                })?
            }
        } else {
            self.create_fresh_slot(FreshSlotOptions {
                repo_id: &repo.id,
                repo_root: &repo_root,
                worktree_root: &repo.worktree_root,
                base_branch: &repo.default_base_branch,
                task_name: &task_name,
                strategy,
                fingerprint_hash: repo_fingerprint.hash.clone(),
            })?
        };

        self.store.upsert_slot(&slot)?;
        self.store.insert_action(
            "slot_acquire",
            &format!(
                "repo_id={} slot_id={} strategy={} path={}",
                repo.id, slot.id, slot.strategy, slot.slot_path
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "slot_acquire".to_string(),
            },
            DomainEvent::SlotAcquired {
                slot_id: slot.id.clone(),
                repo_id: slot.repo_id.clone(),
                branch_name: slot.branch_name.clone(),
                slot_path: slot.slot_path.clone(),
                strategy: slot.strategy.as_str().to_string(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Acquired slot `{}` for task `{}`.", slot.id, slot.task_name),
            events,
        ))
    }

    pub(super) fn create_fresh_slot(&self, options: FreshSlotOptions<'_>) -> AwoResult<SlotRecord> {
        let slot_id = build_slot_id(options.repo_id, options.task_name);
        let branch_name = build_branch_name(options.task_name, &slot_id);
        let slot_path = build_slot_path(
            Path::new(options.worktree_root),
            options.task_name,
            &slot_id,
        );
        let worktree_root = Path::new(options.worktree_root);
        fs::create_dir_all(worktree_root)
            .map_err(|source| AwoError::io("create worktree root", worktree_root, source))?;
        git::create_worktree(
            options.repo_root,
            &slot_path,
            &branch_name,
            options.base_branch,
        )?;

        Ok(SlotRecord {
            id: slot_id,
            repo_id: options.repo_id.to_string(),
            task_name: options.task_name.to_string(),
            slot_path: slot_path.display().to_string(),
            branch_name,
            base_branch: options.base_branch.to_string(),
            strategy: options.strategy,
            status: SlotStatus::Active,
            fingerprint_status: if options.fingerprint_hash.is_some() {
                FingerprintStatus::Ready
            } else {
                FingerprintStatus::Missing
            },
            fingerprint_hash: options.fingerprint_hash,
            dirty: false,
            created_at: String::new(),
            updated_at: String::new(),
        })
    }

    pub(super) fn run_slot_list(&mut self, repo_id: Option<String>) -> AwoResult<CommandOutcome> {
        self.sync_runtime_state(repo_id.as_deref())?;
        let slots = self.store.list_slots(repo_id.as_deref())?;
        self.store
            .insert_action("slot_list", &format!("count={}", slots.len()))?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "slot_list".to_string(),
            },
            DomainEvent::SlotListLoaded { count: slots.len() },
        ];

        Ok(CommandOutcome::with_events(
            format!("Found {} slot(s).", slots.len()),
            events,
        ))
    }

    pub(super) fn run_slot_release(&mut self, slot_id: String) -> AwoResult<CommandOutcome> {
        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .ok_or_else(|| self.slot_not_found_error(&slot_id))?;
        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&slot.repo_id))?;
        let slot_path = PathBuf::from(&slot.slot_path);
        self.sync_runtime_state(Some(&slot.repo_id))?;
        self.refresh_slot_state(&mut slot)?;

        let sessions = self.store.list_sessions_for_slot(&slot.id)?;
        if sessions.iter().any(|session| session.blocks_release()) {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` still has pending session(s); cancel them with `awo session cancel <session_id>` before releasing"
            )));
        }

        let clean = if slot_path.exists() {
            git::is_clean(&slot_path)?
        } else {
            true
        };
        if !clean {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` has uncommitted changes; commit or stash them, then run `awo slot refresh {slot_id}` before releasing"
            )));
        }

        if slot.strategy == SlotStrategy::Warm {
            git::detach_worktree(&slot_path, &slot.base_branch)?;
            slot.status = SlotStatus::Released;
        } else {
            git::remove_worktree(Path::new(&repo.repo_root), &slot_path)?;
            slot.status = SlotStatus::Released;
            slot.fingerprint_status = FingerprintStatus::Missing;
            slot.fingerprint_hash = None;
        }
        slot.dirty = false;
        self.store.upsert_slot(&slot)?;
        self.store.insert_action(
            "slot_release",
            &format!("slot_id={} strategy={}", slot.id, slot.strategy),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "slot_release".to_string(),
            },
            DomainEvent::SlotReleased {
                slot_id: slot.id.clone(),
                strategy: slot.strategy.as_str().to_string(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!("Released slot `{}`.", slot_id),
            events,
        ))
    }

    pub(super) fn run_slot_refresh(&mut self, slot_id: String) -> AwoResult<CommandOutcome> {
        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .ok_or_else(|| self.slot_not_found_error(&slot_id))?;
        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&slot.repo_id))?;
        let slot_path = PathBuf::from(&slot.slot_path);
        let mut resynced = false;

        if slot.strategy == SlotStrategy::Warm
            && slot.status == SlotStatus::Released
            && slot_path.exists()
        {
            if !git::is_clean(Path::new(&repo.repo_root))? {
                return Err(AwoError::invalid_state(format!(
                    "repo `{}` has uncommitted changes; run `git -C {} stash` or commit them before refreshing released warm slots",
                    repo.id, repo.repo_root
                )));
            }
            if !git::is_clean(&slot_path)? {
                return Err(AwoError::invalid_state(format!(
                    "slot `{slot_id}` has uncommitted changes; commit or stash them in `{}` before refreshing",
                    slot.slot_path
                )));
            }
            git::detach_worktree(&slot_path, &slot.base_branch)?;
            resynced = true;
        }

        self.refresh_slot_state(&mut slot)?;
        self.store.upsert_slot(&slot)?;
        self.store.insert_action(
            "slot_refresh",
            &format!(
                "slot_id={} dirty={} fingerprint_status={} resynced={}",
                slot.id, slot.dirty, slot.fingerprint_status, resynced
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "slot_refresh".to_string(),
            },
            DomainEvent::SlotRefreshed {
                slot_id: slot.id.clone(),
                dirty: slot.dirty,
                fingerprint_status: slot.fingerprint_status.as_str().to_string(),
            },
        ];

        Ok(CommandOutcome::with_events(
            if resynced && slot.fingerprint_status == FingerprintStatus::Stale {
                format!(
                    "Refreshed slot `{}` but it remains stale relative to the repo fingerprint.",
                    slot.id
                )
            } else if resynced {
                format!("Refreshed and resynced slot `{}`.", slot.id)
            } else {
                format!("Refreshed slot `{}`.", slot.id)
            },
            events,
        ))
    }

    pub(super) fn refresh_slot_state(&self, slot: &mut SlotRecord) -> AwoResult<()> {
        let repo = match self.store.get_repository(&slot.repo_id)? {
            Some(repo) => repo,
            None => {
                // Orphan slot — its repo was removed. Mark as missing.
                slot.status = SlotStatus::Missing;
                slot.dirty = false;
                slot.fingerprint_hash = None;
                slot.fingerprint_status = FingerprintStatus::Missing;
                return Ok(());
            }
        };

        let slot_path = Path::new(&slot.slot_path);
        if !slot_path.exists() {
            if slot.status == SlotStatus::Released && slot.strategy == SlotStrategy::Fresh {
                slot.fingerprint_status = FingerprintStatus::Missing;
            } else {
                slot.status = SlotStatus::Missing;
                slot.fingerprint_status = FingerprintStatus::Missing;
            }
            slot.dirty = false;
            slot.fingerprint_hash = None;
            return Ok(());
        }

        let repo_fingerprint = fingerprint_for_dir(Path::new(&repo.repo_root))?;
        let slot_fingerprint = fingerprint_for_dir(slot_path)?;
        slot.dirty = !git::is_clean(slot_path)?;
        slot.fingerprint_hash = slot_fingerprint.hash.clone();
        slot.fingerprint_status = if slot_fingerprint.hash == repo_fingerprint.hash {
            FingerprintStatus::Ready
        } else {
            FingerprintStatus::Stale
        };

        Ok(())
    }
}
