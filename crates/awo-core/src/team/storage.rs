use super::TeamManifest;
use crate::app::AppPaths;
use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

pub fn default_team_manifest_path(paths: &AppPaths, team_id: &str) -> PathBuf {
    paths.teams_dir.join(format!("{team_id}.toml"))
}

fn team_manifest_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}.lock"))
        .unwrap_or_else(|| "team.lock".to_string());
    path.with_file_name(file_name)
}

fn open_team_manifest_lock(path: &Path) -> Result<File> {
    let lock_path = team_manifest_lock_path(path);
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| {
            format!(
                "failed to open team manifest lock at {}",
                lock_path.display()
            )
        })
}

fn read_team_manifest_unlocked(path: &Path) -> Result<TeamManifest> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read team manifest at {}", path.display()))?;
    let manifest = toml::from_str::<TeamManifest>(&contents)
        .with_context(|| format!("failed to parse team manifest at {}", path.display()))?;
    manifest.validate()?;
    Ok(manifest)
}

fn write_team_manifest_unlocked(path: &Path, manifest: &TeamManifest) -> Result<()> {
    manifest.validate()?;
    let contents = toml::to_string_pretty(manifest).context("failed to serialize team manifest")?;
    fs::write(path, contents)
        .with_context(|| format!("failed to write team manifest at {}", path.display()))
}

pub struct TeamManifestGuard {
    path: PathBuf,
    _lock: File,
    manifest: TeamManifest,
}

impl TeamManifestGuard {
    pub fn load(paths: &AppPaths, team_id: &str) -> Result<Self> {
        let path = default_team_manifest_path(paths, team_id);
        let lock = open_team_manifest_lock(&path)?;
        lock.lock_exclusive().with_context(|| {
            format!(
                "failed to acquire exclusive lock for team manifest at {}",
                path.display()
            )
        })?;
        let manifest = read_team_manifest_unlocked(&path)?;
        Ok(Self {
            path,
            _lock: lock,
            manifest,
        })
    }

    pub fn manifest(&self) -> &TeamManifest {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut TeamManifest {
        &mut self.manifest
    }

    pub fn save(&mut self) -> Result<()> {
        write_team_manifest_unlocked(&self.path, &self.manifest)
    }

    pub fn into_manifest(self) -> TeamManifest {
        self.manifest
    }
}

pub fn save_team_manifest(paths: &AppPaths, manifest: &TeamManifest) -> Result<PathBuf> {
    fs::create_dir_all(&paths.teams_dir).with_context(|| {
        format!(
            "failed to create team manifest dir at {}",
            paths.teams_dir.display()
        )
    })?;
    let path = default_team_manifest_path(paths, &manifest.team_id);
    let lock = open_team_manifest_lock(&path)?;
    lock.lock_exclusive().with_context(|| {
        format!(
            "failed to acquire exclusive lock for team manifest at {}",
            path.display()
        )
    })?;
    write_team_manifest_unlocked(&path, manifest)?;
    Ok(path)
}

pub fn load_team_manifest(path: &Path) -> Result<TeamManifest> {
    let lock = open_team_manifest_lock(path)?;
    lock.lock_shared().with_context(|| {
        format!(
            "failed to acquire shared lock for team manifest at {}",
            path.display()
        )
    })?;
    read_team_manifest_unlocked(path)
}

pub fn list_team_manifest_paths(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    if !paths.teams_dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = fs::read_dir(&paths.teams_dir)
        .with_context(|| format!("failed to read team dir at {}", paths.teams_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    manifests.sort();
    Ok(manifests)
}
