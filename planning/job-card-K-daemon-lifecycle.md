# Job Card K: Daemon Lifecycle Management

## Objective

Implement `awo daemon start`, `awo daemon stop`, and `awo daemon status` subcommands so the daemon can be managed as a proper long-lived process. Today the daemon server exists (`crates/awo-core/src/daemon.rs`) but there is no way to start/stop/query it from the CLI.

## Context

Read these files first (in order):
1. `CLAUDE.md` — project rules
2. `docs/core-architecture.md` — module structure
3. `crates/awo-core/src/daemon.rs` — existing DaemonServer/DaemonClient/DaemonOptions
4. `crates/awo-core/src/dispatch.rs` — Dispatcher trait, RPC types, command routing
5. `crates/awo-app/src/cli.rs` — CLI argument parsing (clap)
6. `crates/awo-app/src/handlers.rs` — CLI command handlers
7. `crates/awo-mcp/src/main.rs` — MCP binary (reference for how AppCore bootstraps)

## What already exists

- `DaemonServer`: Synchronous JSON-RPC 2.0 server over Unix Domain Sockets. File-lock for single-instance. Non-blocking accept loop with shutdown flag (`Arc<AtomicBool>`). Stale socket cleanup on start.
- `DaemonClient`: Connects to socket, implements `Dispatcher` trait, auto-incrementing message IDs.
- `DaemonOptions`: Socket path (`{data_dir}/awod.sock`) and lock path (`{data_dir}/awod.lock`). Generated from `AppPaths`.
- `AppCore`: Implements `Dispatcher`. Bootstraps from config. Holds `Store` (r2d2 SQLite pool).
- End-to-end test exists in `daemon.rs` tests (echo dispatcher).

## Deliverables

### 1. `awo daemon start` (foreground mode)

Behavior:
- Bootstrap `AppCore` from config (same pattern as `crates/awo-mcp/src/main.rs`).
- Acquire the daemon file lock via `DaemonServer`.
- If lock already held, print error and exit 1.
- Write a pidfile to `{data_dir}/awod.pid` containing the current process PID.
- Run the accept loop (blocking, synchronous — no Tokio).
- On SIGTERM/SIGINT, set the shutdown flag, clean up socket + pidfile, exit 0.

Implementation notes:
- Use `signal-hook` or `ctrlc` crate for signal handling. The shutdown flag already exists in `DaemonServer` as an `Arc<AtomicBool>` — wire the signal to set it.
- The pidfile is a plain text file with just the PID number.

### 2. `awo daemon stop`

Behavior:
- Read `{data_dir}/awod.pid`.
- If pidfile missing → print "daemon not running", exit 0.
- If PID exists, send SIGTERM (`libc::kill` or `nix::sys::signal`).
- Wait up to 5 seconds for pidfile removal (poll at 100ms).
- If still alive after 5s, send SIGKILL and remove pidfile + socket.
- Print confirmation.

### 3. `awo daemon status`

Behavior:
- Check if `awod.pid` exists and the PID in it is alive (`kill(pid, 0)`).
- Check if `awod.sock` exists and is connectable (try `DaemonClient::connect`, timeout 1s).
- Print one of:
  - `running (pid {N}, socket ok)`
  - `running (pid {N}, socket not responding)` — stale state
  - `not running`
- Exit 0 if running, exit 1 if not.

### 4. Tests

Add tests in `crates/awo-core/src/daemon.rs` (unit) and/or `crates/awo-core/tests/` (integration):

- `start` acquires lock and creates pidfile.
- `start` when already running exits with error.
- `stop` sends signal and cleans up pidfile.
- `stop` when not running is a no-op.
- `status` reports correctly for running/not-running states.
- Stale pidfile (process dead but file exists) is handled gracefully by `start` and `status`.
- Stale socket file (no daemon) is cleaned up on `start`.

### 5. CLI wiring

In `crates/awo-app/src/cli.rs`, add a `Daemon` subcommand group:
```
awo daemon start
awo daemon stop
awo daemon status
```

## Write scope

Files you will likely touch:
- `crates/awo-core/src/daemon.rs` — add pidfile helpers, signal wiring, stop/status logic
- `crates/awo-app/src/cli.rs` — add daemon subcommand group
- `crates/awo-app/src/handlers.rs` — add daemon command handlers
- `crates/awo-core/Cargo.toml` — add `signal-hook` or `ctrlc` dependency
- `crates/awo-core/tests/` — integration tests

Files you must NOT touch:
- `crates/awo-core/src/commands.rs` — daemon lifecycle is not a domain command
- `crates/awo-core/src/team.rs` or `team/` — unrelated
- `crates/awo-core/src/store.rs` — unrelated
- `crates/awo-app/src/tui.rs` — unrelated

## Constraints

- `unsafe_code = "forbid"` workspace-wide. No unsafe.
- Synchronous core — no Tokio. The daemon loop is already sync.
- Unix-only for now. Guard daemon subcommands with `#[cfg(unix)]`. Windows can return an "unsupported" error.
- Daemon lifecycle (start/stop/status) is process management, not domain state — it does NOT go through `Command` enum.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

All must pass with zero warnings.

## Definition of done

- `awo daemon start` starts the daemon, acquires lock, writes pidfile, serves JSON-RPC.
- `awo daemon stop` stops the daemon cleanly.
- `awo daemon status` reports whether the daemon is running.
- Stale pidfile/socket are handled gracefully.
- Signal handling (SIGTERM/SIGINT) triggers clean shutdown.
- Tests cover happy path + stale state + already-running + not-running.
- No regressions in existing 379+ tests.
