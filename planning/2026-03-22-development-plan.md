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
- commit: `30ac045`

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

Awo Console is now a robust orchestration substrate with a stable V1 slice.

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
- **Automated task verification**: Executes quality gates (e.g., `cargo test`) on session completion.
- **Result consolidation**: Captures logs, status, and summaries into task cards.
- **Report generation**: Markdown summaries of team missions and outcomes in `analysis/reports/`.

### Operator Interfaces
- **TUI**: Responsive dashboard with background Git ops, panel filtering (`/`), and live log tailing.
- **CLI**: full-lifecycle management via headless dispatcher.
- **MCP Server**: Exposes orchestration tools and resources to LLMs.
- **Daemon (`awod`)**: JSON-RPC 2.0 over UDS for headless brokerage.

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

The most important remaining work falls into six buckets.

### 1. Broker Maturity
- production-grade daemon lifecycle and degraded-state handling
- broker-mode concurrency validation
- push/subscription-style event delivery for live clients

### 2. Platform Maturity
- Windows ConPTY completion and workflow validation
- Named Pipe transport for Windows daemon.

### 3. Immutable Task Recovery
- `task cancel`
- `task supersede`
- TUI/CLI flows that preserve history without task edit/delete

### 4. Reliability And Test Depth
- `team_ops`
- handlers and direct-vs-daemon parity
- fingerprinting and readiness decisions
- reconciliation flows
- broker/event concurrency

### 5. Middleware Enrichment
- automated context-pack generation
- shared RPC type cleanup
- stronger local MCP subscription semantics

### 6. Orchestration Intelligence
- deeper lead/worker handoff flows
- richer synthesis and reporting
- later WASI sandboxing exploration

## Recommended Next Milestones

### Milestone 1: Broker Hardening
Goal: make the daemon feel like a dependable local broker.
- harden lifecycle/status/cleanup behavior
- validate broker-mode concurrency
- upgrade event delivery for live clients

### Milestone 2: Reliability And Test Closure
Goal: close the remaining confidence gaps in orchestration-critical paths.
- deepen `team_ops`, handler, fingerprint, reconciliation, and event tests
- make manual validation a confirmation step rather than a discovery step

### Milestone 3: Immutable Task Recovery
Goal: make immutable tasks practical.
- add cancel/supersede flows
- expose them in CLI and TUI without introducing edit/delete

### Milestone 4: Windows Completion
Goal: achieve honest local parity on Windows.
- finish ConPTY workflow parity
- implement Named Pipes for the daemon

### Milestone 5+: Local-First Enrichment
Goal: deepen local orchestration before any remote expansion.
- middleware subscriptions and context-pack generation
- lead/worker handoff depth
- richer synthesis and reporting

## Recommended Work Order

1. Lock the local-first product contract in durable docs.
2. Finalize daemon lifecycle, degraded-state handling, and broker validation.
3. Close the high-value orchestration test gaps.
4. Implement immutable task recovery.
5. Complete Windows parity.
6. Enrich local middleware and orchestration intelligence.

## Summary

Awo Console has moved from concept to a strong local orchestration substrate. The focus now shifts from "make the features exist" to "make the local product stable, observable, recoverable, and trustworthy enough to finish."
