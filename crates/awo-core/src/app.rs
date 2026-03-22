use crate::commands::{Command, CommandOutcome, CommandRunner};
use crate::config::AppConfig;
use crate::context::{
    ContextDoctorReport, RepoContext, discover_repo_context, doctor_repo_context,
};
use crate::error::{AwoError, AwoResult};
use crate::runtime::{RuntimeKind, SessionLaunchMode};
use crate::skills::{
    RepoSkillCatalog, RuntimeSkillRoots, SkillDoctorReport, SkillLinkMode, SkillLinkReport,
    SkillRuntime, discover_repo_skills, doctor_repo_skills,
};
use crate::slot::SlotStrategy;
use crate::snapshot::AppSnapshot;
use crate::store::Store;
use crate::team::{
    TaskCard, TaskCardState, TeamManifest, TeamManifestGuard, TeamMember, TeamResetSummary,
    TeamTaskExecution, TeamTaskStartOptions, TeamTeardownPlan, TeamTeardownResult,
    list_team_manifest_paths, remove_team_manifest, save_team_manifest,
};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: std::path::PathBuf,
    pub data_dir: std::path::PathBuf,
    pub state_db_path: std::path::PathBuf,
    pub logs_dir: std::path::PathBuf,
    pub repos_dir: std::path::PathBuf,
    pub clones_dir: std::path::PathBuf,
    pub teams_dir: std::path::PathBuf,
}

#[derive(Debug)]
pub struct AppCore {
    config: AppConfig,
    store: Store,
}

impl AppCore {
    pub fn bootstrap() -> AwoResult<Self> {
        let config = AppConfig::load()?;
        Self::from_config(config)
    }

    pub fn from_config(config: AppConfig) -> AwoResult<Self> {
        let store = Store::open(&config.paths.state_db_path)?;
        store.initialize_schema()?;

        Ok(Self { config, store })
    }

