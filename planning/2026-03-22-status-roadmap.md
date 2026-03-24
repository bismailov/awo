# Awo Project Status & Roadmap (March 24, 2026)

## 1. Current State

`awo` is a working orchestrator with a functional CLI, TUI, daemon, and MCP server. The core is hardened with typed errors, typed enums for all state, and 356+ tests. CI is green on macOS/Ubuntu (Windows path canonicalization issue tracked below). The app is ready for its first end-to-end smoke test.

### What's Built

**Core (`awo-core`)**
- Typed state engine: `SessionStatus`, `SlotStatus`, `FingerprintStatus`, `TeamStatus`, `TaskCardState` enums — zero string-based status comparisons
- SQLite persistence with WAL mode, versioned migrations
- Command/Dispatcher pattern: transport-agnostic command execution
- Review engine: risky-overlap and soft-overlap detection across slots (11 tests)
- Multi-runtime support: Codex, Claude, Gemini, Shell
- Platform layer: Unix (tmux PTY supervision), Windows (ConPTY via `portable-pty`)
- Git worktree isolation, context packs, skill catalogs
- Routing engine with cost-tier/capability-aware runtime selection (9 tests)
- Team manifests with task cards, execution modes, member routing (41 tests)
- Actionable error messages with recovery hints throughout
- Collision-proof ID generation with atomic sequence counters

**Daemon (`awod`)**
- JSON-RPC 2.0 over Unix Domain Socket
- Single-writer safety via file lock
- `DaemonClient` with `Dispatcher` trait impl
- CLI auto-detection: `CliBackend` dispatches through daemon when running, falls back to direct

**MCP Server (`awo-mcp`)**
- Stdio JSON-RPC 2.0 transport (synchronous, no Tokio)
- 9 tools: `list_repos`, `acquire_slot`, `release_slot`, `list_slots`, `start_session`, `cancel_session`, `list_sessions`, `get_review_status`, `get_session_log`
- 4 resources: `awo://repos`, `awo://slots`, `awo://sessions`, `awo://review`

**CLI (`awo-app`)**
- 9 subcommands covering full lifecycle including `session log`
- `CliBackend` auto-detects daemon for dispatch, reads directly from SQLite
- TUI: ratatui-based dashboard with full keyboard operability

**TUI Operations**
- `s` acquire slot, `Enter` start session / view log, `x` cancel session, `X` release slot
- `t` start next team task (auto-selects first todo task, acquires slot, launches session)
- `r` refresh review / refresh log, `Esc` close log panel
- `a` add repo, `c` context doctor, `d` skills doctor
- Tab/BackTab panel cycling, Up/Down/j/k navigation
- Inline log viewer overlay with refresh

**Quality**
- `unsafe_code = "forbid"` workspace-wide
- Zero `anyhow` in production core code (typed `AwoError` throughout)
- All error-swallowing patterns replaced with structured tracing logging
- 356+ tests across all modules; CI green on macOS + Ubuntu
- `#[derive(Debug)]` on all public structs for debuggability

## 2. Immediate Priority: Smoke Test

The app has all features needed for end-to-end use. The operator should:

1. `awo repo add .` — register a repository
2. `awo slot acquire <repo_id> <task>` — get a workspace
3. `awo session start <slot_id> shell "ls"` — run a command
4. `awo tui` — open the dashboard, observe state, press Enter to view logs
5. Alternatively: do the entire workflow from within `awo tui`

## 3. Completed Items

All of these were on the roadmap and are now done:

- **TUI interactivity**: Keyboard-driven slot acquire/release, session start/cancel
- **Error UX**: Actionable error messages with recovery hints in CLI and core
- **Log viewing**: Inline session log viewer in TUI (Enter on session, Esc to close, r to refresh)
- **Team run from TUI**: `t` key starts next todo task for selected team
- **Hardening (Milestone A)**: Typed enums, error logging, negative-path tests, sidecar edge cases, review engine tests, error module tests, git happy-path tests
- **CI fixes**: dry_run skips runtime detection, Debug derives for Windows, tracing routed to stderr
- **Audit fixes**: DRY violation in short-ID extraction, collision-proof ID generation

