# Job Card C: Live Session Log Tailing in TUI

## Objective

Make the TUI log viewer auto-refresh while a session is running. Currently pressing `Enter` on a session shows a static snapshot of the log — the user must press `r` repeatedly to see new output. For running sessions, the log should auto-update each TUI tick.

## Scope

**One file**: `crates/awo-app/src/tui.rs`

No changes to `awo-core`. The core's `Command::SessionLog` already reads the log file and returns content. We just call it more often from the TUI.

## What to build

### 1. Auto-refresh log when session is running

In the main event loop, after the background result polling and before `terminal.draw()`, add a log refresh check:

```rust
// Auto-refresh log for running sessions
if state.show_log_panel {
    if let Some(session_id) = &state.log_session_id {
        let session_running = visible_sessions(&snapshot, &state)
            .iter()
            .any(|s| s.id == *session_id && s.status == "running");
        if session_running {
            fetch_session_log(&mut core, &mut state, &session_id.clone());
        }
    }
}
```

This re-reads the log file every 200ms (the existing poll interval) for running sessions only. Once the session completes, auto-refresh stops and the user sees the final output.

### 2. Auto-open log on session start

When the user starts a session (submits `StartSession` input action), automatically open the log panel:

In the `InputAction::StartSession` submit handler, after `apply_command()`:
```rust
// Auto-open log for the newly started session
if let Some(session) = visible_sessions(&snapshot, &state).last() {
    fetch_session_log(&mut core, &mut state, &session.id);
}
```

### 3. Show running indicator in log panel title

Update the log panel title to show whether the session is still running:

```rust
let session_running = snapshot.sessions.iter()
    .any(|s| Some(&s.id) == state.log_session_id.as_ref() && s.status == "running");
let status_indicator = if session_running { " [running]" } else { "" };
let title = format!(
    "Log: {}{} (Esc to close, r to refresh)",
    state.log_session_id.as_deref().unwrap_or("?"),
    status_indicator,
);
```

### 4. Scroll position tracking (minimal)

Add a `log_scroll: u16` field to `TuiState` (default 0). When auto-refreshing:
- If the user hasn't scrolled, keep scroll at the bottom (show latest output)
- Up/Down keys in log panel mode adjust scroll
- `r` key resets scroll to bottom

Add to `TuiState`:
```rust
log_scroll: u16,
```

In the render function, use `.scroll((state.log_scroll, 0))` on the log `Paragraph`.

In event handling, when `show_log_panel` is true and key is Up/Down:
```rust
KeyCode::Up => { state.log_scroll = state.log_scroll.saturating_sub(1); }
KeyCode::Down => { state.log_scroll = state.log_scroll.saturating_add(1); }
```

When auto-refreshing a running session, reset scroll to bottom:
```rust
state.log_scroll = u16::MAX; // Will be clamped by ratatui
```

## Constraints

- Do NOT add any new crate dependencies.
- Do NOT modify `awo-core`.
- Do NOT change the event loop timing (keep 200ms poll).
- Keep the auto-refresh simple — just re-read the file each tick. No inotify, no streaming. 200ms is fast enough for log tailing.
- The `TuiState` struct must remain `#[derive(Debug)]`.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Then manual smoke test:
1. `awo repo add .`
2. `awo tui`
3. Press `s` → type task name → Enter (acquire slot)
4. Select slot → press `Enter` → type `sleep 3 && echo done` → Enter
5. Session log should auto-open and show output updating in real time
6. After 3 seconds, log shows "done" and `[running]` indicator disappears
7. Press Up/Down to scroll, `r` to reset to bottom
8. Press `Esc` to close log panel

## What NOT to do

- Do not add background threading for log reads — file reads are fast (<1ms)
- Do not add inotify/kqueue file watching
- Do not modify `awo-core` or any core commands
- Do not add new CLI subcommands
- Do not refactor the existing render layout beyond the log panel changes
