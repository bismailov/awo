use super::forms::{
    ConfirmAction, ConfirmState, FormKind, FormState, blank_to_none, routing_preferences_from_form,
    split_csv,
};
use super::{
    BackgroundResult, InputAction, InputMode, TeamDashboardFocus, TuiFocus, TuiState,
    append_events, apply_command, dispatch_in_background, fetch_session_log,
    refresh_team_dashboard_data, selected_repo, selected_session, selected_slot, selected_team,
    start_next_team_task, start_team_task_explicit, visible_sessions,
};
use anyhow::{Result, anyhow};
use awo_core::runtime::{RuntimeKind, SessionLaunchMode};
use awo_core::team::{
    DelegationContext, TaskCard, TaskCardState, TeamExecutionMode, TeamMember,
    TeamTaskDelegateOptions,
};
use awo_core::{AppCore, AppSnapshot, Command};
use crossbeam_channel::Sender;
use crossterm::event::KeyCode;

pub(crate) enum KeyOutcome {
    Continue,
    Quit,
}

pub(crate) fn handle_key_event(
    core: &mut AppCore,
    state: &mut TuiState,
    snapshot: &AppSnapshot,
    key: KeyCode,
    tx: Sender<BackgroundResult>,
) -> KeyOutcome {
    if state.show_help {
        state.show_help = false;
        return KeyOutcome::Continue;
    }

    match state.input_mode.clone() {
        InputMode::TextInput { .. } => {
            handle_text_input_key(core, state, snapshot, key, tx);
            return KeyOutcome::Continue;
        }
        InputMode::Form(_) => {
            handle_form_key(core, state, snapshot, key, tx);
            return KeyOutcome::Continue;
        }
        InputMode::Confirm(_) => {
            handle_confirm_key(core, state, key);
            return KeyOutcome::Continue;
        }
        InputMode::Normal => {}
    }

    match key {
        KeyCode::Char('q') => KeyOutcome::Quit,
        KeyCode::Char('?') => {
            state.show_help = !state.show_help;
            KeyOutcome::Continue
        }
        KeyCode::Char('/') => {
            state.input_mode = InputMode::TextInput {
                prompt_label: "Filter: ".to_string(),
                buffer: String::new(),
                on_submit: InputAction::SetFilter,
            };
            KeyOutcome::Continue
        }
        KeyCode::Esc => {
            if state.focus == TuiFocus::TeamDashboard {
                state.focus = TuiFocus::Teams;
            } else if state.filter_query.is_some() {
                state.filter_query = None;
            } else if state.show_log_panel {
                state.show_log_panel = false;
            }
            KeyOutcome::Continue
        }
        KeyCode::Tab => {
            if state.focus == TuiFocus::TeamDashboard {
                state.team_dashboard.focus = match state.team_dashboard.focus {
                    TeamDashboardFocus::Team => TeamDashboardFocus::Member,
                    TeamDashboardFocus::Member => TeamDashboardFocus::Task,
                    TeamDashboardFocus::Task => TeamDashboardFocus::Team,
                };
            } else {
                state.focus = match state.focus {
                    TuiFocus::Repos => TuiFocus::Teams,
                    TuiFocus::Teams => TuiFocus::Slots,
                    TuiFocus::Slots => TuiFocus::Sessions,
                    TuiFocus::Sessions => TuiFocus::Repos,
                    TuiFocus::TeamDashboard => TuiFocus::Repos,
                };
            }
            KeyOutcome::Continue
        }
        KeyCode::BackTab => {
            if state.focus == TuiFocus::TeamDashboard {
                state.team_dashboard.focus = match state.team_dashboard.focus {
                    TeamDashboardFocus::Team => TeamDashboardFocus::Task,
                    TeamDashboardFocus::Member => TeamDashboardFocus::Team,
                    TeamDashboardFocus::Task => TeamDashboardFocus::Member,
                };
            } else {
                state.focus = match state.focus {
                    TuiFocus::Repos => TuiFocus::Sessions,
                    TuiFocus::Teams => TuiFocus::Repos,
                    TuiFocus::Slots => TuiFocus::Teams,
                    TuiFocus::Sessions => TuiFocus::Slots,
                    TuiFocus::TeamDashboard => TuiFocus::Teams,
                };
            }
            KeyOutcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            move_selection_up(state, snapshot);
            KeyOutcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            move_selection_down(state, snapshot);
            KeyOutcome::Continue
        }
        KeyCode::Char('s') => {
            if state.focus == TuiFocus::TeamDashboard {
                if let Some((team_id, task_id)) = selected_dashboard_task_ids(state) {
                    start_team_task_explicit(core, state, &team_id, &task_id);
                }
            } else if selected_repo(snapshot, state).is_some() {
                state.input_mode = InputMode::TextInput {
                    prompt_label: "Task name: ".to_string(),
                    buffer: String::new(),
                    on_submit: InputAction::AcquireSlot,
                };
            }
            KeyOutcome::Continue
        }
        KeyCode::Enter => {
            handle_enter(core, state, snapshot);
            KeyOutcome::Continue
        }
        KeyCode::Char('x') => {
            if let Some(session) = selected_session(snapshot, state) {
                apply_command(
                    core,
                    state,
                    Command::SessionCancel {
                        session_id: session.id.clone(),
                    },
                );
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('X') => {
            if state.focus == TuiFocus::TeamDashboard
                && state.team_dashboard.focus == TeamDashboardFocus::Task
            {
                if let Some(slot_id) =
                    selected_dashboard_task(state).and_then(|task| task.slot_id.clone())
                {
                    state.status = "Working...".to_string();
                    state.pending_ops += 1;
                    dispatch_in_background(
                        core.paths().clone(),
                        Command::SlotRelease { slot_id },
                        tx,
                    );
                } else {
                    state.status =
                        "Error: selected task card has no bound slot to release.".to_string();
                }
            } else if let Some(slot) = selected_slot(snapshot, state) {
                state.status = "Working...".to_string();
                state.pending_ops += 1;
                dispatch_in_background(
                    core.paths().clone(),
                    Command::SlotRelease {
                        slot_id: slot.id.clone(),
                    },
                    tx,
                );
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('a') => {
            open_repo_add_form(state);
            KeyOutcome::Continue
        }
        KeyCode::Char('n') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_add_form(state);
            } else {
                apply_command(
                    core,
                    state,
                    Command::NoOp {
                        label: "manual-noop".to_string(),
                    },
                );
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('r') => {
            if state.show_log_panel {
                if let Some(session_id) = state.log_session_id.clone() {
                    fetch_session_log(core, state, &session_id);
                    state.log_scroll = u16::MAX;
                }
            } else {
                apply_command(core, state, Command::ReviewStatus { repo_id: None });
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('R') => {
            if let Some(team) = selected_team(snapshot, state) {
                apply_command(
                    core,
                    state,
                    Command::TeamReport {
                        team_id: team.team_id.clone(),
                    },
                );
                refresh_team_dashboard_data(core, state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('c') => {
            if state.focus == TuiFocus::Teams {
                open_team_init_form(snapshot, state);
            } else if let Some(repo) = selected_repo(snapshot, state) {
                apply_command(
                    core,
                    state,
                    Command::ContextDoctor {
                        repo_id: repo.id.clone(),
                    },
                );
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('d') => {
            if state.focus == TuiFocus::TeamDashboard
                && state.team_dashboard.focus == TeamDashboardFocus::Member
            {
                open_member_remove_confirm(state);
            } else if let Some(repo) = selected_repo(snapshot, state) {
                apply_command(
                    core,
                    state,
                    Command::SkillsDoctor {
                        repo_id: repo.id.clone(),
                        runtime: None,
                    },
                );
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('D') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_delegate_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('A') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_accept_confirm(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('W') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_rework_confirm(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('o') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_selected_task_log(core, state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('K') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_slot_delete_confirm(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('m') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_member_add_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('L') => {
            if state.focus == TuiFocus::TeamDashboard
                && state.team_dashboard.focus == TeamDashboardFocus::Member
            {
                open_promote_lead_confirm(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('u') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_member_update_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('T') => {
            if state.focus == TuiFocus::TeamDashboard {
                state.focus = TuiFocus::Teams;
            } else {
                let selected_team_id =
                    selected_team(snapshot, state).map(|team| team.team_id.clone());
                refresh_team_dashboard_data(core, state);
                if let Some(team_id) = selected_team_id {
                    state.team_dashboard.selected_team_idx = state
                        .team_dashboard
                        .teams
                        .iter()
                        .position(|team| team.team_id == team_id)
                        .unwrap_or(0);
                }
                state.focus = TuiFocus::TeamDashboard;
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('t') => {
            if let Some(team) = selected_team(snapshot, state) {
                start_next_team_task(core, state, &team.team_id);
                refresh_team_dashboard_data(core, state);
            }
            KeyOutcome::Continue
        }
        _ => KeyOutcome::Continue,
    }
}

fn handle_text_input_key(
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

fn handle_form_key(
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

fn handle_confirm_key(core: &mut AppCore, state: &mut TuiState, key: KeyCode) {
    let InputMode::Confirm(confirm) = state.input_mode.clone() else {
        return;
    };

    match key {
        KeyCode::Esc => state.input_mode = InputMode::Normal,
        KeyCode::Enter => {
            state.input_mode = InputMode::Normal;
            match confirm.action {
                ConfirmAction::RemoveMember { team_id, member_id } => {
                    match core.remove_team_member(&team_id, &member_id) {
                        Ok(manifest) => {
                            state.status =
                                format!("Removed member `{member_id}` from team `{team_id}`.");
                            state.messages.push(format!(
                                "Team `{}` now has {} member(s).",
                                manifest.team_id,
                                manifest.members.len() + 1
                            ));
                            state.last_snapshot_time = None;
                            refresh_team_dashboard_data(core, state);
                        }
                        Err(error) => {
                            state.status = format!("Error: {error:#}");
                            state.messages.push(state.status.clone());
                        }
                    }
                }
                ConfirmAction::PromoteLead { team_id, member_id } => {
                    apply_command(core, state, Command::TeamLeadReplace { team_id, member_id });
                    refresh_team_dashboard_data(core, state);
                }
                ConfirmAction::AcceptTask { team_id, task_id } => {
                    apply_command(core, state, Command::TeamTaskAccept { team_id, task_id });
                    refresh_team_dashboard_data(core, state);
                }
                ConfirmAction::ReworkTask { team_id, task_id } => {
                    apply_command(core, state, Command::TeamTaskRework { team_id, task_id });
                    refresh_team_dashboard_data(core, state);
                }
                ConfirmAction::DeleteSlot { slot_id } => {
                    apply_command(core, state, Command::SlotDelete { slot_id });
                    refresh_team_dashboard_data(core, state);
                }
            }
        }
        _ => {}
    }
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
            refresh_team_dashboard_data(core, state);
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
            refresh_team_dashboard_data(core, state);
        }
        FormKind::MemberUpdate { team_id, member_id } => {
            let runtime = field_to_optional_option(form, "runtime");
            let model = field_to_optional_option(form, "model");
            let fallback_runtime = field_to_optional_option(form, "fallback_runtime");
            let fallback_model = field_to_optional_option(form, "fallback_model");
            let preferences = routing_preferences_from_form(form);
            match core.update_team_member_policy(
                team_id,
                member_id,
                runtime,
                model,
                fallback_runtime,
                fallback_model,
                Some(Some(preferences)),
            ) {
                Ok(_manifest) => {
                    state.input_mode = InputMode::Normal;
                    state.status = format!("Updated member `{member_id}`.");
                    state.last_snapshot_time = None;
                    refresh_team_dashboard_data(core, state);
                }
                Err(error) => return Err(error.into()),
            }
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
                    },
                },
            )?;
            state.input_mode = InputMode::Normal;
            refresh_team_dashboard_data(core, state);
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
            refresh_team_dashboard_data(core, state);
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

fn field_to_optional_option(form: &FormState, key: &str) -> Option<Option<String>> {
    form.value(key).map(blank_to_none)
}

fn move_selection_up(state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.show_log_panel {
        state.log_scroll = state.log_scroll.saturating_sub(1);
    } else if state.focus == TuiFocus::TeamDashboard {
        match state.team_dashboard.focus {
            TeamDashboardFocus::Team => {
                if state.team_dashboard.selected_team_idx > 0 {
                    state.team_dashboard.selected_team_idx -= 1;
                    state.team_dashboard.selected_task_idx = 0;
                    state.team_dashboard.selected_member_idx = 0;
                }
            }
            TeamDashboardFocus::Member => {
                if state.team_dashboard.selected_member_idx > 0 {
                    state.team_dashboard.selected_member_idx -= 1;
                }
            }
            TeamDashboardFocus::Task => {
                if state.team_dashboard.selected_task_idx > 0 {
                    state.team_dashboard.selected_task_idx -= 1;
                }
            }
        }
    } else {
        match state.focus {
            TuiFocus::Repos => {
                if state.selected_repo_index > 0 {
                    state.selected_repo_index -= 1;
                }
            }
            TuiFocus::Teams => {
                if state.selected_team_index > 0 {
                    state.selected_team_index -= 1;
                }
            }
            TuiFocus::Slots => {
                if state.selected_slot_index > 0 {
                    state.selected_slot_index -= 1;
                }
            }
            TuiFocus::Sessions => {
                if state.selected_session_index > 0 {
                    state.selected_session_index -= 1;
                }
            }
            TuiFocus::TeamDashboard => {}
        }
    }
    super::clamp_selection(state, snapshot);
}

fn move_selection_down(state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.show_log_panel {
        state.log_scroll = state.log_scroll.saturating_add(1);
    } else if state.focus == TuiFocus::TeamDashboard {
        match state.team_dashboard.focus {
            TeamDashboardFocus::Team => {
                if state.team_dashboard.selected_team_idx + 1 < state.team_dashboard.teams.len() {
                    state.team_dashboard.selected_team_idx += 1;
                    state.team_dashboard.selected_task_idx = 0;
                    state.team_dashboard.selected_member_idx = 0;
                }
            }
            TeamDashboardFocus::Member => {
                let member_count = selected_dashboard_member_count(state);
                if state.team_dashboard.selected_member_idx + 1 < member_count {
                    state.team_dashboard.selected_member_idx += 1;
                }
            }
            TeamDashboardFocus::Task => {
                if let Some(team) = selected_dashboard_team(state)
                    && state.team_dashboard.selected_task_idx + 1 < team.tasks.len()
                {
                    state.team_dashboard.selected_task_idx += 1;
                }
            }
        }
    } else {
        match state.focus {
            TuiFocus::Repos => {
                if state.selected_repo_index + 1 < super::visible_repos(snapshot, state).len() {
                    state.selected_repo_index += 1;
                }
            }
            TuiFocus::Teams => {
                if state.selected_team_index + 1 < super::visible_teams(snapshot, state).len() {
                    state.selected_team_index += 1;
                }
            }
            TuiFocus::Slots => {
                if state.selected_slot_index + 1 < super::visible_slots(snapshot, state).len() {
                    state.selected_slot_index += 1;
                }
            }
            TuiFocus::Sessions => {
                if state.selected_session_index + 1 < super::visible_sessions(snapshot, state).len()
                {
                    state.selected_session_index += 1;
                }
            }
            TuiFocus::TeamDashboard => {}
        }
    }
    super::clamp_selection(state, snapshot);
}

fn handle_enter(core: &mut AppCore, state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.focus == TuiFocus::TeamDashboard {
        if state.team_dashboard.focus == TeamDashboardFocus::Task {
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

fn open_repo_add_form(state: &mut TuiState) {
    let default_path = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    state.input_mode = InputMode::Form(FormState::repo_add(default_path));
}

fn open_team_init_form(snapshot: &AppSnapshot, state: &mut TuiState) {
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

fn open_member_add_form(state: &mut TuiState) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    state.input_mode = InputMode::Form(FormState::member_add(team.team_id.clone()));
}

fn open_member_update_form(state: &mut TuiState) {
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

fn open_member_remove_confirm(state: &mut TuiState) {
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

fn open_promote_lead_confirm(state: &mut TuiState) {
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

fn open_task_add_form(state: &mut TuiState) {
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

fn open_task_delegate_form(state: &mut TuiState) {
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

fn open_task_accept_confirm(state: &mut TuiState) {
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

fn open_task_rework_confirm(state: &mut TuiState) {
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

fn open_slot_delete_confirm(state: &mut TuiState) {
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card first.".to_string();
        return;
    };
    let Some(slot_id) = task.slot_id.clone() else {
        state.status = "Error: selected task card has no bound slot.".to_string();
        return;
    };
    state.input_mode = InputMode::Confirm(ConfirmState::delete_slot(slot_id));
}

fn selected_dashboard_team(state: &TuiState) -> Option<&awo_core::TeamManifest> {
    state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
}

fn selected_dashboard_task(state: &TuiState) -> Option<&TaskCard> {
    let team = selected_dashboard_team(state)?;
    team.tasks.get(state.team_dashboard.selected_task_idx)
}

fn selected_dashboard_member_count(state: &TuiState) -> usize {
    selected_dashboard_team(state)
        .map(|team| team.members.len() + 1)
        .unwrap_or(0)
}

fn selected_dashboard_member(state: &TuiState) -> Option<&TeamMember> {
    let team = selected_dashboard_team(state)?;
    if state.team_dashboard.selected_member_idx == 0 {
        Some(&team.lead)
    } else {
        team.members
            .get(state.team_dashboard.selected_member_idx.saturating_sub(1))
    }
}

fn selected_dashboard_task_ids(state: &TuiState) -> Option<(String, String)> {
    let team = selected_dashboard_team(state)?;
    let task = selected_dashboard_task(state)?;
    Some((team.team_id.clone(), task.task_id.clone()))
}

fn open_selected_task_log(core: &mut AppCore, state: &mut TuiState) {
    let Some(session_id) =
        selected_dashboard_task(state).and_then(|task| task.result_session_id.clone())
    else {
        if selected_dashboard_task(state).is_none() {
            state.status = "Error: select a task card first.".to_string();
        } else {
            state.status = "Error: selected task card has no result session log yet.".to_string();
        }
        return;
    };
    fetch_session_log(core, state, &session_id);
    state.log_scroll = u16::MAX;
}

fn dashboard_member_ids(team: &awo_core::TeamManifest) -> Vec<String> {
    std::iter::once(team.lead.member_id.clone())
        .chain(team.members.iter().map(|member| member.member_id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{KeyOutcome, handle_key_event};
    use crate::tui::forms::{FormKind, FormState};
    use crate::tui::{
        InputAction, InputMode, TeamDashboardFocus, TeamDashboardState, TuiFocus, TuiState,
    };
    use anyhow::Result;
    use awo_core::capabilities::CostTier;
    use awo_core::{AppCore, Command, TaskCardState, TeamExecutionMode};
    use crossbeam_channel::unbounded;
    use crossterm::event::KeyCode;
    use std::path::{Path, PathBuf};
    use std::process::Command as ProcessCommand;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TestEnv {
        root: PathBuf,
        config_dir: PathBuf,
        data_dir: PathBuf,
    }

    impl TestEnv {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("awo-tui-unit-{}-{id}", std::process::id()));
            let config_dir = root.join("config");
            let data_dir = root.join("data");
            std::fs::create_dir_all(&config_dir).expect("config dir");
            std::fs::create_dir_all(&data_dir).expect("data dir");
            Self {
                root,
                config_dir,
                data_dir,
            }
        }

        fn core(&self) -> AppCore {
            AppCore::with_dirs(self.config_dir.clone(), self.data_dir.clone()).expect("app core")
        }

        fn create_repo(&self, name: &str) -> PathBuf {
            let repo_dir = self.root.join("repos").join(name);
            std::fs::create_dir_all(&repo_dir).expect("repo dir");
            self.git(&repo_dir, &["init", "-b", "main"]);
            std::fs::write(repo_dir.join("README.md"), "hello\n").expect("write readme");
            self.git(&repo_dir, &["add", "README.md"]);
            self.git_with_identity(&repo_dir, &["commit", "-m", "init"]);
            repo_dir
        }

        fn git(&self, dir: &Path, args: &[&str]) {
            let output = ProcessCommand::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn git_with_identity(&self, dir: &Path, args: &[&str]) {
            let output = ProcessCommand::new("git")
                .args([
                    "-c",
                    "user.name=AWO Tests",
                    "-c",
                    "user.email=awo-tests@example.com",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn base_state() -> TuiState {
        TuiState {
            status: String::new(),
            messages: Vec::new(),
            focus: TuiFocus::Repos,
            selected_repo_index: 0,
            selected_team_index: 0,
            selected_slot_index: 0,
            selected_session_index: 0,
            log_content: None,
            log_session_id: None,
            log_path: None,
            show_log_panel: false,
            pending_ops: 0,
            input_mode: InputMode::Normal,
            show_help: false,
            log_scroll: 0,
            filter_query: None,
            team_dashboard: TeamDashboardState {
                selected_team_idx: 0,
                selected_member_idx: 0,
                selected_task_idx: 0,
                teams: Vec::new(),
                focus: TeamDashboardFocus::Team,
            },
            last_snapshot: None,
            last_snapshot_time: None,
        }
    }

    #[test]
    fn teams_tab_create_key_opens_team_init_form() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let snapshot = core.snapshot()?;

        let mut state = base_state();
        state.focus = TuiFocus::Teams;
        let (tx, _rx) = unbounded();

        let outcome = handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('c'), tx);
        assert!(matches!(outcome, KeyOutcome::Continue));
        match &state.input_mode {
            InputMode::Form(form) => assert!(matches!(form.kind, FormKind::TeamInit)),
            other => panic!("expected team init form, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn team_init_validation_error_stays_in_form() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        let form_repo_id = repo_id.clone();
        core.dispatch(Command::TeamInit {
            team_id: "alpha".to_string(),
            repo_id: repo_id.clone(),
            objective: "Existing".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.input_mode = InputMode::Form(FormState::team_init(vec![form_repo_id.clone()], None));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "repo_id", &form_repo_id);
            set_value(form, "team_id", "alpha");
            set_value(form, "objective", "Duplicate");
        }

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        match &state.input_mode {
            InputMode::Form(form) => {
                assert!(matches!(form.kind, FormKind::TeamInit));
                assert!(
                    form.error
                        .as_deref()
                        .is_some_and(|error| error.contains("already exists"))
                );
            }
            other => panic!("expected form to remain open, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn team_dashboard_open_preserves_selected_team() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        for team_id in ["alpha", "beta"] {
            core.dispatch(Command::TeamInit {
                team_id: team_id.to_string(),
                repo_id: repo_id.clone(),
                objective: format!("Objective for {team_id}"),
                lead_runtime: None,
                lead_model: None,
                execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
                fallback_runtime: None,
                fallback_model: None,
                routing_preferences: None,
                force: false,
            })?;
        }

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::Teams;
        state.selected_team_index = 1;

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('T'), tx);

        assert_eq!(state.focus, TuiFocus::TeamDashboard);
        assert_eq!(
            state.team_dashboard.teams[state.team_dashboard.selected_team_idx].team_id,
            "beta"
        );
        Ok(())
    }

    #[test]
    fn repo_add_form_submission_dispatches_background_command() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        let snapshot = core.snapshot()?;
        let (tx, rx) = unbounded();
        let mut state = base_state();
        state.input_mode = InputMode::Form(FormState::repo_add(repo_dir.display().to_string()));

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let result = rx.recv().expect("background repo add result");
        assert!(
            result.error.is_none(),
            "unexpected error: {:?}",
            result.error
        );
        assert!(
            result
                .events
                .iter()
                .any(|event| matches!(event, awo_core::DomainEvent::RepoRegistered { .. }))
        );
        Ok(())
    }

    #[test]
    fn enter_only_opens_session_prompt_from_slots_focus() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        core.dispatch(Command::SlotAcquire {
            repo_id,
            task_name: "slot-task".to_string(),
            strategy: awo_core::SlotStrategy::Fresh,
        })?;
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();

        let mut state = base_state();
        state.focus = TuiFocus::Repos;
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        assert!(matches!(state.input_mode, InputMode::Normal));

        state.focus = TuiFocus::Slots;
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);
        assert!(matches!(
            state.input_mode,
            InputMode::TextInput {
                on_submit: InputAction::StartSession,
                ..
            }
        ));

        Ok(())
    }

    #[test]
    fn filtered_repo_navigation_uses_visible_repo_count() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir_a = env.create_repo("alpha");
        let repo_dir_b = env.create_repo("beta");
        let mut core = env.core();
        core.dispatch(Command::RepoAdd { path: repo_dir_a })?;
        core.dispatch(Command::RepoAdd { path: repo_dir_b })?;
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();

        let mut state = base_state();
        state.focus = TuiFocus::Repos;
        state.filter_query = Some("alpha".to_string());

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Down, tx);
        assert_eq!(state.selected_repo_index, 0);

        Ok(())
    }

    #[test]
    fn member_add_update_remove_and_task_add_delegate_forms_work() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("app");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        core.dispatch(Command::TeamInit {
            team_id: "alpha".to_string(),
            repo_id,
            objective: "Ship feature".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;

        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        super::refresh_team_dashboard_data(&core, &mut state);
        state.team_dashboard.focus = TeamDashboardFocus::Member;
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();

        state.input_mode = InputMode::Form(FormState::member_add("alpha".to_string()));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "member_id", "worker-1");
            set_value(form, "role", "worker");
            set_value(form, "runtime", "codex");
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.members.len(), 1);
        assert_eq!(manifest.members[0].member_id, "worker-1");

        super::refresh_team_dashboard_data(&core, &mut state);
        state.team_dashboard.selected_member_idx = 1;
        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('L'),
            tx.clone(),
        );
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.current_lead_member_id(), "worker-1");

        state.input_mode = InputMode::Form(FormState::member_update(
            "alpha".to_string(),
            &core.load_team_manifest("alpha")?.members[0],
        ));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "runtime", "claude");
            set_value(form, "model", "sonnet");
            set_value(form, "allow_fallback", "false");
            set_value(form, "prefer_local", "true");
            set_value(form, "max_cost_tier", CostTier::Standard.as_str());
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.members[0].runtime.as_deref(), Some("claude"));
        assert_eq!(manifest.members[0].model.as_deref(), Some("sonnet"));
        assert!(
            !manifest.members[0]
                .routing_preferences
                .as_ref()
                .expect("routing prefs")
                .allow_fallback
        );

        state.input_mode = InputMode::Form(FormState::task_add(
            "alpha".to_string(),
            vec!["lead".to_string(), "worker-1".to_string()],
        ));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "task_id", "task-1");
            set_value(form, "owner_id", "worker-1");
            set_value(form, "title", "Implement");
            set_value(form, "summary", "Build the thing");
            set_value(form, "runtime", "codex");
            set_value(form, "deliverable", "Patch");
            set_value(form, "verification", "cargo test");
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks.len(), 1);
        assert_eq!(manifest.tasks[0].owner_id, "worker-1");

        state.team_dashboard.focus = TeamDashboardFocus::Task;
        state.input_mode = InputMode::Form(FormState::task_delegate(
            "alpha".to_string(),
            "task-1".to_string(),
            vec!["lead".to_string()],
        ));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "target_member_id", "lead");
            set_value(form, "auto_start", "false");
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks[0].owner_id, "lead");
        assert_eq!(manifest.tasks[0].state, TaskCardState::Todo);

        state.team_dashboard.focus = TeamDashboardFocus::Member;
        state.team_dashboard.selected_member_idx = 0;
        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('L'),
            tx.clone(),
        );
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());

        state.team_dashboard.selected_member_idx = 1;
        state.input_mode = InputMode::Confirm(super::ConfirmState::remove_member(
            "alpha".to_string(),
            "worker-1".to_string(),
        ));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);
        let manifest = core.load_team_manifest("alpha")?;
        assert!(manifest.members.is_empty());

        Ok(())
    }

    #[test]
    fn accept_action_marks_selected_review_task_done() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("review-accept");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        core.dispatch(Command::TeamInit {
            team_id: "alpha".to_string(),
            repo_id,
            objective: "Review and accept".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;
        core.dispatch(Command::TeamTaskAdd {
            team_id: "alpha".to_string(),
            task: awo_core::TaskCard {
                task_id: "task-1".to_string(),
                title: "Ship it".to_string(),
                summary: "Ready for review".to_string(),
                owner_id: "lead".to_string(),
                runtime: None,
                model: None,
                slot_id: None,
                branch_name: None,
                read_only: false,
                write_scope: Vec::new(),
                deliverable: "Patch".to_string(),
                verification: vec!["cargo test".to_string()],
                verification_command: None,
                depends_on: Vec::new(),
                state: TaskCardState::Todo,
                result_summary: None,
                result_session_id: None,
                handoff_note: None,
                output_log_path: None,
            },
        })?;
        core.dispatch(Command::TeamTaskState {
            team_id: "alpha".to_string(),
            task_id: "task-1".to_string(),
            state: TaskCardState::Review,
        })?;

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Task;
        super::refresh_team_dashboard_data(&core, &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('A'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Confirm(_)));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks[0].state, TaskCardState::Done);
        Ok(())
    }

    #[test]
    fn rework_action_clears_review_result_and_reopens_task() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("review-rework");
        let mut core = env.core();
        let repo_outcome = core.dispatch(Command::RepoAdd { path: repo_dir })?;
        let repo_id = repo_outcome
            .events
            .iter()
            .find_map(|event| match event {
                awo_core::DomainEvent::RepoRegistered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .expect("repo id");
        core.dispatch(Command::TeamInit {
            team_id: "alpha".to_string(),
            repo_id,
            objective: "Review and rework".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;
        core.dispatch(Command::TeamTaskAdd {
            team_id: "alpha".to_string(),
            task: awo_core::TaskCard {
                task_id: "task-1".to_string(),
                title: "Needs another pass".to_string(),
                summary: "Ready for review".to_string(),
                owner_id: "lead".to_string(),
                runtime: None,
                model: None,
                slot_id: None,
                branch_name: None,
                read_only: false,
                write_scope: Vec::new(),
                deliverable: "Patch".to_string(),
                verification: vec!["cargo test".to_string()],
                verification_command: None,
                depends_on: Vec::new(),
                state: TaskCardState::Review,
                result_summary: Some("Found follow-up work".to_string()),
                result_session_id: Some("session-1".to_string()),
                handoff_note: Some("Please tighten the edge cases".to_string()),
                output_log_path: Some("/tmp/log".to_string()),
            },
        })?;
        let path = awo_core::team::default_team_manifest_path(core.paths(), "alpha");
        let mut manifest = awo_core::team::load_team_manifest(&path)?;
        manifest.tasks[0].state = TaskCardState::Review;
        manifest.tasks[0].result_summary = Some("Found follow-up work".to_string());
        manifest.tasks[0].result_session_id = Some("session-1".to_string());
        manifest.tasks[0].handoff_note = Some("Please tighten the edge cases".to_string());
        manifest.tasks[0].output_log_path = Some("/tmp/log".to_string());
        awo_core::team::save_team_manifest(core.paths(), &manifest)?;

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Task;
        super::refresh_team_dashboard_data(&core, &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('W'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Confirm(_)));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks[0].state, TaskCardState::Todo);
        assert!(manifest.tasks[0].result_summary.is_none());
        assert!(manifest.tasks[0].handoff_note.is_none());
        Ok(())
    }

    fn set_value(form: &mut FormState, key: &str, value: &str) {
        let field = form
            .fields
            .iter_mut()
            .find(|field| field.key == key)
            .expect("field should exist");
        field.value = value.to_string();
    }
}
