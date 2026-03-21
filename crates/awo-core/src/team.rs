use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, IntoStaticStr};

mod storage;

pub use storage::{
    TeamManifestGuard, default_team_manifest_path, list_team_manifest_paths, load_team_manifest,
    save_team_manifest,
};

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

#[cfg(test)]
mod tests;
