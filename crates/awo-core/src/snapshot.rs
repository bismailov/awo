use crate::capabilities::{CostTier, RuntimeCapabilityDescriptor, all_runtime_capabilities};
use crate::config::AppConfig;
use crate::context::discover_repo_context;
use crate::diagnostics::DiagnosticSeverity;
use crate::error::AwoResult;
use crate::platform::current_platform_label;
use crate::repo::{RegisteredRepo, remote_label};
use crate::routing::RoutingPreferences;
use crate::runtime::SessionRecord;
use crate::skills::{
    RuntimeSkillRoots, SkillInstallState, SkillRuntime, discover_repo_skills, doctor_repo_skills,
};
use crate::slot::SlotRecord;
use crate::store::Store;
use crate::team::{TeamManifest, list_team_manifest_paths, load_team_manifest};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct CommandLogEntry {
    pub id: i64,
    pub command_name: String,
    pub payload: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub platform_label: String,
    pub config_dir: String,
    pub data_dir: String,
    pub state_db_path: String,
    pub logs_dir: String,
    pub repos_dir: String,
    pub clones_dir: String,
    pub teams_dir: String,
    pub registered_repos: Vec<RepoSummary>,
    pub teams: Vec<TeamSummary>,
    pub runtime_capabilities: Vec<RuntimeCapabilityDescriptor>,
    pub runtime_pressure: String,
    pub slots: Vec<SlotSummary>,
    pub sessions: Vec<SessionSummary>,
    pub review: ReviewSummary,
    pub recent_commands: Vec<CommandLogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoSummary {
    pub id: String,
    pub name: String,
    pub repo_root: String,
    pub default_base_branch: String,
    pub worktree_root: String,
    pub remote_label: String,
    pub shared_manifest_present: bool,
    pub entrypoint_count: usize,
    pub context_pack_count: usize,
    pub shared_skill_count: usize,
    pub mcp_config_present: bool,
    pub context_packs: Vec<RepoContextPackSummary>,
    pub skill_runtimes: Vec<RepoSkillRuntimeSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoContextPackSummary {
    pub name: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoSkillRuntimeSummary {
    pub runtime: String,
    pub strategy: String,
    pub ready: usize,
    pub total: usize,
    pub warnings: usize,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeamSummary {
    pub team_id: String,
    pub repo_id: String,
    pub status: String,
    pub objective: String,
    pub member_count: usize,
    pub write_member_count: usize,
    pub task_count: usize,
    pub open_task_count: usize,
    pub routing_preferences: Option<RoutingPreferencesSummary>,
    pub lead_fallback_runtime: Option<String>,
    pub lead_fallback_model: Option<String>,
    pub lead_runtime: Option<String>,
    pub lead_model: Option<String>,
    pub member_routing: Vec<MemberRoutingSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingPreferencesSummary {
    pub allow_fallback: bool,
    pub prefer_local: bool,
    pub avoid_metered: bool,
    pub max_cost_tier: Option<CostTier>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberRoutingSummary {
    pub member_id: String,
    pub fallback_runtime: Option<String>,
    pub fallback_model: Option<String>,
    pub routing_preferences: Option<RoutingPreferencesSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlotSummary {
    pub id: String,
    pub repo_id: String,
    pub task_name: String,
    pub slot_path: String,
    pub branch_name: String,
    pub strategy: String,
    pub status: String,
    pub dirty: bool,
    pub fingerprint_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub repo_id: String,
    pub slot_id: String,
    pub runtime: String,
    pub supervisor: Option<String>,
    pub status: String,
    pub read_only: bool,
    pub dry_run: bool,
    pub log_path: Option<String>,
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReviewSummary {
    pub dirty_slots: usize,
    pub stale_slots: usize,
    pub active_slots: usize,
    pub releasable_slots: usize,
    pub pending_sessions: usize,
    pub completed_sessions: usize,
    pub failed_sessions: usize,
    pub warnings: Vec<ReviewWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewWarning {
    pub kind: String,
    pub slot_id: Option<String>,
    pub session_id: Option<String>,
    pub message: String,
}

impl AppSnapshot {
    pub fn load(config: &AppConfig, store: &Store) -> AwoResult<Self> {
        let repositories = store.list_repositories()?;
        let slots = store.list_slots(None)?;
        let sessions = store.list_sessions(None)?;
        let review = build_review_summary(&slots, &sessions);
        let teams = list_team_manifest_paths(&config.paths)?
            .into_iter()
            .filter_map(|path| load_team_manifest(&path).ok())
            .map(TeamSummary::from)
            .collect::<Vec<_>>();
        let registered_repos = repositories
            .into_iter()
            .map(build_repo_summary)
            .collect::<Vec<_>>();

        Ok(Self {
            platform_label: current_platform_label().to_string(),
            config_dir: config.paths.config_dir.display().to_string(),
            data_dir: config.paths.data_dir.display().to_string(),
            state_db_path: config.paths.state_db_path.display().to_string(),
            logs_dir: config.paths.logs_dir.display().to_string(),
            repos_dir: config.paths.repos_dir.display().to_string(),
            clones_dir: config.paths.clones_dir.display().to_string(),
            teams_dir: config.paths.teams_dir.display().to_string(),
            registered_repos,
            teams,
            runtime_capabilities: all_runtime_capabilities(),
            runtime_pressure: if config.settings.runtime_pressure_profile.is_empty() {
                "pressure=none".to_string()
            } else {
                let mut entries: Vec<_> = config
                    .settings
                    .runtime_pressure_profile
                    .iter()
                    .map(|(k, v)| format!("{k}={}", v.as_str()))
                    .collect();
                entries.sort();
                format!("pressure: {}", entries.join(", "))
            },
            slots: slots.into_iter().map(SlotSummary::from).collect(),
            sessions: sessions.into_iter().map(SessionSummary::from).collect(),
            review,
            recent_commands: store.recent_actions(10)?,
        })
    }

    pub fn review_for_repo(&self, repo_id: Option<&str>) -> ReviewSummary {
        match repo_id {
            None => self.review.clone(),
            Some(repo_id) => {
                let slots = self
                    .slots
                    .iter()
                    .filter(|slot| slot.repo_id == repo_id)
                    .collect::<Vec<_>>();
                let sessions = self
                    .sessions
                    .iter()
                    .filter(|session| session.repo_id == repo_id)
                    .collect::<Vec<_>>();
                build_review_summary_from_summaries(&slots, &sessions)
            }
        }
    }
}

impl From<SlotRecord> for SlotSummary {
    fn from(value: SlotRecord) -> Self {
        Self {
            id: value.id,
            repo_id: value.repo_id,
            task_name: value.task_name,
            slot_path: value.slot_path,
            branch_name: value.branch_name,
            strategy: value.strategy,
            status: value.status,
            dirty: value.dirty,
            fingerprint_status: value.fingerprint_status,
        }
    }
}

fn build_repo_summary(value: RegisteredRepo) -> RepoSummary {
    let context = discover_repo_context(Path::new(&value.repo_root)).ok();
    let skills = discover_repo_skills(Path::new(&value.repo_root)).ok();
    let roots = RuntimeSkillRoots::from_environment();

    let context_packs = context
        .as_ref()
        .map(|context| {
            context
                .packs
                .iter()
                .map(|pack| RepoContextPackSummary {
                    name: pack.name.clone(),
                    file_count: pack.files.len(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let skill_runtimes = skills
        .as_ref()
        .map(|catalog| {
            SkillRuntime::all()
                .into_iter()
                .filter_map(|runtime| doctor_repo_skills(catalog, runtime, &roots).ok())
                .map(|report| {
                    let ready = report
                        .entries
                        .iter()
                        .filter(|entry| {
                            matches!(
                                entry.state,
                                SkillInstallState::Linked
                                    | SkillInstallState::Copied
                                    | SkillInstallState::ProjectLocal
                            )
                        })
                        .count();
                    let warnings = report
                        .diagnostics
                        .iter()
                        .filter(|diag| diag.severity == DiagnosticSeverity::Warning)
                        .count();
                    RepoSkillRuntimeSummary {
                        runtime: report.runtime.to_string(),
                        strategy: report.policy.discovery.as_str().to_string(),
                        ready,
                        total: report.entries.len(),
                        warnings,
                        note: report.policy.note,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    RepoSummary {
        id: value.id,
        name: value.name,
        repo_root: value.repo_root,
        default_base_branch: value.default_base_branch,
        worktree_root: value.worktree_root,
        remote_label: remote_label(value.remote_url.as_deref()),
        shared_manifest_present: value.shared_manifest_present,
        entrypoint_count: context
            .as_ref()
            .map(|context| context.entrypoints.len())
            .unwrap_or_default(),
        context_pack_count: context
            .as_ref()
            .map(|context| context.packs.len())
            .unwrap_or_default(),
        shared_skill_count: skills
            .as_ref()
            .map(|catalog| catalog.skills.len())
            .unwrap_or_default(),
        mcp_config_present: context
            .as_ref()
            .and_then(|context| context.mcp_config_path.as_ref())
            .is_some(),
        context_packs,
        skill_runtimes,
    }
}

impl From<SessionRecord> for SessionSummary {
    fn from(value: SessionRecord) -> Self {
        Self {
            id: value.id,
            repo_id: value.repo_id,
            slot_id: value.slot_id,
            runtime: value.runtime,
            supervisor: value.supervisor,
            status: value.status,
            read_only: value.read_only,
            dry_run: value.dry_run,
            log_path: value.stdout_path,
            exit_code: value.exit_code,
        }
    }
}

impl From<TeamManifest> for TeamSummary {
    fn from(value: TeamManifest) -> Self {
        let member_count = 1 + value.members.len();
        let write_member_count = usize::from(!value.lead.read_only)
            + value
                .members
                .iter()
                .filter(|member| !member.read_only)
                .count();
        let task_count = value.tasks.len();
        let open_task_count = value
            .tasks
            .iter()
            .filter(|task| task.state.as_str() != "done")
            .count();
        let routing_preferences = value
            .routing_preferences
            .as_ref()
            .map(RoutingPreferencesSummary::from);
        let member_routing = value
            .members
            .iter()
            .filter_map(|member| {
                let routing_preferences = member
                    .routing_preferences
                    .as_ref()
                    .map(RoutingPreferencesSummary::from);
                if member.fallback_runtime.is_none()
                    && member.fallback_model.is_none()
                    && routing_preferences.is_none()
                {
                    None
                } else {
                    Some(MemberRoutingSummary {
                        member_id: member.member_id.clone(),
                        fallback_runtime: member.fallback_runtime.clone(),
                        fallback_model: member.fallback_model.clone(),
                        routing_preferences,
                    })
                }
            })
            .collect();

        Self {
            team_id: value.team_id,
            repo_id: value.repo_id,
            status: value.status.to_string(),
            objective: value.objective,
            member_count,
            write_member_count,
            task_count,
            open_task_count,
            routing_preferences,
            lead_fallback_runtime: value.lead.fallback_runtime,
            lead_fallback_model: value.lead.fallback_model,
            lead_runtime: value.lead.runtime.clone(),
            lead_model: value.lead.model.clone(),
            member_routing,
        }
    }
}

impl From<&RoutingPreferences> for RoutingPreferencesSummary {
    fn from(value: &RoutingPreferences) -> Self {
        Self {
            allow_fallback: value.allow_fallback,
            prefer_local: value.prefer_local,
            avoid_metered: value.avoid_metered,
            max_cost_tier: value.max_cost_tier,
        }
    }
}

impl SessionSummary {
    fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "cancelled")
    }
}

fn build_review_summary(slots: &[SlotRecord], sessions: &[SessionRecord]) -> ReviewSummary {
    let mut summary = ReviewSummary::default();
    let mut pending_sessions_by_slot: HashMap<&str, usize> = HashMap::new();

    for session in sessions {
        if session.status == "completed" {
            summary.completed_sessions += 1;
        } else if session.status == "failed" {
            summary.failed_sessions += 1;
            summary.warnings.push(ReviewWarning {
                kind: "failed-session".to_string(),
                slot_id: Some(session.slot_id.clone()),
                session_id: Some(session.id.clone()),
                message: format!(
                    "Session `{}` on slot `{}` failed and should be reviewed.",
                    session.id, session.slot_id
                ),
            });
        }

        if !session.is_terminal() {
            summary.pending_sessions += 1;
            *pending_sessions_by_slot
                .entry(session.slot_id.as_str())
                .or_insert(0) += 1;
        }
    }

    for slot in slots {
        if slot.status == "active" {
            summary.active_slots += 1;
        }
        if slot.dirty {
            summary.dirty_slots += 1;
        }
        if slot.fingerprint_status == "stale" {
            summary.stale_slots += 1;
        }

        let pending_sessions = pending_sessions_by_slot
            .get(slot.id.as_str())
            .copied()
            .unwrap_or_default();
        if slot.status == "active"
            && !slot.dirty
            && slot.fingerprint_status == "ready"
            && pending_sessions == 0
        {
            summary.releasable_slots += 1;
        }

        if slot.status == "active" && slot.dirty {
            summary.warnings.push(ReviewWarning {
                kind: "dirty-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has uncommitted changes and cannot be safely released.",
                    slot.id
                ),
            });
        }

        if slot.status == "active" && slot.fingerprint_status == "stale" {
            summary.warnings.push(ReviewWarning {
                kind: "stale-active-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` is active but stale relative to the repo fingerprint.",
                    slot.id
                ),
            });
        }

        if slot.strategy == "warm"
            && slot.status == "released"
            && slot.fingerprint_status == "stale"
        {
            summary.warnings.push(ReviewWarning {
                kind: "stale-warm-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Warm slot `{}` is stale and should be refreshed before reuse.",
                    slot.id
                ),
            });
        }

        if slot.status == "missing" {
            summary.warnings.push(ReviewWarning {
                kind: "missing-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` is missing from disk and needs manual cleanup or reacquisition.",
                    slot.id
                ),
            });
        }

        if pending_sessions > 0 {
            summary.warnings.push(ReviewWarning {
                kind: "slot-busy".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has {} pending session(s) and cannot be released yet.",
                    slot.id, pending_sessions
                ),
            });
        }

        if pending_sessions > 1 {
            summary.warnings.push(ReviewWarning {
                kind: "slot-multi-session".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has multiple pending sessions attached; review ownership before continuing.",
                    slot.id
                ),
            });
        }
    }

    summary
}

fn build_review_summary_from_summaries(
    slots: &[&SlotSummary],
    sessions: &[&SessionSummary],
) -> ReviewSummary {
    let mut summary = ReviewSummary::default();
    let mut pending_sessions_by_slot: HashMap<&str, usize> = HashMap::new();

    for session in sessions {
        if session.status == "completed" {
            summary.completed_sessions += 1;
        } else if session.status == "failed" {
            summary.failed_sessions += 1;
            summary.warnings.push(ReviewWarning {
                kind: "failed-session".to_string(),
                slot_id: Some(session.slot_id.clone()),
                session_id: Some(session.id.clone()),
                message: format!(
                    "Session `{}` on slot `{}` failed and should be reviewed.",
                    session.id, session.slot_id
                ),
            });
        }

        if !session.is_terminal() {
            summary.pending_sessions += 1;
            *pending_sessions_by_slot
                .entry(session.slot_id.as_str())
                .or_insert(0) += 1;
        }
    }

    for slot in slots {
        if slot.status == "active" {
            summary.active_slots += 1;
        }
        if slot.dirty {
            summary.dirty_slots += 1;
        }
        if slot.fingerprint_status == "stale" {
            summary.stale_slots += 1;
        }

        let pending_sessions = pending_sessions_by_slot
            .get(slot.id.as_str())
            .copied()
            .unwrap_or_default();
        if slot.status == "active"
            && !slot.dirty
            && slot.fingerprint_status == "ready"
            && pending_sessions == 0
        {
            summary.releasable_slots += 1;
        }

        if slot.status == "active" && slot.dirty {
            summary.warnings.push(ReviewWarning {
                kind: "dirty-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has uncommitted changes and cannot be safely released.",
                    slot.id
                ),
            });
        }

        if slot.status == "active" && slot.fingerprint_status == "stale" {
            summary.warnings.push(ReviewWarning {
                kind: "stale-active-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` is active but stale relative to the repo fingerprint.",
                    slot.id
                ),
            });
        }

        if slot.strategy == "warm"
            && slot.status == "released"
            && slot.fingerprint_status == "stale"
        {
            summary.warnings.push(ReviewWarning {
                kind: "stale-warm-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Warm slot `{}` is stale and should be refreshed before reuse.",
                    slot.id
                ),
            });
        }

        if slot.status == "missing" {
            summary.warnings.push(ReviewWarning {
                kind: "missing-slot".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` is missing from disk and needs manual cleanup or reacquisition.",
                    slot.id
                ),
            });
        }

        if pending_sessions > 0 {
            summary.warnings.push(ReviewWarning {
                kind: "slot-busy".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has {} pending session(s) and cannot be released yet.",
                    slot.id, pending_sessions
                ),
            });
        }

        if pending_sessions > 1 {
            summary.warnings.push(ReviewWarning {
                kind: "slot-multi-session".to_string(),
                slot_id: Some(slot.id.clone()),
                session_id: None,
                message: format!(
                    "Slot `{}` has multiple pending sessions attached; review ownership before continuing.",
                    slot.id
                ),
            });
        }
    }

    summary
}
