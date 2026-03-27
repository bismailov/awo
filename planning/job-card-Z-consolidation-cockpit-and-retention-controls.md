# Job Card Z — Consolidation Cockpit And Retention Controls

## Objective

Turn completed task-card execution into a first-class review and integration workflow in the TUI, including explicit worktree retention and deletion controls.

## Why This Slice

Today Awo is good at launching and tracking work. The product still needs a strong close-the-loop workflow where the lead reviews outputs, accepts or reworks them, and decides what happens to the underlying worktree.

## Primary Write Scope
- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-core/src/commands/slot.rs`
- `crates/awo-core/src/git.rs`
- `crates/awo-app/src/tui/*`
- `crates/awo-app/src/output.rs`
- `MANUAL_TEST_SCENARIOS.md`
- `docs/v1-control-surface.md`

## Scope

### In Scope
- review queue and consolidation queue concepts
- TUI actions for accept/rework/supersede handoff points
- inspect diff/log/slot from a task card
- explicit delete/prune worktree actions
- explicit retain-for-reuse vs delete-after-consolidation decisions

### Out Of Scope
- full automatic git merge orchestration for every project shape
- remote/distributed consolidation

## Deliverables
- TUI review and consolidation flows
- safe worktree deletion/pruning controls
- tests/manual scenarios for retention vs deletion choices
- operator-facing docs for cleanup semantics

## Definition Of Done
- operators can review and close out completed task cards from the TUI
- worktree retention and deletion are explicit choices
- the product no longer leaves consolidation as an entirely manual side process
