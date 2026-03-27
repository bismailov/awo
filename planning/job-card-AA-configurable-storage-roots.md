# Job Card AA — Configurable Storage Roots

## Objective

Make repo clone location and workspace/worktree roots operator-configurable rather than implicitly tied to app-support defaults.

## Why This Slice

Operators need control over where repositories, clones, logs, and worktrees live. This is especially important for local-first heavy usage where multiple worktrees and retained warm slots are normal.

## Primary Write Scope
- `crates/awo-core/src/config.rs`
- `crates/awo-core/src/repo.rs`
- `crates/awo-core/src/app.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-app/src/tui/*`
- `docs/repo-profile-spec.md`
- `docs/product-spec.md`

## Scope

### In Scope
- configurable clone root
- configurable default worktree root
- optional per-repo override story
- UI/CLI visibility into current roots

### Out Of Scope
- remote storage targets
- network filesystem correctness guarantees beyond local documented behavior

## Deliverables
- config settings for storage roots
- repo registration flows that honor configured roots
- tests for default and overridden paths
- docs explaining operator control of storage placement

## Definition Of Done
- clone/worktree locations are no longer effectively hardcoded
- operators can understand and control where local state is stored
