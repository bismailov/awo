# Job Card R — Windows Parity Completion

## Objective

Finish the Windows local-platform story so Awo is genuinely usable and supportable there.

## Why This Matters

A local orchestration product is not finished if one of its claimed platforms is still partial or caveated in core workflows.

## Scope

### In Scope
- session supervision parity
- daemon transport parity
- workflow parity across CLI/TUI/broker paths

### Out Of Scope
- remote Windows workers
- platform-specific feature divergence unless clearly necessary

## Deliverables

### 1. Session Supervision Completion
- audit current ConPTY behavior against Unix behavior
- finish missing lifecycle pieces
- validate cancellation, completion, logs, timeout handling

### 2. Daemon Transport Completion
- implement Windows-native daemon transport
- ensure daemon status/start/stop semantics remain coherent
- keep the broker contract aligned with Unix behavior

### 3. Workflow Validation
- validate repo/slot/session/team flows on Windows
- ensure CLI help/docs do not overpromise
- ensure TUI behavior is acceptable on Windows terminals

## Likely Files

- `crates/awo-core/src/runtime/supervisor/conpty.rs`
- `crates/awo-core/src/daemon.rs`
- `crates/awo-core/src/platform.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/awod.rs`
- CI configuration files if present
- platform docs

## Risks

- transport abstractions can drift between Unix and Windows
- terminal behavior differences can destabilize logs and TUI expectations

## Mitigations

- keep transport and lifecycle contracts explicit
- use platform-specific tests where behavior differs
- validate actual operator workflows, not just helper functions

## Verification

- Windows CI for broker, session, and orchestration paths
- targeted platform tests
- manual smoke matrix on a real Windows environment

## Definition Of Done

- Windows supports the same local mental model as Unix
- daemon mode works natively
- session supervision is trustworthy
- platform limitations, if any remain, are narrow and explicitly documented
