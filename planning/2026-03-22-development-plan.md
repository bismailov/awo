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
- checkpoint: post-Windows validation checkpoint on March 31, 2026

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
- Background TUI snapshot refresh and Team Dashboard selection preservation.
- Bounded TUI router decomposition for dialog/form workflow handling.
- Event-bus poison recovery in production synchronization paths.
- RPC-level daemon health probing with degraded detection for unresponsive brokers.

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

### 1. TUI Responsiveness And Structure
- the TUI now refreshes later snapshots from a background worker instead of doing periodic full refreshes on the render loop
- `snapshot()` still performs runtime sync and manifest reconciliation, so further snapshot-cost reduction remains a polish/performance opportunity rather than a blocker
- `crates/awo-app/src/tui/action_router.rs` has been reduced by extracting dialog handling, but more decomposition is still desirable before another major cockpit expansion

### 2. Broker Maturity
- production-grade daemon lifecycle and degraded-state handling still need more real-world hardening
- daemon health now requires a bounded RPC roundtrip instead of a bare socket connect
- daemon clients now apply bounded stream I/O timeouts so unresponsive brokers fail more predictably
- the TUI now reacts to event-bus wakeups for command-driven changes instead of relying primarily on periodic blind refreshes
- the MCP facade now supports resource subscriptions and emits bounded update notifications for subscribed broker resources
- broker-mode concurrency validation should deepen further
- push/subscription-style event delivery should replace more polling behavior over time

### 3. Platform Maturity
- Windows ConPTY and Named Pipe implementations now exist in the codebase
- a native Windows 10 checklist run now passes repo, slot, session, daemon, team, and TUI smoke flows
- the current macOS environment still cannot finish Windows-target verification because bundled `libsqlite3-sys` cross-compilation fails before the full Rust workspace can be validated
- cross-platform smoke coverage is now scripted and wired into CI/release workflows; the remaining platform work is deeper polish rather than first-line parity validation

### 4. Runtime Usage Truth
- provider-specific usage/capacity telemetry is still mostly advisory rather than structured
- lead/worker recovery guidance is honest, but richer adapter-fed budget and lifetime data still needs work
- runtime capability output now distinguishes `planned` adapter work from permanently `unknown` support for provider-backed telemetry
- provider-specific usage notes now point operators at the best current truth source when the CLI adapter cannot surface spend directly
- provider quota/rate-limit failures are now distinguished from true token/context exhaustion
- capability descriptors now reflect real local CLI surfaces for:
  - Claude budget guardrails and structured output
  - Codex structured output
  - Gemini structured output

### 5. Hardening And CI Maturity
- `EventBus` poison handling now recovers and warns instead of panicking immediately
- output serialization no longer panics on unexpected JSON serialization failures
- CI is now wired to run `cargo audit` and `cargo deny`, and both have now been validated locally
- `cargo audit` currently reports one known warning: `RUSTSEC-2017-0008` (`serial` via `portable-pty`), and `deny.toml` now ignores that advisory explicitly pending an upstream/runtime change
- `deny.toml` now also reflects the real Windows-transport dependency graph, including explicit `0BSD` allowance

### 6. Middleware Enrichment
- automated context-pack generation
- shared RPC type cleanup
- stronger local MCP subscription semantics

### 7. Release Finalization
- help text and contributor docs still need a polish pass
- manual scenario coverage needs a fresh full-product release sweep
- known limitations should be tightened into a cleaner public release story

## Current Objectives

1. Make the TUI feel responsive and maintainable under larger local workloads.
2. Make the daemon truly feel like the default local broker.
3. Keep the automated cross-platform release path healthy and observable.
4. Deepen structured runtime usage/capacity truth without inventing fake telemetry.
5. Finish validating and operationalizing the new CI/security checks.
6. Turn the current strong engineering substrate into a public release-quality local product.

## Recommended Next Milestones

