# Audit And Quality Review (March 28, 2026)

## Scope

This review covered:

- the current orchestration checkpoint in `awo-core`, `awo-app`, and `awo-mcp`
- command-layer parity against the architectural rule in `docs/core-architecture.md`
- roadmap/doc drift against the current implementation
- open-source safety and documentation hygiene
- full verification with `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`

## Overall Assessment

The product is in a strong state for a local-first orchestration tool:

- the team/task-card model is now meaningfully richer
- the TUI is closer to a real orchestration cockpit
- immutable recovery, planning, review, cleanup, and runtime recovery guidance are all present
- the test suite remains strong and the workspace verifies cleanly

I did not find a blocking correctness bug in the newly added orchestration slices during this pass.

See also `planning/2026-03-28-release-finalization-audit.md` for the later same-day release-candidate checkpoint after Windows follow-through, runtime-truth upgrades, local `cargo-deny` validation, and release smoke testing.

## Findings

### Resolved During This Audit

1. Command-surface drift in operator flows.
   Several CLI/TUI paths were still bypassing the dispatcher even when a public `Command` already existed. This weakens direct-vs-daemon parity and undercuts the core mutation invariant.

   Fixed in this audit:
   - `team.task.start` now flows through `AppCore::dispatch`
   - CLI archive/reset/delete now dispatch through commands instead of mutating directly
   - `team.teardown` now has dispatcher-backed handling in `AppCore::dispatch`
   - command outcomes for archive/reset/delete now carry structured data for shell surfaces

2. Open-source safety drift in roadmap docs.
   The master finalization plan had machine-specific absolute filesystem links. Those were replaced with portable relative links.

### External Audit Reconciliation

An additional external audit was reviewed after this document was first written. Its findings were incorporated selectively:

1. TUI blocking snapshot work was a real concern and has now been mitigated.
   The TUI no longer performs its periodic full `snapshot()` refresh on the render loop. Initial load is still synchronous, but later refreshes now arrive from a background worker, which makes this a polish concern rather than a current blocker.

2. `action_router.rs` bloat was real and has been partly addressed.
   Dialog/form/confirm handling now lives in `crates/awo-app/src/tui/action_router/dialogs.rs`, substantially shrinking the main router. More bounded decomposition is still desirable before another large cockpit feature lands.

3. The command-parity warning was valid, but its concrete examples are now outdated.
   The external audit cited member update/remove/assign-slot and task bind-slot as still bypassing commands. That was true before the March 28 parity sweep, but those specific flows are now first-class commands and dispatch-backed across CLI/TUI.

   The broader architectural rule remains important, but those named examples should no longer be treated as open gaps.

4. The `unwrap`/`expect` warning needed nuance and has mostly been addressed where it mattered.
   Most hits in `awo-core` are in tests. The production concern that still matters is concentrated in `EventBus` lock/condvar handling in `crates/awo-core/src/events.rs`, where poisoned synchronization primitives would currently panic the process.
   That was a legitimate hardening task, and it has now been addressed by recovering poisoned state and warning instead of panicking immediately. The external audit still overstated the breadth of production panic exposure, but it usefully pointed at the right hotspot.

5. CI hardening was a valid remaining gap and is now partly closed.
   The GitHub Actions workflow now includes `cargo audit` and `cargo deny`, and the repo now has a `deny.toml`. Local `cargo audit` validation succeeded; `cargo-deny` local validation still needs follow-through.

### Residual Risks

1. Broker live-update delivery is still thinner than the long-term design wants.
   The TUI now reacts to event-bus wakeups for command-driven changes, and the MCP facade now supports resource subscriptions with bounded `notifications/resources/updated` emissions for subscribed resources. That is materially better, but daemon/MCP live delivery still leans on per-request notifications and polling/long-polling more than the final local operating model should.

2. TUI module bloat and boundary drift remain a watch item.
   The router split is materially better than before, but more bounded extraction will still help if another major dashboard slice lands.

3. Runtime usage truth is still mostly advisory.
   The product now gives honest recovery hints, which is good, but adapter-fed token or budget telemetry is still thin. That is acceptable for now as long as `unknown` and `unsupported` remain explicit, but it is still an important remaining product gap.

4. Windows completion remains a release blocker.
   The Windows daemon transport and ConPTY implementation have advanced since this audit, but final confidence still depends on validating the same operator workflows on a real Windows environment.

5. CI security policy still needs finishing work.
   This was true at the time of the audit. It has since been followed through locally: `cargo-deny` was validated, `deny.toml` schema drift was fixed, and `0BSD` was added for the Windows transport dependency chain.

## Strengths

- Clear local-first product identity
- Strong typed domain model
- Command/event model is maturing in the right direction
- Review and cleanup are now first-class operator concerns instead of implicit state
- Planning-to-task-card flow fits naturally into the existing team/task-card model
- Test depth is strong enough that the main audit effort shifted from bug hunting to parity and finish-line quality

## Recommended Next Objectives

1. Finish broker hardening around live-event delivery and any remaining degraded-state visibility.
2. Continue bounded TUI decomposition before another large cockpit expansion.
3. Finish Windows parity before broad release ambitions.
4. Improve structured runtime usage/capacity truth where adapters genuinely support it.
5. Finish validating CI security/supply-chain checks.

## Verification

Passed during this audit:

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo audit` (warning-only: `RUSTSEC-2017-0008` via `portable-pty -> serial`)

Note: the full test suite still emits expected negative-path `git` and `r2d2` error lines for missing directories and invalid SQLite paths, but the suite finishes green.
