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
