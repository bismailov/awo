use crate::capabilities::{CostTier, RuntimeCapabilityDescriptor, all_runtime_capabilities};
use crate::config::AppConfig;
use crate::context::discover_repo_context;
use crate::diagnostics::DiagnosticSeverity;
use crate::error::AwoResult;
use crate::git;
use crate::platform::current_platform_label;
use crate::repo::{RegisteredRepo, remote_label};
use crate::routing::RoutingPreferences;
use crate::runtime::{SessionRecord, SessionStatus};
use crate::skills::{
    RuntimeSkillRoots, SkillInstallState, SkillRuntime, discover_repo_skills, doctor_repo_skills,
};
use crate::slot::{FingerprintStatus, SlotRecord, SlotStatus, SlotStrategy};
use crate::store::Store;
use crate::team::{
    TaskCardState, TeamManifest, TeamStatus, list_team_manifest_paths, load_team_manifest,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::warn;

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
    pub status: TeamStatus,
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
    pub strategy: SlotStrategy,
    pub status: SlotStatus,
    pub dirty: bool,
    pub fingerprint_status: FingerprintStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub repo_id: String,
    pub slot_id: String,
    pub runtime: String,
    pub supervisor: Option<String>,
    pub status: SessionStatus,
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
            .filter_map(|path| match load_team_manifest(&path) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    warn!(
                        path = %path.display(),
                        error = %error,
                        "failed to load team manifest for snapshot"
                    );
                    None
                }
            })
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
    let context = match discover_repo_context(Path::new(&value.repo_root)) {
        Ok(context) => Some(context),
        Err(error) => {
            warn!(
                repo_root = %value.repo_root,
                error = %error,
                "failed to discover repo context while building snapshot"
            );
            None
        }
    };
    let skills = match discover_repo_skills(Path::new(&value.repo_root)) {
        Ok(skills) => Some(skills),
        Err(error) => {
            warn!(
                repo_root = %value.repo_root,
                error = %error,
                "failed to discover repo skills while building snapshot"
            );
            None
        }
    };
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
            .filter(|task| task.state != TaskCardState::Done)
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
            status: value.status,
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
    fn is_completed(&self) -> bool {
        self.status == SessionStatus::Completed
    }

    fn is_failed(&self) -> bool {
        self.status == SessionStatus::Failed
    }

    fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }
}

impl SlotSummary {
    fn is_active(&self) -> bool {
        self.status == SlotStatus::Active
    }

    fn is_released(&self) -> bool {
        self.status == SlotStatus::Released
    }

    fn is_missing(&self) -> bool {
        self.status == SlotStatus::Missing
    }

    fn uses_warm_strategy(&self) -> bool {
        self.strategy == SlotStrategy::Warm
    }

    fn fingerprint_is_ready(&self) -> bool {
        self.fingerprint_status == FingerprintStatus::Ready
    }

    fn fingerprint_is_stale(&self) -> bool {
        self.fingerprint_status == FingerprintStatus::Stale
    }
}

fn build_review_summary(slots: &[SlotRecord], sessions: &[SessionRecord]) -> ReviewSummary {
    build_review_summary_impl(
        slots.iter().map(|slot| SlotReviewView {
            id: slot.id.as_str(),
            is_active: slot.is_active(),
            is_released: slot.is_released(),
            is_missing: slot.is_missing(),
            dirty: slot.dirty,
            dirty_files: if slot.dirty {
                git::dirty_files(Path::new(&slot.slot_path)).unwrap_or_else(|err| {
                    warn!(slot_id = slot.id.as_str(), %err, "failed to list dirty files for slot");
                    vec![]
                })
            } else {
                vec![]
            },
            uses_warm_strategy: slot.uses_warm_strategy(),
            fingerprint_is_ready: slot.fingerprint_is_ready(),
            fingerprint_is_stale: slot.fingerprint_is_stale(),
        }),
        sessions.iter().map(|session| SessionReviewView {
            id: session.id.as_str(),
            slot_id: session.slot_id.as_str(),
            is_completed: session.is_completed(),
            is_failed: session.is_failed(),
            is_terminal: session.is_terminal(),
        }),
    )
}

