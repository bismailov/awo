use crate::diagnostics::Diagnostic;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, EnumString, IntoStaticStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SkillRuntime {
    Codex,
    Claude,
    Gemini,
}

impl SkillRuntime {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    pub fn all() -> [Self; 3] {
        [Self::Codex, Self::Claude, Self::Gemini]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, EnumString, IntoStaticStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SkillLinkMode {
    Symlink,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, IntoStaticStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkillDiscoveryStrategy {
    GlobalProjection,
    RepoLocalPreferred,
}

impl SkillDiscoveryStrategy {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRuntimePolicy {
    pub runtime: SkillRuntime,
    pub discovery: SkillDiscoveryStrategy,
    pub recommended_mode: SkillLinkMode,
    pub note: String,
}

impl SkillLinkMode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    pub fn default_for_platform() -> Self {
        #[cfg(windows)]
        {
            Self::Copy
        }

        #[cfg(not(windows))]
        {
            Self::Symlink
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSkillRoots {
    pub codex: Option<PathBuf>,
    pub claude: Option<PathBuf>,
    pub gemini: Option<PathBuf>,
}

impl RuntimeSkillRoots {
    pub fn from_environment() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let codex_base = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|path| path.join(".codex")));

        Self {
            codex: codex_base.map(|path| path.join("skills")),
            claude: home.as_ref().map(|path| path.join(".claude/skills")),
            gemini: home.as_ref().map(|path| path.join(".gemini/skills")),
        }
    }

    pub fn target_dir(&self, runtime: SkillRuntime) -> Option<&Path> {
        match runtime {
            SkillRuntime::Codex => self.codex.as_deref(),
            SkillRuntime::Claude => self.claude.as_deref(),
            SkillRuntime::Gemini => self.gemini.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredSkill {
    pub directory_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub skill_md_path: String,
    pub source_path: String,
    pub lock_source: Option<String>,
    pub lock_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoSkillCatalog {
    pub repo_root: String,
    pub shared_root: Option<String>,
    pub lockfile_path: Option<String>,
    pub skills: Vec<DiscoveredSkill>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, IntoStaticStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkillInstallState {
    Missing,
    Linked,
    Copied,
    ProjectLocal,
    Drifted,
    Conflict,
}

impl SkillInstallState {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillDoctorEntry {
    pub name: String,
    pub source_path: String,
    pub target_path: String,
    pub state: SkillInstallState,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillDoctorReport {
    pub runtime: SkillRuntime,
    pub policy: SkillRuntimePolicy,
    pub target_dir: Option<String>,
    pub entries: Vec<SkillDoctorEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillLinkReport {
    pub runtime: SkillRuntime,
    pub policy: SkillRuntimePolicy,
    pub mode: SkillLinkMode,
    pub target_dir: String,
    pub linked: Vec<String>,
    pub updated: Vec<String>,
    pub pruned: Vec<String>,
    pub skipped: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn runtime_skill_policy(runtime: SkillRuntime) -> SkillRuntimePolicy {
    let recommended_mode = SkillLinkMode::default_for_platform();
    match runtime {
        SkillRuntime::Gemini => SkillRuntimePolicy {
            runtime,
            discovery: SkillDiscoveryStrategy::RepoLocalPreferred,
            recommended_mode,
            note: "Gemini already discovers project-local `.agents/skills`, so global linking is optional and mainly useful outside the repo root.".to_string(),
        },
        SkillRuntime::Codex => SkillRuntimePolicy {
            runtime,
            discovery: SkillDiscoveryStrategy::GlobalProjection,
            recommended_mode,
            note: "Codex benefits from user-level skill projection; repo-local skills remain the source of truth.".to_string(),
        },
        SkillRuntime::Claude => SkillRuntimePolicy {
            runtime,
            discovery: SkillDiscoveryStrategy::GlobalProjection,
            recommended_mode,
            note: "Claude works well with projected user-level skills; keep repo-local skills as the canonical source.".to_string(),
        },
    }
}

pub fn discover_repo_skills(repo_root: &Path) -> Result<RepoSkillCatalog> {
    let repo_root = fs::canonicalize(repo_root)
        .with_context(|| format!("failed to canonicalize repo root {}", repo_root.display()))?;
    let shared_root = repo_root.join(".agents/skills");
    let lockfile_path = repo_root.join("skills-lock.json");
    let mut diagnostics = Vec::new();
    let mut skills = Vec::new();

    let lockfile = if lockfile_path.exists() {
        Some(load_skills_lock(&lockfile_path, &mut diagnostics)?)
    } else {
        None
    };

    if !shared_root.exists() {
        diagnostics.push(Diagnostic::warning(
            "skills.shared-root.missing",
            "No shared `.agents/skills` directory was detected.",
        ));
        return Ok(RepoSkillCatalog {
            repo_root: repo_root.display().to_string(),
            shared_root: None,
            lockfile_path: lockfile_path
                .exists()
                .then(|| lockfile_path.display().to_string()),
            skills,
            diagnostics,
        });
    }

    let mut seen_names = BTreeSet::new();
    for entry in fs::read_dir(&shared_root)
        .with_context(|| format!("failed to read skill root {}", shared_root.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", shared_root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let directory_name = match path.file_name().and_then(|value| value.to_str()) {
            Some(value) => value.to_string(),
            None => continue,
        };
        let skill_md_path = path.join("SKILL.md");
        if !skill_md_path.exists() {
            diagnostics.push(Diagnostic::warning(
                "skills.skill-missing-entrypoint",
                format!(
                    "Skipping `{}` because it has no `SKILL.md` entry point.",
                    path.display()
                ),
            ));
            continue;
        }

        seen_names.insert(directory_name.clone());
        let frontmatter = match read_skill_frontmatter(&skill_md_path) {
            Ok(frontmatter) => frontmatter,
            Err(error) => {
                diagnostics.push(Diagnostic::warning(
                    "skills.frontmatter.invalid",
                    format!(
                        "Failed to parse frontmatter for `{}`: {error:#}",
                        skill_md_path.display()
                    ),
                ));
                SkillFrontmatter::default()
            }
        };

        if frontmatter
            .description
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            diagnostics.push(Diagnostic::warning(
                "skills.description.missing",
                format!(
                    "Skill `{directory_name}` is missing a description in `SKILL.md` frontmatter."
                ),
            ));
        }

        if let Some(name) = &frontmatter.name
            && name != &directory_name
        {
            diagnostics.push(Diagnostic::warning(
                "skills.name.mismatch",
                format!(
                    "Skill `{directory_name}` declares frontmatter name `{name}`; these should match for best interoperability."
                ),
            ));
        }

        let locked = lockfile
            .as_ref()
            .and_then(|file| file.skills.get(&directory_name));

        skills.push(DiscoveredSkill {
            directory_name: directory_name.clone(),
            name: frontmatter.name,
            description: frontmatter.description,
            skill_md_path: skill_md_path.display().to_string(),
            source_path: path.display().to_string(),
            lock_source: locked.map(|skill| skill.source.clone()),
            lock_hash: locked.and_then(|skill| skill.computed_hash.clone()),
        });
    }

    if let Some(lockfile) = &lockfile {
        for skill_name in lockfile.skills.keys() {
            if !seen_names.contains(skill_name) {
                diagnostics.push(Diagnostic::warning(
                    "skills.lockfile.extra-entry",
                    format!(
                        "`skills-lock.json` contains `{skill_name}`, but the shared skill directory is missing it."
                    ),
                ));
            }
        }
    }

    skills.sort_by(|left, right| left.directory_name.cmp(&right.directory_name));

    Ok(RepoSkillCatalog {
        repo_root: repo_root.display().to_string(),
        shared_root: Some(shared_root.display().to_string()),
        lockfile_path: lockfile_path
            .exists()
            .then(|| lockfile_path.display().to_string()),
        skills,
        diagnostics,
    })
}

pub fn doctor_repo_skills(
    catalog: &RepoSkillCatalog,
    runtime: SkillRuntime,
    roots: &RuntimeSkillRoots,
) -> Result<SkillDoctorReport> {
    let policy = runtime_skill_policy(runtime);
    let target_dir = roots.target_dir(runtime).map(Path::to_path_buf);
    let mut diagnostics = Vec::new();
    let mut entries = Vec::new();

    if catalog.skills.is_empty() {
        diagnostics.push(Diagnostic::warning(
            "skills.catalog.empty",
            "No shared skills were discovered for this repo.",
        ));
    }

    let Some(target_dir) = target_dir else {
        diagnostics.push(Diagnostic::error(
            "skills.target-dir.unresolved",
            format!("Could not resolve the `{runtime}` user skill directory."),
        ));
        return Ok(SkillDoctorReport {
            runtime,
            policy,
            target_dir: None,
            entries,
            diagnostics,
        });
    };

    for skill in &catalog.skills {
        let target_path = target_dir.join(&skill.directory_name);
        let state = match determine_install_state(skill, &target_path)? {
            SkillInstallState::Missing
                if policy.discovery == SkillDiscoveryStrategy::RepoLocalPreferred =>
            {
                diagnostics.push(Diagnostic::info(
                    "skills.runtime.project-local",
                    format!(
                        "Skill `{}` should already be discoverable by `{runtime}` from the repo-local `.agents/skills` directory.",
                        skill.directory_name
                    ),
                ));
                SkillInstallState::ProjectLocal
            }
            state => state,
        };
        if state == SkillInstallState::Missing {
            diagnostics.push(Diagnostic::warning(
                "skills.runtime.missing",
                format!(
                    "Skill `{}` is not linked into the `{runtime}` runtime yet.",
                    skill.directory_name
                ),
            ));
        } else if state == SkillInstallState::Drifted {
            diagnostics.push(Diagnostic::warning(
                "skills.runtime.drifted",
                format!(
                    "Skill `{}` exists in `{runtime}` but has drifted from the shared repo version.",
                    skill.directory_name
                ),
            ));
        } else if state == SkillInstallState::Conflict {
            diagnostics.push(Diagnostic::warning(
                "skills.runtime.conflict",
                format!(
                    "Skill `{}` conflicts with an existing `{runtime}` target at {}.",
                    skill.directory_name,
                    target_path.display()
                ),
            ));
        }

        entries.push(SkillDoctorEntry {
            name: skill.directory_name.clone(),
            source_path: skill.source_path.clone(),
            target_path: target_path.display().to_string(),
            state,
        });
    }

    Ok(SkillDoctorReport {
        runtime,
        policy,
        target_dir: Some(target_dir.display().to_string()),
        entries,
        diagnostics,
    })
}

pub fn link_repo_skills(
    catalog: &RepoSkillCatalog,
    runtime: SkillRuntime,
    roots: &RuntimeSkillRoots,
    mode: SkillLinkMode,
) -> Result<SkillLinkReport> {
    reconcile_repo_skills(catalog, runtime, roots, mode, ReconcileIntent::Link)
}

pub fn sync_repo_skills(
    catalog: &RepoSkillCatalog,
    runtime: SkillRuntime,
    roots: &RuntimeSkillRoots,
    mode: SkillLinkMode,
) -> Result<SkillLinkReport> {
    reconcile_repo_skills(catalog, runtime, roots, mode, ReconcileIntent::Sync)
}

fn reconcile_repo_skills(
    catalog: &RepoSkillCatalog,
    runtime: SkillRuntime,
    roots: &RuntimeSkillRoots,
    mode: SkillLinkMode,
    intent: ReconcileIntent,
) -> Result<SkillLinkReport> {
    let policy = runtime_skill_policy(runtime);
    let Some(target_dir) = roots.target_dir(runtime) else {
        bail!("could not resolve the `{runtime}` user skill directory");
    };
    fs::create_dir_all(target_dir).with_context(|| {
        format!(
            "failed to create `{runtime}` skill directory at {}",
            target_dir.display()
        )
    })?;

    let mut linked = Vec::new();
    let mut updated = Vec::new();
    let mut pruned = Vec::new();
    let mut skipped = Vec::new();
    let mut diagnostics = Vec::new();

    for skill in &catalog.skills {
        let source_path = Path::new(&skill.source_path);
        let target_path = target_dir.join(&skill.directory_name);
        match determine_install_state(skill, &target_path)? {
            SkillInstallState::Missing => {
                install_skill(source_path, &target_path, mode)?;
                linked.push(skill.directory_name.clone());
            }
            SkillInstallState::Linked | SkillInstallState::Copied => {
                if matches_mode(mode, determine_install_state(skill, &target_path)?) {
                    skipped.push(skill.directory_name.clone());
                } else if intent == ReconcileIntent::Sync {
                    remove_target(&target_path)?;
                    install_skill(source_path, &target_path, mode)?;
                    updated.push(skill.directory_name.clone());
                } else {
                    skipped.push(skill.directory_name.clone());
                }
            }
            SkillInstallState::Drifted => {
                if intent == ReconcileIntent::Sync {
                    remove_target(&target_path)?;
                    install_skill(source_path, &target_path, mode)?;
                    updated.push(skill.directory_name.clone());
                } else {
                    diagnostics.push(Diagnostic::warning(
                        "skills.link.drifted",
                        format!(
                            "Skipped `{}` because the existing runtime copy has drifted from the shared repo skill. Use `skills sync` to repair it.",
                            skill.directory_name
                        ),
                    ));
                    skipped.push(skill.directory_name.clone());
                }
            }
            SkillInstallState::Conflict => {
                diagnostics.push(Diagnostic::warning(
                    "skills.link.conflict",
                    format!(
                        "Skipped `{}` because {} already exists and does not match the shared repo skill.",
                        skill.directory_name,
                        target_path.display()
                    ),
                ));
                skipped.push(skill.directory_name.clone());
            }
            SkillInstallState::ProjectLocal => {
                skipped.push(skill.directory_name.clone());
            }
        }
    }

    if intent == ReconcileIntent::Sync {
        let shared_root = catalog.shared_root.as_ref().map(PathBuf::from);
        let known_names = catalog
            .skills
            .iter()
            .map(|skill| skill.directory_name.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(shared_root) = shared_root {
            for entry in fs::read_dir(target_dir).with_context(|| {
                format!("failed to read runtime skill dir {}", target_dir.display())
            })? {
                let entry = entry
                    .with_context(|| format!("failed to read entry in {}", target_dir.display()))?;
                let path = entry.path();
                let name = match path.file_name().and_then(|value| value.to_str()) {
                    Some(value) => value.to_string(),
                    None => continue,
                };
                if known_names.contains(name.as_str()) {
                    continue;
                }
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if metadata.file_type().is_symlink() {
                    let linked_to = fs::read_link(&path)
                        .with_context(|| format!("failed to read symlink {}", path.display()))?;
                    let resolved = if linked_to.is_absolute() {
                        linked_to
                    } else {
                        path.parent().unwrap_or(target_dir).join(linked_to)
                    };
                    if resolved.starts_with(&shared_root) {
                        remove_target(&path)?;
                        pruned.push(name);
                    }
                }
            }
        }
    }

    Ok(SkillLinkReport {
        runtime,
        policy,
        mode,
        target_dir: target_dir.display().to_string(),
        linked,
        updated,
        pruned,
        skipped,
        diagnostics,
    })
}

fn determine_install_state(
    skill: &DiscoveredSkill,
    target_path: &Path,
) -> Result<SkillInstallState> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SkillInstallState::Missing);
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect runtime skill target {}",
                    target_path.display()
                )
            });
        }
    };

    if metadata.file_type().is_symlink() {
        let linked_to = fs::read_link(target_path)
            .with_context(|| format!("failed to read symlink target {}", target_path.display()))?;
        let resolved = if linked_to.is_absolute() {
            linked_to
        } else {
            target_path.parent().unwrap_or(target_path).join(linked_to)
        };
        if same_path(&resolved, Path::new(&skill.source_path))? {
            return Ok(SkillInstallState::Linked);
        }
        return Ok(SkillInstallState::Conflict);
    }

    if metadata.is_dir() {
        let target_skill = target_path.join("SKILL.md");
        if target_skill.exists()
            && same_directory_contents(Path::new(&skill.source_path), target_path)?
        {
            return Ok(SkillInstallState::Copied);
        }
        if target_skill.exists() {
            return Ok(SkillInstallState::Drifted);
        }
    }

    Ok(SkillInstallState::Conflict)
}

fn install_skill(source_path: &Path, target_path: &Path, mode: SkillLinkMode) -> Result<()> {
    match mode {
        SkillLinkMode::Symlink => create_symlink(source_path, target_path),
        SkillLinkMode::Copy => copy_dir_all(source_path, target_path),
    }
}

fn create_symlink(source_path: &Path, target_path: &Path) -> Result<()> {
    create_dir_symlink(source_path, target_path).with_context(|| {
        format!(
            "failed to create skill symlink from {} to {}",
            source_path.display(),
            target_path.display()
        )
    })
}

fn matches_mode(mode: SkillLinkMode, state: SkillInstallState) -> bool {
    matches!(
        (mode, state),
        (SkillLinkMode::Symlink, SkillInstallState::Linked)
            | (SkillLinkMode::Copy, SkillInstallState::Copied)
    )
}

fn remove_target(target_path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(target_path).with_context(|| {
        format!(
            "failed to inspect runtime skill target {}",
            target_path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(target_path)
            .with_context(|| format!("failed to remove {}", target_path.display()))?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(target_path)
            .with_context(|| format!("failed to remove {}", target_path.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source_path, target_path)
}

#[cfg(windows)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source_path, target_path)
}

fn copy_dir_all(source_path: &Path, target_path: &Path) -> Result<()> {
    fs::create_dir_all(target_path).with_context(|| {
        format!(
            "failed to create copied skill directory {}",
            target_path.display()
        )
    })?;

    for entry in fs::read_dir(source_path)
        .with_context(|| format!("failed to read skill source {}", source_path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source_path.display()))?;
        let source = entry.path();
        let target = target_path.join(entry.file_name());
        if source.is_dir() {
            copy_dir_all(&source, &target)?;
        } else {
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "failed to copy skill file from {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
        }
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> Result<bool> {
    let left = fs::canonicalize(left)
        .with_context(|| format!("failed to canonicalize {}", left.display()))?;
    let right = fs::canonicalize(right)
        .with_context(|| format!("failed to canonicalize {}", right.display()))?;
    Ok(left == right)
}

fn same_directory_contents(left: &Path, right: &Path) -> Result<bool> {
    Ok(directory_fingerprint(left)? == directory_fingerprint(right)?)
}

fn directory_fingerprint(path: &Path) -> Result<String> {
    let mut entries = Vec::new();
    collect_directory_entries(path, path, &mut entries)?;
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_directory_entries(root: &Path, path: &Path, entries: &mut Vec<String>) -> Result<()> {
    let mut children = fs::read_dir(path)
        .with_context(|| format!("failed to read directory {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entries in {}", path.display()))?;
    children.sort_by_key(|entry| entry.file_name());

    for entry in children {
        let child_path = entry.path();
        let relative = child_path
            .strip_prefix(root)
            .unwrap_or(&child_path)
            .display()
            .to_string();
        let metadata = fs::symlink_metadata(&child_path)
            .with_context(|| format!("failed to inspect {}", child_path.display()))?;
        if metadata.file_type().is_symlink() {
            let link = fs::read_link(&child_path)
                .with_context(|| format!("failed to read symlink {}", child_path.display()))?;
            entries.push(format!("symlink:{relative}:{}", link.display()));
        } else if metadata.is_dir() {
            entries.push(format!("dir:{relative}"));
            collect_directory_entries(root, &child_path, entries)?;
        } else {
            entries.push(format!("file:{relative}:{}", file_hash(&child_path)?));
        }
    }

    Ok(())
}

fn file_hash(path: &Path) -> Result<String> {
    let contents =
        fs::read(path).with_context(|| format!("failed to read file {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(contents);
    Ok(format!("{:x}", hasher.finalize()))
}

fn load_skills_lock(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> Result<SkillsLockFile> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read skills lockfile {}", path.display()))?;
    match serde_json::from_str::<SkillsLockFile>(&contents) {
        Ok(lockfile) => Ok(lockfile),
        Err(error) => {
            diagnostics.push(Diagnostic::warning(
                "skills.lockfile.invalid",
                format!("Failed to parse `{}`: {error:#}", path.display()),
            ));
            Ok(SkillsLockFile::default())
        }
    }
}

fn read_skill_frontmatter(path: &Path) -> Result<SkillFrontmatter> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read skill file {}", path.display()))?;
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Ok(SkillFrontmatter::default());
    }

    let mut yaml = String::new();
    for line in lines {
        if line.trim() == "---" {
            return serde_yaml::from_str::<SkillFrontmatter>(&yaml)
                .with_context(|| format!("failed to parse frontmatter in {}", path.display()));
        }
        yaml.push_str(line);
        yaml.push('\n');
    }

    Ok(SkillFrontmatter::default())
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillsLockFile {
    skills: BTreeMap<String, LockedSkill>,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedSkill {
    source: String,
    #[serde(rename = "computedHash")]
    computed_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileIntent {
    Link,
    Sync,
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeSkillRoots, SkillDiscoveryStrategy, SkillInstallState, SkillLinkMode, SkillRuntime,
    };
    use super::{discover_repo_skills, doctor_repo_skills, link_repo_skills, sync_repo_skills};
    use crate::diagnostics::DiagnosticSeverity;
    use anyhow::Result;
    use std::fs;
    use std::path::Path;

    #[test]
    fn discovers_shared_skills_and_lock_metadata() -> Result<()> {
        let repo = tempfile::tempdir()?;
        let skill_root = repo.path().join(".agents/skills/rust-skills");
        fs::create_dir_all(&skill_root)?;
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
        )?;
        fs::write(
            repo.path().join("skills-lock.json"),
            r#"{"version":1,"skills":{"rust-skills":{"source":"acme/rust-skills","computedHash":"abc"}}}"#,
        )?;

        let catalog = discover_repo_skills(repo.path())?;
        assert_eq!(catalog.skills.len(), 1);
        let skill = &catalog.skills[0];
        assert_eq!(skill.directory_name, "rust-skills");
        assert_eq!(skill.name.as_deref(), Some("rust-skills"));
        assert_eq!(skill.lock_source.as_deref(), Some("acme/rust-skills"));

        Ok(())
    }

    #[test]
    fn links_skills_into_runtime_directory() -> Result<()> {
        let repo = tempfile::tempdir()?;
        let skill_root = repo.path().join(".agents/skills/rust-skills");
        fs::create_dir_all(&skill_root)?;
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
        )?;

        let runtime_root = tempfile::tempdir()?;
        let roots = RuntimeSkillRoots {
            codex: Some(runtime_root.path().join("codex-skills")),
            claude: None,
            gemini: None,
        };
        let catalog = discover_repo_skills(repo.path())?;
        let report = link_repo_skills(
            &catalog,
            SkillRuntime::Codex,
            &roots,
            SkillLinkMode::Symlink,
        )?;
        assert_eq!(report.linked, vec!["rust-skills".to_string()]);

        let doctor = doctor_repo_skills(&catalog, SkillRuntime::Codex, &roots)?;
        assert_eq!(doctor.entries.len(), 1);
        assert_eq!(doctor.entries[0].state, SkillInstallState::Linked);
        assert!(Path::new(&doctor.entries[0].target_path).exists());

        Ok(())
    }

    #[test]
    fn gemini_doctor_prefers_project_local_skills() -> Result<()> {
        let repo = tempfile::tempdir()?;
        let skill_root = repo.path().join(".agents/skills/rust-skills");
        fs::create_dir_all(&skill_root)?;
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
        )?;

        let runtime_root = tempfile::tempdir()?;
        let roots = RuntimeSkillRoots {
            codex: None,
            claude: None,
            gemini: Some(runtime_root.path().join("gemini-skills")),
        };
        let catalog = discover_repo_skills(repo.path())?;
        let doctor = doctor_repo_skills(&catalog, SkillRuntime::Gemini, &roots)?;
        assert_eq!(
            doctor.policy.discovery,
            SkillDiscoveryStrategy::RepoLocalPreferred
        );
        assert_eq!(doctor.entries[0].state, SkillInstallState::ProjectLocal);
        assert!(
            doctor
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Warning)
        );

        Ok(())
    }

    #[test]
    fn sync_repairs_drifted_copied_skills() -> Result<()> {
        let repo = tempfile::tempdir()?;
        let skill_root = repo.path().join(".agents/skills/rust-skills");
        fs::create_dir_all(skill_root.join("references"))?;
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
        )?;
        fs::write(skill_root.join("references/rules.md"), "rule-a\n")?;

        let runtime_root = tempfile::tempdir()?;
        let roots = RuntimeSkillRoots {
            codex: Some(runtime_root.path().join("codex-skills")),
            claude: None,
            gemini: None,
        };
        let catalog = discover_repo_skills(repo.path())?;
        link_repo_skills(&catalog, SkillRuntime::Codex, &roots, SkillLinkMode::Copy)?;

        fs::write(
            runtime_root
                .path()
                .join("codex-skills/rust-skills/references/rules.md"),
            "drifted\n",
        )?;

        let report = sync_repo_skills(&catalog, SkillRuntime::Codex, &roots, SkillLinkMode::Copy)?;
        assert_eq!(report.updated, vec!["rust-skills".to_string()]);

        let doctor = doctor_repo_skills(&catalog, SkillRuntime::Codex, &roots)?;
        assert_eq!(doctor.entries[0].state, SkillInstallState::Copied);

        Ok(())
    }
}
