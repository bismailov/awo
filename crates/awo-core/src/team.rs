use crate::app::AppPaths;
use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TeamExecutionMode {
    ExternalSlots,
    InlineSubagents,
    MultiSessionTeam,
}

impl TeamExecutionMode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TeamStatus {
    Planning,
    Running,
    Blocked,
    Complete,
}

impl TeamStatus {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskCardState {
    Todo,
    InProgress,
    Review,
    Blocked,
    Done,
}

impl TaskCardState {
    pub fn as_str(self) -> &'static str {
        self.into()
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
pub struct TeamTaskStartOptions {
    pub team_id: String,
    pub task_id: String,
    pub strategy: String,
    pub dry_run: bool,
    pub launch_mode: String,
    pub attach_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskExecution {
    pub team_id: String,
    pub task_id: String,
    pub owner_id: String,
    pub runtime: String,
    pub slot_id: String,
    pub branch_name: String,
    pub session_id: Option<String>,
    pub session_status: String,
    pub acquired_slot: bool,
    pub prompt: String,
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
            if member.member_id.trim().is_empty() {
                anyhow::bail!("team members require non-empty member ids");
            }
            if !member_ids.insert(member.member_id.as_str()) {
                anyhow::bail!("duplicate team member id `{}`", member.member_id);
            }
        }

        let mut task_ids = std::collections::BTreeSet::new();
        for task in &self.tasks {
            if task.task_id.trim().is_empty() {
                anyhow::bail!("task cards require non-empty task ids");
            }
            if !task_ids.insert(task.task_id.as_str()) {
                anyhow::bail!("duplicate task id `{}`", task.task_id);
            }
            if !member_ids.contains(task.owner_id.as_str()) {
                anyhow::bail!(
                    "task `{}` references unknown owner `{}`",
                    task.task_id,
                    task.owner_id
                );
            }
        }

        for task in &self.tasks {
            for dependency in &task.depends_on {
                if !task_ids.contains(dependency.as_str()) {
                    anyhow::bail!(
                        "task `{}` depends on unknown task `{}`",
                        task.task_id,
                        dependency
                    );
                }
            }
        }

        Ok(())
    }

    pub fn member(&self, member_id: &str) -> Option<&TeamMember> {
        if self.lead.member_id == member_id {
            Some(&self.lead)
        } else {
            self.members
                .iter()
                .find(|member| member.member_id == member_id)
        }
    }

    pub fn member_mut(&mut self, member_id: &str) -> Option<&mut TeamMember> {
        if self.lead.member_id == member_id {
            Some(&mut self.lead)
        } else {
            self.members
                .iter_mut()
                .find(|member| member.member_id == member_id)
        }
    }

    pub fn task(&self, task_id: &str) -> Option<&TaskCard> {
        self.tasks.iter().find(|task| task.task_id == task_id)
    }

    pub fn task_mut(&mut self, task_id: &str) -> Option<&mut TaskCard> {
        self.tasks.iter_mut().find(|task| task.task_id == task_id)
    }

    pub fn add_member(&mut self, member: TeamMember) -> Result<()> {
        if self.member(&member.member_id).is_some() {
            anyhow::bail!("team member `{}` already exists", member.member_id);
        }
        self.members.push(member);
        self.validate()
    }

    pub fn remove_member(&mut self, member_id: &str) -> Result<()> {
        if self.lead.member_id == member_id {
            anyhow::bail!("cannot remove the team lead");
        }
        if self.tasks.iter().any(|task| task.owner_id == member_id) {
            anyhow::bail!("cannot remove member `{member_id}` while tasks are still assigned");
        }

        let original_len = self.members.len();
        self.members.retain(|member| member.member_id != member_id);
        if self.members.len() == original_len {
            anyhow::bail!("unknown team member `{member_id}`");
        }

        self.validate()
    }

    pub fn add_task(&mut self, task: TaskCard) -> Result<()> {
        if self.task(&task.task_id).is_some() {
            anyhow::bail!("task `{}` already exists", task.task_id);
        }
        self.tasks.push(task);
        self.validate()
    }

