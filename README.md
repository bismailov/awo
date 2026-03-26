# Awo Console

`Awo Console` is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repositories.

It sits between "run an AI coding agent" and "manually manage Git worktrees." Instead of treating the problem as chat orchestration alone, Awo Console treats the workspace itself as the unit of control: acquire a safe slot, attach the right runtime, inject repo context and skills, track the session, review overlap risk, and recycle the workspace safely.

The project currently ships from a pre-1.0 Rust workspace with three binaries:

- `awo`: the main CLI and TUI
- `awod`: a background daemon for headless JSON-RPC brokerage
- `awo-mcp`: an MCP server that exposes `awo` as tools and resources to external LLM clients

## Why This Exists

Parallel AI coding usually breaks in one of four places:

- multiple agents write in the same checkout or branch
- a fresh worktree is cheap, but a ready worktree is not
- every AI runtime has different launch behavior and constraints
- context, standards, and prior decisions drift across sessions

Awo Console is meant to be the operational layer that fixes those problems.

## The Core Concept

Awo Console is built around a few durable concepts:

- Repository: a registered Git repo plus local metadata and settings
- Slot: an isolated worktree workspace, either fresh or warm
- Session: a runtime invocation in a slot, such as Codex, Claude, Gemini, or shell
- Context pack: the repo guidance and docs that should travel with each session
- Skill catalog: portable `SKILL.md` workflows discovered from shared repo locations
- Team manifest: a durable record of members, tasks, ownership, and verification
- Review state: warnings about dirty slots, overlap risk, blocked cleanup, and failed work

The important design choice is that Awo Console does not make the UI the source of truth. All mutations flow through the orchestration core.

## How Slots, Sessions, and Teams Fit Together

Think of it like renting desks in a coworking space.

A **slot** is the desk — an isolated git worktree with its own branch and working copy. Each agent gets its own slot, so no two agents can overwrite each other's files. Acquiring a slot creates the worktree; releasing it removes it from disk.

A **session** is the person sitting at the desk — a running AI process (Claude, Codex, Gemini, or a plain shell) attached to a specific slot. The session gets the slot's worktree as its working directory, plus injected context docs and repo skills. Starting a session launches the process; cancelling it kills it.

A **team** is the project coordinator above individual desks. A team has members (named agents with roles and permissions) and tasks (units of work with owners, dependencies, and deliverables). When you start a team task, Awo Console automatically acquires a slot and starts a session for the task owner, so you don't have to manage the plumbing yourself.

The layering is: **repo → slot → session**, with teams as an optional orchestration layer on top.

Without teams (solo workflow): register a repo, acquire a slot, start a session, review the work, release the slot.

With teams: define a team with members and tasks, then let Awo Console handle slot acquisition and session launching as tasks become ready.

## How It Works

Typical flow:

1. Register or clone a repository.
2. Discover context from files like `project.md`, `AGENTS.md`, `CLAUDE.md`, `docs/`, `analysis/`, and optional MCP config.
3. Discover shared repo skills from locations such as `.agents/skills/`.
4. Acquire a worktree slot for a task.
5. Start a runtime session in that slot.
6. Track logs, status, review warnings, and team/task state.
7. Release or refresh the slot when the work is complete.

Under the hood, the workspace is split into:

- `crates/awo-core`: orchestration logic, persistence, Git/worktree lifecycle, runtime handling, review, team workflows
- `crates/awo-app`: the human-facing shell for CLI and TUI usage
- `crates/awo-mcp`: the MCP facade for external tool interoperability

## Interface Model

The architectural direction is:

**JSON inside, MCP outside**

That means:

- the canonical local control plane is a structured command model
- the TUI and CLI are both clients of the same core
- the daemon can expose the same command model over JSON-RPC
- MCP is the outer integration layer for agent clients and IDEs

This keeps the local contract predictable and scriptable without giving up interoperability with tool-based LLM systems.

## CLI Vs TUI

Awo Console is TUI-first, but not TUI-only. The CLI and TUI serve different jobs.

| Surface | Best for | Pros | Cons |
| --- | --- | --- | --- |
| TUI | human operator control, live oversight, reviewing active work | fast situational awareness, easier slot/session browsing, team dashboard, log viewing | less scriptable, more manual, not ideal for automation |
| CLI | scripts, repeatable workflows, shell usage, automation, testing | machine-readable output, composable, easy to automate, good fit for CI-like flows | less visual, easier to lose big-picture state, requires more command knowledge |

