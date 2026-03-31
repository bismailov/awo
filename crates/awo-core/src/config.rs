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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clones_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktrees_root: Option<PathBuf>,
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

        Self::with_dirs_and_storage_overrides(
            config_dir,
            data_dir,
            std::env::var("AWO_CLONES_DIR").map(PathBuf::from).ok(),
            std::env::var("AWO_WORKTREES_DIR").map(PathBuf::from).ok(),
        )
    }

    pub fn with_dirs(config_dir: PathBuf, data_dir: PathBuf) -> AwoResult<Self> {
        Self::with_dirs_and_storage_overrides(config_dir, data_dir, None, None)
    }

    fn with_dirs_and_storage_overrides(
        config_dir: PathBuf,
        data_dir: PathBuf,
        clones_override: Option<PathBuf>,
        worktrees_override: Option<PathBuf>,
    ) -> AwoResult<Self> {
        let repos_dir = config_dir.join("repos");
        let teams_dir = config_dir.join("teams");
        let state_db_path = data_dir.join("state.sqlite3");

        fs::create_dir_all(&config_dir)
            .map_err(|source| AwoError::io("create config dir", &config_dir, source))?;
        fs::create_dir_all(&data_dir)
            .map_err(|source| AwoError::io("create data dir", &data_dir, source))?;
        fs::create_dir_all(&repos_dir)
            .map_err(|source| AwoError::io("create repos dir", &repos_dir, source))?;
        fs::create_dir_all(&teams_dir)
            .map_err(|source| AwoError::io("create teams dir", &teams_dir, source))?;

        let settings_path = config_dir.join("settings.json");
        let settings: AppSettings = if settings_path.exists() {
            let settings_str = fs::read_to_string(&settings_path)
                .map_err(|source| AwoError::io("read settings file", &settings_path, source))?;
            serde_json::from_str(&settings_str)
                .map_err(|source| AwoError::config_deserialization("settings.json", source))?
        } else {
            AppSettings::default()
        };
        let logs_dir = data_dir.join("logs");
        let clones_dir = resolve_storage_root(
            clones_override,
            settings.clones_root.clone(),
            data_dir.join("clones"),
        );
        let worktrees_dir = resolve_storage_root(
            worktrees_override,
            settings.worktrees_root.clone(),
            data_dir.join("worktrees"),
        );

        fs::create_dir_all(&logs_dir)
            .map_err(|source| AwoError::io("create logs dir", &logs_dir, source))?;
        fs::create_dir_all(&clones_dir)
            .map_err(|source| AwoError::io("create clones dir", &clones_dir, source))?;
        fs::create_dir_all(&worktrees_dir)
            .map_err(|source| AwoError::io("create worktrees dir", &worktrees_dir, source))?;

        Ok(Self {
            paths: AppPaths {
                config_dir,
                data_dir,
                state_db_path,
                logs_dir,
                repos_dir,
                clones_dir,
                worktrees_dir,
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

fn resolve_storage_root(
    env_override: Option<PathBuf>,
    settings_value: Option<PathBuf>,
    default: PathBuf,
) -> PathBuf {
    env_override.or(settings_value).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, AppSettings, resolve_storage_root};
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn with_dirs_honors_settings_storage_roots() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("settings.json"),
            serde_json::to_string(&AppSettings {
                clones_root: Some(temp_dir.path().join("managed-clones")),
                worktrees_root: Some(temp_dir.path().join("managed-worktrees")),
                ..AppSettings::default()
            })?,
        )?;

        let config = AppConfig::with_dirs(config_dir, data_dir)?;
        assert_eq!(
            config.paths.clones_dir,
            temp_dir.path().join("managed-clones")
        );
        assert_eq!(
            config.paths.worktrees_dir,
            temp_dir.path().join("managed-worktrees")
        );
        assert!(config.paths.clones_dir.exists());
        assert!(config.paths.worktrees_dir.exists());
        Ok(())
    }

    #[test]
    fn resolve_storage_root_prefers_env_override() {
        assert_eq!(
            resolve_storage_root(
                Some(PathBuf::from("/env/clones")),
                Some(PathBuf::from("/settings/clones")),
                PathBuf::from("/default/clones"),
            ),
            PathBuf::from("/env/clones")
        );
    }
}
