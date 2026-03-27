# Development Plan

## Purpose

This document is the shortest reliable way for a new agent or collaborator to continue Awo Console development without reconstructing the entire project history from chat logs.

It answers four questions:

1. What Awo Console is trying to become
2. What has already been built
3. What is still missing
4. What the next implementation waves should prioritize

Current baseline for this document:
- branch: `main`
- checkpoint: post-audit working tree on March 28, 2026

## Product Vision

Awo Console is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repositories.

Its wedge is not "chat with one model" and not "manually create worktrees." Its wedge is the operational layer between them:

- acquire a safe ready workspace
- attach the right runtime
- inject the right repo context and skills
- preserve team and review state
- recycle the workspace safely

Longer term, Awo Console should evolve from a local operator console into a middleware layer that can present itself as one virtual coding agent while internally orchestrating slots, runtimes, team policies, and review flows.

See also:
- [docs/product-spec.md](../docs/product-spec.md)
- [docs/core-architecture.md](../docs/core-architecture.md)
- [docs/middleware-mode.md](../docs/middleware-mode.md)
- [docs/subagent-orchestration.md](../docs/subagent-orchestration.md)

## Product Contract Lock (Milestone 0)

The roadmap is now anchored to a local-first product contract:

- slot pooling is **mostly automatic, but transparent**
- task briefs use **structured fields plus freeform notes**
- history ownership stays bounded to **audit/debug/reporting needs**
- remote execution stays **deferred until the local-slot model is proven**

This means the finish line is not "expand into distributed orchestration quickly." The finish line is "make the local broker, local recovery model, and local operator workflows trustworthy enough for daily use."

## What We Wanted

The project started with a clear V1 goal:

- TUI-first local orchestration
- Rust core with a command model underneath the UI
- safe Git worktree slot management
- AI runtime launching for Codex, Claude, Gemini, and shell
- project-context and skill injection
- persistent session/review state
- operator trust through warnings, recovery, and explicit lifecycle control

## What We Achieved

Awo Console is now a robust local orchestration substrate with a much richer operator loop than the original V1 slice.

### Core Orchestration & Hardening
- Rust workspace split into `awo-core`, `awo-app`, and `awo-mcp`.
- SQLite-backed state with **Version 5 schema** (Timeouts, StartedAt).
- Full typed-state engine (`SessionStatus`, `SlotStatus`, etc.) — zero string-based status comparisons.
- **433+ tests** with exhaustive negative-path coverage for store, commands, and snapshots.
- Cross-platform path normalization via `dunce`.

### Runtime & Platform
- Unix (`tmux`) and Windows (`ConPTY` via `portable-pty`) supervision backends.
- Authoritative process group/tree cancellation.
- Session timeout enforcement and ISO8601 tracking.
- Portable shell-wrapper hardening.

### Review & Safety
- **Multi-tiered overlap detection**: Risky (dirty slot), Soft (directory-level), and File-level grouping.
- `DirtyFileCache` with 5s TTL for git status efficiency.
- Warnings for missing worktrees, stale fingerprints, and blocked sessions.

### Team Execution
- Team manifests with task cards and member routing.
- Replaceable current lead tracking plus lead-session visibility.
- Planning-to-task-card workflow with draft/approved/generated plan items.
- Immutable task recovery with `cancelled` / `superseded` task-card history.
- **Automated task verification**: Executes quality gates (e.g., `cargo test`) on session completion.
- **Result consolidation**: Captures logs, status, and summaries into task cards.
- **Report generation**: Markdown summaries of team missions and outcomes in `analysis/reports/`.

### Operator Interfaces
- **TUI**: Responsive dashboard with background Git ops, plan/task-card panes, review/cleanup queues, bounded diff inspection, and live log tailing.
- **CLI**: full-lifecycle management via headless dispatcher, including review diff, model overrides, and immutable recovery flows.
- **MCP Server**: Exposes orchestration tools and resources to LLMs.
- **Daemon (`awod`)**: JSON-RPC 2.0 over UDS for headless brokerage.

### Recent Local-Product Gains
- Configurable clone/worktree roots and explicit retained-slot pruning.
- Task-card model overrides for budget-aware routing.
- Runtime usage notes and recovery hints surfaced in CLI/TUI.
- Review/consolidation cockpit depth in the Team Dashboard.

## Current Product Shape

Today Awo Console is best described as:

