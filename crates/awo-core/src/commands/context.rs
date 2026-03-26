use super::{CommandOutcome, CommandRunner};
use crate::context::{discover_repo_context, doctor_repo_context};
use crate::diagnostics::DiagnosticSeverity;
use crate::error::AwoResult;
use crate::events::DomainEvent;
use std::path::Path;

impl<'a> CommandRunner<'a> {
    pub(super) fn run_context_pack(&mut self, repo_id: String) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;
        let context = discover_repo_context(Path::new(&repo.repo_root))?;
        self.store.insert_action(
            "context_pack",
            &format!(
                "repo_id={} entrypoints={} packs={} docs={}",
                repo.id,
                context.entrypoints.len(),
                context.packs.len(),
                context.docs.len()
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "context_pack".to_string(),
            },
            DomainEvent::ContextLoaded {
                repo_id: repo.id,
                entrypoints: context.entrypoints.len(),
                packs: context.packs.len(),
            },
        ];

        Ok(CommandOutcome::with_events(
            format!(
                "Discovered {} entrypoint(s), {} standards doc(s), and {} context pack(s).",
                context.entrypoints.len(),
                context.standards.len(),
                context.packs.len()
            ),
            events,
        ))
    }

    pub(super) fn run_context_doctor(&mut self, repo_id: String) -> AwoResult<CommandOutcome> {
        let repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;
        let context = discover_repo_context(Path::new(&repo.repo_root))?;
        let report = doctor_repo_context(&context);
        let error_count = report
            .diagnostics
            .iter()
            .filter(|diag| diag.severity == DiagnosticSeverity::Error)
            .count();
        let warning_count = report
            .diagnostics
            .iter()
            .filter(|diag| diag.severity == DiagnosticSeverity::Warning)
            .count();
        self.store.insert_action(
            "context_doctor",
            &format!(
                "repo_id={} errors={} warnings={}",
                repo.id, error_count, warning_count
            ),
        )?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "context_doctor".to_string(),
            },
            DomainEvent::ContextDoctorCompleted {
                repo_id: repo.id,
                errors: error_count,
                warnings: warning_count,
            },
        ];

        Ok(CommandOutcome::with_events(
            format!(
                "Context doctor finished with {} error(s) and {} warning(s).",
                error_count, warning_count
            ),
            events,
        ))
    }
}
