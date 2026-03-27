use super::super::forms::{
    ConfirmAction, ConfirmState, FormKind, FormState, blank_to_none, routing_preferences_from_form,
    split_csv,
};
use super::dashboard::{
    dashboard_member_ids, selected_dashboard_member, selected_dashboard_plan,
    selected_dashboard_task, selected_dashboard_team,
};
use super::{
    BackgroundResult, InputAction, InputMode, TeamDashboardFocus, TuiFocus, TuiState,
    append_events, apply_command, dispatch_in_background, fetch_session_log,
    refresh_team_dashboard_data, selected_repo, selected_session, selected_slot, visible_sessions,
};
use anyhow::{Result, anyhow};
use awo_core::runtime::{RuntimeKind, SessionLaunchMode};
use awo_core::team::{
    DelegationContext, PlanItem, PlanItemState, TaskCard, TaskCardState, TeamExecutionMode,
    TeamMember, TeamTaskDelegateOptions,
};
use awo_core::{AppCore, AppSnapshot, Command};
use crossbeam_channel::Sender;
use crossterm::event::KeyCode;

pub(super) fn handle_text_input_key(
    core: &mut AppCore,
    state: &mut TuiState,
    snapshot: &AppSnapshot,
    key: KeyCode,
    tx: Sender<BackgroundResult>,
) {
    let InputMode::TextInput {
        prompt_label: _,
        buffer,
        on_submit,
    } = &mut state.input_mode
    else {
        return;
    };

    match key {
        KeyCode::Enter => {
            let input = buffer.clone();
            let action = on_submit.clone();
            state.input_mode = InputMode::Normal;
            match action {
                InputAction::AcquireSlot => {
                    if let Some(repo) = selected_repo(snapshot, state) {
                        state.status = "Working...".to_string();
                        state.pending_ops += 1;
                        dispatch_in_background(
                            core.paths().clone(),
                            Command::SlotAcquire {
                                repo_id: repo.id.clone(),
                                task_name: input,
                                strategy: awo_core::SlotStrategy::Fresh,
                            },
                            tx,
                        );
                    }
                }
                InputAction::StartSession => {
                    let (runtime, prompt) = if let Some(colon) = input.find(':') {
                        let runtime = match &input[..colon] {
                            "claude" => RuntimeKind::Claude,
                            "codex" => RuntimeKind::Codex,
                            "gemini" => RuntimeKind::Gemini,
                            _ => RuntimeKind::Shell,
                        };
                        (runtime, input[colon + 1..].to_string())
                    } else {
                        (RuntimeKind::Shell, input)
                    };
                    if let Some(slot) = selected_slot(snapshot, state) {
                        apply_command(
                            core,
                            state,
                            Command::SessionStart {
                                slot_id: slot.id.clone(),
                                runtime,
                                prompt,
                                read_only: false,
                                dry_run: false,
                                launch_mode: SessionLaunchMode::default_for_environment(),
                                attach_context: false,
                                timeout_secs: None,
                            },
                        );
                        if let Ok(new_snapshot) = core.snapshot()
                            && let Some(session) = visible_sessions(&new_snapshot, state).last()
                        {
                            fetch_session_log(core, state, &session.id);
                            state.log_scroll = u16::MAX;
                        }
                    }
                }
                InputAction::SetFilter => {
                    state.filter_query = if input.trim().is_empty() {
                        None
                    } else {
                        Some(input.trim().to_lowercase())
                    };
                }
            }
        }
        KeyCode::Esc => state.input_mode = InputMode::Normal,
        KeyCode::Backspace => {
            buffer.pop();
        }
        KeyCode::Char(c) => buffer.push(c),
        _ => {}
    }
}

