use super::TeamManifest;
use crate::app::AppPaths;
use crate::error::{AwoError, AwoResult};
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

fn open_team_manifest_lock(path: &Path) -> AwoResult<File> {
    let lock_path = team_manifest_lock_path(path);
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| AwoError::io("open team manifest lock", lock_path, source))
}

fn read_team_manifest_unlocked(path: &Path) -> AwoResult<TeamManifest> {
    let contents = fs::read_to_string(path)
        .map_err(|source| AwoError::io("read team manifest", path, source))?;
    let manifest = toml::from_str::<TeamManifest>(&contents)
        .map_err(|source| AwoError::team_manifest_parse(path, source))?;
    manifest.validate()?;
    Ok(manifest)
}

fn write_team_manifest_unlocked(path: &Path, manifest: &TeamManifest) -> AwoResult<()> {
    manifest.validate()?;
    let contents = toml::to_string_pretty(manifest).map_err(AwoError::team_manifest_serialize)?;
    fs::write(path, contents).map_err(|source| AwoError::io("write team manifest", path, source))
}

pub struct TeamManifestGuard {
    path: PathBuf,
    _lock: File,
    manifest: TeamManifest,
}

impl TeamManifestGuard {
    pub fn load(paths: &AppPaths, team_id: &str) -> AwoResult<Self> {
        let path = default_team_manifest_path(paths, team_id);
        let lock = open_team_manifest_lock(&path)?;
        lock.lock_exclusive()
            .map_err(|source| AwoError::file_lock("exclusive", &path, source))?;
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

    pub fn save(&mut self) -> AwoResult<()> {
        write_team_manifest_unlocked(&self.path, &self.manifest)
    }

    pub fn into_manifest(self) -> TeamManifest {
        self.manifest
    }
}

pub fn save_team_manifest(paths: &AppPaths, manifest: &TeamManifest) -> AwoResult<PathBuf> {
    fs::create_dir_all(&paths.teams_dir)
        .map_err(|source| AwoError::io("create team manifest dir", &paths.teams_dir, source))?;
    let path = default_team_manifest_path(paths, &manifest.team_id);
    let lock = open_team_manifest_lock(&path)?;
    lock.lock_exclusive()
        .map_err(|source| AwoError::file_lock("exclusive", &path, source))?;
    write_team_manifest_unlocked(&path, manifest)?;
    Ok(path)
}

pub fn load_team_manifest(path: &Path) -> AwoResult<TeamManifest> {
    let lock = open_team_manifest_lock(path)?;
    lock.lock_shared()
        .map_err(|source| AwoError::file_lock("shared", path, source))?;
    read_team_manifest_unlocked(path)
}

pub fn remove_team_manifest(paths: &AppPaths, team_id: &str) -> AwoResult<()> {
    let path = default_team_manifest_path(paths, team_id);
    let lock = open_team_manifest_lock(&path)?;
    lock.lock_exclusive()
        .map_err(|source| AwoError::file_lock("exclusive", &path, source))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|source| AwoError::io("remove team manifest", &path, source))?;
    }
    let lock_path = team_manifest_lock_path(&path);
    // Best-effort lock file cleanup — not critical if it fails.
    let _ = fs::remove_file(&lock_path);
    Ok(())
}

pub fn list_team_manifest_paths(paths: &AppPaths) -> AwoResult<Vec<PathBuf>> {
    if !paths.teams_dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = fs::read_dir(&paths.teams_dir)
        .map_err(|source| AwoError::io("read team manifest dir", &paths.teams_dir, source))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    manifests.sort();
    Ok(manifests)
}
