# Job Card L — CLI Auto-Start Daemon

## Objective

When the CLI detects no running daemon, automatically spawn `awod` in the background before connecting. This makes the daemon invisible to the operator — they never need to run `awo daemon start` manually.

## Scope

**Unix only.** The daemon is currently `#[cfg(unix)]` throughout. Do not add Windows support.

## What to change

### 1. Add a `spawn_daemon` function in `crates/awo-core/src/daemon.rs`

Create a new public function:

```rust
/// Spawn `awod` as a detached background process.
/// Returns Ok(pid) on success.
#[cfg(unix)]
pub fn spawn_daemon(paths: &AppPaths) -> AwoResult<u32> { ... }
```

Requirements:
- Use `std::process::Command` to spawn the `awod` binary (same binary, different entrypoint — see `crates/awo-app/src/awod.rs`)
- The spawned process must be fully detached (not a child of the CLI process): redirect stdin/stdout/stderr to `/dev/null`, use `pre_exec` to call `setsid()` via `nix::unistd::setsid`
- After spawning, poll `daemon_is_running(paths)` with a short timeout (up to 3 seconds, 100ms intervals) to confirm the daemon came up
- If the daemon doesn't come up within the timeout, return an error
- Write a tracing::info message when the daemon is successfully spawned

### 2. Modify `CliBackend::bootstrap()` in `crates/awo-app/src/handlers.rs`

Current logic (lines 54–70):
```rust
if awo_core::daemon_is_running(core.paths()) {
    // connect to daemon
} else {
    None  // fall through to direct mode
}
```

Change the `else` branch to:
```rust
} else {
    match awo_core::spawn_daemon(core.paths()) {
        Ok(pid) => {
            tracing::info!(pid, "auto-started awod daemon");
            match awo_core::DaemonClient::connect(&core.paths().daemon_socket_path()) {
                Ok(client) => Some(client),
                Err(error) => {
                    tracing::warn!(%error, "auto-started daemon but connection failed, using direct mode");
                    None
                }
            }
        }
        Err(error) => {
            tracing::debug!(%error, "could not auto-start daemon, using direct mode");
            None
        }
    }
}
```

Key principle: **auto-start is best-effort**. If it fails, fall back silently to direct mode. Never block the operator or produce visible errors for auto-start failures.

### 3. Add `spawn_daemon` to the public API in `crates/awo-core/src/lib.rs`

Add `spawn_daemon` to the existing `pub use daemon::{ ... }` block (inside the `#[cfg(unix)]` gate or the general one — match the existing pattern).

### 4. Locate the `awod` binary path

The `awod` binary lives next to the `awo` binary. Use `std::env::current_exe()` and replace the filename:

```rust
let awo_exe = std::env::current_exe()?;
let awod_exe = awo_exe.with_file_name("awod");
```

If `awod` doesn't exist at that path, return an error (don't search PATH).

### 5. Add tests in `crates/awo-core/src/daemon.rs` (in the existing `#[cfg(test)] mod tests`)

- **`spawn_daemon_fails_when_awod_missing`**: Set up temp AppPaths where no `awod` binary exists. Confirm `spawn_daemon` returns an error.
- **`spawn_daemon_with_mock_binary`** (optional, if feasible): Create a tiny shell script as a stand-in for `awod` that writes a pidfile and exits. Confirm `spawn_daemon` returns a pid and the pidfile exists.

## Files touched

| File | Change |
|------|--------|
| `crates/awo-core/src/daemon.rs` | Add `spawn_daemon` function |
| `crates/awo-core/src/lib.rs` | Export `spawn_daemon` |
| `crates/awo-app/src/handlers.rs` | Auto-start in `CliBackend::bootstrap()` |

## Files NOT to touch

- `crates/awo-core/src/events.rs` — no new domain events needed
- `crates/awo-core/src/commands.rs` — no new commands needed
- `crates/awo-mcp/` — MCP server is unrelated
- `crates/awo-app/src/cli.rs` — no new CLI flags needed
- Anything in `crates/awo-core/src/team*` — completely unrelated

## Constraints

- `unsafe_code = "forbid"` workspace-wide — `pre_exec` closures require `unsafe`, so use `nix` crate's safe wrappers (e.g., `nix::unistd::setsid()` is safe to call from `pre_exec` in a pre-fork context, but the `pre_exec` closure itself is unsafe — this is the one place where you need `unsafe` and it's fine because `setsid` is async-signal-safe). **Wait** — `unsafe_code = "forbid"` means you cannot use `unsafe` at all. Instead of `pre_exec`, use `std::process::Command` with `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())` and accept that the child won't be a full daemon (it'll be reaped when the CLI exits, but since the child writes its own pidfile and the CLI detects it on next run, this is fine in practice). The child process's own signal handling (already implemented in `DaemonServer::run`) will keep it alive.
- Synchronous core — no Tokio, no async
- All state mutations through the command layer
- Run `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test` before declaring done

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Manual smoke test (if possible):
```bash
# Ensure no daemon running
awo daemon stop 2>/dev/null
# Run any CLI command — daemon should auto-start
awo repo list
# Verify daemon is now running
awo daemon status
```

## Definition of done

- `spawn_daemon` function exists and is exported
- `CliBackend::bootstrap()` auto-starts daemon when none is running
- Auto-start failure falls back to direct mode silently
- All existing tests pass
- No new clippy warnings