pub(super) fn handle_form_key(
    core: &mut AppCore,
    state: &mut TuiState,
    _snapshot: &AppSnapshot,
    key: KeyCode,
    tx: Sender<BackgroundResult>,
) {
    let InputMode::Form(mut form) = state.input_mode.clone() else {
        return;
    };

    match key {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            return;
        }
        KeyCode::Tab | KeyCode::Down => form.next_field(),
        KeyCode::BackTab | KeyCode::Up => form.prev_field(),
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(field) = form.selected_field_mut() {
                field.cycle(-1);
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(field) = form.selected_field_mut() {
                field.cycle(1);
            }
        }
        KeyCode::Backspace => {
            if let Some(field) = form.selected_field_mut()
                && !field.is_choice()
            {
                field.value.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(field) = form.selected_field_mut()
                && !field.is_choice()
            {
                field.value.push(c);
            }
        }
        KeyCode::Enter => match submit_form(core, state, &form, tx) {
            Ok(()) => return,
            Err(error) => form.set_error(error.to_string()),
        },
        _ => {}
    }

    state.input_mode = InputMode::Form(form);
}

pub(super) fn handle_confirm_key(core: &mut AppCore, state: &mut TuiState, key: KeyCode) {
    let InputMode::Confirm(confirm) = state.input_mode.clone() else {
        return;
    };

    match key {
        KeyCode::Esc => state.input_mode = InputMode::Normal,
        KeyCode::Enter => {
            state.input_mode = InputMode::Normal;
            match confirm.action {
                ConfirmAction::RemoveMember { team_id, member_id } => {
                    apply_command(
                        core,
                        state,
                        Command::TeamMemberRemove { team_id, member_id },
                    );
                    refresh_team_dashboard_data(core.paths(), state);
                }
                ConfirmAction::PromoteLead { team_id, member_id } => {
                    apply_command(core, state, Command::TeamLeadReplace { team_id, member_id });
                    refresh_team_dashboard_data(core.paths(), state);
                }
                ConfirmAction::AcceptTask { team_id, task_id } => {
                    apply_command(core, state, Command::TeamTaskAccept { team_id, task_id });
                    refresh_team_dashboard_data(core.paths(), state);
                }
                ConfirmAction::ReworkTask { team_id, task_id } => {
                    apply_command(core, state, Command::TeamTaskRework { team_id, task_id });
                    refresh_team_dashboard_data(core.paths(), state);
                }
                ConfirmAction::CancelTask { team_id, task_id } => {
                    apply_command(core, state, Command::TeamTaskCancel { team_id, task_id });
                    refresh_team_dashboard_data(core.paths(), state);
                }
                ConfirmAction::DeleteSlot { slot_id } => {
                    apply_command(core, state, Command::SlotDelete { slot_id });
                    refresh_team_dashboard_data(core.paths(), state);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn handle_enter(core: &mut AppCore, state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.focus == TuiFocus::TeamDashboard {
        if state.team_dashboard.focus == TeamDashboardFocus::Plan {
            if let Some(plan) = selected_dashboard_plan(state) {
                state.status = format!(
                    "Plan item: {} [{}] - {}",
                    plan.plan_id,
                    plan.state.as_str(),
                    plan.title
                );
            }
        } else if state.team_dashboard.focus == TeamDashboardFocus::Task {
            let result_session_id =
                selected_dashboard_task(state).and_then(|task| task.result_session_id.clone());
            if let Some(task) = selected_dashboard_task(state) {
                if let Some(session_id) = result_session_id.as_deref() {
                    fetch_session_log(core, state, session_id);
                    state.log_scroll = u16::MAX;
                } else {
                    state.status = format!("Task: {} - {}", task.task_id, task.title);
                }
            }
        } else if state.team_dashboard.focus == TeamDashboardFocus::Member
            && let Some(member) = selected_dashboard_member(state)
        {
            state.status = format!(
                "Member: {} runtime={} role={}",
                member.member_id,
                member.runtime.as_deref().unwrap_or("-"),
                member.role
            );
        }
    } else if state.focus == TuiFocus::Sessions {
        if let Some(session) = selected_session(snapshot, state) {
            fetch_session_log(core, state, &session.id);
        }
    } else if state.focus == TuiFocus::Slots && selected_slot(snapshot, state).is_some() {
        state.input_mode = InputMode::TextInput {
            prompt_label: "Prompt: ".to_string(),
            buffer: String::new(),
            on_submit: InputAction::StartSession,
        };
    }
}

pub(super) fn open_repo_add_form(state: &mut TuiState) {
    let default_path = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    state.input_mode = InputMode::Form(FormState::repo_add(default_path));
}

pub(super) fn open_team_init_form(snapshot: &AppSnapshot, state: &mut TuiState) {
    let repo_ids: Vec<String> = snapshot
        .registered_repos
        .iter()
        .map(|repo| repo.id.clone())
        .collect();
    if repo_ids.is_empty() {
        state.status = "Error: add a repository before creating a team.".to_string();
        return;
    }
    let selected_repo_id = selected_repo(snapshot, state).map(|repo| repo.id.clone());
    state.input_mode = InputMode::Form(FormState::team_init(repo_ids, selected_repo_id));
}

pub(super) fn open_member_add_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    state.input_mode = InputMode::Form(FormState::member_add(team.team_id.clone()));
}

pub(super) fn open_plan_add_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    state.input_mode = InputMode::Form(FormState::plan_add(
        team.team_id.clone(),
        dashboard_member_ids(team),
    ));
}

pub(super) fn approve_selected_plan_item(core: &mut AppCore, state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(plan) = selected_dashboard_plan(state) else {
        state.status = "Error: select a plan item to approve.".to_string();
        return;
    };
    if plan.state != PlanItemState::Draft {
        state.status = format!(
            "Error: plan item `{}` must be draft before approval.",
            plan.plan_id
        );
        return;
    }
    apply_command(
        core,
        state,
        Command::TeamPlanApprove {
            team_id: team.team_id.clone(),
            plan_id: plan.plan_id.clone(),
        },
    );
    refresh_team_dashboard_data(core.paths(), state);
}

pub(super) fn open_member_update_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(member) = selected_dashboard_member(state) else {
        state.status = "Error: select a member to update.".to_string();
        return;
    };
    state.input_mode = InputMode::Form(FormState::member_update(team.team_id.clone(), member));
}

pub(super) fn open_member_remove_confirm(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(member) = selected_dashboard_member(state) else {
        state.status = "Error: select a member to remove.".to_string();
        return;
    };
    if member.member_id == team.lead.member_id {
        state.status = "Error: the team lead cannot be removed.".to_string();
        return;
    }
    state.input_mode = InputMode::Confirm(ConfirmState::remove_member(
        team.team_id.clone(),
        member.member_id.clone(),
    ));
}

pub(super) fn open_promote_lead_confirm(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(member) = selected_dashboard_member(state) else {
        state.status = "Error: select a member to promote.".to_string();
        return;
    };
    if team.current_lead_member_id() == member.member_id {
        state.status = format!("`{}` is already the current lead.", member.member_id);
        return;
    }
    state.input_mode = InputMode::Confirm(ConfirmState::promote_lead(
        team.team_id.clone(),
        member.member_id.clone(),
    ));
}

pub(super) fn open_task_add_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let owner_ids = dashboard_member_ids(team);
    if owner_ids.is_empty() {
        state.status = "Error: add a team member before creating tasks.".to_string();
        return;
    }
    state.input_mode = InputMode::Form(FormState::task_add(team.team_id.clone(), owner_ids));
}

pub(super) fn open_plan_generate_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(plan) = selected_dashboard_plan(state) else {
        state.status = "Error: select a plan item to generate from.".to_string();
        return;
    };
    if plan.state != PlanItemState::Approved {
        state.status = format!(
            "Error: plan item `{}` must be approved before generating a task card.",
            plan.plan_id
        );
        return;
    }
    let owner_ids = dashboard_member_ids(team);
    if owner_ids.is_empty() {
        state.status = "Error: add a team member before generating task cards.".to_string();
        return;
    }
    let default_task_id = if team.task(&plan.plan_id).is_none() {
        plan.plan_id.clone()
    } else {
        format!("{}-task", plan.plan_id)
    };
    state.input_mode = InputMode::Form(FormState::plan_generate(
        team.team_id.clone(),
        plan,
        owner_ids,
        default_task_id,
    ));
}

