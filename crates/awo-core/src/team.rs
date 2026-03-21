use crate::app::AppPaths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamExecutionMode {
    ExternalSlots,
    InlineSubagents,
    MultiSessionTeam,
}

impl TeamExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExternalSlots => "external_slots",
            Self::InlineSubagents => "inline_subagents",
            Self::MultiSessionTeam => "multi_session_team",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamStatus {
    Planning,
    Running,
    Blocked,
    Complete,
}

impl TeamStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskCardState {
    Todo,
    InProgress,
    Review,
    Blocked,
    Done,
}

impl TaskCardState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Review => "review",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    pub member_id: String,
    pub role: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub execution_mode: TeamExecutionMode,
    pub slot_id: Option<String>,
    pub branch_name: Option<String>,
    pub read_only: bool,
    pub write_scope: Vec<String>,
    pub context_packs: Vec<String>,
    pub skills: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCard {
    pub task_id: String,
    pub title: String,
    pub summary: String,
    pub owner_id: String,
    pub runtime: Option<String>,
    pub slot_id: Option<String>,
    pub branch_name: Option<String>,
    pub read_only: bool,
    pub write_scope: Vec<String>,
    pub deliverable: String,
    pub verification: Vec<String>,
    pub depends_on: Vec<String>,
    pub state: TaskCardState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamManifest {
    pub version: u32,
    pub team_id: String,
    pub repo_id: String,
    pub objective: String,
    pub status: TeamStatus,
    pub lead: TeamMember,
    pub members: Vec<TeamMember>,
    pub tasks: Vec<TaskCard>,
}

impl TeamManifest {
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 {
            anyhow::bail!("team manifest version must be greater than zero");
        }
        if self.team_id.trim().is_empty() {
            anyhow::bail!("team manifest requires a non-empty team_id");
        }
        if self.repo_id.trim().is_empty() {
            anyhow::bail!("team manifest requires a non-empty repo_id");
        }
        if self.objective.trim().is_empty() {
            anyhow::bail!("team manifest requires a non-empty objective");
        }

        let mut member_ids = std::collections::BTreeSet::new();
        member_ids.insert(self.lead.member_id.as_str());
        for member in &self.members {
            if !member_ids.insert(member.member_id.as_str()) {
                anyhow::bail!("duplicate team member id `{}`", member.member_id);
            }
        }

        for task in &self.tasks {
            if !member_ids.contains(task.owner_id.as_str()) {
                anyhow::bail!(
                    "task `{}` references unknown owner `{}`",
                    task.task_id,
                    task.owner_id
                );
            }
        }

        Ok(())
    }
}

pub fn default_team_manifest_path(paths: &AppPaths, team_id: &str) -> PathBuf {
    paths.teams_dir.join(format!("{team_id}.toml"))
}

pub fn save_team_manifest(paths: &AppPaths, manifest: &TeamManifest) -> Result<PathBuf> {
    manifest.validate()?;
    fs::create_dir_all(&paths.teams_dir).with_context(|| {
        format!(
            "failed to create team manifest dir at {}",
            paths.teams_dir.display()
        )
    })?;
    let path = default_team_manifest_path(paths, &manifest.team_id);
    let contents = toml::to_string_pretty(manifest).context("failed to serialize team manifest")?;
    fs::write(&path, contents)
        .with_context(|| format!("failed to write team manifest at {}", path.display()))?;
    Ok(path)
}

pub fn load_team_manifest(path: &Path) -> Result<TeamManifest> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read team manifest at {}", path.display()))?;
    let manifest = toml::from_str::<TeamManifest>(&contents)
        .with_context(|| format!("failed to parse team manifest at {}", path.display()))?;
    manifest.validate()?;
    Ok(manifest)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppPaths;

    fn sample_paths(root: &Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            state_db_path: root.join("data/state.sqlite3"),
            logs_dir: root.join("data/logs"),
            repos_dir: root.join("config/repos"),
            clones_dir: root.join("data/clones"),
            teams_dir: root.join("config/teams"),
        }
    }

    fn sample_manifest() -> TeamManifest {
        TeamManifest {
            version: 1,
            team_id: "team-alpha".to_string(),
            repo_id: "repo-1".to_string(),
            objective: "Ship a safe parallel implementation".to_string(),
            status: TeamStatus::Planning,
            lead: TeamMember {
                member_id: "lead".to_string(),
                role: "lead".to_string(),
                runtime: Some("claude".to_string()),
                model: Some("sonnet".to_string()),
                execution_mode: TeamExecutionMode::ExternalSlots,
                slot_id: None,
                branch_name: None,
                read_only: true,
                write_scope: Vec::new(),
                context_packs: vec!["architecture".to_string()],
                skills: vec!["planning-with-files".to_string()],
                notes: None,
            },
            members: vec![TeamMember {
                member_id: "worker-a".to_string(),
                role: "implementer".to_string(),
                runtime: Some("codex".to_string()),
                model: None,
                execution_mode: TeamExecutionMode::ExternalSlots,
                slot_id: Some("slot-1".to_string()),
                branch_name: Some("awo/worker-a".to_string()),
                read_only: false,
                write_scope: vec!["src/runtime.rs".to_string()],
                context_packs: vec!["architecture".to_string()],
                skills: vec!["rust-skills".to_string()],
                notes: Some("Owns runtime changes".to_string()),
            }],
            tasks: vec![TaskCard {
                task_id: "task-1".to_string(),
                title: "Implement running-session persistence".to_string(),
                summary: "Persist the session before one-shot completion.".to_string(),
                owner_id: "worker-a".to_string(),
                runtime: Some("codex".to_string()),
                slot_id: Some("slot-1".to_string()),
                branch_name: Some("awo/worker-a".to_string()),
                read_only: false,
                write_scope: vec!["crates/awo-core/src/runtime.rs".to_string()],
                deliverable: "A tested runtime/session patch".to_string(),
                verification: vec!["cargo test".to_string()],
                depends_on: Vec::new(),
                state: TaskCardState::Todo,
            }],
        }
    }

    #[test]
    fn saves_and_loads_team_manifest() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let paths = sample_paths(temp_dir.path());
        let manifest = sample_manifest();

        let path = save_team_manifest(&paths, &manifest)?;
        let loaded = load_team_manifest(&path)?;
        assert_eq!(loaded, manifest);

        let manifests = list_team_manifest_paths(&paths)?;
        assert_eq!(manifests, vec![path]);

        Ok(())
    }

    #[test]
    fn manifest_validation_rejects_unknown_task_owner() {
        let mut manifest = sample_manifest();
        manifest.tasks[0].owner_id = "missing".to_string();
        assert!(manifest.validate().is_err());
    }
}
