use super::*;
use crate::app::AppPaths;
use anyhow::Result;
use std::path::Path;
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
        worktrees_dir: root.join("data/worktrees"),
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
        routing_preferences: None,
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
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        },
        current_lead_member_id: Some("lead".to_string()),
        current_lead_session_id: None,
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
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
        }],
        tasks: vec![TaskCard {
            task_id: "task-1".to_string(),
            title: "Implement running-session persistence".to_string(),
            summary: "Persist the session before one-shot completion.".to_string(),
            owner_id: "worker-a".to_string(),
            runtime: Some("codex".to_string()),
            model: None,
            slot_id: Some("slot-1".to_string()),
            branch_name: Some("awo/worker-a".to_string()),
            read_only: false,
            write_scope: vec!["crates/awo-core/src/runtime.rs".to_string()],
            deliverable: "A tested runtime/session patch".to_string(),
            verification: vec!["cargo test".to_string()],
            verification_command: Some("cargo test".to_string()),
            depends_on: Vec::new(),
            result_summary: None,
            result_session_id: None,
            handoff_note: None,
            output_log_path: None,
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
        None,
        None,
    );

    assert_eq!(manifest.status, TeamStatus::Planning);
    assert_eq!(manifest.routing_preferences, None);
    assert_eq!(manifest.lead.member_id, "lead");
    assert_eq!(manifest.current_lead_member_id(), "lead");
    assert_eq!(manifest.current_lead_session_id(), None);
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
        None,
        None,
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
        fallback_runtime: None,
        fallback_model: None,
        routing_preferences: None,
    })?;
    manifest.add_task(TaskCard {
        task_id: "task-1".to_string(),
        title: "Implement feature".to_string(),
        summary: "Add the missing feature.".to_string(),
        owner_id: "worker-a".to_string(),
        runtime: None,
        model: None,
        slot_id: None,
        branch_name: None,
        read_only: false,
        write_scope: vec!["src/lib.rs".to_string()],
        deliverable: "A tested patch".to_string(),
        verification: vec!["cargo test".to_string()],
        verification_command: None,
        depends_on: Vec::new(),
        result_summary: None,
        result_session_id: None,
        handoff_note: None,
        output_log_path: None,
        state: TaskCardState::Todo,
    })?;

    let prompt = manifest.render_task_prompt("task-1")?;
    assert!(prompt.contains("Team objective"));
    assert!(prompt.contains("Relevant skills"));
    assert!(prompt.contains("Verification"));
    Ok(())
}

#[test]
fn current_lead_can_be_promoted_to_member() -> Result<()> {
    let mut manifest = sample_manifest();

    manifest.promote_current_lead("worker-a")?;

    assert_eq!(manifest.current_lead_member_id(), "worker-a");
    assert_eq!(
        manifest
            .current_lead_member()
            .map(|member| member.member_id.as_str()),
        Some("worker-a")
    );
    assert_eq!(manifest.current_lead_session_id(), None);
    Ok(())
}

#[test]
fn removing_current_lead_member_is_rejected() {
    let mut manifest = sample_manifest();
    manifest
        .promote_current_lead("worker-a")
        .expect("promotion should succeed");

    let error = manifest
        .remove_member("worker-a")
        .expect_err("remove should fail");
    assert!(error.to_string().contains("cannot remove the team lead"));
}

#[test]
fn binding_current_lead_session_requires_matching_member() -> Result<()> {
    let mut manifest = sample_manifest();

    manifest.bind_current_lead_session("lead", Some("session-1".to_string()))?;
    assert_eq!(manifest.current_lead_session_id(), Some("session-1"));

    let error = manifest
        .bind_current_lead_session("worker-a", Some("session-2".to_string()))
        .expect_err("binding should fail");
    assert!(error.to_string().contains("current lead is `lead`"));
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
        None,
        None,
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
                fallback_runtime: None,
                fallback_model: None,
                routing_preferences: None,
            })?;
            Ok(guard.save()?)
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

// ── Archive tests ──────────────────────────────────────────────────

#[test]
fn archive_succeeds_when_all_tasks_done() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Done;
    manifest.refresh_status();
    assert!(manifest.can_archive());
    manifest.archive()?;
    assert_eq!(manifest.status, TeamStatus::Archived);
    Ok(())
}

#[test]
fn archive_succeeds_when_tasks_done_or_blocked() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Blocked;
    assert!(manifest.can_archive());
    manifest.archive()?;
    assert_eq!(manifest.status, TeamStatus::Archived);
    Ok(())
}

