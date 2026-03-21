use super::{CommandOutcome, CommandRunner, FreshSlotOptions};
use crate::events::DomainEvent;
use crate::fingerprint::fingerprint_for_dir;
use crate::git;
use crate::slot::{SlotRecord, SlotStrategy, build_branch_name, build_slot_id, build_slot_path};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

impl<'a> CommandRunner<'a> {
    pub(super) fn run_slot_acquire(
        &mut self,
        repo_id: String,
        task_name: String,
        strategy: SlotStrategy,
    ) -> Result<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .with_context(|| format!("unknown repo id `{repo_id}`"))?;
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
                    existing.status = "active".to_string();
                    existing.dirty = false;
                    existing.fingerprint_hash = repo_fingerprint.hash.clone();
                    existing.fingerprint_status = "ready".to_string();
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
                strategy: slot.strategy.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Acquired slot `{}` for task `{}`.", slot.id, slot.task_name),
            events,
        })
    }

    pub(super) fn create_fresh_slot(&self, options: FreshSlotOptions<'_>) -> Result<SlotRecord> {
        let slot_id = build_slot_id(options.repo_id, options.task_name);
        let branch_name = build_branch_name(options.task_name, &slot_id);
        let slot_path = build_slot_path(
            Path::new(options.worktree_root),
            options.task_name,
            &slot_id,
        );
        fs::create_dir_all(Path::new(options.worktree_root)).with_context(|| {
            format!(
                "failed to create worktree root at {}",
                options.worktree_root
            )
        })?;
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
            strategy: options.strategy.as_str().to_string(),
            status: "active".to_string(),
            fingerprint_hash: options.fingerprint_hash,
            fingerprint_status: "ready".to_string(),
            dirty: false,
            created_at: String::new(),
            updated_at: String::new(),
        })
    }

    pub(super) fn run_slot_list(&mut self, repo_id: Option<String>) -> Result<CommandOutcome> {
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

        Ok(CommandOutcome {
            summary: format!("Loaded {} slot(s).", slots.len()),
            events,
        })
    }

    pub(super) fn run_slot_release(&mut self, slot_id: String) -> Result<CommandOutcome> {
        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .with_context(|| format!("unknown slot id `{slot_id}`"))?;
        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .with_context(|| format!("missing repo for slot `{slot_id}`"))?;
        let slot_path = PathBuf::from(&slot.slot_path);
        self.sync_runtime_state(Some(&slot.repo_id))?;
        self.refresh_slot_state(&mut slot)?;

        let sessions = self.store.list_sessions_for_slot(&slot.id)?;
        if sessions.iter().any(|session| session.blocks_release()) {
            bail!("slot `{slot_id}` still has pending session(s); refusing to release");
        }

        let clean = if slot_path.exists() {
            git::is_clean(&slot_path)?
        } else {
            true
        };
        if !clean {
            bail!("slot `{slot_id}` is dirty; refusing to release");
        }

        if slot.strategy == SlotStrategy::Warm.as_str() {
            git::detach_worktree(&slot_path, &slot.base_branch)?;
            slot.status = "released".to_string();
        } else {
            git::remove_worktree(Path::new(&repo.repo_root), &slot_path)?;
            slot.status = "released".to_string();
            slot.fingerprint_status = "released".to_string();
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
                strategy: slot.strategy.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Released slot `{}`.", slot.id),
            events,
        })
    }

    pub(super) fn run_slot_refresh(&mut self, slot_id: String) -> Result<CommandOutcome> {
        let mut slot = self
            .store
            .get_slot(&slot_id)?
            .with_context(|| format!("unknown slot id `{slot_id}`"))?;
        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .with_context(|| format!("missing repo for slot `{slot_id}`"))?;
        let slot_path = PathBuf::from(&slot.slot_path);
        let mut resynced = false;

        if slot.strategy == SlotStrategy::Warm.as_str()
            && slot.status == "released"
            && slot_path.exists()
        {
            if !git::is_clean(Path::new(&repo.repo_root))? {
                bail!(
                    "repo `{}` has uncommitted changes; commit or stash them before refreshing released warm slots",
                    repo.id
                );
            }
            if !git::is_clean(&slot_path)? {
                bail!("slot `{slot_id}` is dirty; refusing to refresh");
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
                fingerprint_status: slot.fingerprint_status.clone(),
            },
        ];

        Ok(CommandOutcome {
            summary: if resynced && slot.fingerprint_status == "stale" {
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
        })
    }

    pub(super) fn refresh_slot_state(&self, slot: &mut SlotRecord) -> Result<()> {
        let repo = self
            .store
            .get_repository(&slot.repo_id)?
            .with_context(|| format!("missing repo for slot `{}`", slot.id))?;

        let slot_path = Path::new(&slot.slot_path);
        if !slot_path.exists() {
            if slot.status == "released" && slot.strategy == SlotStrategy::Fresh.as_str() {
                slot.fingerprint_status = "released".to_string();
            } else {
                slot.status = "missing".to_string();
                slot.fingerprint_status = "missing".to_string();
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
            "ready".to_string()
        } else {
            "stale".to_string()
        };

        Ok(())
    }
}