    pub fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        let mut runner = CommandRunner::new(&self.config, &self.store);
        runner.run(command)
    }

    pub fn snapshot(&self) -> AwoResult<AppSnapshot> {
        self.sync_runtime_state(None)?;
        let _ = self.reconcile_all_team_manifests()?;
        AppSnapshot::load(&self.config, &self.store)
    }

    pub fn context_for_repo(&self, repo_id: &str) -> AwoResult<RepoContext> {
        let repo = self
            .store
            .get_repository(repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(repo_id))?;
        discover_repo_context(Path::new(&repo.repo_root))
    }

    pub fn context_doctor_for_repo(&self, repo_id: &str) -> AwoResult<ContextDoctorReport> {
        let context = self.context_for_repo(repo_id)?;
        Ok(doctor_repo_context(&context))
    }

    pub fn skills_for_repo(&self, repo_id: &str) -> AwoResult<RepoSkillCatalog> {
        let repo = self
            .store
            .get_repository(repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(repo_id))?;
        discover_repo_skills(Path::new(&repo.repo_root))
    }

    pub fn skills_doctor_for_repo(
        &self,
        repo_id: &str,
        runtimes: &[SkillRuntime],
    ) -> AwoResult<Vec<SkillDoctorReport>> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        runtimes
            .iter()
            .copied()
            .map(|runtime| doctor_repo_skills(&catalog, runtime, &roots))
            .collect()
    }

    pub fn skills_link_for_repo(
        &self,
        repo_id: &str,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> AwoResult<SkillLinkReport> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        crate::skills::link_repo_skills(&catalog, runtime, &roots, mode)
    }

    pub fn skills_sync_for_repo(
        &self,
        repo_id: &str,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> AwoResult<SkillLinkReport> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        crate::skills::sync_repo_skills(&catalog, runtime, &roots, mode)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.config.paths
    }

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
                && slot.status != "released"
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
                && slot.status != "released"
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
        let launch_mode: SessionLaunchMode = options
            .launch_mode
            .parse()
            .map_err(|_| AwoError::unsupported("launch mode", &options.launch_mode))?;
        let strategy: SlotStrategy = options
            .strategy
            .parse()
            .map_err(|_| AwoError::unsupported("slot strategy", &options.strategy))?;
        let team_id = options.team_id.clone();
        let task_id = options.task_id.clone();
        let recover_failed_start = |core: &mut Self, slot_id: Option<&str>| {
            let _ = core.set_team_task_state(&team_id, &task_id, TaskCardState::Blocked);
            if let Some(slot_id) = slot_id {
                let _ = core.dispatch(Command::SlotRelease {
                    slot_id: slot_id.to_string(),
                });
            }
        };

        let (
            repo_id,
            task,
            owner,
            runtime_name,
            runtime,
            selected_model,
            routing_source,
            prompt,
            read_only,
        ) = {
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &options.team_id)?;
            let task = manifest
                .manifest()
                .task(&options.task_id)
                .cloned()
                .ok_or_else(|| AwoError::unknown_task(&options.task_id))?;
            let owner = manifest
                .manifest()
                .member(&task.owner_id)
                .cloned()
                .ok_or_else(|| AwoError::unknown_owner(&task.owner_id))?;
            if owner.execution_mode.as_str() != "external_slots" {
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
            let primary_target =
                crate::routing::RoutingTarget::new(primary_runtime, owner.model.clone());
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
            let routing_decision = crate::routing::route_runtime(
                primary_target,
                fallback_target,
                &crate::routing::RoutingPreferences::default(),
            );
            let runtime = routing_decision.selected_runtime;
            let runtime_name = runtime.as_str().to_string();
            let prompt = if runtime.as_str() == "shell" {
                task.summary.clone()
            } else {
                manifest.manifest().render_task_prompt(&task.task_id)?
            };
            let read_only = task.read_only || owner.read_only;
            manifest
                .manifest_mut()
                .set_task_state(&task.task_id, TaskCardState::InProgress)?;
            manifest.save()?;

            (
                manifest.manifest().repo_id.clone(),
                task,
                owner,
                runtime_name,
                runtime,
                routing_decision.selected_model,
                routing_decision.source,
                prompt,
                read_only,
            )
        };

        let mut slot_outcome = None;
        let (slot_id, branch_name, acquired_slot) =
            match task.slot_id.clone().or(owner.slot_id.clone()) {
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
                        task_name: task.task_id.clone(),
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
            let mut manifest = TeamManifestGuard::load(&self.config.paths, &options.team_id)?;
            manifest
                .manifest_mut()
                .assign_member_slot(&task.owner_id, &slot_id, &branch_name)?;
            manifest
                .manifest_mut()
                .bind_task_slot(&task.task_id, &slot_id, &branch_name)?;
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
            dry_run: options.dry_run,
            launch_mode,
            attach_context: options.attach_context,
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
                } => Some((Some(session_id.clone()), status.clone())),
                _ => None,
            })
            .unwrap_or_else(|| (None, "unknown".to_string()));

        let next_state = match session_status.as_str() {
            "completed" => TaskCardState::Review,
            "failed" => TaskCardState::Blocked,
            "running" => TaskCardState::InProgress,
            _ => task.state,
        };
        let manifest = self.set_team_task_state(&options.team_id, &task.task_id, next_state)?;
        self.store.insert_action(
            "team_task_start",
            &format!(
                "team_id={} task_id={} slot_id={} runtime={} session_status={} acquired_slot={}",
                manifest.team_id,
                task.task_id,
                slot_id,
                runtime_name,
                session_status,
                acquired_slot
            ),
        )?;

        let execution = TeamTaskExecution {
            team_id: manifest.team_id.clone(),
            task_id: task.task_id.clone(),
            owner_id: task.owner_id.clone(),
            runtime: runtime_name.to_string(),
            model: selected_model,
            routing_source,
            slot_id,
            branch_name,
            session_id,
            session_status,
            acquired_slot,
            prompt,
        };

        Ok((manifest, slot_outcome, session_outcome, execution))
    }

    fn sync_runtime_state(&self, repo_id: Option<&str>) -> AwoResult<()> {
        let runner = CommandRunner::new(&self.config, &self.store);
        runner.sync_runtime_state(repo_id)?;
        Ok(())
    }

    fn reconcile_team_manifest(&self, team_id: &str) -> AwoResult<TeamManifest> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, team_id)?;
        if reconcile_team_manifest_state(&self.store, guard.manifest_mut())? {
            guard.save()?;
        }
        Ok(guard.into_manifest())
    }

    fn reconcile_all_team_manifests(&self) -> AwoResult<Vec<TeamManifest>> {
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

fn collect_bound_slot_ids(manifest: &TeamManifest) -> Vec<String> {
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

fn build_team_teardown_plan(store: &Store, manifest: &TeamManifest) -> AwoResult<TeamTeardownPlan> {
    let bound_slots = collect_bound_slot_ids(manifest);
    let mut active_slots = BTreeSet::new();
    let mut dirty_slots = BTreeSet::new();
    let mut cancellable_sessions = BTreeSet::new();
    let mut blocking_sessions = BTreeSet::new();

    for slot_id in &bound_slots {
        if let Some(slot) = store.get_slot(slot_id)? {
            if slot.status != "released" {
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

            if session.status == "running" && !session.is_supervised() {
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

fn reconcile_team_manifest_state(store: &Store, manifest: &mut TeamManifest) -> AwoResult<bool> {
    if manifest.status.as_str() == "archived" {
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
        let has_running_session = sessions.iter().any(|session| !session.is_terminal());
        let slot_missing_or_released = slot
            .as_ref()
            .is_none_or(|slot| slot.status.as_str() == "released");

        if has_running_session {
            if task.state != TaskCardState::InProgress {
                task.state = TaskCardState::InProgress;
                changed = true;
            }
            continue;
        }

        if let Some(session) = sessions.iter().find(|session| session.is_terminal()) {
            match session.status.as_str() {
                "completed" => {
                    if !matches!(task.state, TaskCardState::Done | TaskCardState::Review) {
                        task.state = TaskCardState::Review;
                        changed = true;
                    }
                }
                "failed" | "cancelled" => {
                    if task.state != TaskCardState::Blocked {
                        task.state = TaskCardState::Blocked;
                        changed = true;
                    }
                }
                _ => {}
            }
        } else if task.state == TaskCardState::InProgress && slot_missing_or_released {
            task.state = TaskCardState::Blocked;
            changed = true;
        }

        if slot_missing_or_released && !has_running_session {
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

    if changed {
        manifest.refresh_status();
        manifest.validate()?;
    }

    Ok(changed)
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
        .is_none_or(|slot| slot.status.as_str() == "released"))
}

#[cfg(test)]
mod tests;
