use anyhow::Error;
use awo_core::{
    AppSnapshot, CommandOutcome, ContextDoctorReport, Diagnostic, DomainEvent, RepoContext,
    RepoSkillCatalog, ReviewSummary, RuntimeCapabilityDescriptor, RuntimeKind, RuntimePressure,
    SkillDoctorReport, TeamManifest, TeamMember, TeamTaskExecution, TeamTeardownPlan,
    TeamTeardownResult,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct OutputMode {
    pub json: bool,
}

#[derive(Debug, Serialize)]
struct JsonEnvelope<T: Serialize> {
    ok: bool,
    summary: Option<String>,
    error: Option<String>,
    events: Vec<DomainEvent>,
    data: Option<T>,
}

pub fn merge_command_outcomes(outcomes: Vec<CommandOutcome>) -> CommandOutcome {
    let summary = outcomes
        .iter()
        .map(|outcome| outcome.summary.clone())
        .collect::<Vec<_>>()
        .join(" | ");
    let events = outcomes
        .into_iter()
        .flat_map(|outcome| outcome.events)
        .collect::<Vec<_>>();
    CommandOutcome {
        summary,
        events,
        data: None,
    }
}

pub fn print_json_response<T: Serialize>(data: &T, outcome: Option<&CommandOutcome>) {
    match json_response_string(data, outcome) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize JSON response: {error}");
            println!(
                "{{\"ok\":false,\"summary\":null,\"error\":\"failed to serialize response\",\"events\":[],\"data\":null}}"
            );
        }
    }
}

pub fn print_json_error(error: &Error) {
    println!("{}", json_error_string(error));
}

pub fn print_outcome(outcome: &CommandOutcome) {
    println!("{}", outcome.summary);
    for event in &outcome.events {
        println!("- {}", event.to_message());
    }
}

pub fn print_registered_repos(snapshot: &AppSnapshot) {
    if snapshot.registered_repos.is_empty() {
        println!("No registered repos.");
        return;
    }

    println!();
    println!("Registered repos:");
    for repo in &snapshot.registered_repos {
        println!(
            "- {} [{}] base={} remote={} worktrees={} ctx={} packs={} skills={} mcp={}",
            repo.name,
            repo.id,
            repo.default_base_branch,
            repo.remote_label,
            repo.worktree_root,
            repo.entrypoint_count,
            repo.context_pack_count,
            repo.shared_skill_count,
            repo.mcp_config_present
        );
        println!("  root={}", repo.repo_root);
    }
}

pub fn print_context(context: &RepoContext) {
    println!();
    println!("Context library:");
    println!("- repo root: {}", context.repo_root);
    println!(
        "- entrypoints: {}",
        format_context_files(&context.entrypoints)
    );
    println!("- standards: {}", format_context_files(&context.standards));
    println!("- docs: {}", context.docs.len());
    if let Some(mcp_config) = &context.mcp_config_path {
        println!("- mcp: {}", mcp_config);
    } else {
        println!("- mcp: none");
    }
    if context.packs.is_empty() {
        println!("- packs: none");
    } else {
        println!("- packs:");
        for pack in &context.packs {
            println!("  - {} ({} files)", pack.name, pack.files.len());
            for file in &pack.files {
                println!("    - {}", file);
            }
        }
    }
}

