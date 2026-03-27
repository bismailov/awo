# Job Card Q — Immutable Task Recovery

## Objective

Complete the immutable-task model by adding explicit, history-preserving recovery actions instead of edit/delete.

## Product Rule

Tasks remain immutable planning records:

- no edit-in-place
- no hard delete
- recovery happens through explicit lifecycle actions

## Recovery Model

Support two core correction flows:

1. `task cancel`
2. `task supersede`

Supersede should make it possible to preserve the old task while clearly linking it to a replacement task.

## Why This Matters

Without recovery actions, immutable tasks feel principled but inconvenient. This work turns the model into a practical operator workflow.

## Scope

### In Scope
- core state-model additions
- CLI commands for cancel/supersede
- TUI recovery flows
- report and status rendering updates

### Out Of Scope
- task edit
- task delete
- retroactive mutation of historical task content

## Deliverables

### 1. Core Model
- add explicit task terminal/recovery states as needed
- add replacement linkage for superseded tasks
- support recovery notes/reasons

### 2. Command Surface
- `team task cancel`
- `team task supersede`
- validation around which states can be cancelled/superseded

### 3. TUI Support
- expose cancel/supersede from the Team Dashboard
- make status visible and understandable
- support replacement-task creation workflow cleanly

### 4. Reporting
- reflect cancelled/superseded tasks clearly in:
  - team show
  - team report
  - TUI task list and progress

## Likely Files

- `crates/awo-core/src/team.rs`
- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-core/src/commands/team.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/action_router.rs`
- `crates/awo-app/src/tui/forms.rs`
- `docs/team-manifest-spec.md`

## Risks

- state-model changes can affect reports, progress, and reconciliation in subtle ways
- supersede semantics can become ambiguous if replacement linking is not explicit enough

## Mitigations

- keep the lifecycle explicit
- add clear validation rules
- write roundtrip and reconciliation tests before wiring the TUI

## Verification

Automated:
- manifest roundtrip tests
- command tests
- reconciliation tests
- TUI router/state tests

Manual:
- create incorrect task -> supersede it -> verify history and active plan clarity
- cancel obsolete task -> verify reports and TUI state

## Definition Of Done

- operators can correct plans without deleting or editing tasks
- history remains preserved and understandable
- CLI and TUI both support the recovery model
