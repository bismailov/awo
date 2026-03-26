# awo

`awo` is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repositories.

The working interaction model is a single-window controller: `awo` owns the repo, team, slot, session, and review state in one place, while the actual agents run in background sessions or attached terminals.

**Welcome Private-Alpha Testers!**
As we stabilize `awo`, we rely on your feedback to refine the agent orchestration lifecycle. We are currently focusing heavily on improving our machine-readable interfaces to let other tools coordinate via `awo`.

It currently manages:
- repository registration
- managed remote cloning and fetching
- isolated Git worktree slots
- fresh and warm slot reuse
- dependency fingerprint readiness
- repo context discovery from entrypoint docs, standards docs, and `analysis/`
- shared-skill discovery from `.agents/skills/` and `skills-lock.json`
- runtime-specific skills doctor, link, and sync flows for Codex, Claude, and Gemini
- automatic launch-context injection for agent runtimes on session start
- platform-aware defaults for session launch mode and skill projection mode
- tmux-backed PTY session supervision
- crash-recoverable oneshot sessions via PID/exit sidecars
- repo-scoped review summaries in the CLI
- one-shot session visibility while sessions are still running
- review warnings around stale, dirty, blocked, or failed work
- executable team orchestration: runtime capability registry, durable team manifests, task cards, and task-driven session launch
- team archive, reset, teardown, and delete lifecycle controls
- machine-readable JSON output across the main operator commands

## Architecture Direction: "JSON inside, MCP outside"

To support building `awo` out into a true middleware layer (see [docs/middleware-mode.md](docs/middleware-mode.md)), we are standardizing on a **"JSON inside, MCP outside"** pattern:
- **JSON Inside:** The core CLI outputs predictable, structured JSON envelopes for all state changes, errors, and events. This makes it a robust, token-efficient controller for local automation, testbeds, and direct invocation by nearby scripts.
- **MCP Outside:** The Model Context Protocol (MCP) acts as our facade. External virtual agents or orchestrated systems connect to `awo` through MCP to safely acquire slots, discover context, and execute sessions without needing to know the low-level CLI structure.

## Current Status

This repo currently ships a working V1 slice with:
- Rust workspace split into `awo-core` and `awo-app`
- SQLite-backed operational state
- repository registration with local overlay generation
- remote repo clone and fetch flows
- fresh and warm slot acquisition/release
- warm slot refresh from base branch
- Codex, Claude, Gemini, and shell session launch support
- context pack and context doctor commands
- shared skill catalog and install-state diagnostics
- runtime-aware skill policy with repo-local preference for Gemini
- distinct `skills sync` repair semantics for drifted or mode-mismatched installs
- detached tmux-backed PTY supervision with status sync
- persisted supervisor metadata on sessions so PTY backend identity survives restarts and schema evolution
- session cancellation and terminal-session deletion
- review summary and warnings in both CLI and TUI
- repo-filtered `review status` output in the CLI
- TUI repo selection with per-repo context-pack and skill-health detail
- runtime capability inspection in both CLI and TUI
- starter team manifest creation plus CLI/TUI team visibility
- executable team member/task workflows and `team task start`
- regression tests for the trickiest lifecycle edges

What is not done yet:
- embedded terminal sessions
- structured agent output parsing
- true interruption or timeout control for running one-shot sessions
- runtime-agnostic subagent orchestration above vendor-native team features
- repo overlap detection by changed-file classes
- remote machine targets
- Windows-native PTY supervision backend
- richer multi-turn runtime adapters beyond one-shot task execution

See also:
- [docs/middleware-mode.md](docs/middleware-mode.md)
- [docs/interface-strategy.md](docs/interface-strategy.md)
- [docs/subagent-orchestration.md](docs/subagent-orchestration.md)
- [docs/team-manifest-spec.md](docs/team-manifest-spec.md)
- [docs/tokio-decision.md](docs/tokio-decision.md)
- [analysis/2026-03-21-public-trial-findings.md](analysis/2026-03-21-public-trial-findings.md)

## Quick Start

Build and run:

```bash
cargo run
```

Useful CLI commands:

