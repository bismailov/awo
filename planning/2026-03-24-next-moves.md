# Awo Next Moves (March 24, 2026)

## Current Status
- **V1 Core Done**: Slot lifecycle, multi-runtime support, review-based safety, and TUI control surface are fully operational.
- **Hardening Completed**: Typed state, SQLite WAL, negative-path tests, and cross-platform (Windows) parity are established.
- **Performance Optimized**: Background Git operations and status caching prevent TUI freezes.
- **Review Intelligence**: Multi-tiered conflict detection (file-level and directory-level) is live.
- **Session Timeouts**: Timeout enforcement and robust process-tree cancellation are implemented (commit 69e3502).
- **Team Command Layer**: All 11+ team commands routed through the `Command` enum via `commands/team.rs`. Team report generation, task result tracking, verification execution during reconcile, and task delegation are live.
- **Store Pool**: SQLite store migrated from `Mutex<Connection>` to `r2d2` connection pool; `Store` is now `Clone`.
- **Event Bus**: Thread-safe ring buffer with sequence-numbered events and poll-based consumption (commit 174c641). `events.poll` command and MCP `poll_events` tool wired end-to-end.
- **CLI Auto-Start Daemon**: CLI auto-spawns `awod` when no daemon is running; falls back to direct mode on failure (commit f24b36c).

## Completed Work

### Job Card H: Session Timeout & Interruption — DONE
- `timeout_secs` and `started_at` on `SessionRecord` (migration v5).
- Timeout enforcement in `sync_session`.
- Process-group kill with SIGKILL fallback.

### Team Execution Depth — DONE
- `commands/team.rs`: list, show, init, member.add, task.add, task.start, reset, report, archive, teardown, delete.
- `team.report` generates markdown report with per-task result summaries and log paths.
- Reconcile auto-populates `result_summary` and `output_log_path` on completed/failed sessions.
- Reconcile runs `verification_command` and sets task state to Blocked on failure.
- 8 new domain events for team lifecycle.
- TUI `R` keybinding for team report.

### Job Card L: CLI Auto-Start Daemon — DONE
- `spawn_daemon()` in `daemon.rs` spawns `awod` as detached process with polling readiness check.
- `CliBackend::bootstrap()` auto-starts daemon when none is running.
- Best-effort: auto-start failure falls back to direct mode silently.

### Event Bus — DONE
- `EventBus` with bounded ring buffer (1024 entries), monotonic sequence numbers, `Arc<Mutex<>>`.
- `Command::EventsPoll` variant intercepted at `AppCore` level.
- MCP `poll_events` tool registered.
- 7 unit tests + 2 MCP mapping tests.

### Team Task Delegation — DONE
- `team.task.delegate` command with `DelegationContext` (target member, lead notes, focus files, auto_start).
- Unified `execute_team_task` flow shared by `start_team_task` and `delegate_team_task`.
- `render_delegated_prompt` prepends lead notes and appends focus files.
- CLI `Delegate` subcommand, MCP `delegate_team_task` tool.
- `TeamTaskDelegated` domain event.
- 3 integration tests: delegation with auto-start, delegation without auto-start, error paths.

---

## In Progress (Delegated)

### Job Card M: Lead → Worker Task Delegation — ALREADY IMPLEMENTED
The delegation feature was already implemented in the uncommitted work. The external agent assignment is redundant.

### Job Card N: TUI Team Dashboard Panel — DONE
- `TeamDashboardState` and `TeamDashboardFocus` added to `TuiState`.
- `render_team_dashboard` implemented with sidebar team list and detailed team view (objective, members, tasks, progress).
- `T` key toggles dashboard with automatic data refresh.
- Dashboard navigation (`j/k`, `Tab`, `Esc`) and actions (`s` to start task, `Enter` for detail).
- Optimized data loading (loads all team manifests on activation).

---

## Open Work

### Option C: Middleware Foundation — JSON Contract Stabilization
**Objective**: Stabilize and document the JSON CLI command contract.
**Key Tasks**:
- Audit every `Command` variant for consistent serialization.
- Add integration tests that exercise the JSON roundtrip.
- This is the prerequisite for daemon/MCP mode.

### Option D: Review Intelligence — Overlap Detection — DONE
**Objective**: Implement changed-file-class overlap detection across active slots.
**Status**: Consolidated `risky-overlap` and `soft-overlap` logic into `snapshot/overlap.rs`. SURFACES warnings for multi-slot file conflicts and directory-level overlaps.

### Option E: Runtime Maturity — Windows PTY Supervision — DONE
**Status**: `Conpty` backend implemented using `portable-pty` crate. Integrated into `SessionSupervisor` lifecycle. `taskkill` used for authoritative cancellation.

### Option F: Windows Daemon Support — NOT STARTED
**Objective**: Implement Named Pipe transport for `awod` on Windows.
**Key Tasks**:
- Replace `UnixListener`/`UnixStream` with cross-platform abstractions or `tokio-named-pipes` (if we move to async).
- Since we are synchronous, use a synchronous Named Pipe library or conditional compilation.
