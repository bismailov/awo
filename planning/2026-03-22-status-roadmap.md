# Awo Project Status & Roadmap (Updated March 24, 2026)

## 1. Current State

`awo` is a **usable daily-driver orchestrator** with a functional CLI, TUI, daemon, and MCP server. The core is hardened with typed errors, typed enums for all state, and 356+ tests. **CI is green on all three platforms** (macOS, Ubuntu, Windows). All TUI usability blockers are resolved.

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
- Team reconciliation logic extracted to `team/reconcile.rs`
- Cross-platform path canonicalization via `dunce` crate
- Module decomposition: `app/team_ops.rs` (team orchestration), `store/tests.rs` (test extraction)
- `DirtyFileCache` with 5s TTL for git status caching in snapshot hot path

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
- `s` acquire slot (text input for task name), `Enter` start session (text input for runtime:prompt) / view log
- `x` cancel session, `X` release slot (both non-blocking background ops)
- `t` start next team task (auto-selects first todo task, acquires slot, launches session)
- `r` refresh review / refresh log, `Esc` close log panel
- `a` add repo, `c` context doctor, `d` skills doctor
- `?` keybinding help overlay
- Tab/BackTab panel cycling, Up/Down/j/k navigation
- Live log tailing with auto-refresh for running sessions, `[running]` indicator, scroll tracking
- Input bar overlay for text entry
- Background dispatch via `crossbeam-channel` for slot acquire/release

**Quality**
- `unsafe_code = "forbid"` workspace-wide
- Zero `anyhow` in production core code (typed `AwoError` throughout); `anyhow` only in test helpers
- All error-swallowing patterns replaced with structured tracing logging
- 356+ tests across all modules; **CI green on all 3 platforms**
- `#[derive(Debug)]` on all public structs for debuggability

## 2. What Was Just Completed (March 24)

### Wave 1 (earlier March 24)
- **Windows CI fully green**: `dunce::canonicalize`, e2e test isolation, gated unix imports, fixed shell/PTY test assertions
- **Team reconcile extraction**: moved to `team/reconcile.rs` (~160 LOC out of app.rs)
- **Flaky test fix**: PTY timing test uses poll loop instead of fixed sleep

### Wave 2 (March 24)
- **Module decomposition**: `app.rs` reduced from ~890 to ~165 LOC (team ops → `app/team_ops.rs`); `store.rs` reduced from ~1045 to ~675 LOC (tests → `store/tests.rs`)
- **TUI input & help** (Job Card A): text input mode for task names and session prompts, `?` help overlay
- **Background git ops** (Job Card B): `SlotAcquire` and `SlotRelease` via background threads with `crossbeam-channel`
- **Live log tailing** (Job Card C): auto-refresh for running sessions, auto-open on session start, `[running]` indicator, scroll tracking
- **Git status caching** (Job Card D): `DirtyFileCache` with 5s TTL, `RefCell` on `AppCore`, cache invalidation on mutations, stale entry pruning

### All Previous Blockers Resolved

| Previous Gap | Resolution |
|------|--------|
| TUI blocks on git ops | **Done** — background dispatch via crossbeam-channel |
| No TUI input prompts | **Done** — inline text input for task name and runtime:prompt |
| No session output streaming | **Done** — live log tailing with auto-refresh every 200ms |
| `app.rs` ~890 LOC | **Done** — decomposed to ~165 LOC + `app/team_ops.rs` |
| `store.rs` ~1050 LOC | **Done** — reduced to ~675 LOC + `store/tests.rs` |
| Git status caching | **Done** — `DirtyFileCache` with 5s TTL |
| No help overlay | **Done** — `?` keybinding reference |

## 3. Next Wave: Hardening & Depth

Focus: **make the core more reliable and the review/runtime layers more capable** before broadening scope.

### Milestone A: Negative-Path Test Coverage

The happy path is well-tested (356+ tests). The failure path is thin — corrupt state, broken manifests, missing git repos, runtime failures. Add targeted negative-path tests to the modules with the thinnest coverage relative to their complexity.

Priority targets:
- `store.rs` (676 LOC, 11 tests) — corrupt/missing DB, malformed rows, migration edge cases
- `commands/` (1450 LOC, 0 tests) — invalid inputs, missing repos/slots/sessions, permission failures
- `snapshot.rs` — broken slot paths, missing git repos, partial state

See: `planning/job-card-E-negative-path-tests.md`

### Milestone B: Review Intelligence

The review engine produces useful warnings but doesn't yet analyze overlap by changed-file classes or explain *why* a slot is blocked/releasable. Richer review surfaces make the operator's decisions faster and safer.

Priority targets:
- Changed-file overlap detection (two slots touching the same files)
- Richer blocking/releasable explanations in review warnings
- Repo-scoped and team-scoped review depth

See: `planning/job-card-F-review-intelligence.md`

### Milestone C: Runtime Maturity (Future)

- Windows shell hardening
- Session timeout/interruption controls
- Structured runtime output parsing

### Milestone D: Team Execution Depth (Future)

- Result consolidation across workers
- Routing policy reporting
- Lead/worker reconciliation helpers

### Milestone E: Middleware Mode (Future)

- Stabilize JSON command contract
- Broker/daemon mode
- MCP facade

## 4. Deferred (Not Now)

- Async store / Tokio migration
- MCP resource subscriptions
- Named Pipe transport for Windows daemon
- Context pack auto-generation
- Comprehensive Rust doc comments
- Multi-agent handoff flows
- Routing policy engine externalization
- WASI sandboxing
