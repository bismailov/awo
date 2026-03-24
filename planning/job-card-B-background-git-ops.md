# Job Card B: Background Git Operations for TUI

## Objective

Unblock the TUI event loop during slow git operations. Currently, slot acquire (creates worktree), slot release (removes worktree), and slot refresh call git synchronously — the TUI freezes for seconds on large repos. Move these to background threads with channel-based notification.

## Scope

**Two files**:
- `crates/awo-app/src/tui.rs` — event loop changes, background dispatch
- `crates/awo-app/Cargo.toml` — add `crossbeam-channel` dependency

No changes to `awo-core`. The core remains synchronous — we only move the *call site* in the TUI to a background thread.

## What to build

### 1. Add crossbeam-channel dependency

```toml
# crates/awo-app/Cargo.toml
crossbeam-channel = "0.5"
```

### 2. Background command runner

Add to `tui.rs`:

```rust
use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::thread;

struct BackgroundResult {
    summary: String,
    events: Vec<DomainEvent>,
    error: Option<String>,
}
```

Create a channel pair `(tx, rx)` before the TUI event loop. The `tx` end is cloned into spawned threads; the `rx` end is polled in the main loop.

### 3. Background dispatch helper

```rust
fn dispatch_in_background(
    command: Command,
    tx: Sender<BackgroundResult>,
) {
    // Spawn a new AppCore in the thread (they share the same SQLite via WAL)
    // This is safe because WAL mode supports concurrent readers + one writer
    thread::spawn(move || {
        let result = match AppCore::bootstrap() {
            Ok(mut bg_core) => match bg_core.dispatch(command) {
                Ok(outcome) => BackgroundResult {
                    summary: outcome.summary,
                    events: outcome.events,
                    error: None,
                },
                Err(e) => BackgroundResult {
                    summary: String::new(),
                    events: vec![],
                    error: Some(e.to_string()),
                },
            },
            Err(e) => BackgroundResult {
                summary: String::new(),
                events: vec![],
                error: Some(format!("failed to open background core: {e}")),
            },
        };
        let _ = tx.send(result);
    });
}
```

**Note**: `AppCore::bootstrap()` calls `AppConfig::load()` which reads env vars and opens a fresh SQLite connection. This is safe — WAL mode supports concurrent readers + one writer. `DomainEvent` already derives `Clone`. No changes to `awo-core` are needed.

### 4. Modify TUI event loop

In the main loop, after `event::poll`:

```rust
// Check for background results
while let Ok(result) = rx.try_recv() {
    if let Some(error) = result.error {
        state.status = format!("Error: {error}");
    } else {
        state.status = result.summary;
        for event in result.events {
            state.messages.push(event.to_message());
        }
    }
}
```

### 5. Route blocking commands through background

Commands that call git and should go to background:
- `Command::SlotAcquire` (creates worktree)
- `Command::SlotRelease` (removes worktree)
- `Command::SlotRefresh` (git fetch + reset)
- `Command::RepoAdd` (git discovery + canonicalize)

Commands that should stay synchronous (fast, no git):
- `Command::SessionStart` (just writes to SQLite + spawns process)
- `Command::SessionCancel`
- `Command::ReviewStatus`
- `Command::NoOp`
- All other commands

### 6. Pending state indicator

When a background operation is in flight:
- Set `state.status = "Working..."` immediately when dispatching
- Optionally track `pending_ops: usize` in `TuiState` to show a spinner or "Working..." indicator
- Clear when the background result arrives

## Constraints

- Do NOT add Tokio. Use `std::thread` + `crossbeam-channel` only.
- Do NOT change the core's synchronous design.
- Do NOT change `awo-core` at all. `AppCore::bootstrap()` and `DomainEvent: Clone` already exist.
- Do NOT refactor the TUI layout or rendering.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Then manual smoke test:
1. `awo tui`
2. Press `a` to add repo — should NOT freeze the TUI
3. Press `s` to acquire slot — should show "Working..." then update when done
4. Press `X` to release slot — should NOT freeze

## What NOT to do

- Do not add text input or help overlay — that's Job Card A
- Do not add session log tailing
- Do not add Tokio or any async runtime
- Do not decompose app.rs or store.rs
- Do not modify any test files