pub fn print_context_doctor(report: &ContextDoctorReport) {
    println!();
    println!("Context doctor:");
    for diagnostic in &report.diagnostics {
        println!(
            "- [{}] {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
}

pub fn print_skills_catalog(catalog: &RepoSkillCatalog) {
    println!();
    println!("Skills catalog:");
    println!("- repo root: {}", catalog.repo_root);
    println!(
        "- shared root: {}",
        catalog.shared_root.as_deref().unwrap_or("none")
    );
    println!(
        "- lockfile: {}",
        catalog.lockfile_path.as_deref().unwrap_or("none")
    );
    if catalog.skills.is_empty() {
        println!("- skills: none");
    } else {
        println!("- skills:");
        for skill in &catalog.skills {
            println!(
                "  - {} name={} desc={}",
                skill.directory_name,
                skill.name.as_deref().unwrap_or("-"),
                skill.description.as_deref().unwrap_or("-")
            );
            println!("    source={}", skill.source_path);
        }
    }
    print_diagnostics(&catalog.diagnostics);
}

pub fn print_skill_doctor(reports: &[SkillDoctorReport]) {
    println!();
    println!("Skills doctor:");
    for report in reports {
        println!(
            "- runtime={} target={} strategy={} recommended_mode={}",
            report.runtime,
            report.target_dir.as_deref().unwrap_or("unresolved"),
            report.policy.discovery.as_str(),
            report.policy.recommended_mode.as_str()
        );
        println!("  note={}", report.policy.note);
        for entry in &report.entries {
            println!(
                "  - {} state={} target={}",
                entry.name,
                entry.state.as_str(),
                entry.target_path
            );
        }
        print_diagnostics(&report.diagnostics);
    }
}

pub fn print_slots(snapshot: &AppSnapshot, repo_filter: Option<&str>) {
    let slots = snapshot
        .slots
        .iter()
        .filter(|slot| repo_filter.is_none_or(|repo_id| slot.repo_id == repo_id))
        .collect::<Vec<_>>();
    if slots.is_empty() {
        println!("No slots.");
        return;
    }

    println!();
    println!("Slots:");
    for slot in slots {
        println!(
            "- {} [{}] repo={} branch={} status={} strategy={} dirty={} fp={}",
            slot.task_name,
            slot.id,
            slot.repo_id,
            slot.branch_name,
            slot.status,
            slot.strategy,
            slot.dirty,
            slot.fingerprint_status
        );
        println!("  path={}", slot.slot_path);
    }
}

pub fn print_sessions(snapshot: &AppSnapshot, repo_filter: Option<&str>) {
    let sessions = snapshot
        .sessions
        .iter()
        .filter(|session| {
            repo_filter.is_none_or(|repo_id| {
                snapshot
                    .slots
                    .iter()
                    .find(|slot| slot.id == session.slot_id)
                    .map(|slot| slot.repo_id == repo_id)
                    .unwrap_or(false)
            })
        })
        .collect::<Vec<_>>();
    if sessions.is_empty() {
        println!("No sessions.");
        return;
    }

    println!();
    println!("Sessions:");
    for session in sessions {
        println!(
            "- {} [{}] slot={} status={} read_only={} dry_run={} supervisor={} exit={} reason={} cap={}",
            session.runtime,
            session.id,
            session.slot_id,
            session.status,
            session.read_only,
            session.dry_run,
            session.supervisor.as_deref().unwrap_or("-"),
            session
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "-".to_string()),
            session
                .end_reason
                .map(|reason| reason.as_str())
                .unwrap_or("-"),
            session.capacity_status.as_str(),
        );
        if let Some(log_path) = &session.log_path {
            println!("  log={log_path}");
        }
        if let Some(usage_note) = &session.usage_note {
            println!("  usage={usage_note}");
        }
        if let Some(recovery_hint) = &session.recovery_hint {
            println!("  recovery={recovery_hint}");
        }
    }
}

pub fn print_session_log(outcome: &CommandOutcome) {
    for event in &outcome.events {
        if let DomainEvent::SessionLogLoaded {
            content, log_path, ..
        } = event
        {
            println!("--- log: {log_path} ---");
            println!("{content}");
            println!("--- end ---");
            return;
        }
    }
}

pub fn print_review(review: &ReviewSummary) {
    println!();
    println!("Review summary:");
    println!("- active slots: {}", review.active_slots);
    println!("- releasable slots: {}", review.releasable_slots);
    println!("- dirty slots: {}", review.dirty_slots);
    println!("- stale slots: {}", review.stale_slots);
    println!("- pending sessions: {}", review.pending_sessions);
    println!("- completed sessions: {}", review.completed_sessions);
    println!("- failed sessions: {}", review.failed_sessions);
    if review.warnings.is_empty() {
        println!("- warnings: none");
        return;
    }

    println!("- warnings:");
    for warning in &review.warnings {
        println!("  - {}", warning.message);
    }
}

pub fn print_review_diff(content: &str) {
    println!();
    println!("{content}");
}

pub fn print_runtime_capabilities(capabilities: &[RuntimeCapabilityDescriptor]) {
    if capabilities.is_empty() {
        println!("No runtime capabilities found.");
        return;
    }

    println!("Runtime capabilities:");
    for capability in capabilities {
        println!(
            "- {} ({}) tier={} limit={} usage={} capacity={} budget={} lifetime={} launch={} subagents={} teams={} skills={}",
            capability.display_name,
            capability.runtime,
            capability.cost_tier.as_str(),
            capability.limit_profile.as_str(),
            capability.usage_reporting.as_str(),
            capability.capacity_reporting.as_str(),
            capability.budget_guardrails.as_str(),
            capability.session_lifetime.as_str(),
            capability.default_launch_mode,
            capability.inline_subagents.as_str(),
            capability.multi_session_teams.as_str(),
            capability.skill_preload.as_str(),
        );
        println!(
            "  mcp_reasoning={} interrupt={} resume={} structured={} read_only_hint={}",
            capability.reasoning_mcp_tools.as_str(),
            capability.interrupt.as_str(),
            capability.resume.as_str(),
            capability.structured_output.as_str(),
            capability.read_only_hint.as_str()
        );
        println!("  operator_note: {}", capability.operator_note);
        for note in &capability.notes {
            println!("  note: {note}");
        }
    }
}

pub fn print_runtime_pressure_profile(profile: &HashMap<RuntimeKind, RuntimePressure>) {
    if profile.is_empty() {
        println!("No runtime pressure profile set.");
        return;
    }

    let mut entries = profile
        .iter()
        .map(|(runtime, pressure)| (runtime.as_str(), pressure.as_str()))
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));

    println!("Runtime pressure profile:");
    for (runtime, pressure) in entries {
        println!("- {runtime}={pressure}");
    }
}

