# Next Stages Execution Plan (March 28, 2026)

## Purpose

Translate the current roadmap into a practical execution sequence from the latest broker/MCP checkpoint.

This plan assumes the current local state is:

- task-card orchestration is present
- review/diff/cleanup flows are present
- immutable recovery is present
- TUI background refresh and event-triggered wakeups are present
- daemon health and degraded-state handling are materially improved
- MCP resource subscriptions now exist
- runtime usage truth is still only partially structured
- Windows parity and final release polish remain open

## Current Release Blockers

1. Real Windows workflow validation is still incomplete.
2. Adapter-fed runtime usage/capacity truth is still thin.
3. Broker live delivery is good enough for the local product, but still not the final long-term shape.

## Execution Order

### Session A: Broker/MCP Completion Pass

**Goal:** close the remaining bounded broker live-delivery questions without turning the architecture async-heavy.

Scope:
- audit the new MCP subscription path against the actual resource model
- confirm subscribed notifications cover:
  - repo mutations
  - slot lifecycle
  - session lifecycle
  - review state
  - team/task-card mutations
- decide whether any additional daemon-health state should be surfaced to CLI/TUI operators
- add any missing focused MCP or daemon regression tests

Deliverables:
- either no-op confirmation or one more bounded broker/MCP patch
- updated docs if the live-delivery contract changes
- explicit note in planning docs that Session A is closed

Definition of done:
- no obvious subscribed-resource blind spots remain
- broker status/operator visibility is “good enough” for the local product
- remaining broker work is clearly deferred, not fuzzy

Recommended branch/worktree:
- `codex/broker-mcp-completion`

### Session B: Windows Parity Completion

**Goal:** finish the largest remaining release blocker.

Scope:
- audit the current ConPTY implementation against Unix supervision semantics
- implement or finish Named Pipe transport for daemon mode
- validate Windows behavior for:
  - repo add/list
  - slot acquire/release/delete/prune
  - session start/cancel/log
  - team task start/delegate
  - basic TUI operation
- document any intentionally retained platform limitations

Deliverables:
- code fixes for ConPTY and/or Named Pipe transport
- Windows-focused regression coverage where feasible
- updated docs and known-limitations text

Definition of done:
- Windows is no longer “partial support” for the local product
- the main local operator flows are verified on Windows

Recommended branch/worktree:
- `codex/windows-parity-completion`

Delegation note:
- This is the best external-agent lane if a Windows-capable environment or separate machine is available.

### Session C: Runtime Usage Truth, Part 2

**Goal:** improve structured runtime usage/capacity truth only where the adapters can honestly provide it.

Scope:
- inspect each runtime adapter and supporting shell invocation path
- identify any structured output, status files, or CLI flags that can expose:
  - usage
  - quota/capacity
  - budget guardrails
  - session lifetime or timeout hints
- add adapter-fed fields only where the evidence is real
- improve operator wording for:
  - timeout
  - likely exhaustion
  - generic failure
  - unsupported telemetry

Deliverables:
- adapter-level capability improvements
- snapshot/output/TUI messaging improvements
- focused runtime tests

Definition of done:
- operators get stronger truth where available
- unsupported or unknown cases remain explicit and honest

Recommended branch/worktree:
- `codex/runtime-usage-truth-2`

Delegation note:
- A research-oriented side lane can inspect provider/runtime CLI capabilities in parallel without blocking the main implementation lane.

### Session D: CI And Supply-Chain Closure

**Goal:** make the security/tooling lane genuinely complete rather than “configured but not fully validated.”

Scope:
- finish local `cargo-deny` validation
- adjust `deny.toml` only if the local check reveals real policy/config errors
- confirm CI behavior is intentional and documented
- re-check whether any non-test production panic paths remain outside the already-hardened broker code

Deliverables:
- successful local `cargo deny check` or a documented/configured reason it still cannot pass
- any small follow-up fixes in config or docs

Definition of done:
- `cargo audit` and `cargo deny` are both not only wired in CI but also locally validated

Recommended branch/worktree:
- `codex/ci-supply-chain-closure`

### Session E: Release-Finalization Pass

**Goal:** turn the engineering checkpoint into a credible local release.

Scope:
- refresh CLI help/manual wording
- run the full `MANUAL_TEST_SCENARIOS.md` sweep
- validate real workflows against real repos again if behavior changed materially
- tighten known limitations and contributor/operator docs
- do one more release-quality audit

Deliverables:
- refreshed docs/manuals
- release-sweep notes
- final release-quality audit

Definition of done:
- the local product feels shippable
- the remaining limitations are explicit and acceptable

Recommended branch/worktree:
- `codex/release-finalization-pass`

## Recommended Immediate Sequence

If work resumes right away from the current checkpoint, do this:

1. Session B on a real Windows-capable environment
2. Session C if richer adapter-fed usage telemetry is still desired

Rationale:
- Session A, Session D, and Session E are now effectively complete for the local product on the current platform.
- Session B is the only remaining concrete release blocker.
- Session C remains useful follow-on work, but no longer blocks the current local release candidate in the same way.

## Recommended Delegation Pattern

Main lane:
- Session A
- Session C
- Session D
- Session E

External lane:
- Session B if a Windows-capable environment is available
- or a research lane that inspects runtime CLI telemetry for Session C

## Guardrails

- Keep the lead-agent/task-card orchestration workflow as the north star.
- Do not add TUI-only business logic that bypasses the command layer.
- Do not invent fake runtime usage precision.
- Do not broaden broker work into an async/streaming rewrite unless a concrete blocker forces it.
- Prefer bounded, finish-line slices over ambitious subsystem redesigns.