pub(super) fn open_task_delegate_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(task) = team.tasks.get(state.team_dashboard.selected_task_idx) else {
        state.status = "Error: select a task card to delegate.".to_string();
        return;
    };
    let target_member_ids = dashboard_member_ids(team)
        .into_iter()
        .filter(|member_id| member_id != &task.owner_id)
        .collect::<Vec<_>>();
    if target_member_ids.is_empty() {
        state.status = "Error: add another member before delegating this task.".to_string();
        return;
    }
    state.input_mode = InputMode::Form(FormState::task_delegate(
        team.team_id.clone(),
        task.task_id.clone(),
        target_member_ids,
    ));
}

pub(super) fn open_task_accept_confirm(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card to accept.".to_string();
        return;
    };
    state.input_mode = InputMode::Confirm(ConfirmState::accept_task(
        team.team_id.clone(),
        task.task_id.clone(),
    ));
}

pub(super) fn open_task_rework_confirm(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card to send back for rework.".to_string();
        return;
    };
    state.input_mode = InputMode::Confirm(ConfirmState::rework_task(
        team.team_id.clone(),
        task.task_id.clone(),
    ));
}

pub(super) fn open_task_cancel_confirm(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card to cancel.".to_string();
        return;
    };
    state.input_mode = InputMode::Confirm(ConfirmState::cancel_task(
        team.team_id.clone(),
        task.task_id.clone(),
    ));
}

