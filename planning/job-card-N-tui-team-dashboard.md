# Job Card N — TUI Team Dashboard Panel

## Objective

Add a dedicated team dashboard panel to the TUI that shows team status, member assignments, and task progress at a glance. Currently `team show` is CLI-only; the TUI has no team visibility beyond the `R` keybinding for reports.

## Motivation

As teams grow beyond 2-3 tasks, operators need a live view of which workers are active, which tasks are blocked, and overall team progress — without switching to CLI commands. This is the natural next step after the team command layer (completed) and before automated multi-task orchestration.

## What to change

### 1. Add `TeamDashboardState` to TUI state in `crates/awo-app/src/tui.rs`

```rust
pub struct TeamDashboardState {
    pub selected_team_idx: usize,
    pub selected_task_idx: usize,
    pub teams: Vec<TeamSummary>,
}

pub struct TeamSummary {
    pub team_id: String,
    pub repo_id: String,
    pub status: String,
    pub member_count: usize,
    pub tasks_total: usize,
    pub tasks_done: usize,
    pub tasks_in_progress: usize,
    pub tasks_blocked: usize,
}
```

### 2. Add `Panel::Teams` variant

Add a `Teams` variant to the existing `Panel` enum (or whatever the TUI uses for panel selection). Wire the `T` key to toggle this panel.

### 3. Render the team dashboard

Create a `render_team_dashboard` function that draws:
- **Left column**: List of teams with status badges (Planning/Running/Blocked/Complete/Archived)
- **Right column**: For the selected team, show:
  - Objective (truncated to 2 lines)
  - Member list with role and slot assignment status
  - Task table: task_id | owner | state | slot | deliverable (truncated)
  - Progress bar: done/total tasks

Use `ratatui` widgets (Table, List, Gauge) consistent with existing TUI panels.

### 4. Load team data on panel activation

When the Teams panel is activated, call `app.snapshot()` to get current state, then load team manifests via `list_team_manifest_paths` + `load_team_manifest`. Cache in `TeamDashboardState.teams`.

### 5. Add keybindings within the panel

- `j`/`k` or `↑`/`↓`: Navigate task list within selected team
- `Tab`: Switch between team list and task detail
- `Enter` on a task: Show full task detail (summary, deliverable, verification, result_summary if done)
- `d`: Delegate selected task (shells out to `awo team task delegate` or dispatches command directly)
- `s`: Start selected task
- `Esc`: Return to main TUI view

### 6. Add refresh on event bus poll

If the TUI already polls the event bus (via `poll_events`), filter for `TeamTask*` and `Team*` events to trigger a dashboard refresh. If not, refresh on each TUI tick when the Teams panel is active.

## Files touched

| File | Change |
|------|--------|
| `crates/awo-app/src/tui.rs` | Add `TeamDashboardState`, `Panel::Teams`, render function, keybindings |
| `crates/awo-app/src/handlers.rs` | Wire `T` key to Teams panel, load team data |

## Files NOT to touch

- `crates/awo-core/src/team.rs` — read-only access to existing types, no changes needed
- `crates/awo-core/src/commands.rs` — uses existing `TeamShow`/`TeamList` commands
- `crates/awo-core/src/events.rs` — no new events
- `crates/awo-core/src/daemon.rs` — unrelated
- `crates/awo-mcp/` — unrelated

## Constraints

- `unsafe_code = "forbid"` workspace-wide
- Synchronous core — no Tokio, no async
- The TUI must remain responsive; team manifest loading should not block the render loop. If loading is slow, show a "Loading..." placeholder and load on the next tick.
- Use existing `ratatui` patterns from the codebase — match the style of existing panels.
- Team manifests are TOML files on disk (loaded via `load_team_manifest`), not from the SQLite store.

## Tests

- Unit test: `TeamSummary` correctly aggregates task states from a `TeamManifest`
- Verify existing tests still pass (no regressions in TUI logic)

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Manual smoke test:
```bash
# Create a team and add tasks
awo team init my-team <repo_id> "Test objective"
awo team member add my-team worker1 --role worker --runtime claude_code --execution-mode external_slots
awo team task add my-team task-1 worker1 "Test task" "Do something" --deliverable "output.txt"
# Launch TUI and press T
awo
# Should see team dashboard with my-team listed, task-1 in Todo state
```

## Definition of done

- `T` key opens a team dashboard panel in the TUI
- Dashboard shows team list with status, member count, task progress
- Selected team shows member and task details
- Navigation works within the panel (j/k, Tab, Enter, Esc)
- All existing tests pass, no new clippy warnings
