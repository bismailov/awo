use crate::diagnostics::Diagnostic;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use strum_macros::{Display, EnumString, IntoStaticStr};

mod catalog;
mod install;

pub use catalog::discover_repo_skills;
use install::{determine_install_state, install_skill, matches_mode, remove_target};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileIntent {
    Link,
    Sync,
}

#[cfg(test)]
mod tests;
