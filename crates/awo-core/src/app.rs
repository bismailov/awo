mod team_ops;

use crate::commands::{Command, CommandOutcome, CommandRunner};
use crate::config::AppConfig;
use crate::context::{
    ContextDoctorReport, RepoContext, discover_repo_context, doctor_repo_context,
};
use crate::dispatch::Dispatcher;
use crate::error::{AwoError, AwoResult};
use crate::skills::{
    RepoSkillCatalog, RuntimeSkillRoots, SkillDoctorReport, SkillLinkMode, SkillLinkReport,
    SkillRuntime, discover_repo_skills, doctor_repo_skills,
};
use crate::snapshot::{AppSnapshot, DirtyFileCache};
use crate::store::Store;
use std::cell::RefCell;
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

impl AppPaths {
    /// Returns the path for the daemon socket file.
    ///
    /// On Unix: `{data_dir}/awod.sock`
    /// On Windows: this returns a file path but the daemon uses a Named Pipe.
    pub fn daemon_socket_path(&self) -> std::path::PathBuf {
        self.data_dir.join("awod.sock")
    }

    /// Returns the path for the daemon lock file.
    pub fn daemon_lock_path(&self) -> std::path::PathBuf {
        self.data_dir.join("awod.lock")
    }

    /// Returns the path for the daemon PID file.
    pub fn daemon_pid_path(&self) -> std::path::PathBuf {
        self.data_dir.join("awod.pid")
    }
}

#[derive(Debug)]
pub struct AppCore {
    config: AppConfig,
    store: Store,
    dirty_cache: RefCell<DirtyFileCache>,
}

impl AppCore {
    pub fn bootstrap() -> AwoResult<Self> {
        let config = AppConfig::load()?;
        Self::from_config(config)
    }

    pub fn from_config(config: AppConfig) -> AwoResult<Self> {
        let store = Store::open(&config.paths.state_db_path)?;
        store.initialize_schema()?;

        Ok(Self {
            config,
            store,
            dirty_cache: RefCell::new(DirtyFileCache::new()),
        })
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    pub fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        match &command {
            Command::SlotRelease { slot_id } | Command::SlotRefresh { slot_id } => {
                self.dirty_cache.borrow_mut().invalidate(slot_id);
            }
            _ => {}
        }
        Dispatcher::dispatch(self, command)
    }

    pub fn snapshot(&self) -> AwoResult<AppSnapshot> {
        self.sync_runtime_state(None)?;
        let _ = self.reconcile_all_team_manifests()?;
        let mut cache = self.dirty_cache.borrow_mut();
        let snapshot = AppSnapshot::load(&self.config, &self.store, &mut cache)?;
        let slot_ids: Vec<&str> = snapshot.slots.iter().map(|s| s.id.as_str()).collect();
        cache.retain_slots(&slot_ids);
        Ok(snapshot)
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

    fn sync_runtime_state(&self, repo_id: Option<&str>) -> AwoResult<()> {
        let runner = CommandRunner::new(&self.config, &self.store);
        runner.sync_runtime_state(repo_id)?;
        Ok(())
    }
}

impl Dispatcher for AppCore {
    fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
        let mut runner = CommandRunner::new(&self.config, &self.store);
        runner.run(command)
    }
}

#[cfg(test)]
mod tests;
