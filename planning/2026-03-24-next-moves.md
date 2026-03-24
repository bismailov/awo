# Awo Next Moves (March 24, 2026)

## Current Status
- **V1 Core Done**: Slot lifecycle, multi-runtime support, review-based safety, and TUI control surface are fully operational.
- **Hardening Completed**: Typed state, SQLite WAL, negative-path tests, and cross-platform (Windows) parity are established.
- **Performance Optimized**: Background Git operations and status caching prevent TUI freezes.
- **Review Intelligence**: Multi-tiered conflict detection (file-level and directory-level) is live.
- **Session Timeouts**: Timeout enforcement and robust process-tree cancellation are implemented (commit 69e3502).
- **Team Command Layer**: All 11 team commands routed through the `Command` enum via `commands/team.rs`. Team report generation, task result tracking (`result_summary`, `output_log_path`, `verification_command`), and verification execution during reconcile are live.
- **Store Pool**: SQLite store migrated from `Mutex<Connection>` to `r2d2` connection pool; `Store` is now `Clone`.

## Completed Work

### Job Card H: Session Timeout & Interruption — DONE
- `timeout_secs` and `started_at` on `SessionRecord` (migration v5).
- Timeout enforcement in `sync_session`.
- Process-group kill with SIGKILL fallback.

### Team Execution Depth — DONE (uncommitted)
- `commands/team.rs`: list, show, init, member.add, task.add, task.start, reset, report, archive, teardown, delete.
- `team.report` generates markdown report with per-task result summaries and log paths.
- Reconcile auto-populates `result_summary` and `output_log_path` on completed/failed sessions.
- Reconcile runs `verification_command` and sets task state to Blocked on failure.
- 8 new domain events for team lifecycle.
- TUI `R` keybinding for team report.

---

## Open Work

### Job Card G: TUI Filtering & Navigation — NOT STARTED
**Objective**: Add inline filtering to all TUI panels.
**Key Tasks**:
- Add `filter_query: Option<String>` to `TuiState`.
- Implement `/` key to enter filter mode (text input).
- Apply filter to `visible_repos`, `visible_teams`, etc.
- Highlight the filter string in the UI.

---

## Next Move Options

### Option A: Commit & Ship the Team Execution Wave
Commit the uncommitted team command layer + store pool changes, get CI green, and close Wave 3 cleanly before starting new work.

### Option B: TUI Filtering (Job Card G)
Implement inline `/` filtering across all TUI panels. Improves daily usability as entity counts grow. Self-contained, low coupling to other work.

### Option C: Middleware Foundation — JSON Contract Stabilization
Stabilize and document the JSON CLI command contract. Audit every `Command` variant for consistent serialization. Add integration tests that exercise the JSON roundtrip. This is the prerequisite for daemon/MCP mode.

### Option D: Review Intelligence — Overlap Detection
Implement changed-file-class overlap detection across active slots. Surface "slot X and slot Y both touch migrations" warnings. Turns review from a warning list into a real decision tool.

### Option E: Runtime Maturity — Windows PTY Supervision
Design and implement a Windows-native PTY supervision backend (ConPTY or similar). Currently Unix-only via tmux. Blocks cross-platform parity for the supervision layer.
