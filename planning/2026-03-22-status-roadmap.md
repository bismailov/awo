# Awo Project Status & Roadmap (March 23, 2026)

## 1. Current State

`awo` is a working orchestrator with a functional CLI, TUI, daemon, and MCP server. The core is hardened with typed errors, typed enums for all state, and 313+ tests. All roadmap Section 3 items are now complete — the app is ready for its first end-to-end smoke test.

### What's Built

**Core (`awo-core`)**
- Typed state engine: `SessionStatus`, `SlotStatus`, `FingerprintStatus`, `TeamStatus`, `TaskCardState` enums — zero string-based status comparisons
- SQLite persistence with WAL mode, versioned migrations
- Command/Dispatcher pattern: transport-agnostic command execution
- Review engine: risky-overlap and soft-overlap detection across slots
- Multi-runtime support: Codex, Claude, Gemini, Shell
- Platform layer: Unix (tmux PTY supervision), Windows (ConPTY via `portable-pty`)
- Git worktree isolation, context packs, skill catalogs
- Routing engine with cost-tier/capability-aware runtime selection
- Team manifests with task cards, execution modes, member routing
- Actionable error messages with recovery hints throughout

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
- 313+ tests: 166 unit, 29 integration (command_flows), 21 MCP, 35 app, 32 store, 30+ others

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
- **Hardening**: All string-based status comparisons replaced with typed enums, git error swallowing replaced with warn logging, 14 new negative-path tests

## 4. Next Wave (Post-Smoke-Test)

- **Hardening depth**: More negative-path tests for edge filesystems, malformed manifests
- **Review intelligence**: Richer overlap and conflict analysis, decision-quality review output
- **Team execution depth**: Result consolidation, multi-agent handoff flows
- **Middleware mode**: Daemon broker, MCP facade, routing policy engine

## 5. Deferred (Not Now)

- WASI sandboxing research
- Windows CI / ConPTY verification
- MCP resource subscriptions
- Named Pipe transport for Windows daemon
- Context pack auto-generation