fn build_review_summary_from_summaries(
    slots: &[&SlotSummary],
    sessions: &[&SessionSummary],
) -> ReviewSummary {
    build_review_summary_impl(
        slots.iter().map(|slot| SlotReviewView {
            id: slot.id.as_str(),
            is_active: slot.is_active(),
            is_released: slot.is_released(),
            is_missing: slot.is_missing(),
            dirty: slot.dirty,
            dirty_files: if slot.dirty {
                git::dirty_files(Path::new(&slot.slot_path)).unwrap_or_else(|err| {
                    warn!(slot_id = slot.id.as_str(), %err, "failed to list dirty files for slot");
                    vec![]
                })
            } else {
                vec![]
            },
            uses_warm_strategy: slot.uses_warm_strategy(),
            fingerprint_is_ready: slot.fingerprint_is_ready(),
            fingerprint_is_stale: slot.fingerprint_is_stale(),
        }),
        sessions.iter().map(|session| SessionReviewView {
            id: session.id.as_str(),
            slot_id: session.slot_id.as_str(),
            is_completed: session.is_completed(),
            is_failed: session.is_failed(),
            is_terminal: session.is_terminal(),
        }),
    )
}

pub(crate) struct SlotReviewView<'a> {
    id: &'a str,
    is_active: bool,
    is_released: bool,
    is_missing: bool,
    dirty: bool,
    dirty_files: Vec<String>,
    uses_warm_strategy: bool,
    fingerprint_is_ready: bool,
    fingerprint_is_stale: bool,
}

struct SessionReviewView<'a> {
    id: &'a str,
    slot_id: &'a str,
    is_completed: bool,
    is_failed: bool,
    is_terminal: bool,
}