    pub fn set_task_state(&mut self, task_id: &str, state: TaskCardState) -> Result<()> {
        let task = self
            .task_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("unknown task `{task_id}`"))?;
        task.state = state;
        self.refresh_status();
        self.validate()
    }

    pub fn assign_member_slot(
        &mut self,
        member_id: &str,
        slot_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        let member = self
            .member_mut(member_id)
            .ok_or_else(|| anyhow::anyhow!("unknown team member `{member_id}`"))?;
        member.slot_id = Some(slot_id.to_string());
        member.branch_name = Some(branch_name.to_string());
        self.validate()
    }

    pub fn bind_task_slot(
        &mut self,
        task_id: &str,
        slot_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        let task = self
            .task_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("unknown task `{task_id}`"))?;
        task.slot_id = Some(slot_id.to_string());
        task.branch_name = Some(branch_name.to_string());
        self.validate()
    }

    pub fn refresh_status(&mut self) {
        self.status = if self.tasks.is_empty() {
            TeamStatus::Planning
        } else if self
            .tasks
            .iter()
            .all(|task| task.state == TaskCardState::Done)
        {
            TeamStatus::Complete
        } else if self
            .tasks
            .iter()
            .any(|task| task.state == TaskCardState::Blocked)
        {
            TeamStatus::Blocked
        } else if self
            .tasks
            .iter()
            .any(|task| task.state != TaskCardState::Todo)
        {
            TeamStatus::Running
        } else {
            TeamStatus::Planning
        };
    }

    pub fn render_task_prompt(&self, task_id: &str) -> Result<String> {
        let task = self
            .task(task_id)
            .ok_or_else(|| anyhow::anyhow!("unknown task `{task_id}`"))?;
        let owner = self
            .member(&task.owner_id)
            .ok_or_else(|| anyhow::anyhow!("unknown owner `{}`", task.owner_id))?;

        let mut lines = vec![
            format!("Team objective: {}", self.objective),
            format!("Task id: {}", task.task_id),
            format!("Task title: {}", task.title),
            format!("Task summary: {}", task.summary),
            format!("Owner: {} ({})", owner.member_id, owner.role),
            format!("Execution mode: {}", owner.execution_mode),
            format!("Deliverable: {}", task.deliverable),
        ];

        if let Some(runtime) = task.runtime.as_deref().or(owner.runtime.as_deref()) {
            lines.push(format!("Requested runtime: {runtime}"));
        }

        lines.push(if task.read_only || owner.read_only {
            "Mode: read-only".to_string()
        } else {
            "Mode: write-capable".to_string()
        });

        if !task.write_scope.is_empty() {
            lines.push("Write scope:".to_string());
            for path in &task.write_scope {
                lines.push(format!("- {path}"));
            }
        }

        if !task.verification.is_empty() {
            lines.push("Verification:".to_string());
            for command in &task.verification {
                lines.push(format!("- {command}"));
            }
        }

        if !task.depends_on.is_empty() {
            lines.push(format!("Dependencies: {}", task.depends_on.join(", ")));
        }

        if !owner.context_packs.is_empty() {
            lines.push(format!(
                "Preferred context packs: {}",
                owner.context_packs.join(", ")
            ));
        }

        if !owner.skills.is_empty() {
            lines.push(format!("Relevant skills: {}", owner.skills.join(", ")));
        }

        if let Some(notes) = owner.notes.as_deref() {
            lines.push(format!("Owner notes: {notes}"));
        }

        lines.push("When done, summarize what changed, how you verified it, and any blockers or follow-up risk.".to_string());

        Ok(lines.join("\n"))
    }
}