```bash
cargo run -- repo add /path/to/repo
cargo run -- repo clone git@github.com:org/repo.git
cargo run -- repo clone https://bitbucket.org/team/repo.git --destination /path/to/clone
cargo run -- repo fetch <repo-id>
cargo run -- repo list

cargo run -- context pack <repo-id>
cargo run -- context doctor <repo-id>

cargo run -- skills list <repo-id>
cargo run -- skills doctor <repo-id>
cargo run -- skills doctor <repo-id> --runtime codex
cargo run -- skills link <repo-id> gemini
cargo run -- skills sync <repo-id> claude --mode copy

cargo run -- runtime list
cargo run -- runtime show claude
cargo run -- --json runtime list

cargo run -- team init <repo-id> team-alpha "Coordinate a safe parallel task"
cargo run -- team list
cargo run -- team show team-alpha
cargo run -- team member add team-alpha reviewer-a reviewer --runtime gemini --read-only
cargo run -- team task add team-alpha audit reviewer-a "Audit docs" "Review the docs" --deliverable "A concise review"
cargo run -- team task start team-alpha audit --launch-mode oneshot
cargo run -- team teardown team-alpha
cargo run -- team teardown team-alpha --force
cargo run -- team delete team-alpha
cargo run -- --json team show team-alpha

cargo run -- slot acquire <repo-id> my-task
cargo run -- slot acquire <repo-id> my-task --strategy warm
cargo run -- slot list --repo-id <repo-id>
cargo run -- slot refresh <slot-id>
cargo run -- slot release <slot-id>

cargo run -- session start <slot-id> codex "Investigate this bug" --read-only
cargo run -- session start <slot-id> claude "Prepare a plan" --launch-mode oneshot
cargo run -- session start <slot-id> gemini "Review architecture" --read-only
cargo run -- session start <slot-id> shell "printf ok; sleep 1; printf done" --read-only
cargo run -- session list
cargo run -- session cancel <session-id>
cargo run -- session delete <session-id>

cargo run -- review status
cargo run -- review status --repo-id <repo-id>
cargo run -- --json session list --repo-id <repo-id>
```

## TUI Keys

The current TUI is intentionally small and operational:
- `q` quit
- `j` / `k` select repo
- `a` register the current working directory as a repo
- `c` run `context doctor` for the selected repo
- `d` run `skills doctor` for the selected repo
- `n` send a no-op command through the core
- `r` refresh review state
- `T` toggle team dashboard

The TUI now also surfaces:
- repo-scoped team manifests
- runtime capability summaries

## Safety Rules Implemented

- dirty slots cannot be released
- pending sessions block release
- only one pending write-capable session may be attached to a slot
- stale slots block new write-capable sessions
- released fresh slots are treated as intentionally gone, not broken
- released warm slots refuse refresh while the base repo has uncommitted changes
- tmux-backed PTY sessions get unique hashed supervisor refs to avoid name collisions
- team-manifest mutations are protected with cross-process file locks
- long-running team-task launches release manifest locks between reservation, slot binding, and runtime execution phases
- oneshot sessions can be reconciled after an interrupted launcher process via PID and exit-code sidecars
- session supervisor backend identity is stored explicitly in session records instead of being inferred from log layout

## Session Modes

`session start` supports:
- `--launch-mode pty`
  Runs the command inside a detached tmux session, syncs status back into the app, and writes a combined PTY log.
- `--launch-mode oneshot`
  Runs the command directly and waits for completion in the calling process.
  If the launcher process is interrupted after spawn, later `session list`, `slot list`, and `review status` runs can still reconcile the session via PID/exit sidecars.

The default is environment-aware: `pty` when the configured PTY supervisor is available, otherwise `oneshot`.

## Team Lifecycle

- `team archive` shelves a team once all tasks are terminal and no active slot/session bindings remain.
- `team reset` clears task progress and bindings but intentionally does not touch live sessions or slots.
- `team teardown` is the operational cleanup path: it cancels cancellable sessions, releases bound slots, and then resets the team back to planning.
- `team delete` removes the manifest once no slot or session bindings remain.

`team teardown` refuses to hide blockers. Dirty slots and running one-shot sessions still require operator attention before the manifest can be cleaned up.

## Context And Skills

`context pack` discovers repo entrypoints such as `AGENTS.md`, `PROJECT.md`, and `CLAUDE.md`, standards docs under `docs/`, optional `.mcp.json`, and task-heavy material under `analysis/`. `context doctor` turns that discovery into a concise readiness report.

`skills list` inspects shared repo skills under `.agents/skills/` and correlates them with `skills-lock.json` when present. `skills doctor` compares those shared skills against the current user-level runtime directories for Codex, Claude, and Gemini.

`skills link` adds missing shared repo skills into a runtime-specific skills directory using symlinks or copies. It intentionally refuses to overwrite conflicting local content.

`skills sync` is stronger: it repairs drifted copied skills, fixes mode mismatches such as “linked but should be copied”, and prunes stale symlink projections that point back into the repo-managed shared skill root.

`session start` now auto-attaches a launch context for AI runtimes unless `--no-auto-context` is passed. The injected context includes entrypoint files, standards docs, and heuristically selected `analysis/` packs based on the task prompt.

## Platform Notes

- macOS and Linux currently get the strongest experience: worktrees, repo context, skills reconciliation, and tmux-backed PTY supervision.
- Windows is already a viable controller environment for repo, slot, skills, and one-shot session flows.
- Windows PTY supervision is not implemented yet; `awo` falls back to `oneshot` behavior there.
- The default skill projection mode is platform-aware: symlinks on Unix-like systems, copies on Windows.

## Verification

Current automated verification:

```bash
cargo fmt
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

GitHub Actions now runs the core Rust validation matrix on Linux, macOS, and Windows for pushes to `main` and pull requests.