- a working local operator console
- a repository-aware orchestration core
- a stable-enough CLI/TUI control plane
- an early middleware foundation

It is not yet a fully mature "virtual super-agent" product.

## Non-Negotiable Invariants

Any future implementation should preserve these rules.

### Architecture
- `awo-core` owns orchestration logic.
- `awo-app` is a shell over the core, not the source of truth.
- all state mutations flow through the `Dispatcher`.

### Workspace safety
- no unsafe slot reuse when dirty.
- pending sessions block release.
- tasks only reach `Review` if `verification_command` passes.

## Open Source Readiness Rules

As the project moves toward public development, keep these repository rules in place:

- tracked docs and examples should use generic, portable placeholders instead of personal machine paths
- private research, transcript dumps, and scratch planning artifacts should stay out of the public repo unless intentionally curated
- contributor-facing docs should not assume private infrastructure or private repositories
- community-health files (`LICENSE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) should remain accurate as the project evolves

## What Is Still Missing Or Not Finalized

The most important remaining work now falls into six buckets.

### 1. Dispatcher / Control-Surface Parity
- some CLI/TUI team-member and slot-binding mutations still call `AppCore` helpers directly
- `team.member.update`, `team.member.remove`, `team.member.assign_slot`, and `team.task.bind_slot` are still missing as first-class commands
- direct-vs-daemon parity is therefore improved but not fully complete

### 2. Broker Maturity
- production-grade daemon lifecycle and degraded-state handling still need more real-world hardening
- broker-mode concurrency validation should deepen further
- push/subscription-style event delivery should replace more polling behavior over time

### 3. Platform Maturity
- Windows ConPTY completion and workflow validation remain open
- Named Pipe transport for the Windows daemon remains open

### 4. Runtime Usage Truth
- provider-specific usage/capacity telemetry is still mostly advisory rather than structured
- lead/worker recovery guidance is honest, but richer adapter-fed budget and lifetime data still needs work

### 5. Middleware Enrichment
- automated context-pack generation
- shared RPC type cleanup
- stronger local MCP subscription semantics

### 6. Release Finalization
- help text and contributor docs still need a polish pass
- manual scenario coverage needs a fresh full-product release sweep
- known limitations should be tightened into a cleaner public release story

## Current Objectives

1. Finish dispatcher parity for all remaining mutating team flows.
2. Make the daemon truly feel like the default local broker.
3. Finish the honest local-platform story on Windows.
4. Deepen structured runtime usage/capacity truth without inventing fake telemetry.
5. Turn the current strong engineering substrate into a public release-quality local product.

## Recommended Next Milestones

### Milestone 1: Control-Surface Completion
Goal: remove the remaining command-layer gaps between CLI/TUI/direct core paths.
- add missing command variants for team-member update/remove/assign-slot and task-slot binding
- route remaining mutating operator flows through `Command` dispatch
- add direct-vs-daemon parity coverage for those paths

### Milestone 2: Broker Hardening
Goal: make the daemon feel like a dependable local broker.
- harden lifecycle/status/cleanup behavior
- validate broker-mode concurrency
- upgrade event delivery for live clients

### Milestone 3: Windows Completion
Goal: achieve honest local parity on Windows.
- finish ConPTY workflow parity
- implement Named Pipes for the daemon

### Milestone 4: Runtime Usage Truth
Goal: turn advisory recovery messaging into stronger runtime-backed operator signals where possible.
- add adapter-fed usage/capacity support where runtimes expose it
- keep `unknown`/`unsupported` explicit where they do not
- improve lead/worker handoff guidance around timeouts and token exhaustion

### Milestone 5: Local-First Enrichment And Release Finalization
Goal: deepen local orchestration and finish the release story.
- middleware subscriptions and context-pack generation
- richer lead/worker handoff and synthesis
- help text, manual scenarios, known-limitations docs, and release polish

## Recommended Work Order

1. Finish dispatcher/control-surface parity.
2. Finalize daemon lifecycle, degraded-state handling, and broker validation.
3. Complete Windows parity.
4. Strengthen runtime usage/capacity truth where adapters support it.
5. Enrich local middleware and orchestration intelligence.
6. Do the final release-quality documentation and validation pass.

## Summary

Awo Console has moved from concept to a strong local orchestration product with real planning, review, cleanup, and recovery flows. The focus now shifts from "make the features exist" to "finish the last parity gaps, harden the broker/platform story, and ship a local product that feels coherent and trustworthy end to end."
