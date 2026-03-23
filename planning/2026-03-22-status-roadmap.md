# Awo Project Status & Roadmap (March 23, 2026)

## 1. Current State

`awo` is a working orchestrator with a functional CLI, TUI, daemon, and MCP server. The core is hardened with typed errors, typed enums for all state, and 158+ tests. The infrastructure exists — the next priority is **using it end-to-end**.

### What's Built

**Core (`awo-core`)**
- Typed state engine: `SessionStatus`, `SlotStatus`, `FingerprintStatus` enums
- SQLite persistence with WAL mode, versioned migrations
- Command/Dispatcher pattern: transport-agnostic command execution
- Review engine: risky-overlap and soft-overlap detection across slots
- Multi-runtime support: Codex, Claude, Gemini, Shell
- Platform layer: Unix (tmux PTY supervision), Windows (ConPTY via `portable-pty`)
- Git worktree isolation, context packs, skill catalogs
- Routing engine with cost-tier/capability-aware runtime selection
- Team manifests with task cards, execution modes, member routing

**Daemon (`awod`)**
- JSON-RPC 2.0 over Unix Domain Socket
- Single-writer safety via file lock
- `DaemonClient` with `Dispatcher` trait impl
- CLI auto-detection: `CliBackend` dispatches through daemon when running, falls back to direct

**MCP Server (`awo-mcp`)**
- Stdio JSON-RPC 2.0 transport (synchronous, no Tokio)
- 8 tools: `list_repos`, `acquire_slot`, `release_slot`, `list_slots`, `start_session`, `cancel_session`, `list_sessions`, `get_review_status`
- 4 resources: `awo://repos`, `awo://slots`, `awo://sessions`, `awo://review`
- 14 unit tests

**CLI (`awo-app`)**
- 9 subcommands covering full lifecycle
- `CliBackend` auto-detects daemon for dispatch, reads directly from SQLite
- TUI: ratatui-based dashboard with repo/team/slot/session panels

**Quality**
- `unsafe_code = "forbid"` workspace-wide
- Zero `anyhow` in core (typed `AwoError` throughout)
- 158+ tests across all modules

## 2. Immediate Priority: See It Work

The app has never been run end-to-end by the project owner. Before adding anything new, the focus is:

1. **Smoke-test the CLI workflow**: `awo repo add` → `awo slot acquire` → `awo session start` → observe in TUI
2. **Fix any runtime issues** discovered during the smoke test
3. **TUI operability**: The TUI currently shows state but doesn't let you drive operations. Wire up keyboard commands for the slot/session lifecycle (see job card)

## 3. Next Steps (Post-Smoke-Test)

These are the only items on the roadmap until the app is proven usable:

- **TUI interactivity**: Add keyboard-driven slot acquire/release, session start/cancel from TUI panels
- **Error UX**: Surface actionable error messages in TUI and CLI when things go wrong
- **Log tailing**: Show live PTY output in TUI for running sessions
- **Team run from TUI**: Let the operator kick off a team run and watch progress

## 4. Deferred (Not Now)

These are real ideas but explicitly parked to avoid feature creep:

- WASI sandboxing research
- Windows CI / ConPTY verification
- MCP resource subscriptions
- Named Pipe transport for Windows daemon
- Routing auto-selection / policy engine
- Context pack auto-generation
