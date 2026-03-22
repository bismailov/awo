# Development Plan

## Purpose

This document is the shortest reliable way for a new agent or collaborator to continue `awo` development without reconstructing the entire project history from chat logs.

It answers four questions:

1. What `awo` is trying to become
2. What has already been built
3. What is still missing
4. What the next implementation waves should prioritize

Current baseline for this document:
- branch: `main`
- commit: `27188b1`

## Product Vision

`awo` is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repositories.

Its wedge is not "chat with one model" and not "manually create worktrees." Its wedge is the operational layer between them:

- acquire a safe ready workspace
- attach the right runtime
- inject the right repo context and skills
- preserve team and review state
- recycle the workspace safely

Longer term, `awo` should evolve from a local operator console into a middleware layer that can present itself as one virtual coding agent while internally orchestrating slots, runtimes, team policies, and review flows.

See also:
- [docs/product-spec.md](../docs/product-spec.md)
- [docs/core-architecture.md](../docs/core-architecture.md)
- [docs/middleware-mode.md](../docs/middleware-mode.md)
- [docs/subagent-orchestration.md](../docs/subagent-orchestration.md)

## What We Wanted

The project started with a clear V1 goal:

- TUI-first local orchestration
- Rust core with a command model underneath the UI
- safe Git worktree slot management
- AI runtime launching for Codex, Claude, Gemini, and shell
- project-context and skill injection
- persistent session/review state
- operator trust through warnings, recovery, and explicit lifecycle control

The initial roadmap was:

1. repo registration
2. slot lifecycle
3. readiness and fingerprints
4. session engine and adapters
5. review and release-confidence signals
6. hardening

See:
- [docs/v1-roadmap.md](../docs/v1-roadmap.md)
- [docs/v1-control-surface.md](../docs/v1-control-surface.md)

## What We Achieved

`awo` is no longer just a design scaffold. It now ships a real V1 slice with meaningful operator value.

### Core orchestration

- Rust workspace split into `awo-core` and `awo-app`
- SQLite-backed operational state
- repository registration plus local overlay generation
- remote repo clone and fetch flows
- fresh and warm slot acquisition, release, and refresh
- dependency fingerprint readiness and stale detection

### Runtime/session layer

- runtime support for Codex, Claude, Gemini, and shell
- platform-aware defaults for session launch mode
- tmux-backed PTY supervision on Unix-like systems
- persisted supervisor metadata on sessions
- oneshot session crash recovery via PID/exit sidecars
- portable shell-wrapper hardening and safer shell prompt delivery

### Context and skills

- repo context discovery from entrypoint docs, standards docs, and `analysis/`
- context doctor command
- shared skill discovery from `.agents/skills/`
- runtime-specific skills doctor, link, and sync flows
- safer skill sync behavior, including canonicalized symlink pruning
- launch-context injection for AI runtimes

### Review and safety

- review summaries and warnings for dirty, stale, blocked, missing, or failed work
- repo-scoped review output
- reduced comparison scatter via typed slot/session state helpers in critical paths
- better observability for snapshot failures via warning logs instead of silent swallowing

### Team orchestration

- runtime capability registry
- starter team manifests
- team member and task mutation flows
- task-driven session launch
- team archive, reset, teardown, and delete lifecycle commands
- TUI team selection and selected-team detail pane
- improved operator-facing text output for team surfaces

### Machine-readable operator surface

- unified JSON envelopes for the main CLI flows
- middleware-friendly serialization for command results and events

### Hardening and architecture progress

- large app/core monoliths split into smaller internal modules
- typed `AwoError` / `AwoResult` boundary at the public core edge
- versioned SQLite migrations instead of one-off schema repair
- WAL mode enabled during initialization
- cross-platform CI and stronger platform-sensitive tests

## Current Product Shape

Today `awo` is best described as:

- a working local operator console
- a repository-aware orchestration core
- a stable-enough CLI/TUI control plane
- an early middleware foundation

It is already useful for:

- isolating parallel work safely
- launching multiple agent runtimes against managed slots
- attaching project context and shared skills consistently
- running team/task workflows with durable manifests
- reviewing and cleaning up work without losing lifecycle state

It is not yet a fully mature "virtual super-agent" product.

## Non-Negotiable Invariants

Any future implementation should preserve these rules.

### Architecture

- `awo-core` owns orchestration logic
- `awo-app` is a shell over the core, not the source of truth
- all state mutations should continue to flow through commands in the core

### Workspace safety

- no unsafe slot reuse when dirty
- pending sessions continue to block release
- write-capable work should remain slot-isolated by default
- team lifecycle commands must not hide live blockers

### Context discipline

- repo context and shared standards should continue to be first-class
- skill handling should remain repo-aware and runtime-aware
- middleware ambitions must not bypass the core safety model

### Scope discipline

- prefer bounded, high-signal slices over broad rewrites
- preserve real operator trust over flashy UI changes

## What Is Still Missing

The most important remaining work falls into six buckets.

### 1. Core hardening still in progress

- `anyhow` is not yet fully pushed out of the deeper core internals
- some persisted slot/session records are still string-backed rather than fully enum-backed
- negative-path coverage is still thinner than happy-path coverage for several modules
- SQLite migration handling is better now, but malformed metadata and hostile filesystem behavior are still only lightly exercised

### 2. Runtime and supervision maturity

- Windows shell hardening still lags the Unix path
- Windows-native PTY supervision is not implemented
- richer interruption/timeout control for running one-shot sessions is still missing
- structured runtime output parsing is still minimal

### 3. Review and reconciliation depth

- warning logic is useful but still intentionally small
- overlap/conflict detection by changed-file classes is not implemented
- review surfaces remain more operational than analytical

