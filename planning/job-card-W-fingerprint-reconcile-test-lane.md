# Job Card W — Fingerprint And Reconciliation Test Lane

## Objective

Run an **independent external-agent lane** that deepens automated confidence in orchestration paths that do not need to wait on broker-lifecycle implementation.

This lane is intentionally shaped to proceed in parallel with broker hardening.

## Why This Slice

The project already has a large test suite, but some of the remaining risk sits in readiness and reconciliation behavior. Those paths can be strengthened now without colliding with the broker-focused implementation lane.

## Ownership

This lane owns test-depth work for stable orchestration paths and should avoid broker-lifecycle implementation files.

### Primary Write Scope
- `crates/awo-core/src/fingerprint.rs`
- `crates/awo-core/src/team/reconcile.rs`
- `crates/awo-core/tests/`
- `crates/awo-app/tests/`
- `MANUAL_TEST_SCENARIOS.md` only if a gap is discovered during verification and it is directly relevant to the added coverage

### Avoid Touching
- `crates/awo-core/src/daemon.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-app/src/handlers.rs`
- broker lifecycle docs owned by the primary implementation lane

## Suggested Branch And Worktree

- Branch: `codex/fingerprint-reconcile-tests`
- Worktree: `../chaban-worktrees/fingerprint-reconcile-tests`

## Scope

### In Scope
- add missing tests for fingerprint readiness decisions
- add missing tests for reconciliation and task/session state transitions
- add workflow tests for current `team_ops` behavior that do not depend on new broker semantics
- add regression tests for real gaps discovered while auditing those areas

### Out Of Scope
- daemon lifecycle implementation
- event delivery redesign
- immutable task recovery semantics that do not exist yet
- remote execution

## Deliverables

### 1. Fingerprint Coverage
- ready vs stale vs invalid decisions
- missing marker or lockfile cases
- refresh-trigger decision boundaries
- repo/profile edge cases that are currently thin

### 2. Reconciliation Coverage
- released slots clearing task bindings correctly
- failed sessions keeping tasks out of `Review`
- successful verification allowing progress
- partial/mixed team outcomes remaining intelligible

### 3. `team_ops` Workflow Coverage
- start/delegate/report flows that already exist today
- negative-path assertions where orchestration state should block unsafe transitions

### 4. Gap Report
- leave a short note in the final handoff listing:
  - what was covered
  - what remains thin
  - what is blocked on later milestones such as cancel/supersede or broker event changes

## Coordination Notes

- You are **not alone in the codebase**. Another lane is modifying daemon/broker files in parallel.
- Do not revert unrelated changes and do not "clean up" files outside this lane's scope.
- If a helper or fixture can live in a new test file instead of modifying production code, prefer the new test file.

## Recommended Execution Order

1. Audit existing fingerprint and reconciliation tests.
2. Add the highest-value missing failure-path coverage first.
3. Add workflow tests for the current `team_ops` behavior that is already implemented.
4. Run the relevant targeted suites and record any remaining known gaps.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test fingerprint
cargo test reconcile
cargo test team_ops
cargo test
```

## Definition Of Done

- fingerprint and reconciliation behavior have materially better automated coverage
- current `team_ops` behavior is better protected by workflow tests
- the lane stays independent from broker-lifecycle implementation work
- any remaining important gaps are named explicitly for the next milestone
