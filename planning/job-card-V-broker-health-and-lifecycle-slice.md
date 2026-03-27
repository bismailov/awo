# Job Card V — Broker Health And Lifecycle Slice

## Objective

Implement the first concrete Milestone 1 slice: make broker state, daemon lifecycle, and operator-visible fallback behavior more explicit and trustworthy.

This card is intended for the **primary implementation lane**.

## Why This Slice

The broker already exists, but it still needs to feel like the real control plane instead of an optional transport. The fastest way to improve that trust is to harden the daemon state contract and lifecycle behavior first.

## Ownership

This lane owns the broker and daemon implementation surfaces.

### Primary Write Scope
- `crates/awo-core/src/daemon.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-core/src/app.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-app/src/handlers.rs`
- `docs/interface-strategy.md`
- `docs/v1-control-surface.md`

### Avoid Touching
- `crates/awo-core/tests/*` files owned by the external test lane
- reconciliation/fingerprint-focused test additions assigned to the external lane

## Suggested Branch And Worktree

- Branch: `codex/broker-health-lifecycle`
- Worktree: `../chaban-worktrees/broker-health-lifecycle`

## Scope

### In Scope
- define explicit daemon health/degraded states
- harden repeated start/connect/stop flows
- make stale runtime artifacts easier to recover from
- surface broker fallback behavior more clearly in CLI/status output
- add targeted tests for the new lifecycle contract
- update operator docs for broker expectations

### Out Of Scope
- Windows Named Pipes
- remote execution
- broad handler parity coverage outside the touched lifecycle paths
- immutable task recovery
- TUI event-consumer redesign

## Deliverables

### 1. Daemon Health Contract
- define a concrete daemon health model such as:
  - `NotRunning`
  - `Starting`
  - `Healthy`
  - `Degraded`
- make the status surface expose the difference between healthy and degraded instead of collapsing everything into "running"

### 2. Lifecycle Hardening
- harden repeated daemon start when stale PID/socket/runtime artifacts exist
- make stop behavior idempotent and operator-friendly
- tighten connect-after-start timing and retries so auto-start feels deterministic

### 3. Operator Transparency
- make fallback-to-direct behavior intentional and visible
- improve help/status text so operators can tell which mode they are in and why

### 4. Verification
- add focused tests for:
  - cold start
  - repeated start
  - stop when not running
  - stale artifact cleanup
  - degraded-state reporting

## Coordination Notes

- You are **not alone in the codebase**. Another lane may be adding tests in parallel.
- Do not revert or reformat unrelated files owned by the external lane.
- If a new test helper is needed, prefer adding one that the external lane can also reuse rather than editing their files directly.

## Recommended Execution Order

1. Inspect the current daemon lifecycle/status/fallback paths.
2. Define the health-state contract in code and docs.
3. Harden lifecycle cleanup and repeated-start behavior.
4. Add focused tests around the touched broker paths.
5. Update operator-facing docs/help text.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test daemon
cargo test broker
cargo test
```

## Definition Of Done

- daemon state is more explicit than a simple running/not-running check
- repeated lifecycle operations are predictable and safer
- fallback behavior is visible rather than mysterious
- touched broker paths have targeted automated coverage
- docs/help text match the new lifecycle contract