#[test]
fn archive_refuses_in_progress_task() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::InProgress;
    assert!(!manifest.can_archive());
    let blockers = manifest.archive_blockers();
    assert_eq!(blockers.len(), 1);
    assert!(blockers[0].contains("in_progress"));
    assert!(manifest.archive().is_err());
}

#[test]
fn archive_refuses_review_task() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Review;
    assert!(!manifest.can_archive());
    assert!(manifest.archive().is_err());
}

#[test]
fn archive_refuses_todo_task() {
    let mut manifest = sample_manifest();
    // sample_manifest starts with Todo
    assert!(!manifest.can_archive());
    assert!(manifest.archive().is_err());
}

#[test]
fn archive_refuses_already_archived() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Done;
    manifest.status = TeamStatus::Archived;
    assert!(!manifest.can_archive());
    let blockers = manifest.archive_blockers();
    assert!(blockers.iter().any(|b| b.contains("already archived")));
}

#[test]
fn archive_empty_task_list_succeeds() -> Result<()> {
    let mut manifest = starter_team_manifest(
        "repo-1",
        "team-empty",
        "Test empty archive",
        Some("claude"),
        None,
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    assert!(manifest.can_archive());
    manifest.archive()?;
    assert_eq!(manifest.status, TeamStatus::Archived);
    Ok(())
}

// ── Reset tests ────────────────────────────────────────────────────

#[test]
fn reset_clears_all_task_state_and_bindings() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::InProgress;
    manifest.tasks[0].slot_id = Some("slot-1".to_string());
    manifest.tasks[0].branch_name = Some("awo/task-1".to_string());
    manifest.members[0].slot_id = Some("slot-1".to_string());
    manifest.members[0].branch_name = Some("awo/worker-a".to_string());
    manifest.lead.slot_id = Some("slot-lead".to_string());
    manifest.lead.branch_name = Some("awo/lead".to_string());

    manifest.reset();

    assert_eq!(manifest.status, TeamStatus::Planning);
    assert_eq!(manifest.tasks[0].state, TaskCardState::Todo);
    assert!(manifest.tasks[0].slot_id.is_none());
    assert!(manifest.tasks[0].branch_name.is_none());
    assert!(manifest.members[0].slot_id.is_none());
    assert!(manifest.members[0].branch_name.is_none());
    assert!(manifest.lead.slot_id.is_none());
    assert!(manifest.lead.branch_name.is_none());
}

#[test]
fn reset_summary_reports_non_todo_tasks_and_bound_members() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Review;
    manifest.members[0].slot_id = Some("slot-1".to_string());
    manifest.lead.slot_id = Some("slot-lead".to_string());

    let summary = manifest.reset_summary();
    assert_eq!(summary.non_todo_tasks.len(), 1);
    assert!(summary.non_todo_tasks[0].contains("task-1"));
    assert!(summary.non_todo_tasks[0].contains("review"));
    assert_eq!(summary.bound_members.len(), 2); // lead + worker-a
    assert!(summary.bound_members.contains(&"lead".to_string()));
    assert!(summary.bound_members.contains(&"worker-a".to_string()));
}

#[test]
fn reset_summary_empty_when_clean() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Todo;
    manifest.tasks[0].slot_id = None;
    manifest.tasks[0].branch_name = None;
    manifest.members[0].slot_id = None;
    manifest.members[0].branch_name = None;

    let summary = manifest.reset_summary();
    assert!(summary.non_todo_tasks.is_empty());
    assert!(summary.bound_members.is_empty());
}

#[test]
fn reset_archived_team_returns_to_planning() {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Done;
    manifest.status = TeamStatus::Archived;

    manifest.reset();

    assert_eq!(manifest.status, TeamStatus::Planning);
    assert_eq!(manifest.tasks[0].state, TaskCardState::Todo);
}

#[test]
fn accept_task_marks_review_ready_task_done() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Review;
    manifest.tasks[0].result_summary = Some("Ready for review.".to_string());

    manifest.accept_task("task-1")?;

    assert_eq!(manifest.tasks[0].state, TaskCardState::Done);
    assert_eq!(
        manifest.tasks[0].result_summary.as_deref(),
        Some("Ready for review.")
    );
    Ok(())
}

