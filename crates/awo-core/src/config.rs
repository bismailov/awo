use crate::app::AppPaths;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub paths: AppPaths,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let project_dirs = ProjectDirs::from("net", "awo", "awo")
            .context("failed to resolve application directories")?;

        let config_dir = project_dirs.config_dir().to_path_buf();
        let data_dir = project_dirs.data_dir().to_path_buf();
        let logs_dir = data_dir.join("logs");
        let clones_dir = data_dir.join("clones");
        let repos_dir = config_dir.join("repos");
        let teams_dir = config_dir.join("teams");
        let state_db_path = data_dir.join("state.sqlite3");

        fs::create_dir_all(&config_dir)
            .with_context(|| format!("failed to create config dir at {}", config_dir.display()))?;
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create data dir at {}", data_dir.display()))?;
        fs::create_dir_all(&logs_dir)
            .with_context(|| format!("failed to create logs dir at {}", logs_dir.display()))?;
        fs::create_dir_all(&clones_dir)
            .with_context(|| format!("failed to create clones dir at {}", clones_dir.display()))?;
        fs::create_dir_all(&repos_dir)
            .with_context(|| format!("failed to create repos dir at {}", repos_dir.display()))?;
        fs::create_dir_all(&teams_dir)
            .with_context(|| format!("failed to create teams dir at {}", teams_dir.display()))?;

        Ok(Self {
            paths: AppPaths {
                config_dir,
                data_dir,
                state_db_path,
                logs_dir,
                repos_dir,
                clones_dir,
                teams_dir,
            },
        })
    }
}
