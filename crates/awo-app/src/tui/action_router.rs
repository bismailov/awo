mod dashboard;
mod dialogs;

use super::{
    BackgroundResult, InputAction, InputMode, TERMINAL_SCROLL_PAGE, TeamDashboardFocus,
    TerminalLayoutMode, TuiFocus, TuiState, apply_command, dispatch_in_background,
    fetch_session_log, fetch_session_terminal, fetch_slot_diff, refresh_team_dashboard_data,
    selected_repo, selected_session, selected_slot, selected_team, send_terminal_input,
    start_next_team_task, start_team_task_explicit,
};
use awo_core::{AppCore, AppSnapshot, Command, SessionTerminalInput, SessionTerminalKey};
use crossbeam_channel::Sender;
use crossterm::event::KeyCode;
use dashboard::{
    open_selected_task_diff, open_selected_task_log, open_selected_task_terminal,
    open_slot_delete_confirm, select_adjacent_actionable_task, selected_dashboard_member_count,
    selected_dashboard_task, selected_dashboard_task_ids, selected_dashboard_team,
};
use dialogs::{
    approve_selected_plan_item, handle_confirm_key, handle_enter, handle_form_key,
    handle_text_input_key, open_member_add_form, open_member_remove_confirm,
    open_member_update_form, open_plan_add_form, open_plan_generate_form,
    open_promote_lead_confirm, open_repo_add_form, open_task_accept_confirm, open_task_add_form,
    open_task_cancel_confirm, open_task_delegate_form, open_task_rework_confirm,
    open_task_supersede_form, open_team_init_form,
};

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

    if state.terminal_input_mode {
        let Some(session_id) = state.terminal_session_id.clone() else {
            state.terminal_input_mode = false;
            state.status = "Error: no terminal session is attached.".to_string();
            return KeyOutcome::Continue;
        };

        match key {
            KeyCode::Esc => {
                state.terminal_input_mode = false;
                state.status = format!("Terminal `{session_id}` switched to view mode.");
            }
            KeyCode::Char(c) => {
                send_terminal_input(
                    core,
                    state,
                    &session_id,
                    SessionTerminalInput::Text {
                        text: c.to_string(),
                    },
                );
            }
            KeyCode::Enter => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Enter,
                },
            ),
            KeyCode::Backspace => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Backspace,
                },
            ),
            KeyCode::Tab => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Tab,
                },
            ),
            KeyCode::Up => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Up,
                },
            ),
            KeyCode::Down => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Down,
                },
            ),
            KeyCode::Left => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Left,
                },
            ),
            KeyCode::Right => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Right,
                },
            ),
            KeyCode::PageUp => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::PageUp,
                },
            ),
            KeyCode::PageDown => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::PageDown,
                },
            ),
            KeyCode::Home => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Home,
                },
            ),
            KeyCode::End => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::End,
                },
            ),
            KeyCode::Delete => send_terminal_input(
                core,
                state,
                &session_id,
                SessionTerminalInput::Key {
                    key: SessionTerminalKey::Delete,
                },
            ),
            _ => {}
        }
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
            let (prompt_label, buffer, on_submit) = if state.focus == TuiFocus::Terminal {
                (
                    "Terminal search: ".to_string(),
                    state.terminal_search_query.clone().unwrap_or_default(),
                    InputAction::SetTerminalSearch,
                )
            } else {
                (
                    "Filter: ".to_string(),
                    String::new(),
                    InputAction::SetFilter,
                )
            };
            state.input_mode = InputMode::TextInput {
                prompt_label,
                buffer,
                on_submit,
            };
            KeyOutcome::Continue
        }
        KeyCode::Esc => {
            if state.focus == TuiFocus::Terminal {
                state.focus = TuiFocus::Sessions;
                state.show_terminal_panel = false;
                state.terminal_input_mode = false;
                state.terminal_content = None;
                state.terminal_session_id = None;
                state.terminal_scroll = 0;
                state.terminal_search_query = None;
                state.terminal_follow_output = true;
                state.terminal_layout = TerminalLayoutMode::Docked;
            } else if state.focus == TuiFocus::TeamDashboard {
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
                    TeamDashboardFocus::Team => TeamDashboardFocus::Plan,
                    TeamDashboardFocus::Plan => TeamDashboardFocus::Member,
                    TeamDashboardFocus::Member => TeamDashboardFocus::Task,
                    TeamDashboardFocus::Task => TeamDashboardFocus::Team,
                };
            } else {
                state.focus = match state.focus {
                    TuiFocus::Repos => TuiFocus::Teams,
                    TuiFocus::Teams => TuiFocus::Slots,
                    TuiFocus::Slots => TuiFocus::Sessions,
                    TuiFocus::Sessions => {
                        if state.show_terminal_panel {
                            TuiFocus::Terminal
                        } else {
                            TuiFocus::Repos
                        }
                    }
                    TuiFocus::Terminal => TuiFocus::Repos,
                    TuiFocus::TeamDashboard => TuiFocus::Repos,
                };
            }
            KeyOutcome::Continue
        }
        KeyCode::BackTab => {
            if state.focus == TuiFocus::TeamDashboard {
                state.team_dashboard.focus = match state.team_dashboard.focus {
                    TeamDashboardFocus::Team => TeamDashboardFocus::Task,
                    TeamDashboardFocus::Plan => TeamDashboardFocus::Team,
                    TeamDashboardFocus::Member => TeamDashboardFocus::Plan,
                    TeamDashboardFocus::Task => TeamDashboardFocus::Member,
                };
            } else {
                state.focus = match state.focus {
                    TuiFocus::Repos => {
                        if state.show_terminal_panel {
                            TuiFocus::Terminal
                        } else {
                            TuiFocus::Sessions
                        }
                    }
                    TuiFocus::Teams => TuiFocus::Repos,
                    TuiFocus::Slots => TuiFocus::Teams,
                    TuiFocus::Sessions => TuiFocus::Slots,
                    TuiFocus::Terminal => TuiFocus::Sessions,
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
        KeyCode::PageUp => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = state.terminal_scroll.saturating_sub(TERMINAL_SCROLL_PAGE);
                state.terminal_follow_output = false;
            } else if state.show_log_panel {
                state.log_scroll = state.log_scroll.saturating_sub(TERMINAL_SCROLL_PAGE);
            }
            KeyOutcome::Continue
        }
        KeyCode::PageDown => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = state.terminal_scroll.saturating_add(TERMINAL_SCROLL_PAGE);
                state.terminal_follow_output = false;
            } else if state.show_log_panel {
                state.log_scroll = state.log_scroll.saturating_add(TERMINAL_SCROLL_PAGE);
            }
            KeyOutcome::Continue
        }
        KeyCode::Home => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = 0;
                state.terminal_follow_output = false;
            } else if state.show_log_panel {
                state.log_scroll = 0;
            }
            KeyOutcome::Continue
        }
        KeyCode::End => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = u16::MAX;
                state.terminal_follow_output = true;
            } else if state.show_log_panel {
                state.log_scroll = u16::MAX;
            }
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
        KeyCode::Char('p') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_plan_add_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('P') => {
            if state.focus == TuiFocus::TeamDashboard {
                approve_selected_plan_item(core, state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('G') => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = u16::MAX;
                state.terminal_follow_output = true;
            } else if state.focus == TuiFocus::TeamDashboard {
                open_plan_generate_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('r') => {
            if state.focus == TuiFocus::Terminal {
                if let Some(session_id) = state.terminal_session_id.clone() {
                    state.terminal_follow_output = true;
                    fetch_session_terminal(core, state, &session_id);
                    state.terminal_scroll = u16::MAX;
                }
            } else if state.show_log_panel {
                if let Some(session_id) = state.log_session_id.clone() {
                    if let Some(slot_id) = session_id.strip_prefix("slot-diff:") {
                        fetch_slot_diff(core, state, slot_id);
                    } else {
                        fetch_session_log(core, state, &session_id);
                    }
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
                refresh_team_dashboard_data(core.paths(), state);
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
        KeyCode::Char('C') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_cancel_confirm(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('S') => {
            if state.focus == TuiFocus::TeamDashboard {
                open_task_supersede_form(state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('o') => {
            if state.focus == TuiFocus::Terminal {
                if let Some(session_id) = state.terminal_session_id.clone() {
                    fetch_session_log(core, state, &session_id);
                    state.log_scroll = u16::MAX;
                    state.focus = TuiFocus::Sessions;
                }
            } else if state.focus == TuiFocus::TeamDashboard {
                open_selected_task_log(core, state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('e') => {
            if state.focus == TuiFocus::Sessions {
                if let Some(session) = selected_session(snapshot, state) {
                    if session.embedded_terminal_supported {
                        fetch_session_terminal(core, state, &session.id);
                        state.focus = TuiFocus::Terminal;
                        state.terminal_scroll = u16::MAX;
                        state.terminal_follow_output = true;
                        state.terminal_layout = TerminalLayoutMode::Workspace;
                    } else {
                        state.status = format!(
                            "Session `{}` does not support embedded terminal capture on this platform.",
                            session.id
                        );
                    }
                }
            } else if state.focus == TuiFocus::TeamDashboard {
                open_selected_task_terminal(core, state, snapshot);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('i') => {
            if state.focus == TuiFocus::Terminal && state.show_terminal_panel {
                state.terminal_input_mode = true;
                state.status =
                    "Terminal interaction mode enabled. Press Esc to return to view mode."
                        .to_string();
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('f') => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_follow_output = !state.terminal_follow_output;
                if state.terminal_follow_output {
                    state.terminal_scroll = u16::MAX;
                    if let Some(session_id) = state.terminal_session_id.clone() {
                        fetch_session_terminal(core, state, &session_id);
                    }
                }
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('g') => {
            if state.focus == TuiFocus::Terminal {
                state.terminal_scroll = 0;
                state.terminal_follow_output = false;
            } else if state.show_log_panel {
                state.log_scroll = 0;
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('z') => {
            if state.show_terminal_panel {
                state.terminal_layout = state.terminal_layout.next();
                state.status = format!("Terminal layout set to {}.", state.terminal_layout.label());
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('v') => {
            if state.focus == TuiFocus::Terminal {
                if let Some(session) = state.terminal_session_id.as_deref().and_then(|session_id| {
                    snapshot
                        .sessions
                        .iter()
                        .find(|session| session.id == session_id)
                }) {
                    fetch_slot_diff(core, state, &session.slot_id);
                    state.log_scroll = u16::MAX;
                    state.focus = TuiFocus::Sessions;
                }
            } else if state.focus == TuiFocus::TeamDashboard {
                open_selected_task_diff(core, state);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char(']') => {
            if state.focus == TuiFocus::TeamDashboard {
                select_adjacent_actionable_task(state, true);
            }
            KeyOutcome::Continue
        }
        KeyCode::Char('[') => {
            if state.focus == TuiFocus::TeamDashboard {
                select_adjacent_actionable_task(state, false);
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
                refresh_team_dashboard_data(core.paths(), state);
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
                refresh_team_dashboard_data(core.paths(), state);
            }
            KeyOutcome::Continue
        }
        _ => KeyOutcome::Continue,
    }
}

fn move_selection_up(state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.show_log_panel {
        state.log_scroll = state.log_scroll.saturating_sub(1);
    } else if state.focus == TuiFocus::Terminal {
        state.terminal_scroll = state.terminal_scroll.saturating_sub(1);
        state.terminal_follow_output = false;
    } else if state.focus == TuiFocus::TeamDashboard {
        match state.team_dashboard.focus {
            TeamDashboardFocus::Team => {
                if state.team_dashboard.selected_team_idx > 0 {
                    state.team_dashboard.selected_team_idx -= 1;
                    state.team_dashboard.selected_plan_idx = 0;
                    state.team_dashboard.selected_task_idx = 0;
                    state.team_dashboard.selected_member_idx = 0;
                }
            }
            TeamDashboardFocus::Plan => {
                if state.team_dashboard.selected_plan_idx > 0 {
                    state.team_dashboard.selected_plan_idx -= 1;
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
            TuiFocus::Terminal => {}
            TuiFocus::TeamDashboard => {}
        }
    }
    super::clamp_selection(state, snapshot);
}

fn move_selection_down(state: &mut TuiState, snapshot: &AppSnapshot) {
    if state.show_log_panel {
        state.log_scroll = state.log_scroll.saturating_add(1);
    } else if state.focus == TuiFocus::Terminal {
        state.terminal_scroll = state.terminal_scroll.saturating_add(1);
        state.terminal_follow_output = false;
    } else if state.focus == TuiFocus::TeamDashboard {
        match state.team_dashboard.focus {
            TeamDashboardFocus::Team => {
                if state.team_dashboard.selected_team_idx + 1 < state.team_dashboard.teams.len() {
                    state.team_dashboard.selected_team_idx += 1;
                    state.team_dashboard.selected_plan_idx = 0;
                    state.team_dashboard.selected_task_idx = 0;
                    state.team_dashboard.selected_member_idx = 0;
                }
            }
            TeamDashboardFocus::Plan => {
                if let Some(team) = selected_dashboard_team(state)
                    && state.team_dashboard.selected_plan_idx + 1 < team.plan_items.len()
                {
                    state.team_dashboard.selected_plan_idx += 1;
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
            TuiFocus::Terminal => {}
            TuiFocus::TeamDashboard => {}
        }
    }
    super::clamp_selection(state, snapshot);
}

#[cfg(test)]
mod tests {
    use super::{KeyOutcome, handle_key_event};
    use crate::tui::forms::{ConfirmState, FormKind, FormState};
    use crate::tui::{
        InputAction, InputMode, TeamDashboardFocus, TeamDashboardState, TerminalLayoutMode,
        TuiFocus, TuiState,
    };
    use anyhow::Result;
    use awo_core::capabilities::CostTier;
    use awo_core::{AppCore, Command, PlanItemState, TaskCardState, TeamExecutionMode};
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
            terminal_content: None,
            terminal_session_id: None,
            show_terminal_panel: false,
            terminal_input_mode: false,
            terminal_scroll: 0,
            terminal_search_query: None,
            terminal_follow_output: true,
            terminal_layout: TerminalLayoutMode::Docked,
            pending_ops: 0,
            input_mode: InputMode::Normal,
            show_help: false,
            log_scroll: 0,
            filter_query: None,
            team_dashboard: TeamDashboardState {
                selected_team_idx: 0,
                selected_plan_idx: 0,
                selected_member_idx: 0,
                selected_task_idx: 0,
                teams: Vec::new(),
                focus: TeamDashboardFocus::Team,
            },
            last_snapshot: None,
            last_snapshot_time: None,
            snapshot_refresh_in_flight: false,
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
        super::refresh_team_dashboard_data(core.paths(), &mut state);
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

        super::refresh_team_dashboard_data(core.paths(), &mut state);
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
        state.input_mode = InputMode::Confirm(ConfirmState::remove_member(
            "alpha".to_string(),
            "worker-1".to_string(),
        ));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);
        let manifest = core.load_team_manifest("alpha")?;
        assert!(manifest.members.is_empty());

        Ok(())
    }

    #[test]
    fn plan_item_add_approve_and_generate_flow_works() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("plan-flow");
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
            objective: "Plan the rollout".to_string(),
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
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Plan;
        super::refresh_team_dashboard_data(core.paths(), &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('p'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Form(_)));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "plan_id", "plan-1");
            set_value(form, "title", "Break out feature work");
            set_value(form, "summary", "Turn planning into execution");
            set_value(form, "deliverable", "Task card ready for work");
            set_value(form, "verification", "cargo test");
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx.clone());

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.plan_items.len(), 1);
        assert_eq!(manifest.plan_items[0].state, PlanItemState::Draft);

        super::refresh_team_dashboard_data(core.paths(), &mut state);
        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('P'),
            tx.clone(),
        );
        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.plan_items[0].state, PlanItemState::Approved);

        super::refresh_team_dashboard_data(core.paths(), &mut state);
        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('G'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Form(_)));
        if let InputMode::Form(form) = &mut state.input_mode {
            set_value(form, "task_id", "task-from-plan");
            set_value(form, "deliverable", "Task card ready for work");
        }
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks.len(), 1);
        assert_eq!(manifest.tasks[0].task_id, "task-from-plan");
        assert_eq!(manifest.plan_items[0].state, PlanItemState::Generated);
        assert_eq!(
            manifest.plan_items[0].generated_task_id.as_deref(),
            Some("task-from-plan")
        );

        Ok(())
    }

    #[test]
    fn actionable_task_navigation_jumps_between_review_and_cleanup() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("actionable-nav");
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
            objective: "Navigate actionable tasks".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;
        core.dispatch(Command::SlotAcquire {
            repo_id: core
                .snapshot()?
                .registered_repos
                .first()
                .expect("repo exists")
                .id
                .clone(),
            task_name: "cleanup-task".to_string(),
            strategy: awo_core::SlotStrategy::Fresh,
        })?;
        let cleanup_slot = core
            .snapshot()?
            .slots
            .into_iter()
            .find(|slot| slot.task_name == "cleanup-task")
            .expect("cleanup slot");
        for (task_id, state, slot_id) in [
            ("todo-1", TaskCardState::Todo, None),
            ("review-1", TaskCardState::Review, None),
            (
                "done-1",
                TaskCardState::Done,
                Some(cleanup_slot.id.as_str()),
            ),
        ] {
            core.dispatch(Command::TeamTaskAdd {
                team_id: "alpha".to_string(),
                task: awo_core::TaskCard {
                    task_id: task_id.to_string(),
                    title: task_id.to_string(),
                    summary: "Task".to_string(),
                    owner_id: "lead".to_string(),
                    runtime: None,
                    model: None,
                    slot_id: slot_id.map(str::to_string),
                    branch_name: slot_id.map(|_| cleanup_slot.branch_name.clone()),
                    read_only: false,
                    write_scope: Vec::new(),
                    deliverable: "Patch".to_string(),
                    verification: Vec::new(),
                    verification_command: None,
                    depends_on: Vec::new(),
                    state,
                    result_summary: None,
                    result_session_id: None,
                    handoff_note: None,
                    output_log_path: None,
                    superseded_by_task_id: None,
                },
            })?;
        }

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Task;
        super::refresh_team_dashboard_data(core.paths(), &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char(']'),
            tx.clone(),
        );
        assert_eq!(state.team_dashboard.selected_task_idx, 1);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char(']'),
            tx.clone(),
        );
        assert_eq!(state.team_dashboard.selected_task_idx, 2);

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('['), tx);
        assert_eq!(state.team_dashboard.selected_task_idx, 1);

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
                superseded_by_task_id: None,
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
        super::refresh_team_dashboard_data(core.paths(), &mut state);

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
                superseded_by_task_id: None,
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
        super::refresh_team_dashboard_data(core.paths(), &mut state);

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

    #[test]
    fn cancel_action_marks_task_card_cancelled() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("review-cancel");
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
            objective: "Cancel task card".to_string(),
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
                title: "Not needed".to_string(),
                summary: "Cancel it".to_string(),
                owner_id: "lead".to_string(),
                runtime: None,
                model: None,
                slot_id: None,
                branch_name: None,
                read_only: false,
                write_scope: Vec::new(),
                deliverable: "Patch".to_string(),
                verification: Vec::new(),
                verification_command: None,
                depends_on: Vec::new(),
                state: TaskCardState::Todo,
                result_summary: None,
                result_session_id: None,
                handoff_note: None,
                output_log_path: None,
                superseded_by_task_id: None,
            },
        })?;

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Task;
        super::refresh_team_dashboard_data(core.paths(), &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('C'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Confirm(_)));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks[0].state, TaskCardState::Cancelled);
        Ok(())
    }

    #[test]
    fn supersede_action_links_task_card_replacement() -> Result<()> {
        let env = TestEnv::new();
        let repo_dir = env.create_repo("review-supersede");
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
            objective: "Supersede task card".to_string(),
            lead_runtime: None,
            lead_model: None,
            execution_mode: TeamExecutionMode::ExternalSlots.as_str().to_string(),
            fallback_runtime: None,
            fallback_model: None,
            routing_preferences: None,
            force: false,
        })?;
        for (task_id, title) in [("task-1", "Original"), ("task-2", "Replacement")] {
            core.dispatch(Command::TeamTaskAdd {
                team_id: "alpha".to_string(),
                task: awo_core::TaskCard {
                    task_id: task_id.to_string(),
                    title: title.to_string(),
                    summary: "Task".to_string(),
                    owner_id: "lead".to_string(),
                    runtime: None,
                    model: None,
                    slot_id: None,
                    branch_name: None,
                    read_only: false,
                    write_scope: Vec::new(),
                    deliverable: "Patch".to_string(),
                    verification: Vec::new(),
                    verification_command: None,
                    depends_on: Vec::new(),
                    state: TaskCardState::Todo,
                    result_summary: None,
                    result_session_id: None,
                    handoff_note: None,
                    output_log_path: None,
                    superseded_by_task_id: None,
                },
            })?;
        }

        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::TeamDashboard;
        state.team_dashboard.focus = TeamDashboardFocus::Task;
        super::refresh_team_dashboard_data(core.paths(), &mut state);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('S'),
            tx.clone(),
        );
        assert!(matches!(state.input_mode, InputMode::Form(_)));
        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Enter, tx);

        let manifest = core.load_team_manifest("alpha")?;
        assert_eq!(manifest.tasks[0].state, TaskCardState::Superseded);
        assert_eq!(
            manifest.tasks[0].superseded_by_task_id.as_deref(),
            Some("task-2")
        );
        Ok(())
    }

    #[test]
    fn terminal_search_uses_dedicated_prompt() -> Result<()> {
        let env = TestEnv::new();
        let mut core = env.core();
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::Terminal;
        state.show_terminal_panel = true;
        state.terminal_search_query = Some("error".to_string());

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('/'), tx);

        match &state.input_mode {
            InputMode::TextInput {
                prompt_label,
                buffer,
                on_submit,
            } => {
                assert_eq!(prompt_label, "Terminal search: ");
                assert_eq!(buffer, "error");
                assert_eq!(*on_submit, InputAction::SetTerminalSearch);
            }
            other => panic!("expected terminal search prompt, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn terminal_escape_closes_panel_and_resets_workspace_state() -> Result<()> {
        let env = TestEnv::new();
        let mut core = env.core();
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::Terminal;
        state.show_terminal_panel = true;
        state.terminal_input_mode = true;
        state.terminal_session_id = Some("session-1".to_string());
        state.terminal_content = Some("hello".to_string());
        state.terminal_search_query = Some("error".to_string());
        state.terminal_layout = TerminalLayoutMode::Workspace;

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Esc, tx.clone());
        assert_eq!(state.focus, TuiFocus::Terminal);
        assert!(!state.terminal_input_mode);

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Esc, tx);

        assert_eq!(state.focus, TuiFocus::Sessions);
        assert!(!state.show_terminal_panel);
        assert!(!state.terminal_input_mode);
        assert!(state.terminal_session_id.is_none());
        assert!(state.terminal_content.is_none());
        assert!(state.terminal_search_query.is_none());
        assert_eq!(state.terminal_layout, TerminalLayoutMode::Docked);
        Ok(())
    }

    #[test]
    fn terminal_layout_toggle_cycles_modes() -> Result<()> {
        let env = TestEnv::new();
        let mut core = env.core();
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.show_terminal_panel = true;

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('z'),
            tx.clone(),
        );
        assert_eq!(state.terminal_layout, TerminalLayoutMode::Workspace);

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::Char('z'),
            tx.clone(),
        );
        assert_eq!(state.terminal_layout, TerminalLayoutMode::Focus);

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('z'), tx);
        assert_eq!(state.terminal_layout, TerminalLayoutMode::Docked);
        Ok(())
    }

    #[test]
    fn terminal_scroll_shortcuts_toggle_follow_mode() -> Result<()> {
        let env = TestEnv::new();
        let mut core = env.core();
        let snapshot = core.snapshot()?;
        let (tx, _rx) = unbounded();
        let mut state = base_state();
        state.focus = TuiFocus::Terminal;
        state.show_terminal_panel = true;
        state.terminal_scroll = 20;

        handle_key_event(
            &mut core,
            &mut state,
            &snapshot,
            KeyCode::PageUp,
            tx.clone(),
        );
        assert_eq!(state.terminal_scroll, 8);
        assert!(!state.terminal_follow_output);

        handle_key_event(&mut core, &mut state, &snapshot, KeyCode::Char('G'), tx);
        assert_eq!(state.terminal_scroll, u16::MAX);
        assert!(state.terminal_follow_output);
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
