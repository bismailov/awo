# Awo Console Project Context

`project.md` is the source of truth for project onboarding, reading order, key rules, and linked context.

Awo Console is a TUI-first agent workspace orchestrator for safe parallel AI work on local Git repos.

Internal codename and binary names remain `awo`, `awod`, and `awo-mcp`.

## Thin Handles

`AGENTS.md` and `CLAUDE.md` exist as thin compatibility handles. They should point here and should not duplicate the full project guidance.

## Read First

1. This file
2. [`planning/2026-03-22-development-plan.md`](planning/2026-03-22-development-plan.md) for current state, milestones, and priorities
3. [`README.md`](README.md) for user-facing product framing and usage
4. [`docs/core-architecture.md`](docs/core-architecture.md) for module structure and design rules
5. [`docs/product-spec.md`](docs/product-spec.md) for the product wedge and workflows
6. [`docs/middleware-mode.md`](docs/middleware-mode.md) for the future direction

## Key Rules

- All state mutations flow through the command layer in `awo-core`.
- `awo-app` is a thin shell over the core, never the source of truth.
- `unsafe_code = "forbid"` workspace-wide.
- The core stays synchronous for now; see [`docs/tokio-decision.md`](docs/tokio-decision.md).
- Safety before convenience; prefer bounded slices over broad rewrites.
- If the user starts a prompt with `?`, treat it as planning or discussion only and do not edit files.

## Open Source Safety

- Keep tracked files free of secrets, credentials, private repository names, and personal machine-specific paths.
- Use generic placeholders in public docs and examples such as `/path/to/repo`, `org/repo`, and `local-dev`.
- Do not commit transcript dumps, local planning scratch files, editor state, or one-off research artifacts unless they are intentionally curated for public documentation.
- Prefer contributor-facing docs that can be understood without access to private services, private repos, or a specific local machine setup.
- When in doubt, bias toward public-safe wording and reproducible examples.

## Project Layout

```text
crates/awo-core/    # orchestration library
crates/awo-app/     # CLI + TUI binary
docs/               # durable product and architecture specs
planning/           # time-stamped planning artifacts
analysis/           # research, findings, code audits
scripts/            # build and maintenance scripts
```

## Docs Library

- [docs/agent-neutral-context-skills.md](docs/agent-neutral-context-skills.md) - repo entrypoints, context strategy, and skill portability
- [docs/core-architecture.md](docs/core-architecture.md) - crate boundaries and command-layer invariants
- [docs/interface-strategy.md](docs/interface-strategy.md) - control-surface direction across CLI, TUI, and integrations
- [docs/middleware-design.md](docs/middleware-design.md) - middleware architecture ideas
- [docs/middleware-mode.md](docs/middleware-mode.md) - long-term virtual-agent direction
- [docs/naming.md](docs/naming.md) - current naming decisions and terminology
- [docs/open-source-release-checklist.md](docs/open-source-release-checklist.md) - release hygiene checklist
- [docs/platform-strategy.md](docs/platform-strategy.md) - platform and runtime strategy
- [docs/product-spec.md](docs/product-spec.md) - product goals, workflows, and safety model
- [docs/repo-profile-spec.md](docs/repo-profile-spec.md) - repo profile and context-pack configuration shape
- [docs/runtime-adapter-spec.md](docs/runtime-adapter-spec.md) - runtime adapter contract
- [docs/subagent-orchestration.md](docs/subagent-orchestration.md) - delegation and orchestration direction
- [docs/team-manifest-spec.md](docs/team-manifest-spec.md) - team and task manifest design
- [docs/tokio-decision.md](docs/tokio-decision.md) - why the core is still synchronous
- [docs/v1-control-surface.md](docs/v1-control-surface.md) - planned command and control flows
- [docs/v1-roadmap.md](docs/v1-roadmap.md) - staged roadmap

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```
