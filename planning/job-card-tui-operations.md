# Job Card: Wire TUI Keyboard Commands for Slot/Session Lifecycle

## 1. Context & Motivation
The TUI (`crates/awo-app/src/tui.rs`) currently displays repos, teams, slots, and sessions, but the operator can only navigate and run diagnostics. There is no way to acquire a slot, start a session, or cancel a session from within the TUI. The project owner wants to see the app in action — this means driving the full lifecycle from the dashboard.

## 2. Key Files
- `crates/awo-app/src/tui.rs` — the TUI event loop and rendering (~650 lines)
- `crates/awo-core/src/commands.rs` — the `Command` enum (all available operations)
- `crates/awo-core/src/snapshot.rs` — `AppSnapshot`, `SlotSummary`, `SessionSummary` (what's displayed)

## 3. Implementation Steps

### 3.1. Add Slot Acquisition (key: `s`)
When a repo is selected, pressing `s` should acquire a new slot:
```rust
Command::SlotAcquire {
    repo_id: selected_repo.id.clone(),
    task_name: "tui-task".to_string(),  // prompt user or use default
    strategy: None,  // use default
}
```
Display the result in the status bar and event log.

### 3.2. Add Session Start (key: `Enter`)
When a slot is selected (need to add slot selection state), pressing `Enter` should start a session:
```rust
Command::SessionStart {
    slot_id: selected_slot.id.clone(),
    runtime: None,      // use default
    prompt: "...".to_string(),  // prompt user or use placeholder
    read_only: false,
    dry_run: false,
}
```

### 3.3. Add Session Cancel (key: `x`)
When a session is selected, pressing `x` should cancel it:
```rust
Command::SessionCancel {
    session_id: selected_session.id.clone(),
}
```

### 3.4. Add Slot Release (key: `X`)
When a slot is selected, pressing `X` (shift-x) should release it:
```rust
Command::SlotRelease {
    slot_id: selected_slot.id.clone(),
}
```

### 3.5. Add Slot Selection State
Currently `TuiState` tracks `selected_repo_index` and `selected_team_index`. Add `selected_slot_index` with corresponding `Tab`/`Shift-Tab` or similar navigation to focus between the slot and session panels.

### 3.6. Update Help Bar
Update the status bar string to include the new keybindings.

## 4. Verification
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- Manual: launch `awo tui`, add a repo, acquire a slot, verify it appears in the Slots panel.

## 5. Constraints
- Do NOT add async or Tokio. The TUI uses a synchronous poll loop.
- Do NOT refactor the rendering code. Only add to `TuiState` and the `match key.code` block.
- Use `apply_command()` for all dispatches — it already handles error display.
- Keep the command values simple (defaults/placeholders). A text-input prompt widget is out of scope for this card.
- The `Command` variants and their fields are the source of truth. Check `commands.rs` for exact field names before implementing.
