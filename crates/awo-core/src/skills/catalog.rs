use super::{DiscoveredSkill, RepoSkillCatalog};
use crate::diagnostics::Diagnostic;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
