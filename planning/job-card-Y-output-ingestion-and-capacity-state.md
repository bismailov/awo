# Job Card Y — Output Ingestion And Capacity State

## Objective

Add the missing middle layer between worker execution and lead review: structured output ingestion, review-ready state, and honest capacity/usage signaling.

## Why This Slice

The orchestration workflow breaks down if worker outputs only live in raw logs and if token/time-limit failures are indistinguishable from generic failures.

## Primary Write Scope
- `crates/awo-core/src/runtime/*`
- `crates/awo-core/src/session.rs`
- `crates/awo-core/src/team.rs`
- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-app/src/output.rs`
- `crates/awo-app/src/tui/*`
- `docs/product-spec.md`

## Scope

### In Scope
- task-card result summary / handoff-note ingestion
- review-ready task-card state
- session end-reason model for timeout, token exhaustion, operator cancel, and generic failure
- runtime capability matrix for usage/capacity stats
- TUI visibility for capacity state where supported

### Out Of Scope
- automatic merge/consolidation
- remote runtime telemetry
- perfect token accounting for runtimes that do not expose it

## Deliverables
- structured task-card result fields
- review queue input data
- capacity-status model with honest `unknown`/`unsupported` cases
- tests for timeout/exhaustion handling and result ingestion

## Definition Of Done
- completed worker runs can be reviewed without digging through raw logs alone
- exhaustion/timeout states are operationally visible and recoverable
- unsupported runtimes report uncertainty honestly
