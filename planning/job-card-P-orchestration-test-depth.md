# Job Card P — Orchestration Test Depth Closure

## Objective

Close the highest-value test gaps in orchestration-critical code so the product can be finalized with confidence.

## Why This Matters

The suite is already large, but the remaining weak spots sit in important workflow code rather than leaf utilities.

## Priority Targets

1. `team_ops`
2. app handlers and daemon/direct-mode parity
3. fingerprinting and readiness decisions
4. team reconciliation flows
5. event-bus concurrency and broker interaction

## Scope

### In Scope
- missing workflow tests
- failure-path tests
- broker/direct parity tests
- regression tests for every real operator bug found

### Out Of Scope
- test count inflation without risk reduction
- cosmetic snapshot tests with low behavioral value

## Deliverables

### 1. `team_ops` Coverage
- task start/delegate/retry/recovery workflows
- task verification and reconciliation transitions
- cancel/supersede support after it lands
- report-generation correctness on mixed outcomes

### 2. Handler Coverage
- same command through direct mode and daemon mode
- JSON/text output parity where applicable
- fallback behavior when broker is unavailable

### 3. Fingerprint Coverage
- ready/stale/invalid states
- missing dependency markers
- refresh behavior
- cross-repo or profile-specific edge cases

### 4. Reconciliation Coverage
- released slots clearing bindings
- failed sessions blocking tasks
- completed sessions moving tasks to review only when verification passes
- cancelled/superseded semantics after immutable recovery lands

### 5. Concurrency And Event Coverage
- event bus under concurrent publishers/readers
- broker event delivery under load
- no silent poisoning or ordering regressions

## Likely Files

- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-core/src/app/tests.rs`
- `crates/awo-core/src/fingerprint.rs`
- `crates/awo-core/src/team/reconcile.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/tests/operator_flows.rs`
- `crates/awo-app/tests/json_cli.rs`

## Test Strategy

- prefer full workflow assertions over micro-tests
- exercise actual persisted state whenever practical
- keep negative-path testing strong
- when a manual bug is found, add a regression before closing it

## Verification

- targeted module tests
- full workspace `cargo test`
- manual scenario spot checks only for flows that are difficult to encode automatically

## Definition Of Done

- the previously thin orchestration paths have meaningful automated coverage
- failure modes are tested, not just happy paths
- real manual defects are captured as regressions
- remaining untested areas are low-risk and explicitly known
