# Awo Project Status & Roadmap (Updated March 24, 2026)

## 1. Current State

`awo` is a working orchestrator with a functional CLI, TUI, daemon, and MCP server. The core is hardened with typed errors, typed enums for all state, and 356+ tests. **CI is green on all three platforms** (macOS, Ubuntu, Windows). The app is ready for its first end-to-end smoke test.

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
- 356+ tests across all modules; **CI green on all 3 platforms**
- `#[derive(Debug)]` on all public structs for debuggability

## 2. What Was Just Completed (March 24)

- **Windows CI fully green**: `dunce::canonicalize` replacing `fs::canonicalize` across all call sites; e2e test isolation via `AWO_DATA_DIR`/`AWO_CONFIG_DIR`; gated unix-only imports; fixed shell/PTY test assertions
- **Team reconcile extraction**: `reconcile_team_manifest_state`, `build_team_teardown_plan`, `collect_bound_slot_ids` moved to `team/reconcile.rs` (~160 LOC out of app.rs)
- **Flaky test fix**: PTY timing test uses poll loop instead of fixed sleep

### Items Closed From Previous Roadmap

| Item | Status |
|------|--------|
| Windows path canonicalization | **Done** — `dunce` crate |
| Extract reconcile logic | **Done** — `team/reconcile.rs` |
| ConPTY completion | **Partially done** — CI green, tests pass, but real-device verification still pending |

## 3. Smoke Test Readiness

The app has all features needed for end-to-end use:

1. `awo repo add .` — register a repository
2. `awo slot acquire <repo_id> <task>` — get a workspace
3. `awo session start <slot_id> shell "ls"` — run a command
4. `awo tui` — open the dashboard, observe state, press Enter to view logs
5. Alternatively: do the entire workflow from within `awo tui`

## 4. What's Missing For a Working Daily-Driver App

The app is architecturally complete but has two categories of gaps that prevent comfortable daily use:

### Category A: Blocking for real use

| Gap | Impact |
|-----|--------|
| TUI blocks on git ops | Slot acquire/release/refresh call git synchronously — TUI freezes for seconds on large repos |
| No TUI input prompts | `s` acquires with hardcoded "tui-task" name, `Enter` starts with hardcoded `echo` command. No way to enter task name, runtime, prompt from TUI |
| No session output streaming | Log viewer shows final output only. No live tail during running sessions |

### Category B: Important but not blocking

| Gap | Impact |
|-----|--------|
| `app.rs` still ~890 LOC | Readable but could be cleaner — team orchestration, slot mgmt, snapshot building mixed |
| `store.rs` ~1050 LOC | Query builders and migrations interleaved |
| Git status caching | Every snapshot calls `git status --porcelain` per slot — slow with many slots |
| No help overlay in TUI | Users must remember keybindings |

## 5. Next Wave: Path to Working App

Focus: **make the app usable for real daily work**, not feature-complete.

### Phase 1: TUI Usability (Highest Priority)

| Item | Description | Files |
|------|-------------|-------|
| TUI input prompts | Add simple text input for task name (on `s`), prompt/runtime (on `Enter`). Minimal inline input, not a full dialog system. | `tui.rs` |
| Background git ops | Move slot acquire/release/refresh to `std::thread` with `crossbeam-channel` notification to unblock TUI event loop | `tui.rs`, `app.rs` |
| Help overlay | `?` key shows keybinding reference | `tui.rs` |

### Phase 2: Session Experience

| Item | Description | Files |
|------|-------------|-------|
| Live log tailing | Tail session log file during running sessions, update on `r` or auto-refresh | `tui.rs` |
| Session status auto-sync | Periodic oneshot session sync in TUI loop (already polls at 200ms, just needs sync call) | `tui.rs` |

### Phase 3: Code Organization (Can parallelize with Phase 1-2)

| Item | Description | Files |
|------|-------------|-------|
| app.rs decomposition | Extract team orchestration and snapshot building into sub-modules | `app.rs` |
| store.rs decomposition | Extract migrations and query builders | `store.rs` |

## 6. Deferred (Not Now)

- Async store / Tokio migration
- MCP resource subscriptions
- Named Pipe transport for Windows daemon
- Context pack auto-generation
- Comprehensive Rust doc comments
- Richer conflict analysis (semantic overlaps)
- Multi-agent handoff flows
- Routing policy engine externalization
- WASI sandboxing