In practice:

- use the TUI when you want to supervise a repo, inspect warnings, watch sessions, or operate several parallel tasks by hand
- use the CLI when you want deterministic commands, JSON output, wrappers, scripts, or external automation

## Why MCP

MCP matters when Awo Console needs to be consumed by another agent system instead of directly by a human.

Reasons to use MCP:

- it gives external LLM clients a standard tool protocol instead of a bespoke shell contract
- tool and resource discovery are built in
- it lets Awo Console appear as one orchestration backend even if it is managing many slots and sessions internally
- it is the cleanest path toward the future "virtual coding agent" middleware shape

Reasons not to reach for MCP first:

- if you are a human operator, the TUI is usually better
- if you are writing local scripts, the CLI is usually simpler and more token-efficient
- MCP adds protocol overhead that is unnecessary for basic shell automation

Short version:

- CLI is the best inside contract
- TUI is the best operator console
- MCP is the best outside integration surface

## How To Use MCP

`awo-mcp` is a stdio MCP server. A compatible client starts the binary, sends newline-delimited JSON-RPC messages on stdin, and receives responses on stdout.

At a high level, the flow is:

1. start `awo-mcp`
2. send `initialize`
3. send `tools/list` or `resources/list`
4. call tools like `acquire_slot`, `start_session`, or `get_review_status`

Current MCP tool coverage includes:

- repositories: list and remove
- slots: acquire, release, list
- sessions: start, cancel, list, read logs
- review: status
- teams: list, show, init, add member, add task, reset, report, archive, delete, delegate
- events: poll domain events

Current MCP resources include:

- `awo://repos`
- `awo://slots`
- `awo://sessions`
- `awo://review`
- `awo://teams`

This is most useful when you want a model client to ask Awo Console for safe workspaces and orchestration operations without teaching that client the full CLI surface.

Minimal manual smoke test:

```bash
cargo run --bin awo-mcp
```

Then send newline-delimited JSON-RPC messages such as:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

In a real MCP client, you normally point the client at the `awo-mcp` binary as a stdio server and let the client handle the protocol details.

## Install From Source

```bash
cargo install --path crates/awo-app --bin awo --bin awod
cargo install --path crates/awo-mcp --bin awo-mcp
```

Or run directly with Cargo while developing:

```bash
cargo run --bin awo
```

## Configuration And Storage

By default, Awo Console uses platform-specific config and data directories via the Rust `directories` crate.

You can override them with:

- `AWO_CONFIG_DIR`
- `AWO_DATA_DIR`

`awod` also accepts `--config-dir` and `--data-dir`.

Operational state is stored in SQLite, while larger logs stay file-based.

## Quick Start

Launch the TUI:

```bash
cargo run --bin awo
```

Register a repository and inspect its agent readiness:

```bash
cargo run --bin awo -- repo add /path/to/repo
cargo run --bin awo -- repo list
cargo run --bin awo -- context doctor <repo-id>
cargo run --bin awo -- skills doctor <repo-id>
```

Acquire a workspace and start a session:

```bash
cargo run --bin awo -- slot acquire <repo-id> fix-login
cargo run --bin awo -- session start <slot-id> codex "Fix the login race condition"
cargo run --bin awo -- review status --repo-id <repo-id>
cargo run --bin awo -- slot release <slot-id>
```

Machine-readable output is available with `--json`:

```bash
cargo run --bin awo -- --json repo list
cargo run --bin awo -- --json session list --repo-id <repo-id>
```

## Common Commands

Repository lifecycle:

```bash
awo repo add /path/to/repo
awo repo clone git@github.com:org/repo.git
awo repo fetch <repo-id>
awo repo list
awo repo remove <repo-id>
```

Context and skills:

```bash
awo context pack <repo-id>
awo context doctor <repo-id>
awo skills list <repo-id>
awo skills doctor <repo-id>
awo skills doctor <repo-id> --runtime codex
awo skills link <repo-id> claude
awo skills sync <repo-id> gemini --mode copy
```

Slots and sessions:

