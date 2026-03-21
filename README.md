# awo

`awo` is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repositories.

The working interaction model is a single-window controller: `awo` owns the repo, team, slot, session, and review state in one place, while the actual agents run in background sessions or attached terminals.

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
- repo-scoped review summaries in the CLI
- one-shot session visibility while sessions are still running
- review warnings around stale, dirty, blocked, or failed work
- early team-orchestration foundations: runtime capability registry and durable team manifests

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
- session cancellation and terminal-session deletion
- review summary and warnings in both CLI and TUI
- repo-filtered `review status` output in the CLI
- TUI repo selection with per-repo context-pack and skill-health detail
- regression tests for the trickiest lifecycle edges

What is not done yet:
- embedded terminal sessions
- structured agent output parsing
- machine-readable JSON result mode across the full CLI surface
- true interruption or timeout control for running one-shot sessions
- team manifests and runtime-agnostic subagent orchestration
- repo overlap detection by changed-file classes
- remote machine targets
- Windows-native PTY supervision backend
- richer multi-turn runtime adapters beyond one-shot task execution

See also:
- [SUBAGENT_ORCHESTRATION.md](/Users/bismailov/Documents/chaban/SUBAGENT_ORCHESTRATION.md)
- [TEAM_MANIFEST_SPEC.md](/Users/bismailov/Documents/chaban/TEAM_MANIFEST_SPEC.md)

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

## Safety Rules Implemented

- dirty slots cannot be released
- pending sessions block release
- only one pending write-capable session may be attached to a slot
- stale slots block new write-capable sessions
- released fresh slots are treated as intentionally gone, not broken
- released warm slots refuse refresh while the base repo has uncommitted changes
- tmux-backed PTY sessions get unique hashed supervisor refs to avoid name collisions

## Session Modes

`session start` supports:
- `--launch-mode pty`
  Runs the command inside a detached tmux session, syncs status back into the app, and writes a combined PTY log.
- `--launch-mode oneshot`
  Runs the command directly and waits for completion in the calling process.

The default is environment-aware: `pty` when the configured PTY supervisor is available, otherwise `oneshot`.

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