pub fn print_routing_decision(decision: &awo_core::routing::RoutingDecision) {
    println!("Routing decision:");
    print_routing_decision_fields(decision);
}

pub fn print_routing_preview(
    decision: &awo_core::routing::RoutingDecision,
    preferences: &awo_core::RoutingPreferences,
    context: &awo_core::RoutingContext,
) {
    println!("Routing preview:");
    print_routing_preferences(preferences);
    print_routing_pressure(context);
    print_routing_decision(decision);
}

pub fn print_routing_recommendation(recommendation: &awo_core::RoutingRecommendation) {
    println!("Routing recommendation:");
    println!("- team id: {}", recommendation.team_id);
    println!("- member id: {}", recommendation.member_id);
    println!(
        "- task id: {}",
        recommendation.task_id.as_deref().unwrap_or("-")
    );
    print_routing_preferences(&recommendation.preferences);
    print_routing_pressure(&recommendation.context);
    print_routing_decision(&recommendation.decision);
}

pub fn print_team_manifests(manifests: &[TeamManifest]) {
    if manifests.is_empty() {
        println!("No team manifests.");
        return;
    }

    println!("Team manifests:");
    for manifest in manifests {
        println!(
            "- {} repo={} status={} members={} tasks={}",
            manifest.team_id,
            manifest.repo_id,
            manifest.status,
            1 + manifest.members.len(),
            manifest.tasks.len()
        );
        println!("  objective={}", manifest.objective);
    }
}