#[test]
fn request_task_rework_clears_review_result_and_reopens_todo() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Review;
    manifest.tasks[0].result_summary = Some("Needs follow-up".to_string());
    manifest.tasks[0].result_session_id = Some("session-1".to_string());
    manifest.tasks[0].handoff_note = Some("please tighten tests".to_string());
    manifest.tasks[0].output_log_path = Some("/tmp/log".to_string());

    manifest.request_task_rework("task-1")?;

    assert_eq!(manifest.tasks[0].state, TaskCardState::Todo);
    assert!(manifest.tasks[0].result_summary.is_none());
    assert!(manifest.tasks[0].result_session_id.is_none());
    assert!(manifest.tasks[0].handoff_note.is_none());
    assert!(manifest.tasks[0].output_log_path.is_none());
    Ok(())
}

// ── Storage remove test ────────────────────────────────────────────

#[test]
fn remove_team_manifest_deletes_file() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let manifest = sample_manifest();
    let path = save_team_manifest(&paths, &manifest)?;
    assert!(path.exists());

    remove_team_manifest(&paths, &manifest.team_id)?;
    assert!(!path.exists());

    let manifests = list_team_manifest_paths(&paths)?;
    assert!(manifests.is_empty());
    Ok(())
}

// ── Archived manifest round-trip through TOML ──────────────────────

#[test]
fn archived_status_survives_toml_roundtrip() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let mut manifest = sample_manifest();
    manifest.tasks[0].state = TaskCardState::Done;
    manifest.refresh_status();
    manifest.archive()?;

    let path = save_team_manifest(&paths, &manifest)?;
    let loaded = load_team_manifest(&path)?;
    assert_eq!(loaded.status, TeamStatus::Archived);
    Ok(())
}

#[test]
fn starter_manifest_with_fallback_fields() {
    let manifest = starter_team_manifest(
        "repo-1",
        "team-fb",
        "Test fallbacks",
        Some("claude"),
        Some("opus"),
        TeamExecutionMode::ExternalSlots,
        Some("gemini"),
        Some("flash"),
    );

    assert_eq!(manifest.lead.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(manifest.lead.fallback_model.as_deref(), Some("flash"));
}

#[test]
fn starter_manifest_without_fallback_fields() {
    let manifest = starter_team_manifest(
        "repo-1",
        "team-nofb",
        "Test no fallbacks",
        Some("claude"),
        None,
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );

    assert!(manifest.lead.fallback_runtime.is_none());
    assert!(manifest.lead.fallback_model.is_none());
}

#[test]
fn fallback_fields_survive_toml_roundtrip() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let mut manifest = sample_manifest();
    manifest.lead.fallback_runtime = Some("gemini".to_string());
    manifest.lead.fallback_model = Some("flash".to_string());
    manifest.members[0].fallback_runtime = Some("shell".to_string());

    let path = save_team_manifest(&paths, &manifest)?;
    let loaded = load_team_manifest(&path)?;
    assert_eq!(loaded.lead.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(loaded.lead.fallback_model.as_deref(), Some("flash"));
    assert_eq!(loaded.members[0].fallback_runtime.as_deref(), Some("shell"));
    assert!(loaded.members[0].fallback_model.is_none());
    Ok(())
}

#[test]
fn manifest_without_fallback_fields_loads_as_none() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let paths = sample_paths(temp_dir.path());
    let manifest = sample_manifest();

    let path = save_team_manifest(&paths, &manifest)?;
    let loaded = load_team_manifest(&path)?;
    assert!(loaded.lead.fallback_runtime.is_none());
    assert!(loaded.lead.fallback_model.is_none());
    assert!(loaded.members[0].fallback_runtime.is_none());
    assert!(loaded.members[0].fallback_model.is_none());
    Ok(())
}

#[test]
fn add_member_with_fallback_fields() -> Result<()> {
    let mut manifest = starter_team_manifest(
        "repo-1",
        "team-fb-member",
        "Test member fallbacks",
        Some("claude"),
        None,
        TeamExecutionMode::ExternalSlots,
        None,
        None,
    );
    manifest.add_member(TeamMember {
        member_id: "worker-fb".to_string(),
        role: "implementer".to_string(),
        runtime: Some("claude".to_string()),
        model: Some("opus".to_string()),
        execution_mode: TeamExecutionMode::ExternalSlots,
        slot_id: None,
        branch_name: None,
        read_only: false,
        write_scope: Vec::new(),
        context_packs: Vec::new(),
        skills: Vec::new(),
        notes: None,
        fallback_runtime: Some("codex".to_string()),
        fallback_model: Some("mini".to_string()),
        routing_preferences: None,
    })?;

    let member = manifest.member("worker-fb").expect("member should exist");
    assert_eq!(member.fallback_runtime.as_deref(), Some("codex"));
    assert_eq!(member.fallback_model.as_deref(), Some("mini"));
    Ok(())
}

