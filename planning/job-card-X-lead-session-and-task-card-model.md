# Job Card X — Lead Session And Task-Card Model

## Objective

Make the current lead session a first-class operational concept and standardize the product language around `task cards`.

## Why This Slice

The current team/task model is close to the desired workflow, but it still treats the lead mostly as a manifest role instead of a live orchestrator session that can be replaced, promoted, or also execute work.

## Primary Write Scope
- `crates/awo-core/src/team.rs`
- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-core/src/commands/team.rs`
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/*`
- `docs/product-spec.md`
- `docs/team-manifest-spec.md`

## Scope

### In Scope
- introduce first-class lead-session state
- support lead replacement and promotion
- allow lead-as-worker behavior
- standardize `task card` terminology in operator-facing surfaces
- expose lead identity/state in the TUI

### Out Of Scope
- full output ingestion
- consolidation/merge flows
- runtime capacity telemetry

## Deliverables
- team-level current lead session metadata
- TUI affordance to show and replace the lead
- task-card terminology cleanup in docs/help/TUI
- tests for lead replacement and lead-as-worker behavior

## Definition Of Done
- a team can survive lead-session replacement
- the lead can own executable task cards
- user-facing language consistently prefers `task card`