### Milestone 1: TUI Responsiveness And Decomposition
Goal: keep the operator surface fast and maintainable as orchestration state grows.
- completed: move periodic snapshot refresh off the render loop
- completed: extract dialog/form-confirm workflow handling from `action_router.rs`
- next: continue bounded decomposition only where new cockpit work would otherwise regrow the router

### Milestone 2: Broker Hardening
Goal: make the daemon feel like a dependable local broker.
- completed so far: harden lifecycle/status/cleanup behavior around stale artifacts, degraded states, RPC health checks, client I/O timeouts, event-driven TUI refresh triggers, and MCP resource-subscription notifications
- validate broker-mode concurrency
- upgrade event delivery for live clients

### Milestone 3: Windows Completion And Regression Protection
Goal: keep honest local parity on Windows proven and repeatable.
- completed so far: implement Named Pipes for the daemon
- completed so far: fix the concrete ConPTY exit-code and process-tree cancellation issues found during audit
- completed so far: validate the main operator workflows on a real Windows 10 environment and record the checklist in `windows_checklist_report.md`
- completed so far: replace the stale ad hoc Windows smoke harness with the maintained `scripts/awo_smoke.py` workflow plus refreshed Windows wrapper
- completed so far: wire the smoke workflow into CI and release packaging
- next: decide how much off-host Windows-target compilation still matters once native validation and automated smoke coverage are the primary truth sources

### Milestone 4: Runtime Usage Truth
Goal: turn advisory recovery messaging into stronger runtime-backed operator signals where possible.
- completed so far: mark provider-backed telemetry for Claude/Codex/Gemini as `planned` rather than permanently `unknown`
- completed so far: point usage notes at provider-specific truth sources instead of generic “check dashboards” guidance
- completed so far: distinguish `provider_limited` failures from `exhausted` failures in runtime/session truth
- completed so far: reflect real local CLI capability surfaces for Claude/Codex/Gemini structured output, and Claude budget guardrails
- add adapter-fed usage/capacity support where runtimes expose it
- keep `unknown`/`unsupported` explicit where they do not
- improve lead/worker handoff guidance around timeouts, quota pressure, and token exhaustion

### Milestone 5: Hardening And CI Maturity
Goal: reduce avoidable crash/supply-chain risk before final release work.
- completed so far: remove the remaining high-risk production panic paths in `EventBus` and JSON output handling
- completed so far: add `cargo audit` and `cargo deny` steps plus `deny.toml`
- completed so far: record an explicit temporary ignore for `RUSTSEC-2017-0008` (`portable-pty -> serial`) in `deny.toml`
- completed so far: validate `cargo deny` locally and fix `deny.toml` drift

### Milestone 6: Local-First Enrichment And Release Finalization
Goal: deepen local orchestration and finish the release story.
- completed so far: bounded MCP subscriptions and broker-backed update notifications
- completed so far: help text, manual scenarios, platform docs, and release-audit refresh
- completed so far: clarify the release/deployment path with `docs/release-process.md`, automated packaging, and GitHub release workflow wiring
- next: cut and observe the first tagged release candidate, then decide whether any richer adapter-fed telemetry work should still land pre-release

## Recommended Work Order

1. Finish broker live-event delivery for daemon/MCP clients and any remaining degraded-state operator visibility.
2. Cut and observe the first tagged release candidate through the new release workflow.
3. Strengthen runtime usage/capacity truth where adapters support it, if that is still worth doing pre-release.
4. Finish validating CI/security checks and set advisory policy.
5. Enrich local middleware and orchestration intelligence only if it materially improves the release candidate.
6. Keep the release docs and smoke expectations aligned with the shipped behavior.

## Summary

Awo Console has moved from concept to a strong local orchestration product with real planning, review, cleanup, and recovery flows. The focus now shifts from "make the features exist" to "preserve the new cross-platform confidence, lock down the release path, and ship a local product that feels coherent and trustworthy end to end."
