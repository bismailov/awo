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
    TaskCard, TaskCardState, TeamManifest, TeamManifestGuard, TeamMember, TeamTaskExecution,
    TeamTaskStartOptions, list_team_manifest_paths, load_team_manifest, save_team_manifest,
};
use anyhow::Result;
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
        let runner = CommandRunner::new(&self.config, &self.store);
        runner.sync_runtime_state(None)?;
        Ok(AppSnapshot::load(&self.config, &self.store)?)
    }

    pub fn context_for_repo(&self, repo_id: &str) -> AwoResult<RepoContext> {
        let repo = self
            .store
            .get_repository(repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(repo_id))?;
        Ok(discover_repo_context(Path::new(&repo.repo_root))?)
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
        Ok(discover_repo_skills(Path::new(&repo.repo_root))?)
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
            .map(|runtime| doctor_repo_skills(&catalog, runtime, &roots).map_err(Into::into))
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
        Ok(crate::skills::link_repo_skills(
            &catalog, runtime, &roots, mode,
        )?)
    }

    pub fn skills_sync_for_repo(
        &self,
        repo_id: &str,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> AwoResult<SkillLinkReport> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        Ok(crate::skills::sync_repo_skills(
            &catalog, runtime, &roots, mode,
        )?)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.config.paths
    }

    pub fn save_team_manifest(&self, manifest: &TeamManifest) -> AwoResult<std::path::PathBuf> {
        Ok(save_team_manifest(&self.config.paths, manifest)?)
    }

    pub fn load_team_manifest(&self, team_id: &str) -> AwoResult<TeamManifest> {
        let path = crate::team::default_team_manifest_path(&self.config.paths, team_id);
        Ok(load_team_manifest(&path)?)
    }

    pub fn list_team_manifests(&self) -> AwoResult<Vec<TeamManifest>> {
        list_team_manifest_paths(&self.config.paths)?
            .into_iter()
            .map(|path| load_team_manifest(&path).map_err(Into::into))
            .collect()
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

        let (repo_id, task, owner, runtime_name, runtime, prompt, read_only) = {
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

            let runtime_name = task
                .runtime
                .as_deref()
                .or(owner.runtime.as_deref())
                .ok_or_else(|| {
                    AwoError::invalid_state(format!(
                        "task `{}` has no runtime; set one on the task or owner `{}`",
                        task.task_id, owner.member_id
                    ))
                })?;
            let runtime_name = runtime_name.to_string();
            let runtime: RuntimeKind = runtime_name
                .parse()
                .map_err(|_| AwoError::unsupported("runtime", &runtime_name))?;
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

        if let Err(error) = (|| -> Result<()> {
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
            return Err(error.into());
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
            slot_id,
            branch_name,
            session_id,
            session_status,
            acquired_slot,
            prompt,
        };

        Ok((manifest, slot_outcome, session_outcome, execution))
    }
}

#[cfg(test)]
mod tests;
