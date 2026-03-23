# Job Card: Wire Session Log Viewer into TUI

## 1. Context & Motivation
The `SessionLog` command (`awo session log <session_id>`) already exists and returns the last N lines of a session's output. But the TUI has no way to view logs — operators must exit to the CLI. This card adds an inline log viewer panel so the operator can inspect session output without leaving the dashboard.

## 2. Key Files
- `crates/awo-app/src/tui.rs` — the TUI event loop and rendering (~950 lines after recent changes)
- `crates/awo-core/src/commands.rs` — `Command::SessionLog { session_id, lines, stream }` (already exists)
- `crates/awo-core/src/events.rs` — `DomainEvent::SessionLogLoaded { session_id, stream, lines_returned, log_path, content }` (already exists)

## 3. Current TUI State
The TUI already has:
- Panel focus system: `FocusPanel` enum with `Tab`/`BackTab` cycling (Repositories, Teams, Slots, Sessions)
- `selected_slot_index` and `selected_session_index` tracking
- `apply_command()` helper that dispatches commands and captures outcomes/errors
- Session panel showing runtime, slot_id, status, exit_code for each session

## 4. Implementation Steps

### 4.1. Add Log Content to `TuiState`
Add fields to `TuiState`:
```rust
log_content: Option<String>,
log_session_id: Option<String>,
log_path: Option<String>,
show_log_panel: bool,
```

### 4.2. Add Log Fetch Key (`Enter` on session)
When the Sessions panel is focused and `Enter` is pressed on a selected session:
```rust
KeyCode::Enter if state.focus == FocusPanel::Sessions => {
    if let Some(session) = selected_session(&snapshot, &state) {
        let outcome = core.dispatch(Command::SessionLog {
            session_id: session.id.clone(),
            lines: Some(100),
            stream: None,
        });
        match outcome {
            Ok(outcome) => {
                // Extract content from SessionLogLoaded event
                for event in &outcome.events {
                    if let DomainEvent::SessionLogLoaded { content, log_path, session_id, .. } = event {
                        state.log_content = Some(content.clone());
                        state.log_session_id = Some(session_id.clone());
                        state.log_path = Some(log_path.clone());
                        state.show_log_panel = true;
                    }
                }
                append_events(state, outcome.events);
            }
            Err(error) => {
                state.status = format!("Error: {error:#}");
            }
        }
    }
}
```

### 4.3. Add `Escape` to Close Log Panel
```rust
KeyCode::Esc => {
    if state.show_log_panel {
        state.show_log_panel = false;
    }
}
```

### 4.4. Render Log Panel as Overlay
When `show_log_panel` is true, replace the bottom half of the layout with a log viewer:
```rust
if state.show_log_panel {
    let title = format!(
        "Log: {} (Esc to close)",
        state.log_session_id.as_deref().unwrap_or("?")
    );
    let content = state.log_content.as_deref().unwrap_or("(empty)");
    let log_widget = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    // Render over the bottom section of the layout
    frame.render_widget(log_widget, bottom_area);
}
```

### 4.5. Add `r` to Refresh Log
When the log panel is visible, pressing `r` should re-fetch the log for the same session (useful for watching running sessions).

### 4.6. Update Help Bar
Add `Enter=view log` and `Esc=close log` to the status bar hints.

## 5. Verification
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- Manual: launch `awo tui`, start a shell session, press `Enter` on it in the Sessions panel, verify log content appears.

## 6. Constraints
- Do NOT add async or Tokio. The TUI uses a synchronous poll loop.
- Do NOT modify any core crate files. The `SessionLog` command already exists.
- Only modify `crates/awo-app/src/tui.rs`.
- Use `apply_command()` pattern where possible, but for log viewing you may need to call `core.dispatch()` directly to extract the event content.
- The `DomainEvent::SessionLogLoaded` variant carries the `content: String` field — match on it to extract log text.
- Keep the log panel simple. No scrolling state is needed for V1 — just show the last N lines that fit the panel.