pub fn starter_team_manifest(
    repo_id: &str,
    team_id: &str,
    objective: &str,
    lead_runtime: Option<&str>,
    lead_model: Option<&str>,
    execution_mode: TeamExecutionMode,
) -> TeamManifest {
    TeamManifest {
        version: 1,
        team_id: team_id.to_string(),
        repo_id: repo_id.to_string(),
        objective: objective.to_string(),
        status: TeamStatus::Planning,
        lead: TeamMember {
            member_id: "lead".to_string(),
            role: "lead".to_string(),
            runtime: lead_runtime.map(str::to_string),
            model: lead_model.map(str::to_string),
            execution_mode,
            slot_id: None,
            branch_name: None,
            read_only: true,
            write_scope: Vec::new(),
            context_packs: vec!["entrypoints".to_string()],
            skills: vec!["planning-with-files".to_string()],
            notes: Some("Starter manifest generated by awo.".to_string()),
        },
        members: Vec::new(),
        tasks: Vec::new(),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppPaths;
    use std::sync::{Arc, Barrier};
    use std::thread;

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

    #[test]
    fn manifest_validation_rejects_unknown_dependencies() {
        let mut manifest = sample_manifest();
        manifest.tasks[0].depends_on = vec!["missing-task".to_string()];
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn starter_manifest_defaults_to_planning_lead() {
        let manifest = starter_team_manifest(
            "repo-1",
            "team-alpha",
            "Ship a safe release",
            Some("claude"),
            Some("sonnet"),
            TeamExecutionMode::ExternalSlots,
        );

        assert_eq!(manifest.status, TeamStatus::Planning);
        assert_eq!(manifest.lead.member_id, "lead");
        assert_eq!(manifest.lead.runtime.as_deref(), Some("claude"));
        assert!(manifest.members.is_empty());
        assert!(manifest.tasks.is_empty());
    }

    #[test]
    fn add_member_and_task_render_prompt() -> Result<()> {
        let mut manifest = starter_team_manifest(
            "repo-1",
            "team-alpha",
            "Ship a safe release",
            Some("claude"),
            Some("sonnet"),
            TeamExecutionMode::ExternalSlots,
        );
        manifest.add_member(TeamMember {
            member_id: "worker-a".to_string(),
            role: "implementer".to_string(),
            runtime: Some("codex".to_string()),
            model: None,
            execution_mode: TeamExecutionMode::ExternalSlots,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["src/lib.rs".to_string()],
            context_packs: vec!["architecture".to_string()],
            skills: vec!["rust-skills".to_string()],
            notes: Some("Own the runtime layer.".to_string()),
        })?;
        manifest.add_task(TaskCard {
            task_id: "task-1".to_string(),
            title: "Implement feature".to_string(),
            summary: "Add the missing feature.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: None,
            slot_id: None,
            branch_name: None,
            read_only: false,
            write_scope: vec!["src/lib.rs".to_string()],
            deliverable: "A tested patch".to_string(),
            verification: vec!["cargo test".to_string()],
            depends_on: Vec::new(),
            state: TaskCardState::Todo,
        })?;

        let prompt = manifest.render_task_prompt("task-1")?;
        assert!(prompt.contains("Team objective"));
        assert!(prompt.contains("Relevant skills"));
        assert!(prompt.contains("Verification"));
        Ok(())
    }

    #[test]
    fn concurrent_manifest_mutations_preserve_all_members() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let paths = Arc::new(sample_paths(temp_dir.path()));
        let manifest = starter_team_manifest(
            "repo-1",
            "demo-team",
            "Exercise manifest locking",
            Some("claude"),
            Some("sonnet"),
            TeamExecutionMode::ExternalSlots,
        );
        save_team_manifest(&paths, &manifest)?;

        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for member_id in ["worker-a", "worker-b"] {
            let paths = Arc::clone(&paths);
            let barrier = Arc::clone(&barrier);
            let member_id = member_id.to_string();
            handles.push(thread::spawn(move || -> Result<()> {
                barrier.wait();
                let mut guard = TeamManifestGuard::load(&paths, "demo-team")?;
                guard.manifest_mut().add_member(TeamMember {
                    member_id,
                    role: "reviewer".to_string(),
                    runtime: Some("shell".to_string()),
                    model: None,
                    execution_mode: TeamExecutionMode::ExternalSlots,
                    slot_id: None,
                    branch_name: None,
                    read_only: true,
                    write_scope: Vec::new(),
                    context_packs: Vec::new(),
                    skills: Vec::new(),
                    notes: None,
                })?;
                guard.save()
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().expect("thread panicked")?;
        }

        let loaded = load_team_manifest(&default_team_manifest_path(&paths, "demo-team"))?;
        assert!(loaded.member("worker-a").is_some());
        assert!(loaded.member("worker-b").is_some());
        Ok(())
    }
}
