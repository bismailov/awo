use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use directories::ProjectDirs;
use std::fs;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub paths: AppPaths,
}

impl AppConfig {
    pub fn load() -> AwoResult<Self> {
        let project_dirs = ProjectDirs::from("net", "awo", "awo")
            .ok_or_else(AwoError::project_directories_unavailable)?;

        let config_dir = project_dirs.config_dir().to_path_buf();
        let data_dir = project_dirs.data_dir().to_path_buf();
        let logs_dir = data_dir.join("logs");
        let clones_dir = data_dir.join("clones");
        let repos_dir = config_dir.join("repos");
        let teams_dir = config_dir.join("teams");
        let state_db_path = data_dir.join("state.sqlite3");

        fs::create_dir_all(&config_dir)
            .map_err(|source| AwoError::io("create config dir", &config_dir, source))?;
        fs::create_dir_all(&data_dir)
            .map_err(|source| AwoError::io("create data dir", &data_dir, source))?;
        fs::create_dir_all(&logs_dir)
            .map_err(|source| AwoError::io("create logs dir", &logs_dir, source))?;
        fs::create_dir_all(&clones_dir)
            .map_err(|source| AwoError::io("create clones dir", &clones_dir, source))?;
        fs::create_dir_all(&repos_dir)
            .map_err(|source| AwoError::io("create repos dir", &repos_dir, source))?;
        fs::create_dir_all(&teams_dir)
            .map_err(|source| AwoError::io("create teams dir", &teams_dir, source))?;

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