#[test]
fn update_member_fallback_fields() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.update_member_policy(
        "worker-a",
        None,
        None,
        Some(Some("gemini".to_string())),
        Some(Some("flash".to_string())),
        None,
    )?;
    let member = manifest.member("worker-a").expect("member exists");
    assert_eq!(member.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(member.fallback_model.as_deref(), Some("flash"));
    assert_eq!(member.runtime.as_deref(), Some("codex"));
    assert_eq!(member.role, "implementer");
    Ok(())
}

#[test]
fn update_member_routing_defaults() -> Result<()> {
    let mut manifest = sample_manifest();
    let prefs = crate::routing::RoutingPreferences {
        prefer_local: true,
        avoid_metered: true,
        max_cost_tier: Some(crate::capabilities::CostTier::Standard),
        allow_fallback: false,
    };
    manifest.update_member_policy(
        "worker-a",
        None,
        None,
        None,
        None,
        Some(Some(prefs.clone())),
    )?;
    let member = manifest.member("worker-a").expect("member exists");
    assert_eq!(member.routing_preferences, Some(prefs));
    assert_eq!(member.runtime.as_deref(), Some("codex"));
    Ok(())
}

#[test]
fn clear_member_fallback_fields() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.update_member_policy(
        "worker-a",
        None,
        None,
        Some(Some("gemini".to_string())),
        Some(Some("flash".to_string())),
        None,
    )?;
    manifest.update_member_policy("worker-a", None, None, Some(None), Some(None), None)?;
    let member = manifest.member("worker-a").expect("member exists");
    assert!(member.fallback_runtime.is_none());
    assert!(member.fallback_model.is_none());
    Ok(())
}

#[test]
fn clear_member_routing_defaults() -> Result<()> {
    let mut manifest = sample_manifest();
    let prefs = crate::routing::RoutingPreferences {
        prefer_local: true,
        ..Default::default()
    };
    manifest.update_member_policy("worker-a", None, None, None, None, Some(Some(prefs)))?;
    manifest.update_member_policy("worker-a", None, None, None, None, Some(None))?;
    let member = manifest.member("worker-a").expect("member exists");
    assert!(member.routing_preferences.is_none());
    Ok(())
}

#[test]
fn update_member_omitted_flags_preserve_existing() -> Result<()> {
    let mut manifest = sample_manifest();
    let prefs = crate::routing::RoutingPreferences {
        prefer_local: true,
        ..Default::default()
    };
    manifest.update_member_policy(
        "worker-a",
        None,
        None,
        Some(Some("gemini".to_string())),
        Some(Some("flash".to_string())),
        Some(Some(prefs.clone())),
    )?;

    manifest.update_member_policy(
        "worker-a",
        None,
        Some(Some("opus".to_string())),
        None,
        None,
        None,
    )?;
    let member = manifest.member("worker-a").expect("member exists");
    assert_eq!(member.model.as_deref(), Some("opus"));
    assert_eq!(member.runtime.as_deref(), Some("codex"));
    assert_eq!(member.fallback_runtime.as_deref(), Some("gemini"));
    assert_eq!(member.fallback_model.as_deref(), Some("flash"));
    assert_eq!(member.routing_preferences, Some(prefs));
    Ok(())
}

#[test]
fn update_member_policy_unknown_member_fails() {
    let mut manifest = sample_manifest();
    let result = manifest.update_member_policy("nonexistent", None, None, None, None, None);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("unknown team member")
    );
}

#[test]
fn update_lead_member_policy() -> Result<()> {
    let mut manifest = sample_manifest();
    manifest.update_member_policy(
        "lead",
        Some(Some("gemini".to_string())),
        Some(Some("flash".to_string())),
        None,
        None,
        None,
    )?;
    let lead = manifest.member("lead").expect("lead exists");
    assert_eq!(lead.runtime.as_deref(), Some("gemini"));
    assert_eq!(lead.model.as_deref(), Some("flash"));
    Ok(())
}

// ── Negative-path parsing tests ────────────────────────────────────

