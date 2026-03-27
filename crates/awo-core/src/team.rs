use crate::awo_bail;
use crate::error::{AwoError, AwoResult};
use crate::runtime::SessionStatus;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, IntoStaticStr};

pub mod reconcile;
mod storage;

pub use reconcile::{
    build_team_teardown_plan, collect_bound_slot_ids, reconcile_team_manifest_state,
};
pub use storage::{
    TeamManifestGuard, default_team_manifest_path, list_team_manifest_paths, load_team_manifest,
    remove_team_manifest, save_team_manifest,
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TeamExecutionMode {
    #[serde(alias = "ExternalSlots")]
    ExternalSlots,
    #[serde(alias = "InlineSubagents")]
    InlineSubagents,
    #[serde(alias = "MultiSessionTeam")]
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
    #[serde(alias = "Planning")]
    Planning,
    #[serde(alias = "Running")]
    Running,
    #[serde(alias = "Blocked")]
    Blocked,
    #[serde(alias = "Complete")]
    Complete,
    #[serde(alias = "Archived")]
    Archived,
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
    #[serde(alias = "Todo")]
    Todo,
    #[serde(alias = "InProgress")]
    InProgress,
    #[serde(alias = "Review")]
    Review,
    #[serde(alias = "Blocked")]
    Blocked,
    #[serde(alias = "Done")]
    Done,
    #[serde(alias = "Cancelled")]
    Cancelled,
    #[serde(alias = "Superseded")]
    Superseded,
}

impl TaskCardState {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    pub fn is_archive_terminal(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Blocked | Self::Cancelled | Self::Superseded
        )
    }

    pub fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled | Self::Superseded)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PlanItemState {
    #[serde(alias = "Draft")]
    Draft,
    #[serde(alias = "Approved")]
    Approved,
    #[serde(alias = "Generated")]
    Generated,
}

