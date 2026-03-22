# awo

TUI-first agent workspace orchestrator for safe parallel AI work on local Git repos.

Rust workspace: `awo-core` (orchestration logic) and `awo-app` (CLI/TUI shell).

## Reading order

1. This file
2. `planning/2026-03-22-development-plan.md` -- current state, milestones, priorities
3. `docs/core-architecture.md` -- module structure and design rules
4. `docs/product-spec.md` -- product wedge and workflows
5. `docs/middleware-mode.md` -- future direction

## Key rules

- All state mutations flow through the command layer in `awo-core`
- `awo-app` is a thin shell over the core, never the source of truth
- `unsafe_code = "forbid"` workspace-wide
- Synchronous core (no Tokio yet) -- see `docs/tokio-decision.md`
- Safety before convenience; bounded slices over broad rewrites
- If the user starts a prompt with `?`, treat it as planning/discussion only -- no code changes

## Project layout

```
crates/awo-core/    # orchestration library
crates/awo-app/     # CLI + TUI binary
docs/               # durable product and architecture specs
planning/           # time-stamped planning artifacts
analysis/           # research, findings, code audits
scripts/            # build and maintenance scripts
```

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```
