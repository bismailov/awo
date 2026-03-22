use super::*;
use crate::app::AppPaths;
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
        None,
        None,
    );

    assert_eq!(manifest.status, TeamStatus::Planning);
    assert_eq!(manifest.routing_preferences, None);
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
