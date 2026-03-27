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

### Residual Risks

1. Remaining command-layer parity gap.
   Some mutating team-management flows still use direct `AppCore` helpers because the command surface is incomplete.

   Current examples:
   - team-member policy update
   - team-member remove
   - team-member assign-slot
   - task-slot binding

   This is now the most important architecture-cleanup item because it directly affects daemon/direct parity and the credibility of the “all mutations flow through commands” rule.

2. Runtime usage truth is still mostly advisory.
   The product now gives honest recovery hints, which is good, but adapter-fed token or budget telemetry is still thin. That is acceptable for now as long as `unknown` and `unsupported` remain explicit, but it is still an important remaining product gap.

3. Windows completion remains a release blocker.
   The Unix/local story is much stronger than the Windows story today. Release confidence still depends on finishing the Windows daemon/supervision path and validating the same operator workflows there.

## Strengths

- Clear local-first product identity
- Strong typed domain model
- Command/event model is maturing in the right direction
- Review and cleanup are now first-class operator concerns instead of implicit state
- Planning-to-task-card flow fits naturally into the existing team/task-card model
- Test depth is strong enough that the main audit effort shifted from bug hunting to parity and finish-line quality

## Recommended Next Objectives

1. Complete the missing command surface for mutating team-management flows.
2. Continue broker hardening until daemon mode feels like the default local operating model.
3. Finish Windows parity before broad release ambitions.
4. Improve structured runtime usage/capacity truth where adapters genuinely support it.
5. Use the next release-prep pass to tighten docs, help text, and known limitations.

## Verification

Passed during this audit:

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

Note: the full test suite still emits expected negative-path `git` and `r2d2` error lines for missing directories and invalid SQLite paths, but the suite finishes green.
