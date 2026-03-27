use super::{CommandOutcome, CommandRunner};
use crate::error::{AwoError, AwoResult};
use crate::events::DomainEvent;
use crate::team::{
    PlanItem, TaskCard, TaskCardState, TeamManifestGuard, TeamMember, TeamTaskStartOptions,
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

        Ok(CommandOutcome::with_data(
            format!("Found {} team manifest(s).", manifests.len()),
            serde_json::to_value(&manifests).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_show(&self, team_id: String) -> AwoResult<CommandOutcome> {
        let manifest = crate::team::load_team_manifest(&crate::team::default_team_manifest_path(
            &self.config.paths,
            &team_id,
        ))?;

        Ok(CommandOutcome::with_data(
            format!("Loaded team `{}`.", team_id),
            serde_json::to_value(&manifest).unwrap_or(serde_json::Value::Null),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_team_init(
        &self,
        team_id: String,
        repo_id: String,
        objective: String,
        lead_runtime: Option<String>,
        lead_model: Option<String>,
        execution_mode: String,
        fallback_runtime: Option<String>,
        fallback_model: Option<String>,
        routing_preferences: Option<crate::routing::RoutingPreferences>,
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

        let execution_mode = execution_mode
            .parse::<crate::team::TeamExecutionMode>()
            .map_err(|e| AwoError::validation(format!("invalid execution mode: {e}")))?;

        if let Some(ref r) = lead_runtime {
            r.parse::<crate::runtime::RuntimeKind>()
                .map_err(|_| AwoError::unsupported("lead runtime", r))?;
        }
        if let Some(ref r) = fallback_runtime {
            r.parse::<crate::runtime::RuntimeKind>()
                .map_err(|_| AwoError::unsupported("fallback runtime", r))?;
        }

        let mut manifest = crate::team::starter_team_manifest(
            &repo_id,
            &team_id,
            &objective,
            lead_runtime.as_deref(),
            lead_model.as_deref(),
            execution_mode,
            fallback_runtime.as_deref(),
            fallback_model.as_deref(),
        );
        manifest.routing_preferences = routing_preferences;

        crate::team::save_team_manifest(&self.config.paths, &manifest)?;

        Ok(CommandOutcome::with_all(
            format!("Initialized team `{}` at {}.", team_id, path.display()),
            vec![DomainEvent::TeamCreated { team_id, repo_id }],
            serde_json::json!({
                "manifest": manifest,
                "manifest_path": path.display().to_string(),
            }),
        ))
    }

    pub(super) fn run_team_member_add(
        &self,
        team_id: String,
        member: TeamMember,
    ) -> AwoResult<CommandOutcome> {
        if let Some(ref r) = member.runtime {
            r.parse::<crate::runtime::RuntimeKind>()
                .map_err(|_| AwoError::unsupported("runtime", r))?;
        }
        if let Some(ref r) = member.fallback_runtime {
            r.parse::<crate::runtime::RuntimeKind>()
                .map_err(|_| AwoError::unsupported("fallback runtime", r))?;
        }

        let member_id = member.member_id.clone();
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().add_member(member)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Added member `{}` to team `{}`.", member_id, team_id),
            vec![DomainEvent::TeamMemberAdded { team_id, member_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_lead_replace(
        &self,
        team_id: String,
        member_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().promote_current_lead(&member_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!(
                "Current lead for team `{}` is now `{}`.",
                team_id, member_id
            ),
            vec![DomainEvent::TeamLeadReplaced { team_id, member_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_plan_add(
        &self,
        team_id: String,
        plan: PlanItem,
    ) -> AwoResult<CommandOutcome> {
        let plan_id = plan.plan_id.clone();
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().add_plan_item(plan)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Added plan item `{}` to team `{}`.", plan_id, team_id),
            vec![DomainEvent::TeamPlanAdded { team_id, plan_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_plan_approve(
        &self,
        team_id: String,
        plan_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().approve_plan_item(&plan_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Approved plan item `{}` in team `{}`.", plan_id, team_id),
            vec![DomainEvent::TeamPlanApproved { team_id, plan_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_plan_generate(
        &self,
        team_id: String,
        plan_id: String,
        task: TaskCard,
    ) -> AwoResult<CommandOutcome> {
        let task_id = task.task_id.clone();
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard
            .manifest_mut()
            .generate_task_from_plan_item(&plan_id, task)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!(
                "Generated task `{}` from plan item `{}` in team `{}`.",
                task_id, plan_id, team_id
            ),
            vec![DomainEvent::TeamPlanGenerated {
                team_id,
                plan_id,
                task_id,
            }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
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

        Ok(CommandOutcome::with_all(
            format!("Added task `{}` to team `{}`.", task_id, team_id),
            vec![DomainEvent::TeamTaskAdded { team_id, task_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_task_state(
        &self,
        team_id: String,
        task_id: String,
        state: TaskCardState,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().set_task_state(&task_id, state)?;
        guard.save()?;

        Ok(CommandOutcome::with_data(
            format!(
                "Set task `{}` in team `{}` to `{}`.",
                task_id, team_id, state
            ),
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_task_accept(
        &self,
        team_id: String,
        task_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().accept_task(&task_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Accepted task `{}` in team `{}`.", task_id, team_id),
            vec![DomainEvent::TeamTaskAccepted { team_id, task_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_task_rework(
        &self,
        team_id: String,
        task_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().request_task_rework(&task_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!(
                "Sent task `{}` back for rework in team `{}`.",
                task_id, team_id
            ),
            vec![DomainEvent::TeamTaskReworkRequested { team_id, task_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_task_cancel(
        &self,
        team_id: String,
        task_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        ensure_task_has_no_live_sessions(self, guard.manifest(), &task_id)?;
        guard.manifest_mut().cancel_task(&task_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Cancelled task `{}` in team `{}`.", task_id, team_id),
            vec![DomainEvent::TeamTaskCancelled { team_id, task_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
    }

    pub(super) fn run_team_task_supersede(
        &self,
        team_id: String,
        task_id: String,
        replacement_task_id: String,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        ensure_task_has_no_live_sessions(self, guard.manifest(), &task_id)?;
        guard
            .manifest_mut()
            .supersede_task(&task_id, &replacement_task_id)?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!(
                "Superseded task `{}` in team `{}` with `{}`.",
                task_id, team_id, replacement_task_id
            ),
            vec![DomainEvent::TeamTaskSuperseded {
                team_id,
                task_id,
                replacement_task_id,
            }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
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

    pub(super) fn run_team_task_delegate(
        &self,
        _options: crate::team::TeamTaskDelegateOptions,
    ) -> AwoResult<CommandOutcome> {
        Err(AwoError::unsupported(
            "team.task.delegate",
            "use AppCore::delegate_team_task directly for now",
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

        Ok(CommandOutcome::with_all(
            format!("Reset team `{}` to planning state.", team_id),
            vec![DomainEvent::TeamReset {
                team_id,
                tasks_reset: summary.non_todo_tasks.len(),
                slots_unbound: summary.bound_members.len(),
            }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
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

        let draft_plan_count = manifest
            .plan_items
            .iter()
            .filter(|plan| plan.state == crate::team::PlanItemState::Draft)
            .count();
        let approved_plan_count = manifest
            .plan_items
            .iter()
            .filter(|plan| plan.state == crate::team::PlanItemState::Approved)
            .count();
        let generated_plan_count = manifest
            .plan_items
            .iter()
            .filter(|plan| plan.state == crate::team::PlanItemState::Generated)
            .count();
        let review_tasks = manifest
            .tasks
            .iter()
            .filter(|task| task.state == crate::team::TaskCardState::Review)
            .collect::<Vec<_>>();
        let cleanup_tasks = manifest
            .tasks
            .iter()
            .filter(|task| task.state == crate::team::TaskCardState::Done && task.slot_id.is_some())
            .collect::<Vec<_>>();
        let history_tasks = manifest
            .tasks
            .iter()
            .filter(|task| {
                matches!(
                    task.state,
                    crate::team::TaskCardState::Cancelled | crate::team::TaskCardState::Superseded
                )
            })
            .collect::<Vec<_>>();

        report.push_str("## Queues\n\n");
        report.push_str(&format!(
            "- Planning: {} draft / {} approved / {} generated\n",
            draft_plan_count, approved_plan_count, generated_plan_count
        ));
        report.push_str(&format!("- Review: {}\n", review_tasks.len()));
        report.push_str(&format!("- Cleanup: {}\n", cleanup_tasks.len()));
        report.push_str(&format!("- History: {}\n\n", history_tasks.len()));

        report.push_str("## Plan Items\n\n");
        if manifest.plan_items.is_empty() {
            report.push_str("_No plan items._\n\n");
        } else {
            for plan in &manifest.plan_items {
                report.push_str(&format!("### Plan: {} ({})\n", plan.title, plan.plan_id));
                report.push_str(&format!("- **State**: {}\n", plan.state));
                report.push_str(&format!(
                    "- **Owner Intent**: {}\n",
                    plan.owner_id.as_deref().unwrap_or("-")
                ));
                if let Some(task_id) = &plan.generated_task_id {
                    report.push_str(&format!("- **Generated Task**: {}\n", task_id));
                }
                report.push_str(&format!("- **Summary**: {}\n\n", plan.summary));
            }
        }

        report.push_str("## Review Queue\n\n");
        if review_tasks.is_empty() {
            report.push_str("_No review-ready task cards._\n\n");
        } else {
            for task in &review_tasks {
                report.push_str(&format!(
                    "- `{}` owned by `{}` result={} handoff={}\n",
                    task.task_id,
                    task.owner_id,
                    task.result_summary.as_deref().unwrap_or("-"),
                    task.handoff_note.as_deref().unwrap_or("-")
                ));
            }
            report.push('\n');
        }

        report.push_str("## Cleanup Queue\n\n");
        if cleanup_tasks.is_empty() {
            report.push_str("_No done task cards still holding a slot._\n\n");
        } else {
            for task in &cleanup_tasks {
                report.push_str(&format!(
                    "- `{}` slot={} branch={}\n",
                    task.task_id,
                    task.slot_id.as_deref().unwrap_or("-"),
                    task.branch_name.as_deref().unwrap_or("-")
                ));
            }
            report.push('\n');
        }

        report.push_str("## History Queue\n\n");
        if history_tasks.is_empty() {
            report.push_str("_No cancelled or superseded task cards._\n\n");
        } else {
            for task in &history_tasks {
                report.push_str(&format!(
                    "- `{}` state={} replacement={}\n",
                    task.task_id,
                    task.state,
                    task.superseded_by_task_id.as_deref().unwrap_or("-")
                ));
            }
            report.push('\n');
        }

        report.push_str("## Task Cards\n\n");
        for task in &manifest.tasks {
            report.push_str(&format!("### Task: {} ({})\n", task.title, task.task_id));
            report.push_str(&format!("- **State**: {}\n", task.state));
            report.push_str(&format!("- **Owner**: {}\n", task.owner_id));
            if let Some(session_id) = &task.result_session_id {
                report.push_str(&format!("- **Result Session**: {}\n", session_id));
            }
            if let Some(summary) = &task.result_summary {
                report.push_str(&format!("- **Result**: {}\n", summary));
            }
            if let Some(handoff_note) = &task.handoff_note {
                report.push_str(&format!("- **Handoff**: {}\n", handoff_note));
            }
            report.push_str(&format!("- **Deliverable**: {}\n\n", task.deliverable));

            if let Some(log_path) = &task.output_log_path {
                report.push_str(&format!("#### Output Log\n`{}`\n\n", log_path));
            }
        }

        std::fs::write(&report_path, report)
            .map_err(|e| AwoError::io("write team report", &report_path, e))?;

        Ok(CommandOutcome::with_events(
            format!(
                "Generated report for team `{}` at `{}`.",
                team_id,
                report_path.display()
            ),
            vec![DomainEvent::TeamReportGenerated {
                team_id,
                report_path: report_path.display().to_string(),
            }],
        ))
    }

    pub(super) fn run_team_archive(
        &self,
        team_id: String,
        _force: bool,
    ) -> AwoResult<CommandOutcome> {
        let mut guard = TeamManifestGuard::load(&self.config.paths, &team_id)?;
        guard.manifest_mut().archive()?;
        guard.save()?;

        Ok(CommandOutcome::with_all(
            format!("Archived team `{}`.", team_id),
            vec![DomainEvent::TeamArchived { team_id }],
            serde_json::to_value(guard.manifest()).unwrap_or(serde_json::Value::Null),
        ))
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

        Ok(CommandOutcome::with_all(
            format!("Deleted team `{}` manifest.", team_id),
            vec![DomainEvent::TeamDeleted {
                team_id: team_id.clone(),
            }],
            serde_json::json!({
                "team_id": team_id,
                "deleted": true,
            }),
        ))
    }
}

fn ensure_task_has_no_live_sessions(
    runner: &CommandRunner<'_>,
    manifest: &crate::team::TeamManifest,
    task_id: &str,
) -> AwoResult<()> {
    let Some(slot_id) = manifest
        .task(task_id)
        .and_then(|task| task.slot_id.as_deref())
    else {
        return Ok(());
    };

    let live_sessions = runner
        .store
        .list_sessions_for_slot(slot_id)?
        .into_iter()
        .filter(|session| !session.is_terminal())
        .map(|session| session.id)
        .collect::<Vec<_>>();
    if live_sessions.is_empty() {
        return Ok(());
    }

    Err(AwoError::validation(format!(
        "task `{task_id}` still has active session(s) on slot `{slot_id}`: {}; cancel or release them first",
        live_sessions.join(", ")
    )))
}