fn build_review_summary_impl<'a>(
    slots: impl Iterator<Item = SlotReviewView<'a>>,
    sessions: impl Iterator<Item = SessionReviewView<'a>>,
) -> ReviewSummary {
    let slots: Vec<SlotReviewView<'a>> = slots.collect();
    let sessions: Vec<SessionReviewView<'a>> = sessions.collect();
    let mut summary = ReviewSummary::default();
    let mut pending_sessions_by_slot: HashMap<&str, usize> = HashMap::new();

    for session in &sessions {
        if session.is_completed {
            summary.completed_sessions += 1;
        } else if session.is_failed {
            summary.failed_sessions += 1;
            summary.warnings.push(ReviewWarning {
                kind: "failed-session".to_string(),
                slot_id: Some(session.slot_id.to_string()),
                session_id: Some(session.id.to_string()),
                message: format!(
                    "Session `{}` on slot `{}` failed and should be reviewed.",
                    session.id, session.slot_id
                ),
            });
        }

        if !session.is_terminal {
            summary.pending_sessions += 1;
            *pending_sessions_by_slot.entry(session.slot_id).or_insert(0) += 1;
        }
    }

    let mut dirty_slot_map: HashMap<&str, (Vec<String>, HashSet<String>)> = HashMap::new();
    for slot in &slots {
        if slot.dirty {
            let mut dirs = HashSet::new();
            for file in &slot.dirty_files {
                if let Some(parent) = Path::new(file).parent()
                    && let Some(parent_str) = parent.to_str()
                    && !parent_str.is_empty()
                {
                    dirs.insert(parent_str.to_string());
                }
            }
            dirty_slot_map.insert(slot.id, (slot.dirty_files.clone(), dirs));
        }
    }
    let mut dirty_ids: Vec<&str> = dirty_slot_map.keys().copied().collect();
    dirty_ids.sort();
    for i in 0..dirty_ids.len() {
        for j in i + 1..dirty_ids.len() {
            let id_a = dirty_ids[i];
            let id_b = dirty_ids[j];
            let (files_a, dirs_a) = &dirty_slot_map[id_a];
            let (files_b, dirs_b) = &dirty_slot_map[id_b];

            // Direct file overlap (risky-overlap)
            let common: Vec<_> = files_a
                .iter()
                .filter(|f| files_b.contains(f))
                .cloned()
                .collect();
            if !common.is_empty() {
                let mut common_sorted = common;
                common_sorted.sort();
                let file_list = common_sorted.join(", ");
                summary.warnings.push(ReviewWarning {
                    kind: "risky-overlap".to_string(),
                    slot_id: None,
                    session_id: None,
                    message: format!(
                        "Slots '{}' and '{}' both modified: {}",
                        id_a, id_b, file_list
                    ),
                });
            }

            // Directory-level overlap (soft-overlap)
            let mut common_dirs: Vec<_> = dirs_a.intersection(dirs_b).cloned().collect();
            common_dirs.sort();

            for dir in common_dirs {
                // Check if they modified DIFFERENT files in this directory
                // to avoid redundant reporting when the files are identical.
                let mut files_in_dir_a: Vec<_> = files_a
                    .iter()
                    .filter(|f| {
                        Path::new(f)
                            .parent()
                            .and_then(|p| p.to_str())
                            .is_some_and(|p| p == dir)
                    })
                    .collect();
                let mut files_in_dir_b: Vec<_> = files_b
                    .iter()
                    .filter(|f| {
                        Path::new(f)
                            .parent()
                            .and_then(|p| p.to_str())
                            .is_some_and(|p| p == dir)
                    })
                    .collect();

                files_in_dir_a.sort();
                files_in_dir_b.sort();

                if files_in_dir_a != files_in_dir_b {
                    summary.warnings.push(ReviewWarning {
                        kind: "soft-overlap".to_string(),
                        slot_id: None,
                        session_id: None,
                        message: format!(
                            "Slots '{}' and '{}' both modified files in module: {}",
                            id_a, id_b, dir
                        ),
                    });
                }
            }
        }
    }

    for slot in &slots {
        if slot.is_active {
            summary.active_slots += 1;
        }
        if slot.dirty {
            summary.dirty_slots += 1;
        }
        if slot.fingerprint_is_stale {
            summary.stale_slots += 1;
        }

        let pending_sessions = pending_sessions_by_slot
            .get(slot.id)
            .copied()
            .unwrap_or_default();
        if slot.is_active && !slot.dirty && slot.fingerprint_is_ready && pending_sessions == 0 {
            summary.releasable_slots += 1;
        }

        if slot.is_active && slot.dirty {
            summary.warnings.push(ReviewWarning {
                kind: "dirty-slot".to_string(),
                slot_id: Some(slot.id.to_string()),
                session_id: None,
                message: format!(
                    "Slot `{}` has uncommitted changes and cannot be safely released.",
                    slot.id
                ),
            });
        }

        if slot.is_active && slot.fingerprint_is_stale {
            summary.warnings.push(ReviewWarning {
                kind: "stale-active-slot".to_string(),
                slot_id: Some(slot.id.to_string()),
                session_id: None,
                message: format!(
                    "Slot `{}` is active but stale relative to the repo fingerprint.",
                    slot.id
                ),
            });
        }

        if slot.uses_warm_strategy && slot.is_released && slot.fingerprint_is_stale {
            summary.warnings.push(ReviewWarning {
                kind: "stale-warm-slot".to_string(),
                slot_id: Some(slot.id.to_string()),
                session_id: None,
                message: format!(
                    "Warm slot `{}` is stale and should be refreshed before reuse.",
                    slot.id
                ),
            });
        }

        if slot.is_missing {
            summary.warnings.push(ReviewWarning {
                kind: "missing-slot".to_string(),
                slot_id: Some(slot.id.to_string()),
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
                slot_id: Some(slot.id.to_string()),
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
                slot_id: Some(slot.id.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_risky_overlap_between_dirty_slots() {
        let slot1 = SlotReviewView {
            id: "slot1",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/lib.rs".to_string(), "src/error.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let slot2 = SlotReviewView {
            id: "slot2",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/main.rs".to_string(), "src/error.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot1, slot2].into_iter(), vec![].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "risky-overlap"));
    }

    #[test]
    fn detects_soft_overlap_between_modules() {
        let slot1 = SlotReviewView {
            id: "slot1",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/runtime/executor.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let slot2 = SlotReviewView {
            id: "slot2",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/runtime/supervisor.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot1, slot2].into_iter(), vec![].into_iter());
        let warnings: Vec<_> = summary
            .warnings
            .iter()
            .filter(|w| w.kind == "soft-overlap")
            .collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].message,
            "Slots 'slot1' and 'slot2' both modified files in module: src/runtime"
        );
    }

    #[test]
    fn reports_both_risky_and_soft_overlap_if_different_files_exist_in_same_module() {
        let slot1 = SlotReviewView {
            id: "slot1",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/lib.rs".to_string(), "src/error.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let slot2 = SlotReviewView {
            id: "slot2",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/lib.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot1, slot2].into_iter(), vec![].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "risky-overlap"));
        assert!(summary.warnings.iter().any(|w| w.kind == "soft-overlap"));
    }

    #[test]
    fn no_overlap_when_slots_touch_different_files() {
        let slot1 = SlotReviewView {
            id: "slot1",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/foo.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let slot2 = SlotReviewView {
            id: "slot2",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["tests/bar.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot1, slot2].into_iter(), vec![].into_iter());
        assert!(
            !summary
                .warnings
                .iter()
                .any(|w| w.kind == "risky-overlap" || w.kind == "soft-overlap"),
            "no overlap expected for disjoint files/modules"
        );
    }

    #[test]
    fn dirty_slot_warning_emitted() {
        let slot = SlotReviewView {
            id: "slot-dirty",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: vec!["src/lib.rs".to_string()],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "dirty-slot"));
        assert_eq!(summary.dirty_slots, 1);
    }

    #[test]
    fn stale_active_slot_warning_emitted() {
        let slot = SlotReviewView {
            id: "slot-stale",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: false,
            fingerprint_is_ready: false,
            fingerprint_is_stale: true,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![].into_iter());
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.kind == "stale-active-slot")
        );
        assert_eq!(summary.stale_slots, 1);
    }

    #[test]
    fn stale_warm_released_slot_warning() {
        let slot = SlotReviewView {
            id: "slot-warm",
            is_active: false,
            is_released: true,
            is_missing: false,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: true,
            fingerprint_is_ready: false,
            fingerprint_is_stale: true,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "stale-warm-slot"));
    }

    #[test]
    fn missing_slot_warning() {
        let slot = SlotReviewView {
            id: "slot-gone",
            is_active: true,
            is_released: false,
            is_missing: true,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "missing-slot"));
    }

    #[test]
    fn slot_busy_with_pending_sessions() {
        let slot = SlotReviewView {
            id: "slot-busy",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let session = SessionReviewView {
            id: "sess-1",
            slot_id: "slot-busy",
            is_completed: false,
            is_failed: false,
            is_terminal: false,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![session].into_iter());
        assert!(summary.warnings.iter().any(|w| w.kind == "slot-busy"));
    }

    #[test]
    fn slot_multi_session_warning() {
        let slot = SlotReviewView {
            id: "slot-multi",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let sess1 = SessionReviewView {
            id: "sess-1",
            slot_id: "slot-multi",
            is_completed: false,
            is_failed: false,
            is_terminal: false,
        };
        let sess2 = SessionReviewView {
            id: "sess-2",
            slot_id: "slot-multi",
            is_completed: false,
            is_failed: false,
            is_terminal: false,
        };
        let summary =
            build_review_summary_impl(vec![slot].into_iter(), vec![sess1, sess2].into_iter());
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.kind == "slot-multi-session")
        );
    }

    #[test]
    fn releasable_slot_counted_when_clean_and_no_sessions() {
        let slot = SlotReviewView {
            id: "slot-clean",
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: false,
            dirty_files: vec![],
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        };
        let summary = build_review_summary_impl(vec![slot].into_iter(), vec![].into_iter());
        assert_eq!(summary.releasable_slots, 1);
        assert_eq!(summary.active_slots, 1);
        assert!(
            !summary.warnings.iter().any(|w| w.kind == "dirty-slot"
                || w.kind == "stale-active-slot"
                || w.kind == "missing-slot"),
            "clean active slot should produce no warnings"
        );
    }
}
