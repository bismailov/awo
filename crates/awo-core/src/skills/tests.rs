use super::{
    RuntimeSkillRoots, SkillDiscoveryStrategy, SkillInstallState, SkillLinkMode, SkillRuntime,
};
use super::{discover_repo_skills, doctor_repo_skills, link_repo_skills, sync_repo_skills};
use crate::diagnostics::DiagnosticSeverity;
use anyhow::Result;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn discovers_shared_skills_and_lock_metadata() -> Result<()> {
    let repo = tempfile::tempdir()?;
    let skill_root = repo.path().join(".agents/skills/rust-skills");
    fs::create_dir_all(&skill_root)?;
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
    )?;
    fs::write(
        repo.path().join("skills-lock.json"),
        r#"{"version":1,"skills":{"rust-skills":{"source":"acme/rust-skills","computedHash":"abc"}}}"#,
    )?;

    let catalog = discover_repo_skills(repo.path())?;
    assert_eq!(catalog.skills.len(), 1);
    let skill = &catalog.skills[0];
    assert_eq!(skill.directory_name, "rust-skills");
    assert_eq!(skill.name.as_deref(), Some("rust-skills"));
    assert_eq!(skill.lock_source.as_deref(), Some("acme/rust-skills"));

    Ok(())
}

#[test]
fn links_skills_into_runtime_directory() -> Result<()> {
    let repo = tempfile::tempdir()?;
    let skill_root = repo.path().join(".agents/skills/rust-skills");
    fs::create_dir_all(&skill_root)?;
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
    )?;

    let runtime_root = tempfile::tempdir()?;
    let roots = RuntimeSkillRoots {
        codex: Some(runtime_root.path().join("codex-skills")),
        claude: None,
        gemini: None,
    };
    let catalog = discover_repo_skills(repo.path())?;
    let report = link_repo_skills(
        &catalog,
        SkillRuntime::Codex,
        &roots,
        SkillLinkMode::Symlink,
    )?;
    assert_eq!(report.linked, vec!["rust-skills".to_string()]);

    let doctor = doctor_repo_skills(&catalog, SkillRuntime::Codex, &roots)?;
    assert_eq!(doctor.entries.len(), 1);
    #[cfg(windows)]
    assert_eq!(doctor.entries[0].state, SkillInstallState::Copied);
    #[cfg(not(windows))]
    assert_eq!(doctor.entries[0].state, SkillInstallState::Linked);
    assert!(Path::new(&doctor.entries[0].target_path).exists());

    Ok(())
}

#[test]
fn gemini_doctor_prefers_project_local_skills() -> Result<()> {
    let repo = tempfile::tempdir()?;
    let skill_root = repo.path().join(".agents/skills/rust-skills");
    fs::create_dir_all(&skill_root)?;
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
    )?;

    let runtime_root = tempfile::tempdir()?;
    let roots = RuntimeSkillRoots {
        codex: None,
        claude: None,
        gemini: Some(runtime_root.path().join("gemini-skills")),
    };
    let catalog = discover_repo_skills(repo.path())?;
    let doctor = doctor_repo_skills(&catalog, SkillRuntime::Gemini, &roots)?;
    assert_eq!(
        doctor.policy.discovery,
        SkillDiscoveryStrategy::RepoLocalPreferred
    );
    assert_eq!(doctor.entries[0].state, SkillInstallState::ProjectLocal);
    assert!(
        doctor
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Warning)
    );

    Ok(())
}

#[test]
fn sync_repairs_drifted_copied_skills() -> Result<()> {
    let repo = tempfile::tempdir()?;
    let skill_root = repo.path().join(".agents/skills/rust-skills");
    fs::create_dir_all(skill_root.join("references"))?;
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
    )?;
    fs::write(skill_root.join("references/rules.md"), "rule-a\n")?;

    let runtime_root = tempfile::tempdir()?;
    let roots = RuntimeSkillRoots {
        codex: Some(runtime_root.path().join("codex-skills")),
        claude: None,
        gemini: None,
    };
    let catalog = discover_repo_skills(repo.path())?;
    link_repo_skills(&catalog, SkillRuntime::Codex, &roots, SkillLinkMode::Copy)?;

    fs::write(
        runtime_root
            .path()
            .join("codex-skills/rust-skills/references/rules.md"),
        "drifted\n",
    )?;

    let report = sync_repo_skills(&catalog, SkillRuntime::Codex, &roots, SkillLinkMode::Copy)?;
    assert_eq!(report.updated, vec!["rust-skills".to_string()]);

    let doctor = doctor_repo_skills(&catalog, SkillRuntime::Codex, &roots)?;
    assert_eq!(doctor.entries[0].state, SkillInstallState::Copied);

    Ok(())
}

#[cfg(unix)]
#[test]
fn sync_does_not_prune_symlink_that_escapes_shared_root_via_dotdot() -> Result<()> {
    let repo = tempfile::tempdir()?;
    let skill_root = repo.path().join(".agents/skills/rust-skills");
    fs::create_dir_all(&skill_root)?;
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: rust-skills\ndescription: Rust help\n---\n# Skill\n",
    )?;

    let outside_root = repo.path().join("outside");
    fs::create_dir_all(&outside_root)?;
    let escaped_target = repo.path().join(".agents/skills/../outside");

    let runtime_root = tempfile::tempdir()?;
    let target_dir = runtime_root.path().join("codex-skills");
    fs::create_dir_all(&target_dir)?;
    symlink(&escaped_target, target_dir.join("rogue-skill"))?;

    let roots = RuntimeSkillRoots {
        codex: Some(target_dir),
        claude: None,
        gemini: None,
    };
    let catalog = discover_repo_skills(repo.path())?;

    let report = sync_repo_skills(
        &catalog,
        SkillRuntime::Codex,
        &roots,
        SkillLinkMode::Symlink,
    )?;
    assert!(
        report.pruned.is_empty(),
        "escaped symlink target should not be pruned"
    );
    assert!(outside_root.exists());

    Ok(())
}