#[test]
fn load_manifest_fails_on_empty_file() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("empty.toml");
    std::fs::write(&path, "")?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("missing field `version`"));
    Ok(())
}

#[test]
fn load_manifest_fails_on_malformed_toml() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("malformed.toml");
    std::fs::write(&path, "this is not valid toml = = =")?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn load_manifest_fails_on_missing_team_id() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("missing_team_id.toml");
    std::fs::write(
        &path,
        r#"version = 1
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []
tasks = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("missing field `team_id`"));
    Ok(())
}

#[test]
fn load_manifest_fails_on_missing_repo_id() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("missing_repo_id.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []
tasks = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("missing field `repo_id`"));
    Ok(())
}

#[test]
fn load_manifest_fails_on_missing_objective() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("missing_objective.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
status = "planning"
members = []
tasks = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("missing field `objective`"));
    Ok(())
}

#[test]
fn load_manifest_fails_on_missing_lead() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("missing_lead.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []
tasks = []
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("missing field `lead`"));
    Ok(())
}

#[test]
fn load_manifest_fails_on_invalid_task_state() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("invalid_task_state.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[tasks]]
task_id = "task-1"
title = "Implement running-session persistence"
summary = "Persist the session before one-shot completion."
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "A tested patch"
verification = []
depends_on = []
state = "exploded"
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn validation_catches_duplicate_member_ids() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("duplicate_member.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
tasks = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[members]]
member_id = "lead"
role = "implementer"
execution_mode = "external_slots"
read_only = false
write_scope = []
context_packs = []
skills = []
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("duplicate team member id `lead`"));
    Ok(())
}

#[test]
fn validation_catches_duplicate_task_ids() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("duplicate_task.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[tasks]]
task_id = "task-1"
title = "Implement 1"
summary = "Do it"
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "Code"
verification = []
depends_on = []
state = "todo"

[[tasks]]
task_id = "task-1"
title = "Implement 2"
summary = "Do it again"
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "Code"
verification = []
depends_on = []
state = "todo"
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(format!("{:?}", result.unwrap_err()).contains("duplicate task id `task-1`"));
    Ok(())
}

#[test]
fn validation_catches_nonexistent_task_dependency() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("unknown_dependency.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "team-alpha"
repo_id = "repo-1"
objective = "Ship a safe parallel implementation"
status = "planning"
members = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[tasks]]
task_id = "task-1"
title = "Implement 1"
summary = "Do it"
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "Code"
verification = []
depends_on = ["nonexistent-task"]
state = "todo"
"#,
    )?;

    let result = load_team_manifest(&path);
    assert!(result.is_err());
    assert!(
        format!("{:?}", result.unwrap_err()).contains("depends on unknown task `nonexistent-task`")
    );
    Ok(())
}

#[test]
fn legacy_pascal_case_enums_are_accepted() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("legacy.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "legacy-team"
repo_id = "repo-1"
objective = "Test backward compatibility"
status = "Running"
members = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "ExternalSlots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[tasks]]
task_id = "task-1"
title = "Legacy task"
summary = "Uses PascalCase enums"
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "Code"
verification = []
depends_on = []
state = "InProgress"
"#,
    )?;

    let manifest = load_team_manifest(&path)?;
    assert_eq!(manifest.status, TeamStatus::Running);
    assert_eq!(
        manifest.lead.execution_mode,
        TeamExecutionMode::ExternalSlots
    );
    assert_eq!(manifest.tasks[0].state, TaskCardState::InProgress);
    Ok(())
}

#[test]
fn snake_case_enums_are_canonical() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("canonical.toml");
    std::fs::write(
        &path,
        r#"version = 1
team_id = "canonical-team"
repo_id = "repo-1"
objective = "Test canonical format"
status = "running"
members = []

[lead]
member_id = "lead"
role = "lead"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = []
skills = []

[[tasks]]
task_id = "task-1"
title = "Canonical task"
summary = "Uses snake_case enums"
owner_id = "lead"
read_only = false
write_scope = []
deliverable = "Code"
verification = []
depends_on = []
state = "in_progress"
"#,
    )?;

    let manifest = load_team_manifest(&path)?;
    assert_eq!(manifest.status, TeamStatus::Running);
    assert_eq!(
        manifest.lead.execution_mode,
        TeamExecutionMode::ExternalSlots
    );
    assert_eq!(manifest.tasks[0].state, TaskCardState::InProgress);
    Ok(())
}
