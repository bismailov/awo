use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
use crate::routing::RuntimePressure;
use crate::runtime::RuntimeKind;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub runtime_pressure_profile: HashMap<RuntimeKind, RuntimePressure>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub paths: AppPaths,
    pub settings: AppSettings,
}

impl AppConfig {
    pub fn load() -> AwoResult<Self> {
        let project_dirs = ProjectDirs::from("net", "awo", "awo")
            .ok_or_else(AwoError::project_directories_unavailable)?;

        let config_dir = if let Ok(value) = std::env::var("AWO_CONFIG_DIR") {
            PathBuf::from(value)
        } else {
            project_dirs.config_dir().to_path_buf()
        };
        let data_dir = if let Ok(value) = std::env::var("AWO_DATA_DIR") {
            PathBuf::from(value)
        } else {
            project_dirs.data_dir().to_path_buf()
        };
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

        let settings_path = config_dir.join("settings.json");
        let settings = if settings_path.exists() {
            let settings_str = fs::read_to_string(&settings_path)
                .map_err(|source| AwoError::io("read settings file", &settings_path, source))?;
            serde_json::from_str(&settings_str)
                .map_err(|source| AwoError::config_deserialization("settings.json", source))?
        } else {
            AppSettings::default()
        };

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
            settings,
        })
    }

    pub fn save_settings(&self) -> AwoResult<()> {
        let settings_path = self.paths.config_dir.join("settings.json");
        let settings_str = serde_json::to_string_pretty(&self.settings)
            .map_err(|source| AwoError::config_serialization("settings.json", source))?;
        fs::write(&settings_path, settings_str)
            .map_err(|source| AwoError::io("write settings file", &settings_path, source))?;
        Ok(())
    }
}