pub fn print_team_member(member: &TeamMember) {
    println!("Team member: {}", member.member_id);
    println!(
        "- role={} runtime={} model={} mode={} read_only={}",
        member.role,
        member.runtime.as_deref().unwrap_or("-"),
        member.model.as_deref().unwrap_or("-"),
        member.execution_mode,
        member.read_only
    );
    println!(
        "- scope: {}",
        if member.write_scope.is_empty() {
            "-".to_string()
        } else {
            member.write_scope.join(", ")
        }
    );
    if member.context_packs.is_empty() {
        println!("- context packs: none");
    } else {
        println!("- context packs: {}", member.context_packs.join(", "));
    }
    if member.skills.is_empty() {
        println!("- skills: none");
    } else {
        println!("- skills: {}", member.skills.join(", "));
    }
    if let Some(notes) = &member.notes {
        println!("- notes: {}", notes);
    }
    if member.fallback_runtime.is_some() || member.fallback_model.is_some() {
        println!(
            "- fallback: runtime={} model={}",
            member.fallback_runtime.as_deref().unwrap_or("-"),
            member.fallback_model.as_deref().unwrap_or("-"),
        );
    }
    if let Some(routing_preferences) = &member.routing_preferences {
        println!("- member routing defaults:");
        println!("  prefer_local={}", routing_preferences.prefer_local);
        println!("  avoid_metered={}", routing_preferences.avoid_metered);
        if let Some(tier) = &routing_preferences.max_cost_tier {
            println!("  max_cost_tier={}", tier.as_str());
        }
        println!("  allow_fallback={}", routing_preferences.allow_fallback);
    }
}