pub(super) fn open_task_supersede_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card to supersede.".to_string();
        return;
    };
    let replacement_task_ids = team
        .tasks
        .iter()
        .filter(|candidate| candidate.task_id != task.task_id)
        .filter(|candidate| {
            !matches!(
                candidate.state,
                TaskCardState::Cancelled | TaskCardState::Superseded
            )
        })
        .map(|candidate| candidate.task_id.clone())
        .collect::<Vec<_>>();
    if replacement_task_ids.is_empty() {
        state.status =
            "Error: add another non-cancelled task card before superseding this one.".to_string();
        return;
    }
    state.input_mode = InputMode::Form(FormState::task_supersede(
        team.team_id.clone(),
        task.task_id.clone(),
        replacement_task_ids,
    ));
}

fn submit_form(
    core: &mut AppCore,
    state: &mut TuiState,
    form: &FormState,
    tx: Sender<BackgroundResult>,
) -> Result<()> {
    match &form.kind {
        FormKind::RepoAdd => {
            let path = required_value(form, "path", "Repository path")?;
            state.input_mode = InputMode::Normal;
            state.status = "Working...".to_string();
            state.pending_ops += 1;
            dispatch_in_background(
                core.paths().clone(),
                Command::RepoAdd { path: path.into() },
                tx,
            );
        }
        FormKind::TeamInit => {
            let repo_id = required_value(form, "repo_id", "Repository")?;
            let team_id = required_value(form, "team_id", "Team ID")?;
            let objective = required_value(form, "objective", "Objective")?;
            let lead_runtime = form.value("lead_runtime").and_then(blank_to_none);
            let lead_model = form.value("lead_model").and_then(blank_to_none);
            apply_form_command(
                core,
                state,
                Command::TeamInit {
                    team_id,
                    repo_id,
                    objective,
                    lead_runtime,
                    lead_model,
                    execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
                    fallback_runtime: None,
                    fallback_model: None,
                    routing_preferences: None,
                    force: false,
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::PlanAdd { team_id } => {
            let plan_id = required_value(form, "plan_id", "Plan ID")?;
            let title = required_value(form, "title", "Title")?;
            let summary = required_value(form, "summary", "Summary")?;
            let owner_id = form.value("owner_id").and_then(blank_to_none);
            let runtime = form.value("runtime").and_then(blank_to_none);
            let model = form.value("model").and_then(blank_to_none);
            let read_only = parse_bool(form, "read_only")?;
            let write_scope = form.value("write_scope").map(split_csv).unwrap_or_default();
            let deliverable = form.value("deliverable").and_then(blank_to_none);
            let verification = form
                .value("verification")
                .map(split_csv)
                .unwrap_or_default();
            let depends_on = form.value("depends_on").map(split_csv).unwrap_or_default();
            let notes = form.value("notes").and_then(blank_to_none);
            apply_form_command(
                core,
                state,
                Command::TeamPlanAdd {
                    team_id: team_id.clone(),
                    plan: PlanItem {
                        plan_id,
                        title,
                        summary,
                        owner_id,
                        runtime,
                        model,
                        read_only,
                        write_scope,
                        deliverable,
                        verification,
                        depends_on,
                        notes,
                        state: PlanItemState::Draft,
                        generated_task_id: None,
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::PlanGenerate { team_id, plan_id } => {
            let task_id = required_value(form, "task_id", "Task ID")?;
            let owner_id = required_value(form, "owner_id", "Owner")?;
            let manifest = core.load_team_manifest(team_id)?;
            let plan = manifest
                .plan_item(plan_id)
                .cloned()
                .ok_or_else(|| anyhow!("unknown plan item `{plan_id}`"))?;
            let title = plan.title.clone();
            let summary = plan.summary.clone();
            let deliverable = form
                .value("deliverable")
                .and_then(blank_to_none)
                .or(plan.deliverable.clone())
                .ok_or_else(|| anyhow!("Deliverable is required"))?;
            apply_form_command(
                core,
                state,
                Command::TeamPlanGenerate {
                    team_id: team_id.clone(),
                    plan_id: plan_id.clone(),
                    task: TaskCard {
                        task_id,
                        title,
                        summary,
                        owner_id,
                        runtime: plan.runtime.clone(),
                        model: plan.model.clone(),
                        slot_id: None,
                        branch_name: None,
                        read_only: plan.read_only,
                        write_scope: plan.write_scope.clone(),
                        deliverable,
                        verification: plan.verification.clone(),
                        verification_command: None,
                        depends_on: plan.depends_on.clone(),
                        state: TaskCardState::Todo,
                        result_summary: None,
                        result_session_id: None,
                        handoff_note: None,
                        output_log_path: None,
                        superseded_by_task_id: None,
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::MemberAdd { team_id } => {
            let member_id = required_value(form, "member_id", "Member ID")?;
            let role = required_value(form, "role", "Role")?;
            let runtime = form.value("runtime").and_then(blank_to_none);
            let model = form.value("model").and_then(blank_to_none);
            let read_only = parse_bool(form, "read_only")?;
            apply_form_command(
                core,
                state,
                Command::TeamMemberAdd {
                    team_id: team_id.clone(),
                    member: TeamMember {
                        member_id,
                        role,
                        runtime,
                        model,
                        execution_mode: TeamExecutionMode::ExternalSlots,
                        slot_id: None,
                        branch_name: None,
                        read_only,
                        write_scope: Vec::new(),
                        context_packs: Vec::new(),
                        skills: Vec::new(),
                        notes: None,
                        fallback_runtime: None,
                        fallback_model: None,
                        routing_preferences: None,
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::MemberUpdate { team_id, member_id } => {
            let runtime = form.value("runtime").and_then(blank_to_none);
            let model = form.value("model").and_then(blank_to_none);
            let fallback_runtime = form.value("fallback_runtime").and_then(blank_to_none);
            let fallback_model = form.value("fallback_model").and_then(blank_to_none);
            let preferences = routing_preferences_from_form(form);
            apply_form_command(
                core,
                state,
                Command::TeamMemberUpdate {
                    team_id: team_id.clone(),
                    member_id: member_id.clone(),
                    runtime,
                    model,
                    fallback_runtime,
                    fallback_model,
                    clear_fallback: false,
                    routing_preferences: Some(preferences),
                    clear_routing_preferences: false,
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::TaskAdd { team_id } => {
            let task_id = required_value(form, "task_id", "Task ID")?;
            let owner_id = required_value(form, "owner_id", "Owner")?;
            let title = required_value(form, "title", "Title")?;
            let summary = required_value(form, "summary", "Summary")?;
            let deliverable = required_value(form, "deliverable", "Deliverable")?;
            let runtime = form.value("runtime").and_then(blank_to_none);
            let model = form.value("model").and_then(blank_to_none);
            let read_only = parse_bool(form, "read_only")?;
            let write_scope = form.value("write_scope").map(split_csv).unwrap_or_default();
            let verification = form
                .value("verification")
                .map(split_csv)
                .unwrap_or_default();
            let depends_on = form.value("depends_on").map(split_csv).unwrap_or_default();

            apply_form_command(
                core,
                state,
                Command::TeamTaskAdd {
                    team_id: team_id.clone(),
                    task: TaskCard {
                        task_id,
                        title,
                        summary,
                        owner_id,
                        runtime,
                        model,
                        slot_id: None,
                        branch_name: None,
                        read_only,
                        write_scope,
                        deliverable,
                        verification,
                        verification_command: None,
                        depends_on,
                        state: TaskCardState::Todo,
                        result_summary: None,
                        result_session_id: None,
                        handoff_note: None,
                        output_log_path: None,
                        superseded_by_task_id: None,
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::TaskDelegate { team_id, task_id } => {
            let target_member_id = required_value(form, "target_member_id", "Target member")?;
            let lead_notes = form.value("lead_notes").and_then(blank_to_none);
            let focus_files = form.value("focus_files").map(split_csv).unwrap_or_default();
            let auto_start = parse_bool(form, "auto_start")?;
            apply_form_command(
                core,
                state,
                Command::TeamTaskDelegate {
                    options: TeamTaskDelegateOptions {
                        team_id: team_id.clone(),
                        task_id: task_id.clone(),
                        delegation: DelegationContext {
                            target_member_id,
                            lead_notes,
                            focus_files,
                            auto_start,
                        },
                        strategy: "fresh".to_string(),
                        dry_run: !auto_start,
                        launch_mode: SessionLaunchMode::default_for_environment()
                            .as_str()
                            .to_string(),
                        attach_context: true,
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
        FormKind::TaskSupersede { team_id, task_id } => {
            let replacement_task_id =
                required_value(form, "replacement_task_id", "Replacement task card")?;
            apply_form_command(
                core,
                state,
                Command::TeamTaskSupersede {
                    team_id: team_id.clone(),
                    task_id: task_id.clone(),
                    replacement_task_id,
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core.paths(), state);
        }
    }

    Ok(())
}

fn apply_form_command(core: &mut AppCore, state: &mut TuiState, command: Command) -> Result<()> {
    match core.dispatch(command) {
        Ok(outcome) => {
            state.status = outcome.summary;
            append_events(state, outcome.events);
            state.last_snapshot_time = None;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn required_value(form: &FormState, key: &str, label: &str) -> Result<String> {
    form.value(key)
        .and_then(blank_to_none)
        .ok_or_else(|| anyhow!("{label} is required"))
}

fn parse_bool(form: &FormState, key: &str) -> Result<bool> {
    form.value(key)
        .ok_or_else(|| anyhow!("missing field `{key}`"))?
        .parse::<bool>()
        .map_err(|_| anyhow!("invalid boolean value for `{key}`"))
}
