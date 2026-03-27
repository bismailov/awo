use crate::commands::{Command, CommandOutcome};
use crate::error::{AwoError, AwoResult};
use crate::runtime::{RuntimeKind, SessionLaunchMode, SessionStatus};
use crate::slot::SlotStatus;
use crate::team::{
    DelegationContext, TaskCard, TaskCardState, TeamExecutionMode, TeamManifest, TeamManifestGuard,
    TeamMember, TeamResetSummary, TeamTaskDelegateOptions, TeamTaskExecution, TeamTaskStartOptions,
    TeamTeardownPlan, TeamTeardownResult, build_team_teardown_plan, collect_bound_slot_ids,
    list_team_manifest_paths, reconcile_team_manifest_state, remove_team_manifest,
    save_team_manifest,
};
use std::str::FromStr;
use tracing::warn;

use super::AppCore;

impl AppCore {
    pub fn save_team_manifest(&self, manifest: &TeamManifest) -> AwoResult<std::path::PathBuf> {
        save_team_manifest(&self.config.paths, manifest)
    }

    pub fn load_team_manifest(&self, team_id: &str) -> AwoResult<TeamManifest> {
        self.sync_runtime_state(None)?;
        self.reconcile_team_manifest(team_id)
    }

    pub fn list_team_manifests(&self) -> AwoResult<Vec<TeamManifest>> {
        self.sync_runtime_state(None)?;
        self.reconcile_all_team_manifests()
    }

