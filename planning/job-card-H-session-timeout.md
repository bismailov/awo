# Job Card H: Session Timeout & Interruption

## Objective
Implement session-level timeouts and robust process tree termination. This ensures that hanging agents don't consume system resources indefinitely and that manual cancellation is authoritative.

## Scope
**Files to modify**:
- `crates/awo-core/src/runtime.rs` — update `SessionRecord` and `SessionStartOptions`
- `crates/awo-core/src/commands/session.rs` — implement timeout logic in `run_session_start`
- `crates/awo-core/src/runtime/supervisor.rs` — add timeout monitoring to `sync_session`
- `crates/awo-core/src/runtime/supervisor/tmux.rs` — implement robust kill logic
- `crates/awo-core/src/runtime/supervisor/conpty.rs` — implement robust kill logic (even if stubbed)

## What to build

### 1. Schema Expansion
- Add `timeout_secs: Option<u64>` to `SessionRecord` table in SQLite.
- Update `SessionRecord` struct.
- Add `started_at: Option<String>` (ISO8601) to track when the process actually began.

### 2. Timeout Enforcement
In `sync_session` (which is called by `snapshot()`):
- Calculate duration since `started_at`.
- If duration > `timeout_secs`, call `supervisor.cancel()` and mark status as `Failed`.
- Emit a `DomainEvent::SessionTimedOut`.

### 3. Authoritative Cancellation
Update `cancel` in supervisors to ensure the entire process group is terminated.
- For `tmux`: `tmux kill-session -t <name>` is already quite strong, but verify it kills children.
- For `oneshot`: Use `nix` or similar to send signals to the PID group if possible, or at least `kill -9` the parent.

### 4. CLI & TUI Integration
- Expose `--timeout <seconds>` in `awo session start`.
- Update TUI to show remaining time or "Timed Out" status.

## Verification
- `cargo test`
- New integration test: Start a session with `sleep 10` and a `timeout_secs: 2`, verify it marks as `Failed` after 2 seconds.

## Constraints
- Preserve backward compatibility for sessions without timeouts.
- Ensure the database migration handles existing sessions correctly.