```bash
awo slot acquire <repo-id> my-task
awo slot acquire <repo-id> my-task --strategy warm
awo slot list --repo-id <repo-id>
awo slot refresh <slot-id>
awo slot release <slot-id>

awo session start <slot-id> codex "Investigate this bug"
awo session start <slot-id> claude "Prepare a plan" --launch-mode oneshot
awo session start <slot-id> gemini "Review architecture" --read-only
awo session list
awo session cancel <session-id>
awo session delete <session-id>
```

Teams:

```bash
awo team init <repo-id> team-alpha "Coordinate a safe parallel task"
awo team member add team-alpha reviewer-a reviewer --runtime gemini --read-only
awo team task add team-alpha audit reviewer-a "Audit docs" "Review the docs" --deliverable "A concise review"
awo team task start team-alpha audit --launch-mode oneshot
awo team show team-alpha
awo team report team-alpha
awo team teardown team-alpha
```

Daemon:

```bash
awo daemon start
awo daemon status
awo daemon stop
```

## TUI Controls

The TUI is intentionally operational rather than decorative.

- `q`: quit
- `?`: toggle help
- `/`: filter repos, teams, slots, and sessions
- `Tab` / `Shift+Tab`: cycle focus
- `j` / `k`: move selection
- `s`: acquire a slot or start a selected team task
- `Enter`: start a session on a selected slot or open a session log
- `x`: cancel the selected session
- `X`: release the selected slot
- `c`: run `context doctor` for the selected repo
- `d`: run `skills doctor` for the selected repo
- `r`: refresh review state or refresh the current log panel
- `R`: generate a report for the selected team
- `T`: toggle the team dashboard
- `t`: start the next team task
- `Esc`: close panels or clear the current filter

## Session Modes

`session start` supports two launch modes:

- `pty`: launch in a detached supervised terminal session
- `oneshot`: run directly and wait for completion in the calling process

The default depends on the environment. On Unix-like systems, Awo Console prefers PTY-backed supervision when available. On platforms where PTY support is incomplete, it falls back to `oneshot`.

## Daemon Mode

`awod` is the headless broker layer. On Unix it serves JSON-RPC over a Unix domain socket and holds an exclusive lock so only one daemon instance owns the local store at a time.

The daemon is useful when you want:

- a long-lived local broker
- headless orchestration from other tools
- a stable RPC surface around the same command model

## What Awo Console Is Good At Today

- safe slot acquisition and release for local Git repos
- runtime-aware session launching for Codex, Claude, Gemini, and shell
- repo-context discovery and context health checks
- shared skill discovery and runtime skill projection
- review warnings for dirty, stale, blocked, or overlapping work
- team manifests, task tracking, and task-driven session launch
- a usable operator TUI plus scriptable CLI and early MCP surface

## Current Limitations

Awo Console is promising, but still early.

Known gaps and active areas:

- embedded terminals in the TUI are not finished
- Windows PTY supervision is not complete yet
- daemon lifecycle UX is still maturing
- richer output normalization and higher-level middleware behavior are still in progress
- the product is not yet a fully mature "virtual super-agent" layer

## Community

- Contributions are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
- Project participation is governed by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
- Security-sensitive issues should follow [`SECURITY.md`](SECURITY.md).
- Maintainers can use [`docs/open-source-release-checklist.md`](docs/open-source-release-checklist.md) before wider public releases.

## Development

Project onboarding starts with [`project.md`](project.md).

Compatibility handles:

- [`AGENTS.md`](AGENTS.md)
- [`CLAUDE.md`](CLAUDE.md)

Useful follow-up docs:

- [`planning/2026-03-22-development-plan.md`](planning/2026-03-22-development-plan.md)
- [`docs/core-architecture.md`](docs/core-architecture.md)
- [`docs/product-spec.md`](docs/product-spec.md)
- [`docs/interface-strategy.md`](docs/interface-strategy.md)
- [`docs/middleware-mode.md`](docs/middleware-mode.md)
- [`docs/subagent-orchestration.md`](docs/subagent-orchestration.md)
- [`docs/open-source-release-checklist.md`](docs/open-source-release-checklist.md)

Verification:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

Awo Console is available under the terms of the [`MIT License`](LICENSE).

## Philosophy

Awo Console is not trying to be "yet another chat shell."

The real bet is that safe parallel AI coding needs a workspace control plane:

- one slot per write-capable task
- one orchestration core that owns state transitions
- one place to attach context, skills, and runtime policy
- one review layer that warns before parallel work turns into cleanup work

If that layer is solid, the UI surface can change over time. The core value remains the same.