    pub fn add_team_member(&self, team_id: &str, member: TeamMember) -> AwoResult<TeamManifest> {
        let member_id = member.member_id.clone();
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().add_member(member)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_member_add",
            &format!("team_id={} member_id={}", manifest.team_id, member_id),
        )?;
        Ok(manifest)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_team_member_policy(
        &self,
        team_id: &str,
        member_id: &str,
        runtime: Option<Option<String>>,
        model: Option<Option<String>>,
        fallback_runtime: Option<Option<String>>,
        fallback_model: Option<Option<String>>,
        routing_preferences: Option<Option<crate::routing::RoutingPreferences>>,
    ) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().update_member_policy(
            member_id,
            runtime,
            model,
            fallback_runtime,
            fallback_model,
            routing_preferences,
        )?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_member_update",
            &format!("team_id={} member_id={member_id}", manifest.team_id),
        )?;
        Ok(manifest)
    }

    pub fn remove_team_member(&self, team_id: &str, member_id: &str) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().remove_member(member_id)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_member_remove",
            &format!("team_id={} member_id={member_id}", manifest.team_id),
        )?;
        Ok(manifest)
    }

    pub fn replace_team_lead(&self, team_id: &str, member_id: &str) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().promote_current_lead(member_id)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_lead_replace",
            &format!("team_id={} member_id={member_id}", manifest.team_id),
        )?;
        Ok(manifest)
    }

    pub fn add_team_task(&self, team_id: &str, task: TaskCard) -> AwoResult<TeamManifest> {
        let task_id = task.task_id.clone();
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().add_task(task)?;
        manifest.manifest_mut().refresh_status();
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_task_add",
            &format!("team_id={} task_id={task_id}", manifest.team_id),
        )?;
        Ok(manifest)
    }

    pub fn set_team_task_state(
        &self,
        team_id: &str,
        task_id: &str,
        state: TaskCardState,
    ) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        manifest.manifest_mut().set_task_state(task_id, state)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_task_state",
            &format!(
                "team_id={} task_id={} state={}",
                manifest.team_id, task_id, state
            ),
        )?;
        Ok(manifest)
    }

    pub fn assign_team_member_slot(
        &self,
        team_id: &str,
        member_id: &str,
        slot_id: &str,
    ) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        let slot = self
            .store
            .get_slot(slot_id)?
            .ok_or_else(|| AwoError::unknown_slot(slot_id))?;
        if slot.repo_id != manifest.manifest().repo_id {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` belongs to repo `{}`, not team repo `{}`",
                slot.repo_id,
                manifest.manifest().repo_id
            )));
        }
        manifest
            .manifest_mut()
            .assign_member_slot(member_id, &slot.id, &slot.branch_name)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_member_assign_slot",
            &format!(
                "team_id={} member_id={} slot_id={}",
                manifest.team_id, member_id, slot.id
            ),
        )?;
        Ok(manifest)
    }

    pub fn bind_team_task_slot(
        &self,
        team_id: &str,
        task_id: &str,
        slot_id: &str,
    ) -> AwoResult<TeamManifest> {
        let mut manifest = TeamManifestGuard::load(&self.config.paths, team_id)?;
        let slot = self
            .store
            .get_slot(slot_id)?
            .ok_or_else(|| AwoError::unknown_slot(slot_id))?;
        if slot.repo_id != manifest.manifest().repo_id {
            return Err(AwoError::invalid_state(format!(
                "slot `{slot_id}` belongs to repo `{}`, not team repo `{}`",
                slot.repo_id,
                manifest.manifest().repo_id
            )));
        }
        manifest
            .manifest_mut()
            .bind_task_slot(task_id, &slot.id, &slot.branch_name)?;
        manifest.save()?;
        let manifest = manifest.into_manifest();
        self.store.insert_action(
            "team_task_bind_slot",
            &format!(
                "team_id={} task_id={} slot_id={}",
                manifest.team_id, task_id, slot.id
            ),
        )?;
        Ok(manifest)
    }

    pub fn archive_team(&self, team_id: &str) -> AwoResult<TeamManifest> {
        self.sync_runtime_state(None)?;
        let mut guard = TeamManifestGuard::load(&self.config.paths, team_id)?;
        let changed = reconcile_team_manifest_state(&self.store, guard.manifest_mut())?;
        if changed {
            guard.save()?;
        }
        let mut blockers = guard.manifest().archive_blockers();
        let bound_slot_ids = collect_bound_slot_ids(guard.manifest());
        for slot_id in bound_slot_ids {
            if let Some(slot) = self.store.get_slot(&slot_id)?
                && slot.status != SlotStatus::Released
            {
                blockers.push(format!(
                    "slot `{slot_id}` is still active with status `{}`",
                    slot.status
                ));
            }

            for session in self.store.list_sessions_for_slot(&slot_id)? {
                if !session.is_terminal() {
                    blockers.push(format!(
                        "session `{}` for slot `{slot_id}` is still `{}`",
                        session.id, session.status
                    ));
                }
            }
        }
        if !blockers.is_empty() {
            return Err(AwoError::invalid_state(format!(
                "cannot archive team `{team_id}`: {}",
                blockers.join("; ")
            )));
        }
        guard.manifest_mut().archive()?;
        guard.save()?;
        let manifest = guard.into_manifest();
        self.store
            .insert_action("team_archive", &format!("team_id={}", manifest.team_id))?;
        Ok(manifest)
    }

    pub fn reset_team(&self, team_id: &str) -> AwoResult<(TeamManifest, TeamResetSummary)> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, team_id)?;
        let summary = guard.manifest().reset_summary();
        guard.manifest_mut().reset();
        guard.save()?;
        let manifest = guard.into_manifest();
        self.store
            .insert_action("team_reset", &format!("team_id={}", manifest.team_id))?;
        Ok((manifest, summary))
    }

    pub fn plan_team_teardown(&self, team_id: &str) -> AwoResult<TeamTeardownPlan> {
        self.sync_runtime_state(None)?;
        let manifest = self.reconcile_team_manifest(team_id)?;
        build_team_teardown_plan(&self.store, &manifest)
    }

    pub fn teardown_team(
        &mut self,
        team_id: &str,
    ) -> AwoResult<(TeamManifest, TeamTeardownResult)> {
        let plan = self.plan_team_teardown(team_id)?;
        if plan.has_blockers() {
            let mut blockers = Vec::new();
            if !plan.dirty_slots.is_empty() {
                blockers.push(format!("dirty slots: {}", plan.dirty_slots.join(", ")));
            }
            blockers.extend(plan.blocking_sessions.iter().cloned());
            return Err(AwoError::invalid_state(format!(
                "cannot teardown team `{team_id}`: {}",
                blockers.join("; ")
            )));
        }

        let mut cancelled_sessions = Vec::new();
        for session_id in &plan.cancellable_sessions {
            self.dispatch(Command::SessionCancel {
                session_id: session_id.clone(),
            })?;
            cancelled_sessions.push(session_id.clone());
        }

        self.sync_runtime_state(None)?;
        let manifest = self.reconcile_team_manifest(team_id)?;
        let mut released_slots = Vec::new();
        for slot_id in collect_bound_slot_ids(&manifest) {
            if let Some(slot) = self.store.get_slot(&slot_id)?
                && slot.status != SlotStatus::Released
            {
                self.dispatch(Command::SlotRelease {
                    slot_id: slot_id.clone(),
                })?;
                released_slots.push(slot_id);
            }
        }

        let (manifest, reset_summary) = self.reset_team(team_id)?;
        self.store.insert_action(
            "team_teardown",
            &format!(
                "team_id={} cancelled_sessions={} released_slots={}",
                manifest.team_id,
                cancelled_sessions.len(),
                released_slots.len()
            ),
        )?;

        Ok((
            manifest,
            TeamTeardownResult {
                cancelled_sessions,
                released_slots,
                reset_summary,
            },
        ))
    }

    pub fn delete_team(&self, team_id: &str) -> AwoResult<()> {
        let plan = self.plan_team_teardown(team_id)?;
        if !plan.bound_slots.is_empty() {
            return Err(AwoError::invalid_state(format!(
                "cannot delete team `{team_id}` while slot bindings remain (run `team teardown` first): {}",
                plan.bound_slots.join(", ")
            )));
        }
        if !plan.blocking_sessions.is_empty() || !plan.cancellable_sessions.is_empty() {
            return Err(AwoError::invalid_state(format!(
                "cannot delete team `{team_id}` while sessions remain attached (run `team teardown` first)"
            )));
        }

        self.remove_team(team_id)?;
        self.store
            .insert_action("team_delete", &format!("team_id={team_id}"))?;
        Ok(())
    }

    pub fn remove_team(&self, team_id: &str) -> AwoResult<()> {
        remove_team_manifest(&self.config.paths, team_id)
    }

    pub fn start_team_task(
        &mut self,
        options: TeamTaskStartOptions,
    ) -> AwoResult<(
        TeamManifest,
        Option<CommandOutcome>,
        CommandOutcome,
        TeamTaskExecution,
    )> {
        self.execute_team_task(
            options.team_id,
            options.task_id,
            options.strategy,
            options.dry_run,
            options.launch_mode,
            options.attach_context,
            options.routing_preferences,
            None,
        )
    }

    pub fn delegate_team_task(
        &mut self,
        options: TeamTaskDelegateOptions,
    ) -> AwoResult<(
        TeamManifest,
        Option<CommandOutcome>,
        CommandOutcome,
        TeamTaskExecution,
    )> {
        self.execute_team_task(
            options.team_id,
            options.task_id,
            options.strategy,
            options.dry_run,
            options.launch_mode,
            options.attach_context,
            None,
            Some(options.delegation),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_team_task(
        &mut self,
        team_id: String,
        task_id: String,
        strategy_name: String,
        dry_run: bool,
        launch_mode_name: String,
        attach_context: bool,
        routing_preferences: Option<crate::routing::RoutingPreferences>,
        delegation: Option<DelegationContext>,
    ) -> AwoResult<(
        TeamManifest,
        Option<CommandOutcome>,
        CommandOutcome,
        TeamTaskExecution,
    )> {
        let launch_mode: SessionLaunchMode = launch_mode_name
            .parse()
            .map_err(|_| AwoError::unsupported("launch mode", &launch_mode_name))?;
        let strategy: crate::slot::SlotStrategy = strategy_name
            .parse()
            .map_err(|_| AwoError::unsupported("slot strategy", &strategy_name))?;

        let recover_failed_start = |core: &mut Self, slot_id: Option<&str>| {
            if let Err(err) = core.set_team_task_state(&team_id, &task_id, TaskCardState::Blocked) {
                warn!(team_id = team_id.as_str(), task_id = task_id.as_str(), %err, "failed to mark task as blocked during recovery");
            }
            if let Some(slot_id) = slot_id
                && let Err(err) = core.dispatch(Command::SlotRelease {
                    slot_id: slot_id.to_string(),
                })
            {
                warn!(slot_id, %err, "failed to release slot during recovery");
            }
        };

        let (
            repo_id,
            owner_id,
            runtime_name,
            runtime,
            selected_model,
            routing_source,
            routing_reason,
            prompt,
            read_only,
            auto_start,
            task_slot_id,
            owner_slot_id,
        ) = {
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &team_id)?;

            if let Some(ref delegation) = delegation {
                if !manifest
                    .manifest()
                    .members
                    .iter()
                    .any(|m| m.member_id == delegation.target_member_id)
                    && manifest.manifest().lead.member_id != delegation.target_member_id
                {
                    return Err(AwoError::validation(format!(
                        "unknown target member `{}`",
                        delegation.target_member_id
                    )));
                }

                let task = manifest
                    .manifest_mut()
                    .task_mut(&task_id)
                    .ok_or_else(|| AwoError::unknown_task(&task_id))?;

                if task.state != TaskCardState::Todo {
                    return Err(AwoError::invalid_state(format!(
                        "cannot delegate task `{task_id}` in state `{}`; expected `todo`",
                        task.state
                    )));
                }

                task.owner_id = delegation.target_member_id.clone();
            }

            let task = manifest
                .manifest()
                .task(&task_id)
                .ok_or_else(|| AwoError::unknown_task(&task_id))?;
            let owner = manifest
                .manifest()
                .member(&task.owner_id)
                .ok_or_else(|| AwoError::unknown_owner(&task.owner_id))?;

            if owner.execution_mode != TeamExecutionMode::ExternalSlots {
                return Err(AwoError::invalid_state(format!(
                    "team task execution currently supports only `external_slots`; owner `{}` uses `{}`",
                    owner.member_id, owner.execution_mode
                )));
            }

            if task.state == TaskCardState::InProgress {
                return Err(AwoError::invalid_state(format!(
                    "task `{}` is already in progress",
                    task.task_id
                )));
            }

            let primary_runtime_name = task
                .runtime
                .as_deref()
                .or(owner.runtime.as_deref())
                .ok_or_else(|| {
                    AwoError::invalid_state(format!(
                        "task `{}` has no runtime; set one on the task or owner `{}`",
                        task.task_id, owner.member_id
                    ))
                })?;
            let primary_runtime: RuntimeKind = primary_runtime_name
                .parse()
                .map_err(|_| AwoError::unsupported("runtime", primary_runtime_name))?;
            let primary_target = crate::routing::RoutingTarget::new(
                primary_runtime,
                task.model.clone().or(owner.model.clone()),
            );
            let fallback_target = if let Some(fallback_runtime_name) = &owner.fallback_runtime {
                let fallback_runtime: RuntimeKind =
                    fallback_runtime_name.parse().map_err(|_| {
                        AwoError::unsupported("fallback runtime", fallback_runtime_name)
                    })?;
                Some(crate::routing::RoutingTarget::new(
                    fallback_runtime,
                    owner.fallback_model.clone(),
                ))
            } else {
                None
            };

            let routing_preferences = resolve_effective_routing_preferences(
                routing_preferences,
                owner,
                manifest.manifest(),
            );
            let routing_decision = crate::routing::route_runtime(
                primary_target,
                fallback_target,
                &routing_preferences,
                &crate::routing::RoutingContext::default(),
            );
            let runtime = routing_decision.selected_runtime;
            let runtime_name = runtime.as_str().to_string();

            let auto_start = delegation.as_ref().map(|d| d.auto_start).unwrap_or(true);

            let prompt = if let Some(ref delegation) = delegation {
                manifest
                    .manifest()
                    .render_delegated_prompt(&task.task_id, delegation)?
            } else if runtime == RuntimeKind::Shell {
                task.summary.clone()
            } else {
                manifest.manifest().render_task_prompt(&task.task_id)?
            };

            let read_only = task.read_only || owner.read_only;

            let repo_id = manifest.manifest().repo_id.clone();
            let owner_id = task.owner_id.clone();
            let task_slot_id = task.slot_id.clone();
            let owner_slot_id = owner.slot_id.clone();

            manifest.save()?;

            (
                repo_id,
                owner_id,
                runtime_name,
                runtime,
                routing_decision.selected_model,
                routing_decision.source,
                routing_decision.reason,
                prompt,
                read_only,
                auto_start,
                task_slot_id,
                owner_slot_id,
            )
        };

        if !auto_start {
            let manifest = self.load_team_manifest(&team_id)?;
            let execution = TeamTaskExecution {
                team_id: manifest.team_id.clone(),
                task_id,
                owner_id,
                runtime: runtime_name,
                model: selected_model,
                routing_source,
                routing_reason,
                slot_id: String::new(),
                branch_name: String::new(),
                session_id: None,
                session_status: SessionStatus::Prepared,
                acquired_slot: false,
                prompt,
            };
            return Ok((manifest, None, CommandOutcome::new("Delegated."), execution));
        }

        // Only mark as in-progress if we are actually starting and not in dry-run
        if !dry_run {
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &team_id)?;
            manifest
                .manifest_mut()
                .set_task_state(&task_id, TaskCardState::InProgress)?;
            manifest.save()?;
        }

        let mut slot_outcome = None;
        let (slot_id, branch_name, acquired_slot) = match task_slot_id.or(owner_slot_id) {
            Some(slot_id) => {
                let slot = match self.store.get_slot(&slot_id)? {
                    Some(slot) => slot,
                    None => {
                        recover_failed_start(self, None);
                        return Err(AwoError::unknown_slot(slot_id));
                    }
                };
                if slot.repo_id != repo_id {
                    recover_failed_start(self, None);
                    return Err(AwoError::invalid_state(format!(
                        "slot `{slot_id}` belongs to repo `{}`, not team repo `{}`",
                        slot.repo_id, repo_id
                    )));
                }
                (slot.id, slot.branch_name, false)
            }
            None => {
                let outcome = match self.dispatch(Command::SlotAcquire {
                    repo_id: repo_id.clone(),
                    task_name: task_id.clone(),
                    strategy,
                }) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        recover_failed_start(self, None);
                        return Err(error);
                    }
                };
                let slot_id = match outcome.events.iter().find_map(|event| match event {
                    crate::events::DomainEvent::SlotAcquired { slot_id, .. } => {
                        Some(slot_id.clone())
                    }
                    _ => None,
                }) {
                    Some(slot_id) => slot_id,
                    None => {
                        recover_failed_start(self, None);
                        return Err(AwoError::invalid_state(
                            "slot acquire did not yield a slot id",
                        ));
                    }
                };
                let slot = match self.store.get_slot(&slot_id)? {
                    Some(slot) => slot,
                    None => {
                        recover_failed_start(self, None);
                        return Err(AwoError::unknown_slot(slot_id));
                    }
                };
                slot_outcome = Some(outcome);
                (slot.id, slot.branch_name, true)
            }
        };

        if let Err(error) = (|| -> AwoResult<()> {
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &team_id)?;
            manifest
                .manifest_mut()
                .assign_member_slot(&owner_id, &slot_id, &branch_name)?;
            manifest
                .manifest_mut()
                .bind_task_slot(&task_id, &slot_id, &branch_name)?;
            manifest.save()
        })() {
            recover_failed_start(self, acquired_slot.then_some(slot_id.as_str()));
            return Err(error);
        }

        let session_outcome = match self.dispatch(Command::SessionStart {
            slot_id: slot_id.clone(),
            runtime,
            prompt: prompt.clone(),
            read_only,
            dry_run,
            launch_mode,
            attach_context,
            timeout_secs: None,
        }) {
            Ok(outcome) => outcome,
            Err(error) => {
                recover_failed_start(self, acquired_slot.then_some(slot_id.as_str()));
                return Err(error);
            }
        };

        let (session_id, session_status) = session_outcome
            .events
            .iter()
            .find_map(|event| match event {
                crate::events::DomainEvent::SessionStarted {
                    session_id, status, ..
                } => {
                    let status = SessionStatus::from_str(status).unwrap_or(SessionStatus::Prepared);
                    Some((Some(session_id.clone()), status))
                }
                _ => None,
            })
            .unwrap_or((None, SessionStatus::Prepared));

        let next_state = match session_status {
            SessionStatus::Completed => TaskCardState::Review,
            SessionStatus::Failed => TaskCardState::Blocked,
            SessionStatus::Running => TaskCardState::InProgress,
            _ => {
                if dry_run {
                    TaskCardState::Todo
                } else {
                    TaskCardState::InProgress
                }
            }
        };

        tracing::debug!(
            ?session_status,
            ?next_state,
            dry_run,
            "determined next task state"
        );

        let manifest = if !dry_run || next_state != TaskCardState::Todo {
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &team_id)?;
            manifest.manifest_mut().clear_task_result(&task_id)?;
            manifest
                .manifest_mut()
                .set_task_state(&task_id, next_state)?;
            if !dry_run && manifest.manifest().current_lead_member_id() == owner_id {
                manifest
                    .manifest_mut()
                    .bind_current_lead_session(&owner_id, session_id.clone())?;
            }
            manifest.save()?;
            let manifest = manifest.into_manifest();
            self.store.insert_action(
                "team_task_state",
                &format!(
                    "team_id={} task_id={} state={}",
                    manifest.team_id, task_id, next_state
                ),
            )?;
            manifest
        } else {
            self.load_team_manifest(&team_id)?
        };

        let execution = TeamTaskExecution {
            team_id,
            task_id,
            owner_id,
            runtime: runtime_name,
            model: selected_model,
            routing_source,
            routing_reason,
            slot_id,
            branch_name,
            session_id,
            session_status,
            acquired_slot,
            prompt,
        };

        Ok((manifest, slot_outcome, session_outcome, execution))
    }

    pub fn recommend_team_routing(
        &self,
        team_id: &str,
        member_id: Option<&str>,
        task_id: Option<&str>,
        context: &crate::routing::RoutingContext,
    ) -> AwoResult<crate::routing::RoutingRecommendation> {
        match (member_id, task_id) {
            (Some(_), Some(_)) => {
                return Err(AwoError::invalid_state(
                    "choose either `--member` or `--task`, not both",
                ));
            }
            (None, None) => {
                return Err(AwoError::invalid_state(
                    "choose one selector: `--member` or `--task`",
                ));
            }
            _ => {}
        }

        let manifest = self.load_team_manifest(team_id)?;
        let (member, task_id, primary_target, fallback_target) =
            resolve_team_routing_targets(&manifest, member_id, task_id)?;
        let preferences = resolve_effective_routing_preferences(None, member, &manifest);
        let decision =
            crate::routing::route_runtime(primary_target, fallback_target, &preferences, context);
        let team_id = manifest.team_id.clone();
        let member_id = member.member_id.clone();
        let task_id = task_id.map(|value| value.to_string());

        Ok(crate::routing::RoutingRecommendation {
            team_id,
            member_id,
            task_id,
            preferences,
            context: context.clone(),
            decision,
        })
    }

    pub(super) fn reconcile_team_manifest(&self, team_id: &str) -> AwoResult<TeamManifest> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, team_id)?;
        if reconcile_team_manifest_state(&self.store, guard.manifest_mut())? {
            guard.save()?;
        }
        Ok(guard.into_manifest())
    }

    pub(super) fn reconcile_all_team_manifests(&self) -> AwoResult<Vec<TeamManifest>> {
        list_team_manifest_paths(&self.config.paths)?
            .into_iter()
            .map(|path| {
                let team_id = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| {
                        AwoError::invalid_state(format!(
                            "team manifest path `{}` has no valid team id stem",
                            path.display()
                        ))
                    })?;
                self.reconcile_team_manifest(team_id)
            })
            .collect()
    }
}

pub(crate) fn resolve_team_routing_targets<'a>(
    manifest: &'a TeamManifest,
    member_id: Option<&str>,
    task_id: Option<&'a str>,
) -> AwoResult<(
    &'a TeamMember,
    Option<&'a str>,
    crate::routing::RoutingTarget,
    Option<crate::routing::RoutingTarget>,
)> {
    let (member, selected_task_id, primary_runtime_name) = if let Some(task_id) = task_id {
        let task = manifest
            .task(task_id)
            .ok_or_else(|| AwoError::unknown_task(task_id))?;
        let member = manifest
            .member(&task.owner_id)
            .ok_or_else(|| AwoError::unknown_owner(&task.owner_id))?;
        let primary_runtime_name = task
            .runtime
            .as_deref()
            .or(member.runtime.as_deref())
            .ok_or_else(|| {
                AwoError::invalid_state(format!(
                    "task `{}` has no runtime; set one on the task or owner `{}`",
                    task.task_id, member.member_id
                ))
            })?;
        (member, Some(task.task_id.as_str()), primary_runtime_name)
    } else if let Some(member_id) = member_id {
        let member = manifest
            .member(member_id)
            .ok_or_else(|| AwoError::unknown_owner(member_id))?;
        let primary_runtime_name = member.runtime.as_deref().ok_or_else(|| {
            AwoError::invalid_state(format!("member `{}` has no runtime", member.member_id))
        })?;
        (member, None, primary_runtime_name)
    } else {
        unreachable!("selectors are validated before target resolution");
    };

    let primary_runtime: RuntimeKind = primary_runtime_name
        .parse()
        .map_err(|_| AwoError::unsupported("runtime", primary_runtime_name))?;
    let primary_target = crate::routing::RoutingTarget::new(primary_runtime, member.model.clone());
    let fallback_target = if let Some(fallback_runtime_name) = &member.fallback_runtime {
        let fallback_runtime: RuntimeKind = fallback_runtime_name
            .parse()
            .map_err(|_| AwoError::unsupported("fallback runtime", fallback_runtime_name))?;
        Some(crate::routing::RoutingTarget::new(
            fallback_runtime,
            member.fallback_model.clone(),
        ))
    } else {
        None
    };

    Ok((member, selected_task_id, primary_target, fallback_target))
}

pub(crate) fn resolve_effective_routing_preferences(
    explicit: Option<crate::routing::RoutingPreferences>,
    member: &TeamMember,
    manifest: &TeamManifest,
) -> crate::routing::RoutingPreferences {
    explicit
        .or_else(|| member.routing_preferences.clone())
        .or_else(|| manifest.routing_preferences.clone())
        .unwrap_or_default()
}
