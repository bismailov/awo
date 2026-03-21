use crate::commands::{Command, CommandOutcome, CommandRunner};
use crate::config::AppConfig;
use crate::context::{
    ContextDoctorReport, RepoContext, discover_repo_context, doctor_repo_context,
};
use crate::skills::{
    RepoSkillCatalog, RuntimeSkillRoots, SkillDoctorReport, SkillLinkMode, SkillLinkReport,
    SkillRuntime, discover_repo_skills, doctor_repo_skills,
};
use crate::snapshot::AppSnapshot;
use crate::store::Store;
use crate::team::{TeamManifest, list_team_manifest_paths, load_team_manifest, save_team_manifest};
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
    pub fn bootstrap() -> Result<Self> {
        let config = AppConfig::load()?;
        let store = Store::open(&config.paths.state_db_path)?;
        store.initialize_schema()?;

        Ok(Self { config, store })
    }

    pub fn dispatch(&mut self, command: Command) -> Result<CommandOutcome> {
        let mut runner = CommandRunner::new(&self.config, &self.store);
        runner.run(command)
    }

    pub fn snapshot(&self) -> Result<AppSnapshot> {
        let runner = CommandRunner::new(&self.config, &self.store);
        runner.sync_runtime_state(None)?;
        AppSnapshot::load(&self.config, &self.store)
    }

    pub fn context_for_repo(&self, repo_id: &str) -> Result<RepoContext> {
        let repo = self
            .store
            .get_repository(repo_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown repo id `{repo_id}`"))?;
        discover_repo_context(Path::new(&repo.repo_root))
    }

    pub fn context_doctor_for_repo(&self, repo_id: &str) -> Result<ContextDoctorReport> {
        let context = self.context_for_repo(repo_id)?;
        Ok(doctor_repo_context(&context))
    }

    pub fn skills_for_repo(&self, repo_id: &str) -> Result<RepoSkillCatalog> {
        let repo = self
            .store
            .get_repository(repo_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown repo id `{repo_id}`"))?;
        discover_repo_skills(Path::new(&repo.repo_root))
    }

    pub fn skills_doctor_for_repo(
        &self,
        repo_id: &str,
        runtimes: &[SkillRuntime],
    ) -> Result<Vec<SkillDoctorReport>> {
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
    ) -> Result<SkillLinkReport> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        crate::skills::link_repo_skills(&catalog, runtime, &roots, mode)
    }

    pub fn skills_sync_for_repo(
        &self,
        repo_id: &str,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> Result<SkillLinkReport> {
        let catalog = self.skills_for_repo(repo_id)?;
        let roots = RuntimeSkillRoots::from_environment();
        crate::skills::sync_repo_skills(&catalog, runtime, &roots, mode)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.config.paths
    }

    pub fn save_team_manifest(&self, manifest: &TeamManifest) -> Result<std::path::PathBuf> {
        save_team_manifest(&self.config.paths, manifest)
    }

    pub fn load_team_manifest(&self, team_id: &str) -> Result<TeamManifest> {
        let path = crate::team::default_team_manifest_path(&self.config.paths, team_id);
        load_team_manifest(&path)
    }

    pub fn list_team_manifests(&self) -> Result<Vec<TeamManifest>> {
        list_team_manifest_paths(&self.config.paths)?
            .into_iter()
            .map(|path| load_team_manifest(&path))
            .collect()
    }
}
