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

## What Is Still Missing

The most important remaining work falls into four buckets.

### 1. Middleware Maturity
- Persistence scaling (connection pooling).
- Daemon lifecycle management (`awod status`, `awod stop`, CLI auto-start).
- JSON-RPC event bus (push notifications).

### 2. Platform Maturity
- Windows ConPTY master/slave logic (currently a stub).
- Named Pipe transport for Windows daemon.

### 3. Orchestration Intelligence
- Lead/Worker handoff flows (delegation).
- Automated context pack generation.
- WASI sandboxing for adapters.

### 4. UX & Polish
- Embedded terminal rendering in TUI.
- Richer result synthesis (LLM-assisted).

## Recommended Next Milestones

### Milestone A: Daemon Lifecycle & Stability
Goal: make the daemon a production-grade background service.
- Implement `awod` start/stop/status.
- Add CLI auto-startup for the daemon.
- Transition to a database connection pool.

### Milestone B: Task Handoff & Coordination
Goal: Move from task tracking to active delegation.
- Implement `Command::TeamTaskDelegate` to hand off sub-tasks from a Lead slot to a Worker slot.
- Automate context sharing between delegated slots.

### Milestone C: Windows Completion
Goal: achieve full platform parity.
- Finalize ConPTY supervision.
- Implement Named Pipes for the Windows daemon.

## Recommended Work Order

1. Finalize daemon lifecycle and auto-start.
2. Scale the database layer for high concurrency.
3. Complete the Windows ConPTY implementation.
4. Implement Lead/Worker handoff logic.

## Summary

Awo Console has moved from concept to a hardened orchestration substrate. The focus now shifts from "making it work" to "making it scale" as a headless broker for AI agents.