## 4. Next Wave (Post-Smoke-Test)

Organized by milestone per external audit recommendations.

### Milestone B: Runtime Reliability (High Priority)

| Item | Priority | Description |
|------|----------|-------------|
| Async Git ops | High | Long git operations (fetch, clone, worktree add) block TUI. Spawn on background thread with channel notification. `std::thread` + `crossbeam-channel`, no Tokio required. |
| ConPTY completion | Medium | Untested on real Windows. Need: Windows CI green, integration tests with actual PTY, verify taskkill reliability. |
| Windows path canonicalization | Medium | `fs::canonicalize` produces `\\?\` UNC paths on Windows, causing test failures. Consider `dunce::canonicalize` or normalize paths at storage boundary. |
| process_is_running robustness | Low | Windows implementation uses `tasklist` (fragile). Consider `OpenProcess` Win32 API via `windows-sys` crate. |

### Milestone C: Review Intelligence (Medium Priority)

| Item | Priority | Description |
|------|----------|-------------|
| Git status caching | High | `git status --porcelain` runs per-slot on every review/snapshot build. Cache results with TTL or fingerprint-based invalidation. |
| Canonical path normalization | Medium | Overlap detection compares raw git-porcelain paths. Currently safe (repo-relative), but add explicit normalization for symlink resilience. |
| Richer conflict analysis | Medium | Go beyond file/directory overlap: detect semantic conflicts (same function modified), suggest resolution strategies. |
| Decision-quality review output | Low | Structure review output for MCP consumption: JSON warnings with severity, affected slots, suggested actions. |

### Milestone D: Team Execution & Code Organization (Medium Priority)

| Item | Priority | Description |
|------|----------|-------------|
| Extract reconcile logic | High | `reconcile_team_manifest_state` in app.rs mixes reconciliation with command layer. Extract to dedicated module. |
| app.rs decomposition | Medium | ~1050 LOC monolith. Split into focused modules: team orchestration, slot management, snapshot building. |
| store.rs decomposition | Medium | ~870 LOC. Extract query builders, migration logic into sub-modules. |
| Result consolidation | Medium | Multi-agent task results need aggregation and summary generation. |
| Multi-agent handoff flows | Low | Agent-to-agent task delegation with context transfer. |

### Milestone E: Middleware Mode (Lower Priority)

| Item | Priority | Description |
|------|----------|-------------|
| Async Store | Medium | Move to `tokio-rusqlite` or `r2d2` connection pool for high-concurrency daemon. Depends on Tokio decision. |
| Daemon broker | Medium | Route commands between multiple concurrent MCP clients through the daemon. |
| MCP facade | Low | Expose full orchestration capabilities through MCP resource subscriptions. |
| Routing policy engine | Low | Externalize routing rules for operator customization. |

## 5. Deferred (Not Now)

- WASI sandboxing research
- MCP resource subscriptions
- Named Pipe transport for Windows daemon
- Context pack auto-generation
- Comprehensive Rust doc comments for public API (~28% coverage currently; improve incrementally)
- `AwoError` variant refinement (RuntimeLaunch/Supervisor use generic String; consider structured variants)

## 6. External Audit Summary

Two external audits (March 23, 2026) rated the project **Excellent (Production-Ready V1)**. Key findings:

**Strengths identified:**
- Transport-agnostic Dispatcher + JSON-RPC 2.0 architecture
- Git worktree isolation as workspace foundation
- Typed status enums + `thiserror`-based error handling
- Safety-critical "soft signals" (overlaps, fingerprints)
- `unsafe_code = "forbid"` + sync core design

**All high-severity findings addressed:**
- String-backed status comparisons → typed enums
- Error swallowing → structured logging
- Thin negative-path tests → 356+ tests with full coverage
- DRY violations → extracted helpers
- ID collision risk → atomic sequence counters

**Remaining items tracked in Milestones B-E above.**