### 4. Team-orchestration depth

- team manifests are useful, but portable multi-agent orchestration above vendor-native team features is still early
- result consolidation across multiple workers/runtimes is still mostly a human lead task
- task execution output is better, but end-to-end multi-agent planning/execution loops are not yet automated

### 5. Middleware evolution

- the JSON CLI contract is strong enough to build on, but there is no daemon/broker mode yet
- there is no MCP facade yet
- routing and policy selection remain mostly operator-driven rather than centrally automated

### 6. UX/product polish

- TUI is operational but not yet deeply navigable or inspectable across all entities
- there is no embedded terminal/session transcript view
- docs are rich but scattered across many files

## Recommended Next Milestones

The next steps should continue the current strategy: harden the core and narrow the biggest remaining product gaps before chasing broader surface area.

### Milestone A: Core hardening and reliability

Goal:
- make the current V1 slice more trustworthy before broadening it

Recommended work:
- continue shrinking `anyhow` usage in deeper core modules
- convert more slot/session status call sites to typed helpers or typed enums
- add negative-path tests for:
  - corrupt or partial SQLite metadata
  - broken or malformed team manifests
  - Git discovery failures
  - runtime launch and reconciliation failures
- review WAL behavior and fallback expectations on edge filesystems

Exit criteria:
- clearer typed error boundaries
- stronger failure-path coverage
- fewer string-literal state comparisons in core review/session paths

### Milestone B: Runtime maturity and cross-platform readiness

Goal:
- make runtime execution safer and more portable

Recommended work:
- Windows-specific shell hardening
- Windows-native PTY/supervision path design and first implementation slice
- better interruption and timeout controls for oneshot sessions
- richer session status normalization

Exit criteria:
- shell/runtime behavior is predictable on Windows as well as Unix
- supervision backend abstraction supports a second real backend beyond tmux

### Milestone C: Review intelligence

Goal:
- help operators make safer decisions faster

Recommended work:
- overlap detection by changed-file classes
- richer repo-scoped and team-scoped review surfaces
- stronger explanation around why a slot/team is blocked or releasable

Exit criteria:
- review output becomes a real decision tool, not just a warning list

### Milestone D: Team execution maturity

Goal:
- move from durable team/task bookkeeping toward repeatable multi-agent execution

Recommended work:
- improve result consolidation and handoff flows
- strengthen team-task verification contracts
- add clearer routing/policy reporting around why a runtime was chosen
- introduce more explicit lead/worker reconciliation helpers

Exit criteria:
- multi-agent task execution is easier to run repeatedly without manual glue work

### Milestone E: Middleware mode

Goal:
- turn `awo` into a reusable orchestration substrate for other systems

Recommended work:
- stabilize and document the JSON command contract
- add broker/daemon mode
- expose `awo` through MCP
- add routing/policy logic for runtime selection and multi-runtime plans

Exit criteria:
- an external orchestrator can safely treat `awo` as one orchestration backend

## Recommended Work Order

If one agent or team is continuing from the current baseline, the most sensible order is:

1. finish the remaining hardening from the current audit wave
2. deepen tests and failure handling
3. improve runtime portability and supervision
4. improve review intelligence
5. deepen team execution
6. build daemon/MCP middleware mode on top of the stabilized core

Do not jump to daemon mode or MCP before the current local reliability gaps are smaller.

## Suggested Agent Workstreams

When parallelizing work, prefer bounded lanes with disjoint write scopes.

### Lane 1: reliability/core

Good targets:
- `store.rs`
- `snapshot.rs`
- `slot.rs`
- `runtime.rs`
- failure-path tests

### Lane 2: runtime/platform

Good targets:
- `runtime.rs`
- `runtime/supervisor.rs`
- `runtime/supervisor/*`
- platform-sensitive tests

### Lane 3: team/review/operator UX

Good targets:
- `team.rs`
- `app.rs`
- `awo-app/src/output.rs`
- `awo-app/src/tui.rs`
- operator flow tests

### Lane 4: middleware contract

Good targets:
- JSON command surface
- docs around stable machine-readable behavior
- future broker/MCP facade design and implementation

## What An Agent Should Read First

A fresh implementation agent should start with these files:

1. [README.md](../README.md)
2. [docs/product-spec.md](../docs/product-spec.md)
3. [docs/core-architecture.md](../docs/core-architecture.md)
4. [docs/middleware-mode.md](../docs/middleware-mode.md)
5. [docs/team-manifest-spec.md](../docs/team-manifest-spec.md)
6. [docs/tokio-decision.md](../docs/tokio-decision.md)
7. this file

Then inspect the latest commits on `main` to understand the most recent hardening wave.

## Near-Term Priorities Right Now

From the current baseline, the highest-value near-term priorities are:

1. finish the remaining typed-state and typed-error cleanup in the core
2. add more negative-path and recovery-path tests
3. harden Windows shell/runtime behavior
4. improve review intelligence beyond the current warning set
5. tighten team-task verification and result consolidation

## Success Criteria For The Next Stage

The next stage of development should leave `awo` in a state where:

- the local operator experience is robust enough for daily use
- runtime/session behavior is explainable and recoverable
- team execution feels safer and less ad hoc
- the JSON contract is strong enough to support a real facade layer
- future daemon/MCP work can build on a stable orchestration core rather than a moving target

## Summary

`awo` has already crossed the line from concept to working product.

The right move now is not to broaden scope recklessly. It is to:

- harden the core
- deepen review and team execution
- close the most important platform/runtime gaps
- then turn the now-stable orchestration engine into a proper middleware layer

That path preserves the product’s actual wedge: safe, repo-aware, multi-agent workspace orchestration.
