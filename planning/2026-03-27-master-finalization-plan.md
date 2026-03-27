# Master Finalization Plan (March 27, 2026)

## Purpose

This document is the concrete end-to-end execution plan to finish Awo Console as a **locally usable, stable, high-quality orchestration product**.

It supersedes ad hoc next-step notes by aligning:

- the current product vision
- the remaining technical gaps
- the intended local-first end state
- the execution order needed to reach release quality

Use this plan as the primary roadmap for the next implementation waves.

## Final Product Goal

Finalize Awo Console as a **local-first orchestration console and broker** that an operator can trust for daily use.

At the end of this plan, the product should be:

- daemon-backed by default on supported platforms
- safe and transparent about workspace reuse
- strong at local team orchestration and delegation
- TUI-first for operators, CLI/MCP-first for automation
- explicit about recovery and task history
- well-tested at the orchestration-path level
- honest and functional on both Unix and Windows

This plan does **not** target remote/distributed execution. Remote remains deferred until the local model is fully proven.

## Product Decisions Locked For This Plan

These decisions are now treated as settled unless a later roadmap revision explicitly changes them.

### 1. Slot Pooling
- Slot pooling should be **mostly automatic, but transparent**.
- Operators should not have to manually manage a "pool" as a first-class object for common use.
- The product should clearly expose when a slot was reused, why it was reused, and why reuse was blocked.

### 2. Task Brief Shape
- Task briefs should use a **hybrid model**:
  - structured fields for routing, verification, scope, ownership, and lifecycle
  - freeform notes for nuance and operator intent
- Avoid a pure markdown-only model and avoid a rigid schema that removes operator flexibility.

### 3. History Ownership
- V1/V1.5 should own **enough history for audit, debugging, and reports**:
  - session metadata
  - log locations
  - task result summaries
  - recent/fetchable logs
- V1/V1.5 should **not** try to become a full transcript archive product.

### 4. Remote Execution
- Remote execution should wait until the **local-slot model is clearly stable and proven**.
- Do not expand into distributed orchestration while the broker, local recovery, and platform parity work are still unfinished.

## Scope Boundaries

### In Scope
- daemon/broker hardening
- orchestration-path test depth
- immutable-task recovery via cancel/supersede
- TUI continuation as the primary operator surface
- Windows local parity
- middleware improvements for local automation and integration
- delegation depth and local orchestration intelligence
- release-quality docs, verification, and packaging polish

### Out Of Scope
- remote worker execution
- remote repo/slot orchestration
- full transcript-product ownership
- task edit/delete
- TUI-only mutations that bypass the orchestration core

## Reality Check: Current State vs Remaining Gaps

Some roadmap documents in the repo have drifted slightly. This plan resolves that drift by distinguishing:

### Already Present But Not Fully Finalized
- daemon exists
- CLI auto-start exists
- event bus exists
- team delegation exists
- TUI setup parity exists
- connection pooling appears partially present in the store layer
- some Windows support exists

### Still Needing Product-Grade Completion
- production-grade daemon lifecycle and degraded-state handling
- push/subscription-style event delivery for live interfaces
- deeper orchestration-path testing
- immutable task recovery model
- Windows daemon transport parity
- richer local orchestration and handoff depth
- release-grade documentation and workflow confidence

## Master Milestones

### Milestone 0: Product Contract Lock

**Goal:** remove ambiguity before more implementation spreads assumptions.

Deliverables:
- update durable docs to encode the four product decisions above
- define the "finished local product" explicitly
- document slot-pooling transparency rules
- document hybrid task-brief model
- document bounded history ownership
- explicitly defer remote execution

Exit criteria:
- major roadmap ambiguity is removed
- future job cards can rely on one consistent local-first contract

### Milestone 1: Broker Hardening

**Goal:** make Awo feel like a real local broker, not a command wrapper with a daemon on the side.

Focus:
- daemon lifecycle reliability
- daemon health/degraded-state handling
- broker concurrency validation
- event delivery for live clients

Primary job card:
- [job-card-O-broker-hardening-and-daemon-ux.md](/Users/bismailov/Documents/chaban/planning/job-card-O-broker-hardening-and-daemon-ux.md)

Exit criteria:
- broker mode is the default mental model for supported platforms
- repeated CLI/TUI/MCP use is stable under concurrent access

### Milestone 2: Reliability And Test Closure

**Goal:** close the remaining confidence gaps around orchestration-critical code paths.

Focus:
- `team_ops`
- handlers / direct-vs-daemon behavior
- fingerprinting
- reconciliation
- daemon/event concurrency

Primary job card:
- [job-card-P-orchestration-test-depth.md](/Users/bismailov/Documents/chaban/planning/job-card-P-orchestration-test-depth.md)

Exit criteria:
- critical lifecycle paths are deeply covered
- manual validation becomes confirmation rather than bug discovery

### Milestone 3: Immutable Task Recovery

**Goal:** make the immutable task model practical, not merely principled.