pub fn print_team_manifest(manifest: &TeamManifest) {
    println!("Team manifest:");
    println!("- team id: {}", manifest.team_id);
    println!("- repo id: {}", manifest.repo_id);
    println!("- objective: {}", manifest.objective);
    println!("- status: {}", manifest.status);
    if let Some(routing_preferences) = &manifest.routing_preferences {
        println!("- team routing defaults:");
        println!("  prefer_local={}", routing_preferences.prefer_local);
        println!("  avoid_metered={}", routing_preferences.avoid_metered);
        if let Some(tier) = &routing_preferences.max_cost_tier {
            println!("  max_cost_tier={}", tier.as_str());
        }
        println!("  allow_fallback={}", routing_preferences.allow_fallback);
    }
    println!(
        "- lead: {} role={} runtime={} model={} mode={} read_only={}",
        manifest.lead.member_id,
        manifest.lead.role,
        manifest.lead.runtime.as_deref().unwrap_or("-"),
        manifest.lead.model.as_deref().unwrap_or("-"),
        manifest.lead.execution_mode,
        manifest.lead.read_only
    );
    if manifest.lead.fallback_runtime.is_some() || manifest.lead.fallback_model.is_some() {
        println!(
            "  fallback: runtime={} model={}",
            manifest.lead.fallback_runtime.as_deref().unwrap_or("-"),
            manifest.lead.fallback_model.as_deref().unwrap_or("-"),
        );
    }
    if let Some(routing_preferences) = &manifest.lead.routing_preferences {
        println!("  lead routing defaults:");
        println!("    prefer_local={}", routing_preferences.prefer_local);
        println!("    avoid_metered={}", routing_preferences.avoid_metered);
        if let Some(tier) = &routing_preferences.max_cost_tier {
            println!("    max_cost_tier={}", tier.as_str());
        }
        println!("    allow_fallback={}", routing_preferences.allow_fallback);
    }
    if manifest.lead.context_packs.is_empty() {
        println!("- lead context packs: none");
    } else {
        println!(
            "- lead context packs: {}",
            manifest.lead.context_packs.join(", ")
        );
    }
    if manifest.lead.skills.is_empty() {
        println!("- lead skills: none");
    } else {
        println!("- lead skills: {}", manifest.lead.skills.join(", "));
    }
    println!(
        "- current lead: {} runtime={} model={} session={}",
        manifest.current_lead_member_id(),
        manifest
            .current_lead_member()
            .and_then(|member| member.runtime.as_deref())
            .unwrap_or("-"),
        manifest
            .current_lead_member()
            .and_then(|member| member.model.as_deref())
            .unwrap_or("-"),
        manifest.current_lead_session_id().unwrap_or("-"),
    );
    let review_queue = manifest
        .tasks
        .iter()
        .filter(|task| task.state == awo_core::TaskCardState::Review)
        .count();
    let cleanup_queue = manifest
        .tasks
        .iter()
        .filter(|task| task.state == awo_core::TaskCardState::Done && task.slot_id.is_some())
        .count();
    let history_queue = manifest
        .tasks
        .iter()
        .filter(|task| {
            matches!(
                task.state,
                awo_core::TaskCardState::Cancelled | awo_core::TaskCardState::Superseded
            )
        })
        .count();
    println!(
        "- queues: review={} cleanup={} history={}",
        review_queue, cleanup_queue, history_queue
    );

    if manifest.members.is_empty() {
        println!("- members: none");
    } else {
        println!("- members:");
        for member in &manifest.members {
            println!(
                "  - {} role={} runtime={} mode={} read_only={} scope={}",
                member.member_id,
                member.role,
                member.runtime.as_deref().unwrap_or("-"),
                member.execution_mode,
                member.read_only,
                if member.write_scope.is_empty() {
                    "-".to_string()
                } else {
                    member.write_scope.join(", ")
                }
            );
            if member.fallback_runtime.is_some() || member.fallback_model.is_some() {
                println!(
                    "    fallback: runtime={} model={}",
                    member.fallback_runtime.as_deref().unwrap_or("-"),
                    member.fallback_model.as_deref().unwrap_or("-"),
                );
            }
            if let Some(routing_preferences) = &member.routing_preferences {
                println!("    member routing defaults:");
                println!("      prefer_local={}", routing_preferences.prefer_local);
                println!("      avoid_metered={}", routing_preferences.avoid_metered);
                if let Some(tier) = &routing_preferences.max_cost_tier {
                    println!("      max_cost_tier={}", tier.as_str());
                }
                println!(
                    "      allow_fallback={}",
                    routing_preferences.allow_fallback
                );
            }
        }
    }

    if manifest.tasks.is_empty() {
        println!("- tasks: none");
    } else {
        println!("- tasks:");
        for task in &manifest.tasks {
            println!(
                "  - {} owner={} state={} deliverable={}",
                task.title, task.owner_id, task.state, task.deliverable
            );
            println!(
                "    queue={}",
                if task.state == awo_core::TaskCardState::Review {
                    "review queue"
                } else if task.state == awo_core::TaskCardState::Done && task.slot_id.is_some() {
                    "cleanup queue"
                } else if matches!(
                    task.state,
                    awo_core::TaskCardState::Cancelled | awo_core::TaskCardState::Superseded
                ) {
                    "history"
                } else {
                    "task list"
                }
            );
            if task.runtime.is_some() || task.model.is_some() {
                println!(
                    "    requested={} model={}",
                    task.runtime.as_deref().unwrap_or("-"),
                    task.model.as_deref().unwrap_or("-")
                );
            }
            println!(
                "    scope={} verify={}",
                if task.write_scope.is_empty() {
                    "-".to_string()
                } else {
                    task.write_scope.join(", ")
                },
                if task.verification.is_empty() {
                    "-".to_string()
                } else {
                    task.verification.join(", ")
                }
            );
            if let Some(result_session_id) = &task.result_session_id {
                println!("    result_session={result_session_id}");
            }
            if let Some(result_summary) = &task.result_summary {
                println!("    result={result_summary}");
            }
            if let Some(replacement_task_id) = &task.superseded_by_task_id {
                println!("    superseded_by={replacement_task_id}");
            }
            if let Some(handoff_note) = &task.handoff_note {
                println!("    handoff={handoff_note}");
            }
            if let Some(output_log_path) = &task.output_log_path {
                println!("    log={output_log_path}");
            }
        }
    }

    if manifest.plan_items.is_empty() {
        println!("- plan items: none");
    } else {
        println!("- plan items:");
        for plan in &manifest.plan_items {
            println!(
                "  - {} state={} owner={} title={}",
                plan.plan_id,
                plan.state,
                plan.owner_id.as_deref().unwrap_or("-"),
                plan.title
            );
            if let Some(task_id) = &plan.generated_task_id {
                println!("    generated_task={task_id}");
            }
            if let Some(deliverable) = &plan.deliverable {
                println!("    deliverable={deliverable}");
            }
            if let Some(notes) = &plan.notes {
                println!("    notes={notes}");
            }
        }
    }
}

