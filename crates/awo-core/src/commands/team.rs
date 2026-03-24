use super::{CommandOutcome, CommandRunner};
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::team::{
    TaskCard, TeamExecutionMode, TeamManifestGuard, TeamMember, TeamTaskStartOptions,
};

impl CommandRunner<'_> {
    pub(super) fn run_team_list(&self, repo_id: Option<String>) -> AwoResult<CommandOutcome> {
        let manifests = if let Some(ref repo_id) = repo_id {
            let _repo = self
                .store
                .get_repository(repo_id)?
                .ok_or_else(|| self.repo_not_found_error(repo_id))?;
            crate::team::list_team_manifest_paths(&self.config.paths)?
                .into_iter()
                .filter_map(|path| crate::team::load_team_manifest(&path).ok())
                .filter(|m| m.repo_id == *repo_id)
                .collect()
        } else {
            crate::team::list_team_manifest_paths(&self.config.paths)?
                .into_iter()
                .filter_map(|path| crate::team::load_team_manifest(&path).ok())
                .collect::<Vec<_>>()
        };

        Ok(CommandOutcome {
            summary: format!("Found {} team manifest(s).", manifests.len()),
            events: vec![DomainEvent::TeamListLoaded {
                repo_id,
                count: manifests.len(),
            }],
        })
    }

    pub(super) fn run_team_show(&self, team_id: String) -> AwoResult<CommandOutcome> {
        let _manifest = crate::team::load_team_manifest(&crate::team::default_team_manifest_path(
            &self.config.paths,
            &team_id,
        ))?;

        Ok(CommandOutcome {
            summary: format!("Loaded team `{}`.", team_id),
            events: vec![DomainEvent::TeamLoaded { team_id }],
        })
    }

    pub(super) fn run_team_init(
        &self,
        team_id: String,
        repo_id: String,
        objective: String,
        force: bool,
    ) -> AwoResult<CommandOutcome> {
        let _repo = self
            .store
            .get_repository(&repo_id)?
            .ok_or_else(|| self.repo_not_found_error(&repo_id))?;
        let path = crate::team::default_team_manifest_path(&self.config.paths, &team_id);

        if path.exists() && !force {
            return Err(AwoError::validation(format!(
                "team manifest `{}` already exists; use --force to overwrite",
                team_id
            )));
        }

        let manifest = crate::team::starter_team_manifest(
            &repo_id,
            &team_id,
            &objective,
            None,
            None,
            TeamExecutionMode::ExternalSlots,
            None,
            None,
        );
        crate::team::save_team_manifest(&self.config.paths, &manifest)?;

        Ok(CommandOutcome {
            summary: format!("Initialized team `{}` for repo `{}`.", team_id, repo_id),
            events: vec![DomainEvent::TeamCreated { team_id, repo_id }],
        })
    }

    pub(super) fn run_team_member_add(
        &self,
        team_id: String,
        member: TeamMember,
    ) -> AwoResult<CommandOutcome> {
        let member_id = member.member_id.clone();
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().add_member(member)?;
        guard.save()?;

        Ok(CommandOutcome {
            summary: format!("Added member `{}` to team `{}`.", member_id, team_id),
            events: vec![DomainEvent::TeamMemberAdded { team_id, member_id }],
        })
    }

    pub(super) fn run_team_task_add(
        &self,
        team_id: String,
        task: TaskCard,
    ) -> AwoResult<CommandOutcome> {
        let task_id = task.task_id.clone();
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().add_task(task)?;
        guard.save()?;

        Ok(CommandOutcome {
            summary: format!("Added task `{}` to team `{}`.", task_id, team_id),
            events: vec![DomainEvent::TeamTaskAdded { team_id, task_id }],
        })
    }

    pub(super) fn run_team_task_start(
        &self,
        _options: TeamTaskStartOptions,
    ) -> AwoResult<CommandOutcome> {
        Err(AwoError::unsupported(
            "team.task.start",
            "use AppCore::start_team_task directly for now",
        ))
    }

    pub(super) fn run_team_reset(
        &self,
        team_id: String,
        _force: bool,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        let summary = guard.manifest().reset_summary();
        guard.manifest_mut().reset();
        guard.save()?;

        Ok(CommandOutcome {
            summary: format!("Reset team `{}` to planning state.", team_id),
            events: vec![DomainEvent::TeamReset {
                team_id,
                tasks_reset: summary.non_todo_tasks.len(),
                slots_unbound: summary.bound_members.len(),
            }],
        })
    }

    pub(super) fn run_team_report(&self, team_id: String) -> AwoResult<CommandOutcome> {
        let manifest = crate::team::load_team_manifest(&crate::team::default_team_manifest_path(
            &self.config.paths,
            &team_id,
        ))?;

        let report_dir = self.config.paths.data_dir.join("analysis").join("reports");
        std::fs::create_dir_all(&report_dir)
            .map_err(|e| AwoError::io("create reports directory", &report_dir, e))?;

        let filename = format!(
            "team-report-{}-{}.md",
            team_id,
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );
        let report_path = report_dir.join(filename);

        let mut report = format!("# Team Report: {}\n\n", team_id);
        report.push_str(&format!("**Objective**: {}\n", manifest.objective));
        report.push_str(&format!("**Status**: {}\n\n", manifest.status));

        report.push_str("## Tasks\n\n");
        for task in &manifest.tasks {
            report.push_str(&format!("### Task: {} ({})\n", task.title, task.task_id));
            report.push_str(&format!("- **State**: {}\n", task.state));
            report.push_str(&format!("- **Owner**: {}\n", task.owner_id));
            if let Some(summary) = &task.result_summary {
                report.push_str(&format!("- **Result**: {}\n", summary));
            }
            report.push_str(&format!("- **Deliverable**: {}\n\n", task.deliverable));

            if let Some(log_path) = &task.output_log_path {
                report.push_str(&format!("#### Output Log\n`{}`\n\n", log_path));
            }
        }

        std::fs::write(&report_path, report)
            .map_err(|e| AwoError::io("write team report", &report_path, e))?;

        Ok(CommandOutcome {
            summary: format!(
                "Generated report for team `{}` at `{}`.",
                team_id,
                report_path.display()
            ),
            events: vec![DomainEvent::TeamReportGenerated {
                team_id,
                report_path: report_path.display().to_string(),
            }],
        })
    }

    pub(super) fn run_team_archive(
        &self,
        team_id: String,
        _force: bool,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().archive()?;
        guard.save()?;

        Ok(CommandOutcome {
            summary: format!("Archived team `{}`.", team_id),
            events: vec![DomainEvent::TeamArchived { team_id }],
        })
    }

    pub(super) fn run_team_teardown(
        &self,
        _team_id: String,
        _force: bool,
    ) -> AwoResult<CommandOutcome> {
        Err(AwoError::unsupported(
            "team.teardown",
            "use AppCore::teardown_team directly for now",
        ))
    }

    pub(super) fn run_team_delete(&self, team_id: String) -> AwoResult<CommandOutcome> {
        let path = crate::team::default_team_manifest_path(&self.config.paths, &team_id);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| AwoError::io("delete team manifest", &path, e))?;
        }

        Ok(CommandOutcome {
            summary: format!("Deleted team `{}` manifest.", team_id),
            events: vec![DomainEvent::TeamDeleted { team_id }],
        })
    }
}
