# Job Card O — Broker Hardening And Daemon UX

## Objective

Finish the daemon/broker layer so Awo behaves like a dependable local background service rather than a CLI that sometimes talks to a daemon.

## Why This Matters

This is the highest-leverage remaining workstream for a local release:

- TUI, CLI, and MCP all become more reliable if the broker is reliable
- concurrency and state ownership become clearer
- operator trust improves when lifecycle behavior is predictable

## Scope

### In Scope
- harden daemon lifecycle (`status`, `stop`, auto-start, stale state cleanup)
- explicitly model healthy vs degraded daemon states
- validate connection-pool behavior under broker load
- add live event delivery / subscription path for local clients
- document broker behavior and fallback paths

### Out Of Scope
- remote execution
- distributed brokers
- full async rewrite

## Current Reality

Already present in some form:
- `awod`
- CLI auto-start
- event bus
- JSON-RPC transport
- pooled storage appears partially present

Still not finalized:
- degraded-state handling
- robust repeated-start / stale-socket behavior
- stronger broker-mode tests
- first-class push/event subscription semantics

## Deliverables

### 1. Daemon Lifecycle Hardening
- define daemon health states:
  - not running
  - starting
  - running but degraded
  - healthy
- make `daemon status` expose these states clearly
- harden stale PID/socket/lock cleanup
- harden repeated auto-start behavior and connect-after-start timing
- ensure fallback to direct mode is intentional and documented

### 2. Broker Concurrency Validation
- confirm store/pool behavior under:
  - repeated CLI dispatch
  - TUI snapshot + mutation overlap
  - MCP polling/subscription overlap
- eliminate hidden assumptions about single-writer timing

### 3. Event Delivery Upgrade
- design and implement local event subscription / long-poll / push semantics
- use the event bus as the canonical source for live updates
- wire TUI and MCP to consume live updates where appropriate

### 4. Operator Transparency
- make daemon state and fallback behavior visible enough for debugging
- improve help/docs around broker-mode expectations

## Likely Files

- `crates/awo-core/src/daemon.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-core/src/app.rs`
- `crates/awo-core/src/store.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-mcp/src/server.rs`
- `docs/interface-strategy.md`
- `docs/v1-control-surface.md`

## Risks

- lifecycle bugs can create confusing direct-mode vs daemon-mode divergence
- event streaming can introduce subtle concurrency regressions
- transport improvements can accidentally overcomplicate the synchronous core

## Mitigations

- keep health states explicit
- add workflow tests for daemon-backed command paths
- validate fallback behavior deliberately, not just implicitly
- prefer bounded subscription semantics over open-ended complexity

## Verification

Automated:
- daemon lifecycle tests
- repeated auto-start/connect tests
- stale socket/pid cleanup tests
- concurrent event polling/subscription tests
- direct-mode vs daemon-mode parity tests for representative commands

Manual:
- repeated CLI invocations from a cold state
- TUI + CLI overlap against the same app state
- MCP client plus CLI against the same broker state

## Definition Of Done

- daemon lifecycle is predictable and documented
- degraded states are surfaced clearly
- broker-mode concurrency is tested and trustworthy
- live local event delivery exists and is usable
- TUI/CLI/MCP all behave coherently in broker mode