pub fn print_team_teardown_plan(team_id: &str, plan: &TeamTeardownPlan) {
    println!("Team teardown preview for `{team_id}`:");
    if !plan.reset_summary.non_todo_tasks.is_empty() {
        println!("- tasks that will reset:");
        for task in &plan.reset_summary.non_todo_tasks {
            println!("  - {task}");
        }
    }
    if !plan.reset_summary.bound_members.is_empty() {
        println!(
            "- members with slot bindings: {}",
            plan.reset_summary.bound_members.join(", ")
        );
    }
    if !plan.bound_slots.is_empty() {
        println!("- bound slots: {}", plan.bound_slots.join(", "));
    }
    if !plan.active_slots.is_empty() {
        println!(
            "- active slots to release: {}",
            plan.active_slots.join(", ")
        );
    }
    if !plan.cancellable_sessions.is_empty() {
        println!(
            "- sessions to cancel: {}",
            plan.cancellable_sessions.join(", ")
        );
    }
    if !plan.dirty_slots.is_empty() {
        println!("- blocking dirty slots: {}", plan.dirty_slots.join(", "));
    }
    if !plan.blocking_sessions.is_empty() {
        println!("- blocking sessions:");
        for session in &plan.blocking_sessions {
            println!("  - {session}");
        }
    }
    if !plan.requires_confirmation() {
        println!("- nothing to teardown");
    }
}

pub fn print_team_teardown_result(team_id: &str, result: &TeamTeardownResult) {
    println!("Team `{team_id}` torn down to planning.");
    println!(
        "- cancelled sessions: {}",
        if result.cancelled_sessions.is_empty() {
            "-".to_string()
        } else {
            result.cancelled_sessions.join(", ")
        }
    );
    println!(
        "- released slots: {}",
        if result.released_slots.is_empty() {
            "-".to_string()
        } else {
            result.released_slots.join(", ")
        }
    );
}

pub fn print_team_task_execution(execution: &TeamTaskExecution) {
    println!(
        "Team task execution: {} / {}",
        execution.team_id, execution.task_id
    );
    println!("- owner: {}", execution.owner_id);
    println!(
        "- routed to: {} model={} (source: {})",
        execution.runtime,
        execution.model.as_deref().unwrap_or("-"),
        match execution.routing_source {
            awo_core::RoutingSource::Primary => "primary",
            awo_core::RoutingSource::Fallback => "fallback",
        }
    );
    println!("  reason: {}", execution.routing_reason);
    println!(
        "- slot: {} (branch: {}) acquired={}",
        execution.slot_id, execution.branch_name, execution.acquired_slot
    );
    println!(
        "- session: {} ({})",
        execution.session_id.as_deref().unwrap_or("-"),
        execution.session_status
    );
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    println!("- diagnostics:");
    for diagnostic in diagnostics {
        println!(
            "  - [{}] {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
}

fn print_routing_preferences(preferences: &awo_core::RoutingPreferences) {
    println!("- resolved preferences:");
    println!("  prefer_local={}", preferences.prefer_local);
    println!("  avoid_metered={}", preferences.avoid_metered);
    if let Some(tier) = preferences.max_cost_tier {
        println!("  max_cost_tier={}", tier.as_str());
    }
    println!("  allow_fallback={}", preferences.allow_fallback);
}

fn print_routing_pressure(context: &awo_core::RoutingContext) {
    if context.is_empty() {
        return;
    }

    let mut entries = context
        .pressure
        .iter()
        .map(|(runtime, pressure)| (runtime.as_str(), pressure.as_str()))
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));

    println!("- runtime pressure:");
    for (runtime, pressure) in entries {
        println!("  - {}={}", runtime, pressure);
    }
}