impl PlanItemState {
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
    pub fallback_runtime: Option<String>,
    pub fallback_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_preferences: Option<crate::routing::RoutingPreferences>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub plan_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub read_only: bool,
    pub write_scope: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliverable: Option<String>,
    #[serde(default)]
    pub verification: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub state: PlanItemState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCard {
    pub task_id: String,
    pub title: String,
    pub summary: String,
    pub owner_id: String,
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub slot_id: Option<String>,
    pub branch_name: Option<String>,
    pub read_only: bool,
    pub write_scope: Vec<String>,
    pub deliverable: String,
    pub verification: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_command: Option<String>,
    pub depends_on: Vec<String>,
    pub state: TaskCardState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_log_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskStartOptions {
    pub team_id: String,
    pub task_id: String,
    pub strategy: String,
    pub dry_run: bool,
    pub launch_mode: String,
    pub attach_context: bool,
    pub routing_preferences: Option<crate::routing::RoutingPreferences>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationContext {
    /// The member_id of the worker being delegated to.
    pub target_member_id: String,
    /// Free-form notes from the lead to prepend to the worker's prompt.
    pub lead_notes: Option<String>,
    /// Specific files the lead wants the worker to focus on.
    pub focus_files: Vec<String>,
    /// Whether to auto-start a session after delegation.
    pub auto_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskDelegateOptions {
    pub team_id: String,
    pub task_id: String,
    pub delegation: DelegationContext,
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
    pub model: Option<String>,
    pub routing_source: crate::routing::RoutingSource,
    pub routing_reason: String,
    pub slot_id: String,
    pub branch_name: String,
    pub session_id: Option<String>,
    pub session_status: SessionStatus,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_preferences: Option<crate::routing::RoutingPreferences>,
    pub lead: TeamMember,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_lead_member_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_lead_session_id: Option<String>,
    #[serde(default)]
    pub plan_items: Vec<PlanItem>,
    pub members: Vec<TeamMember>,
    pub tasks: Vec<TaskCard>,
}

impl TeamManifest {
    pub fn validate(&self) -> AwoResult<()> {
        if self.version == 0 {
            awo_bail!("team manifest version must be greater than zero");
        }
        if self.team_id.trim().is_empty() {
            awo_bail!("team manifest requires a non-empty team_id");
        }
        if self.repo_id.trim().is_empty() {
            awo_bail!("team manifest requires a non-empty repo_id");
        }
        if self.objective.trim().is_empty() {
            awo_bail!("team manifest requires a non-empty objective");
        }

        let mut member_ids = std::collections::BTreeSet::new();
        member_ids.insert(self.lead.member_id.as_str());
        for member in &self.members {
            if member.member_id.trim().is_empty() {
                awo_bail!("team members require non-empty member ids");
            }
            if !member_ids.insert(member.member_id.as_str()) {
                awo_bail!("duplicate team member id `{}`", member.member_id);
            }
        }

        if let Some(member_id) = self.current_lead_member_id.as_deref()
            && !member_ids.contains(member_id)
        {
            awo_bail!("current lead references unknown member `{member_id}`");
        }

        let mut task_ids = std::collections::BTreeSet::new();
        for task in &self.tasks {
            if task.task_id.trim().is_empty() {
                awo_bail!("task cards require non-empty task ids");
            }
            if !task_ids.insert(task.task_id.as_str()) {
                awo_bail!("duplicate task id `{}`", task.task_id);
            }
            if !member_ids.contains(task.owner_id.as_str()) {
                awo_bail!(
                    "task `{}` references unknown owner `{}`",
                    task.task_id,
                    task.owner_id
                );
            }
        }

        let mut plan_ids = std::collections::BTreeSet::new();
        for plan in &self.plan_items {
            if plan.plan_id.trim().is_empty() {
                awo_bail!("plan items require non-empty plan ids");
            }
            if !plan_ids.insert(plan.plan_id.as_str()) {
                awo_bail!("duplicate plan item id `{}`", plan.plan_id);
            }
            if let Some(owner_id) = plan.owner_id.as_deref()
                && !member_ids.contains(owner_id)
            {
                awo_bail!(
                    "plan item `{}` references unknown owner `{}`",
                    plan.plan_id,
                    owner_id
                );
            }
            match (plan.state, plan.generated_task_id.as_deref()) {
                (PlanItemState::Generated, Some(task_id)) => {
                    if !task_ids.contains(task_id) {
                        awo_bail!(
                            "plan item `{}` references unknown generated task `{}`",
                            plan.plan_id,
                            task_id
                        );
                    }
                }
                (PlanItemState::Generated, None) => {
                    awo_bail!(
                        "plan item `{}` must reference a generated task when state is `generated`",
                        plan.plan_id
                    );
                }
                (_, Some(_)) => {
                    awo_bail!(
                        "plan item `{}` can only set generated_task_id when state is `generated`",
                        plan.plan_id
                    );
                }
                (_, None) => {}
            }
        }

        for task in &self.tasks {
            for dependency in &task.depends_on {
                if !task_ids.contains(dependency.as_str()) {
                    awo_bail!(
                        "task `{}` depends on unknown task `{}`",
                        task.task_id,
                        dependency
                    );
                }
            }

            match (task.state, task.superseded_by_task_id.as_deref()) {
                (TaskCardState::Superseded, Some(replacement_id)) => {
                    if replacement_id == task.task_id {
                        awo_bail!("task `{}` cannot supersede itself", task.task_id);
                    }
                    if !task_ids.contains(replacement_id) {
                        awo_bail!(
                            "task `{}` is superseded by unknown task `{}`",
                            task.task_id,
                            replacement_id
                        );
                    }
                }
                (TaskCardState::Superseded, None) => {
                    awo_bail!(
                        "task `{}` must reference a replacement task when superseded",
                        task.task_id
                    );
                }
                (_, Some(_)) => {
                    awo_bail!(
                        "task `{}` can only set superseded_by_task_id when state is `superseded`",
                        task.task_id
                    );
                }
                (_, None) => {}
            }
        }

        Ok(())
    }

    pub fn current_lead_member_id(&self) -> &str {
        self.current_lead_member_id
            .as_deref()
            .unwrap_or(self.lead.member_id.as_str())
    }

    pub fn current_lead_member(&self) -> Option<&TeamMember> {
        self.member(self.current_lead_member_id())
    }

    pub fn current_lead_session_id(&self) -> Option<&str> {
        self.current_lead_session_id.as_deref()
    }

    pub fn promote_current_lead(&mut self, member_id: &str) -> AwoResult<()> {
        if self.member(member_id).is_none() {
            awo_bail!("unknown team member `{member_id}`");
        }
        self.current_lead_member_id = Some(member_id.to_string());
        self.current_lead_session_id = None;
        self.validate()
    }

    pub fn bind_current_lead_session(
        &mut self,
        member_id: &str,
        session_id: Option<String>,
    ) -> AwoResult<()> {
        if self.current_lead_member_id() != member_id {
            awo_bail!(
                "cannot bind current lead session for `{member_id}` because current lead is `{}`",
                self.current_lead_member_id()
            );
        }
        self.current_lead_session_id = session_id;
        self.validate()
    }

    pub fn clear_current_lead_session(&mut self) {
        self.current_lead_session_id = None;
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

    pub fn plan_item(&self, plan_id: &str) -> Option<&PlanItem> {
        self.plan_items.iter().find(|plan| plan.plan_id == plan_id)
    }

    pub fn plan_item_mut(&mut self, plan_id: &str) -> Option<&mut PlanItem> {
        self.plan_items
            .iter_mut()
            .find(|plan| plan.plan_id == plan_id)
    }

    pub fn update_member_policy(
        &mut self,
        member_id: &str,
        runtime: Option<Option<String>>,
        model: Option<Option<String>>,
        fallback_runtime: Option<Option<String>>,
        fallback_model: Option<Option<String>>,
        routing_preferences: Option<Option<crate::routing::RoutingPreferences>>,
    ) -> AwoResult<()> {
        let member = self
            .member_mut(member_id)
            .ok_or_else(|| AwoError::validation("unknown team member `{member_id}`"))?;
        if let Some(value) = runtime {
            member.runtime = value;
        }
        if let Some(value) = model {
            member.model = value;
        }
        if let Some(value) = fallback_runtime {
            member.fallback_runtime = value;
        }
        if let Some(value) = fallback_model {
            member.fallback_model = value;
        }
        if let Some(value) = routing_preferences {
            member.routing_preferences = value;
        }
        self.validate()
    }

    pub fn add_member(&mut self, member: TeamMember) -> AwoResult<()> {
        if self.member(&member.member_id).is_some() {
            awo_bail!("team member `{}` already exists", member.member_id);
        }
        self.members.push(member);
        self.validate()
    }

    pub fn remove_member(&mut self, member_id: &str) -> AwoResult<()> {
        if self.current_lead_member_id() == member_id {
            awo_bail!("cannot remove the team lead");
        }
        if self.tasks.iter().any(|task| task.owner_id == member_id) {
            awo_bail!(
                "cannot remove member `{}` while tasks are still assigned",
                member_id
            );
        }

        let original_len = self.members.len();
        self.members.retain(|member| member.member_id != member_id);
        if self.members.len() == original_len {
            awo_bail!("unknown team member `{}`", member_id);
        }

        self.validate()
    }

    pub fn add_task(&mut self, task: TaskCard) -> AwoResult<()> {
        if self.task(&task.task_id).is_some() {
            awo_bail!("task `{}` already exists", task.task_id);
        }
        self.tasks.push(task);
        self.validate()
    }

    pub fn add_plan_item(&mut self, plan: PlanItem) -> AwoResult<()> {
        if self.plan_item(&plan.plan_id).is_some() {
            awo_bail!("plan item `{}` already exists", plan.plan_id);
        }
        self.plan_items.push(plan);
        self.validate()
    }

    pub fn approve_plan_item(&mut self, plan_id: &str) -> AwoResult<()> {
        let plan = self
            .plan_item_mut(plan_id)
            .ok_or_else(|| AwoError::validation("unknown plan item `{plan_id}`"))?;
        if plan.state != PlanItemState::Draft {
            awo_bail!(
                "plan item `{}` must be in `draft` before it can be approved",
                plan_id
            );
        }
        plan.state = PlanItemState::Approved;
        self.validate()
    }

    pub fn generate_task_from_plan_item(&mut self, plan_id: &str, task: TaskCard) -> AwoResult<()> {
        let plan = self
            .plan_item(plan_id)
            .ok_or_else(|| AwoError::validation("unknown plan item `{plan_id}`"))?;
        if plan.state != PlanItemState::Approved {
            awo_bail!(
                "plan item `{}` must be `approved` before generating a task card",
                plan_id
            );
        }

        self.add_task(task.clone())?;
        let plan = self
            .plan_item_mut(plan_id)
            .ok_or_else(|| AwoError::validation("unknown plan item `{plan_id}`"))?;
        plan.state = PlanItemState::Generated;
        plan.generated_task_id = Some(task.task_id);
        self.validate()
    }

    pub fn set_task_state(&mut self, task_id: &str, state: TaskCardState) -> AwoResult<()> {
        let task = self
            .task_mut(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        task.state = state;
        if state != TaskCardState::Superseded {
            task.superseded_by_task_id = None;
        }
        self.refresh_status();
        self.validate()
    }

    pub fn accept_task(&mut self, task_id: &str) -> AwoResult<()> {
        let task = self
            .task(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        if task.state != TaskCardState::Review && task.state != TaskCardState::Done {
            awo_bail!(
                "task `{}` must be in `review` before it can be accepted",
                task_id
            );
        }
        self.set_task_state(task_id, TaskCardState::Done)
    }

    pub fn request_task_rework(&mut self, task_id: &str) -> AwoResult<()> {
        let task = self
            .task(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        if task.state != TaskCardState::Review && task.state != TaskCardState::Done {
            awo_bail!(
                "task `{}` must be in `review` or `done` before it can be sent back for rework",
                task_id
            );
        }
        self.clear_task_result(task_id)?;
        self.set_task_state(task_id, TaskCardState::Todo)
    }

    pub fn cancel_task(&mut self, task_id: &str) -> AwoResult<()> {
        let task = self
            .task(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        if matches!(
            task.state,
            TaskCardState::Done | TaskCardState::Cancelled | TaskCardState::Superseded
        ) {
            awo_bail!(
                "task `{}` cannot be cancelled from `{}`",
                task_id,
                task.state
            );
        }
        self.set_task_state(task_id, TaskCardState::Cancelled)
    }

    pub fn supersede_task(&mut self, task_id: &str, replacement_task_id: &str) -> AwoResult<()> {
        if task_id == replacement_task_id {
            awo_bail!("task `{task_id}` cannot supersede itself");
        }

        let replacement = self
            .task(replacement_task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{replacement_task_id}`"))?;
        if matches!(
            replacement.state,
            TaskCardState::Cancelled | TaskCardState::Superseded
        ) {
            awo_bail!(
                "replacement task `{replacement_task_id}` must not be `{}`",
                replacement.state
            );
        }

        let task = self
            .task_mut(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        if matches!(
            task.state,
            TaskCardState::Cancelled | TaskCardState::Superseded
        ) {
            awo_bail!(
                "task `{}` cannot be superseded from `{}`",
                task_id,
                task.state
            );
        }
        task.state = TaskCardState::Superseded;
        task.superseded_by_task_id = Some(replacement_task_id.to_string());
        self.refresh_status();
        self.validate()
    }

    pub fn assign_member_slot(
        &mut self,
        member_id: &str,
        slot_id: &str,
        branch_name: &str,
    ) -> AwoResult<()> {
        let member = self
            .member_mut(member_id)
            .ok_or_else(|| AwoError::validation("unknown team member `{member_id}`"))?;
        member.slot_id = Some(slot_id.to_string());
        member.branch_name = Some(branch_name.to_string());
        self.validate()
    }

    pub fn bind_task_slot(
        &mut self,
        task_id: &str,
        slot_id: &str,
        branch_name: &str,
    ) -> AwoResult<()> {
        let task = self
            .task_mut(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        task.slot_id = Some(slot_id.to_string());
        task.branch_name = Some(branch_name.to_string());
        self.validate()
    }

    pub fn clear_task_result(&mut self, task_id: &str) -> AwoResult<()> {
        let task = self
            .task_mut(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        task.result_summary = None;
        task.result_session_id = None;
        task.handoff_note = None;
        task.output_log_path = None;
        self.validate()
    }

    pub fn refresh_status(&mut self) {
        if self.status == TeamStatus::Archived {
            return;
        }
        self.status = if self.tasks.is_empty() {
            TeamStatus::Planning
        } else if self.tasks.iter().all(|task| task.state.is_closed()) {
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

    pub fn render_task_prompt(&self, task_id: &str) -> AwoResult<String> {
        let task = self
            .task(task_id)
            .ok_or_else(|| AwoError::validation("unknown task `{task_id}`"))?;
        let owner = self
            .member(&task.owner_id)
            .ok_or_else(|| AwoError::validation(format!("unknown owner `{}`", task.owner_id)))?;

        let mut lines = vec![
            format!("Team objective: {}", self.objective),
            format!("Task id: {}", task.task_id),
            format!("Task title: {}", task.title),
            format!("Task summary: {}", task.summary),
            format!("Owner: {} ({})", owner.member_id, owner.role),
            format!("Execution mode: {}", owner.execution_mode),
            format!("Deliverable: {}", task.deliverable),
        ];

        if task.runtime.is_some()
            || owner.runtime.is_some()
            || task.model.is_some()
            || owner.model.is_some()
        {
            let runtime = task
                .runtime
                .as_deref()
                .or(owner.runtime.as_deref())
                .unwrap_or("-");
            let model = task
                .model
                .as_deref()
                .or(owner.model.as_deref())
                .unwrap_or("-");
            lines.push(format!("Requested runtime/model: {runtime}/{model}"));
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

    pub fn render_delegated_prompt(
        &self,
        task_id: &str,
        delegation: &DelegationContext,
    ) -> AwoResult<String> {
        let mut prompt = self.render_task_prompt(task_id)?;

        if let Some(notes) = &delegation.lead_notes {
            prompt = format!("### Lead Notes\n{}\n\n{}", notes, prompt);
        }

        if !delegation.focus_files.is_empty() {
            let files = delegation
                .focus_files
                .iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n");
            prompt = format!("{}\n\n### Focus Files\n{}", prompt, files);
        }

        Ok(prompt)
    }

    /// Returns a list of reasons why this team cannot be archived, or an empty
    /// vec when archive is safe. Archive requires all tasks to be in a terminal
    /// state and the team must not already be archived.
    pub fn archive_blockers(&self) -> Vec<String> {
        let mut blockers = Vec::new();
        if self.status == TeamStatus::Archived {
            blockers.push("team is already archived".to_string());
        }
        for task in &self.tasks {
            if !task.state.is_archive_terminal() {
                blockers.push(format!(
                    "task `{}` is in non-terminal state `{}`",
                    task.task_id, task.state
                ));
            }
        }
        blockers
    }

    /// Returns true when archive is safe (no blockers).
    pub fn can_archive(&self) -> bool {
        self.archive_blockers().is_empty()
    }

    /// Transition the team to `Archived`. Fails if `can_archive()` is false.
    pub fn archive(&mut self) -> AwoResult<()> {
        let blockers = self.archive_blockers();
        if !blockers.is_empty() {
            awo_bail!(
                "cannot archive team `{}`: {}",
                self.team_id,
                blockers.join("; ")
            );
        }
        self.status = TeamStatus::Archived;
        Ok(())
    }

    /// Returns a summary of what reset will discard so the operator can
    /// make an informed decision. Lists tasks that are not in `Todo` state
    /// and members that have bound slots.
    pub fn reset_summary(&self) -> TeamResetSummary {
        let non_todo_tasks: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.state != TaskCardState::Todo)
            .map(|t| format!("{} ({})", t.task_id, t.state))
            .collect();
        let bound_members: Vec<String> = std::iter::once(&self.lead)
            .chain(self.members.iter())
            .filter(|m| m.slot_id.is_some())
            .map(|m| m.member_id.clone())
            .collect();
        TeamResetSummary {
            non_todo_tasks,
            bound_members,
        }
    }

    /// Reset the team to planning state: all task states become `Todo`, all
    /// slot/branch bindings on tasks and members are cleared, and team status
    /// returns to `Planning`. This makes alpha-stage cleanup practical without
    /// hiding risk — the caller should present `reset_summary()` first.
    pub fn reset(&mut self) {
        for task in &mut self.tasks {
            task.state = TaskCardState::Todo;
            task.slot_id = None;
            task.branch_name = None;
            task.result_summary = None;
            task.result_session_id = None;
            task.handoff_note = None;
            task.output_log_path = None;
            task.superseded_by_task_id = None;
        }
        for plan in &mut self.plan_items {
            plan.state = PlanItemState::Draft;
            plan.generated_task_id = None;
        }
        self.lead.slot_id = None;
        self.lead.branch_name = None;
        for member in &mut self.members {
            member.slot_id = None;
            member.branch_name = None;
        }
        self.current_lead_session_id = None;
        self.status = TeamStatus::Planning;
    }
}

/// Summary of what a reset would discard, presented to the operator before
/// confirming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamResetSummary {
    pub non_todo_tasks: Vec<String>,
    pub bound_members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTeardownPlan {
    pub reset_summary: TeamResetSummary,
    pub bound_slots: Vec<String>,
    pub active_slots: Vec<String>,
    pub dirty_slots: Vec<String>,
    pub cancellable_sessions: Vec<String>,
    pub blocking_sessions: Vec<String>,
}

impl TeamTeardownPlan {
    pub fn has_blockers(&self) -> bool {
        !self.dirty_slots.is_empty() || !self.blocking_sessions.is_empty()
    }

    pub fn requires_confirmation(&self) -> bool {
        self.has_blockers()
            || !self.bound_slots.is_empty()
            || !self.active_slots.is_empty()
            || !self.cancellable_sessions.is_empty()
            || !self.reset_summary.non_todo_tasks.is_empty()
            || !self.reset_summary.bound_members.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTeardownResult {
    pub cancelled_sessions: Vec<String>,
    pub released_slots: Vec<String>,
    pub reset_summary: TeamResetSummary,
}

#[allow(clippy::too_many_arguments)]
pub fn starter_team_manifest(
    repo_id: &str,
    team_id: &str,
    objective: &str,
    lead_runtime: Option<&str>,
    lead_model: Option<&str>,
    execution_mode: TeamExecutionMode,
    fallback_runtime: Option<&str>,
    fallback_model: Option<&str>,
) -> TeamManifest {
    TeamManifest {
        version: 1,
        team_id: team_id.to_string(),
        repo_id: repo_id.to_string(),
        objective: objective.to_string(),
        status: TeamStatus::Planning,
        routing_preferences: None,
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
            fallback_runtime: fallback_runtime.map(str::to_string),
            fallback_model: fallback_model.map(str::to_string),
            routing_preferences: None,
        },
        current_lead_member_id: Some("lead".to_string()),
        current_lead_session_id: None,
        plan_items: Vec::new(),
        members: Vec::new(),
        tasks: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
