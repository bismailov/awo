# Tokio Decision

## Status

Deferred as of 2026-03-21.

`awo` does not need a full Tokio migration yet.

## Why We Re-Evaluated It

The product now has:
- slot/session orchestration across multiple runtimes
- tmux-backed PTY supervision
- oneshot crash recovery via PID and exit-code sidecars
- cross-process team-manifest locking
- read-time reconciliation of sessions, slots, and team manifests

That is enough complexity that an async runtime is worth revisiting deliberately rather than assuming either "obviously yes" or "obviously no".

## Current Evidence

The most important blocking/operator issues so far were solved without Tokio:
- long lock windows during `team task start`
  - fixed by splitting reservation, slot binding, and finalization phases
- stale `running` oneshot sessions after interrupted launchers
  - fixed with PID and exit-code sidecars plus sync logic
- invisible oneshot sessions while work was in flight
  - fixed by persisting the session record before completion
- reconciliation drift between team/task state and runtime state
  - fixed by read-time runtime sync plus manifest reconciliation

Those were lifecycle and state-model problems, not "the runtime is synchronous" problems.

## Current Decision

Keep the core synchronous for now.

Reasons:
- the main operator paths are already fast enough at the current product stage
- detached PTY work is already delegated to `tmux`
- one-shot launches are intentionally short-lived and persist enough metadata for later sync
- the supervisor seam now exists, so a future async transition has a cleaner place to land

## When Tokio Becomes Worth It

Introduce Tokio when one of these becomes a priority:
- a long-lived middleware daemon or broker process
- Windows ConPTY-backed interactive supervision
- many concurrent live sessions that need active background polling in one process
- remote-machine orchestration with multiplexed subprocess/session IO
- richer in-app live terminal/session streaming

## Practical Next Steps Before Tokio

1. Keep shrinking `anyhow` usage below the `AppCore` boundary.
2. Strengthen Linux/macOS/Windows CI and smoke coverage.
3. Keep the supervisor abstraction backend-driven.
4. Measure actual latency or responsiveness pain before changing runtimes.

## Trigger To Reopen The Decision

Reopen this decision when we start one of:
- middleware daemon mode
- Windows PTY backend implementation
- multi-session live streaming inside one persistent process