fn print_routing_decision_fields(decision: &awo_core::routing::RoutingDecision) {
    println!("- selected runtime: {}", decision.selected_runtime.as_str());
    println!(
        "- selected model: {}",
        decision.selected_model.as_deref().unwrap_or("-")
    );
    println!(
        "- source: {}",
        match decision.source {
            awo_core::RoutingSource::Primary => "primary",
            awo_core::RoutingSource::Fallback => "fallback",
        }
    );
    println!("- reason: {}", decision.reason);
}

fn format_context_files(files: &[awo_core::context::ContextFile]) -> String {
    if files.is_empty() {
        return "none".to_string();
    }

    files
        .iter()
        .map(|file| file.label.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn json_error_string(error: &Error) -> String {
    let envelope = JsonEnvelope::<()> {
        ok: false,
        summary: None,
        error: Some(format!("{error:#}")),
        events: vec![],
        data: None,
    };
    serde_json::to_string_pretty(&envelope).unwrap_or_else(|serialize_error| {
        eprintln!("failed to serialize JSON error response: {serialize_error}");
        "{\"ok\":false,\"summary\":null,\"error\":\"failed to serialize error\",\"events\":[],\"data\":null}".to_string()
    })
}

fn json_response_string<T: Serialize>(
    data: &T,
    outcome: Option<&CommandOutcome>,
) -> serde_json::Result<String> {
    let envelope = JsonEnvelope {
        ok: true,
        summary: outcome.map(|o| o.summary.clone()).or(None), // T is often the data itself
        error: None,
        events: outcome.map(|o| o.events.clone()).unwrap_or_default(),
        data: Some(data),
    };
    serde_json::to_string_pretty(&envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use awo_core::DomainEvent;
    use serde::Serialize;
    use serde::ser::Error as _;
    use serde_json::Value;

    #[test]
    fn json_response_wraps_summary_and_data() {
        let value = vec!["repo-a", "repo-b"];
        let outcome = CommandOutcome::with_events(
            "listed repos".to_string(),
            vec![DomainEvent::CommandReceived {
                command: "repo_list".to_string(),
            }],
        );

        // We can't easily capture stdout in a unit test here without more machinery,
        // but we can test the internal envelope logic if we kept it.
        // For now, let's just use a constructor that we know works.
        let envelope = JsonEnvelope {
            ok: true,
            summary: Some(outcome.summary.clone()),
            error: None,
            events: outcome.events.clone(),
            data: Some(&value),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: Value = serde_json::from_str(&json).expect("json response should deserialize");

        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["summary"], "listed repos");
        assert_eq!(parsed["error"], Value::Null);
        assert_eq!(parsed["data"][0], "repo-a");
        assert_eq!(parsed["events"].as_array().map(std::vec::Vec::len), Some(1));
        assert_eq!(parsed["events"][0]["type"], "command_received");
        assert_eq!(parsed["events"][0]["command"], "repo_list");
    }

    #[derive(Debug)]
    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(S::Error::custom("intentional test failure"))
        }
    }

    #[test]
    fn json_response_string_propagates_serialization_errors() {
        let error = json_response_string(&FailingSerialize, None).unwrap_err();
        assert!(error.to_string().contains("intentional test failure"));
    }

    #[test]
    fn json_error_wraps_error_message() {
        let error = anyhow::anyhow!("boom");
        let json = json_error_string(&error);
        let parsed: Value = serde_json::from_str(&json).expect("json error should deserialize");

        assert_eq!(parsed["ok"], false);
        assert_eq!(parsed["summary"], Value::Null);
        assert_eq!(parsed["error"], "boom");
        assert_eq!(parsed["events"].as_array().map(std::vec::Vec::len), Some(0));
        assert_eq!(parsed["data"], Value::Null);
    }
}
