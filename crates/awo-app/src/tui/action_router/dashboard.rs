use super::{TeamDashboardFocus, TuiState};
use crate::tui::{fetch_session_log, fetch_slot_diff};
use awo_core::AppCore;
use awo_core::team::{PlanItem, TaskCard, TaskCardState, TeamMember};

pub(super) fn open_slot_delete_confirm(state: &mut TuiState) {
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card first.".to_string();
        return;
    };
    let Some(slot_id) = task.slot_id.clone() else {
        state.status = "Error: selected task card has no bound slot.".to_string();
        return;
    };
    state.input_mode =
        crate::tui::InputMode::Confirm(crate::tui::forms::ConfirmState::delete_slot(slot_id));
}

pub(super) fn selected_dashboard_team(state: &TuiState) -> Option<&awo_core::TeamManifest> {
    state
        .team_dashboard
        .teams
        .get(state.team_dashboard.selected_team_idx)
}

pub(super) fn selected_dashboard_task(state: &TuiState) -> Option<&TaskCard> {
    let team = selected_dashboard_team(state)?;
    team.tasks.get(state.team_dashboard.selected_task_idx)
}

pub(super) fn selected_dashboard_plan(state: &TuiState) -> Option<&PlanItem> {
    let team = selected_dashboard_team(state)?;
    team.plan_items.get(state.team_dashboard.selected_plan_idx)
}

pub(super) fn selected_dashboard_member_count(state: &TuiState) -> usize {
    selected_dashboard_team(state)
        .map(|team| team.members.len() + 1)
        .unwrap_or(0)
}

pub(super) fn selected_dashboard_member(state: &TuiState) -> Option<&TeamMember> {
    let team = selected_dashboard_team(state)?;
    if state.team_dashboard.selected_member_idx == 0 {
        Some(&team.lead)
    } else {
        team.members
            .get(state.team_dashboard.selected_member_idx.saturating_sub(1))
    }
}

pub(super) fn selected_dashboard_task_ids(state: &TuiState) -> Option<(String, String)> {
    let team = selected_dashboard_team(state)?;
    let task = selected_dashboard_task(state)?;
    Some((team.team_id.clone(), task.task_id.clone()))
}

pub(super) fn open_selected_task_log(core: &mut AppCore, state: &mut TuiState) {
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

pub(super) fn open_selected_task_diff(core: &mut AppCore, state: &mut TuiState) {
    let Some(task) = selected_dashboard_task(state) else {
        state.status = "Error: select a task card first.".to_string();
        return;
    };
    let Some(slot_id) = task.slot_id.clone() else {
        state.status = "Error: selected task card has no bound slot.".to_string();
        return;
    };
    fetch_slot_diff(core, state, &slot_id);
    state.log_scroll = u16::MAX;
}

pub(super) fn dashboard_member_ids(team: &awo_core::TeamManifest) -> Vec<String> {
    std::iter::once(team.lead.member_id.clone())
        .chain(team.members.iter().map(|member| member.member_id.clone()))
        .collect()
}

pub(super) fn select_adjacent_actionable_task(state: &mut TuiState, forward: bool) {
    let Some(team) = selected_dashboard_team(state) else {
        state.status = "Error: select a team in the Team Dashboard first.".to_string();
        return;
    };
    let actionable_indices = team
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| matches!(task.state, TaskCardState::Review) || task.slot_id.is_some())
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    if actionable_indices.is_empty() {
        state.status = "No review or cleanup task cards are queued.".to_string();
        return;
    }
    let selected_task = {
        let current = state.team_dashboard.selected_task_idx;
        let selected =
            if let Some(position) = actionable_indices.iter().position(|idx| *idx == current) {
                if forward {
                    actionable_indices[(position + 1) % actionable_indices.len()]
                } else {
                    actionable_indices
                        [(position + actionable_indices.len() - 1) % actionable_indices.len()]
                }
            } else if forward {
                actionable_indices[0]
            } else {
                *actionable_indices.last().unwrap_or(&0)
            };
        team.tasks.get(selected).map(|task| {
            (
                selected,
                task.task_id.clone(),
                if task.state == TaskCardState::Review {
                    "review task card"
                } else {
                    "cleanup task card"
                },
                task_queue_label(task),
            )
        })
    };
    state.team_dashboard.focus = TeamDashboardFocus::Task;
    if let Some((selected, task_id, task_kind, queue_label)) = selected_task {
        state.team_dashboard.selected_task_idx = selected;
        state.status = format!("Selected {} `{}` in {}.", task_kind, task_id, queue_label);
    }
}

fn task_queue_label(task: &TaskCard) -> &'static str {
    if task.state == TaskCardState::Review {
        "review queue"
    } else if task.slot_id.is_some() {
        "cleanup queue"
    } else {
        "task list"
    }
}
