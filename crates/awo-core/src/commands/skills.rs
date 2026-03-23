use super::{CommandOutcome, CommandRunner};
use crate::diagnostics::DiagnosticSeverity;
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::skills::{
    RuntimeSkillRoots, SkillLinkMode, SkillRuntime, discover_repo_skills, doctor_repo_skills,
    link_repo_skills, sync_repo_skills,
};
use std::path::Path;

impl<'a> CommandRunner<'a> {
    pub(super) fn run_skills_list(&mut self, repo_id: String) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(&repo_id))?;
        let catalog = discover_repo_skills(Path::new(&repo.repo_root))?;
        self.store.insert_action(
            "skills_list",
            &format!("repo_id={} skills={}", repo.id, catalog.skills.len()),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "skills_list".to_string(),
            },
            DomainEvent::SkillsCatalogLoaded {
                repo_id: repo.id,
                skills: catalog.skills.len(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!("Discovered {} shared skill(s).", catalog.skills.len()),
            events,
        })
    }

    pub(super) fn run_skills_doctor(
        &mut self,
        repo_id: String,
        runtime: Option<SkillRuntime>,
    ) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(&repo_id))?;
        let catalog = discover_repo_skills(Path::new(&repo.repo_root))?;
        let roots = RuntimeSkillRoots::from_environment();
        let runtimes = runtime
            .map(|runtime| vec![runtime])
            .unwrap_or_else(|| SkillRuntime::all().to_vec());
        let reports = runtimes
            .iter()
            .copied()
            .map(|runtime| doctor_repo_skills(&catalog, runtime, &roots))
            .collect::<AwoResult<Vec<_>>>()?;
        let warning_count = reports
            .iter()
            .flat_map(|report| report.diagnostics.iter())
            .filter(|diag| diag.severity == DiagnosticSeverity::Warning)
            .count();
        self.store.insert_action(
            "skills_doctor",
            &format!(
                "repo_id={} runtimes={} warnings={}",
                repo.id,
                reports.len(),
                warning_count
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "skills_doctor".to_string(),
            },
            DomainEvent::SkillsDoctorCompleted {
                repo_id: repo.id,
                runtimes: reports.len(),
                warnings: warning_count,
            },
        ];

        Ok(CommandOutcome {
            summary: format!(
                "Skills doctor finished across {} runtime(s) with {} warning(s).",
                reports.len(),
                warning_count
            ),
            events,
        })
    }

    pub(super) fn run_skills_link(
        &mut self,
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(&repo_id))?;
        let catalog = discover_repo_skills(Path::new(&repo.repo_root))?;
        let roots = RuntimeSkillRoots::from_environment();
        let report = link_repo_skills(&catalog, runtime, &roots, mode)?;
        self.store.insert_action(
            "skills_link",
            &format!(
                "repo_id={} runtime={} mode={} linked={} skipped={}",
                repo.id,
                runtime,
                mode,
                report.linked.len(),
                report.skipped.len()
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "skills_link".to_string(),
            },
            DomainEvent::SkillsLinked {
                repo_id: repo.id,
                runtime: runtime.to_string(),
                linked: report.linked.len(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!(
                "Linked {} shared skill(s) into `{runtime}` using {mode}.",
                report.linked.len()
            ),
            events,
        })
    }

    pub(super) fn run_skills_sync(
        &mut self,
        repo_id: String,
        runtime: SkillRuntime,
        mode: SkillLinkMode,
    ) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| AwoError::unknown_repo(&repo_id))?;
        let catalog = discover_repo_skills(Path::new(&repo.repo_root))?;
        let roots = RuntimeSkillRoots::from_environment();
        let report = sync_repo_skills(&catalog, runtime, &roots, mode)?;
        self.store.insert_action(
            "skills_sync",
            &format!(
                "repo_id={} runtime={} mode={} linked={} updated={} pruned={} skipped={}",
                repo.id,
                runtime,
                mode,
                report.linked.len(),
                report.updated.len(),
                report.pruned.len(),
                report.skipped.len()
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "skills_sync".to_string(),
            },
            DomainEvent::SkillsSynced {
                repo_id: repo.id,
                runtime: runtime.to_string(),
                linked: report.linked.len() + report.updated.len(),
            },
        ];

        Ok(CommandOutcome {
            summary: format!(
                "Synced shared skills into `{runtime}`: {} added, {} repaired, {} pruned, {} already current.",
                report.linked.len(),
                report.updated.len(),
                report.pruned.len(),
                report.skipped.len()
            ),
            events,
        })
    }
}