Focus:
- `task cancel`
- `task supersede`
- TUI/CLI support for history-preserving correction

Primary job card:
- [job-card-Q-immutable-task-recovery.md](/Users/bismailov/Documents/chaban/planning/job-card-Q-immutable-task-recovery.md)

Exit criteria:
- operators can correct plans without editing/deleting tasks
- task history stays intelligible in reports and TUI

### Milestone 4: Windows Parity Completion

**Goal:** finish the local product on Windows with the same trust story as Unix.

Focus:
- session supervision parity
- daemon transport parity
- workflow parity

Primary job card:
- [job-card-R-windows-parity-completion.md](/Users/bismailov/Documents/chaban/planning/job-card-R-windows-parity-completion.md)

Exit criteria:
- core local workflows work on Windows and Unix with honest, documented behavior

### Milestone 5: Middleware Enrichment (Local-First)

**Goal:** make CLI/MCP integrations first-class without jumping to remote execution.

Focus:
- subscriptions / live updates
- context-pack auto-generation
- shared RPC cleanup
- control-surface consistency

Primary job card:
- [job-card-S-middleware-local-enrichment.md](/Users/bismailov/Documents/chaban/planning/job-card-S-middleware-local-enrichment.md)

Exit criteria:
- local automation and integrations feel stable and coherent

### Milestone 6: Orchestration Intelligence

**Goal:** deepen the lead/worker orchestration wedge.

Focus:
- stronger delegation/handoff
- automatic context passing
- improved result synthesis and reporting
- first-class lead-session orchestration
- planning-to-task-card workflow
- review/consolidation cockpit in the TUI
- capacity-aware recovery for lead/worker sessions

Primary job card:
- [job-card-T-orchestration-intelligence.md](/Users/bismailov/Documents/chaban/planning/job-card-T-orchestration-intelligence.md)

Supporting orchestration package:
- [2026-03-27-lead-agent-task-card-orchestration-plan.md](/Users/bismailov/Documents/chaban/planning/2026-03-27-lead-agent-task-card-orchestration-plan.md)
- [job-card-X-lead-session-and-task-card-model.md](/Users/bismailov/Documents/chaban/planning/job-card-X-lead-session-and-task-card-model.md)
- [job-card-Y-output-ingestion-and-capacity-state.md](/Users/bismailov/Documents/chaban/planning/job-card-Y-output-ingestion-and-capacity-state.md)
- [job-card-Z-consolidation-cockpit-and-retention-controls.md](/Users/bismailov/Documents/chaban/planning/job-card-Z-consolidation-cockpit-and-retention-controls.md)
- [job-card-AA-configurable-storage-roots.md](/Users/bismailov/Documents/chaban/planning/job-card-AA-configurable-storage-roots.md)

Exit criteria:
- local multi-agent coordination is a meaningful product advantage

### Milestone 7: Release Finalization

**Goal:** turn a strong engineering substrate into a finishable local product.

Focus:
- docs
- manual scenarios
- help text
- packaging/release confidence
- stable known-limitations documentation

Primary job card:
- [job-card-U-release-finalization.md](/Users/bismailov/Documents/chaban/planning/job-card-U-release-finalization.md)

Exit criteria:
- the product is locally release-ready

## Recommended Work Order

This is the sequencing that best matches the current product shape and risk profile.

1. Milestone 0: Product Contract Lock
2. Milestone 1: Broker Hardening
3. Milestone 2: Reliability And Test Closure
4. Milestone 3: Immutable Task Recovery
5. Milestone 4: Windows Parity Completion
6. Milestone 5: Middleware Enrichment
7. Milestone 6: Orchestration Intelligence
8. Milestone 7: Release Finalization

## Cross-Cutting Rules

### TUI Rule
- The TUI remains command-backed.
- No TUI-only business logic for core mutations.
- New TUI flows must route through existing commands or newly added `awo-core` orchestration APIs.

### Safety Rule
- Safety beats convenience.
- If a workflow has destructive implications, require explicit operator intent and visible state.

### Local-First Rule
- If a feature can be solved by making local orchestration stronger, prefer that over introducing remote complexity.

### Testing Rule
- Every orchestration feature needs:
  - automated coverage for core lifecycle behavior
  - at least one realistic manual scenario

## Definition Of Finished Local Product

The project is finished for the intended local scope when an operator can reliably:

1. register and inspect repositories
2. acquire fresh or warm slots safely
3. reuse slots automatically with visible reasoning
4. launch supported runtimes in those slots
5. use the daemon transparently and safely
6. operate primarily through the TUI
7. orchestrate teams and delegated work locally
8. recover from planning mistakes through cancel/supersede flows
9. inspect enough history for audit/debug/reporting
10. trust the same core workflows on Unix and Windows
11. automate the same workflows through CLI/MCP without control-surface divergence

## Execution Notes

- Existing job cards `K`, `L`, and `M` should be treated as historical context, not the final roadmap on their own.
- This plan should be the base document for any new implementation wave.
- If the local-first scope changes, update this file first before creating new execution cards.
