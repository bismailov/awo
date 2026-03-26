# Contributing

Thanks for helping improve Awo Console.

The public product name is `Awo Console`. Internal code, crate, and binary names such as `awo`, `awod`, and `awo-core` remain current technical identifiers.

## Before You Start

- Read [`CLAUDE.md`](CLAUDE.md) first for project context and architectural rules.
- Skim [`planning/2026-03-22-development-plan.md`](planning/2026-03-22-development-plan.md) for current priorities.
- Prefer small, bounded changes over broad rewrites.
- If you are exploring or proposing a change, open an issue or discussion first when the direction is unclear.

## Development Setup

Requirements:

- Rust stable toolchain
- `git`
- `tmux` for the strongest Unix PTY experience

Common commands:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Run the main app locally:

```bash
cargo run --bin awo
```

## Contribution Workflow

1. Create a focused branch.
2. Make the smallest change that solves the problem well.
3. Add or update tests when behavior changes.
4. Update docs when command surfaces, safety rules, or workflows change.
5. Run the verification commands before opening a pull request.

## What Good Changes Look Like Here

- All state mutations flow through the command layer in `awo-core`.
- `awo-app` stays a shell over the core instead of becoming a second source of truth.
- Safety rules are preserved or strengthened.
- New behavior is explained in user-facing docs when needed.
- Tests cover both happy paths and important failure paths.

## Open-Source Safety Rules

Please keep the public repository safe and portable:

- Never commit secrets, tokens, credentials, or private keys.
- Do not commit local editor state, transcript dumps, scratch planning files, or machine-specific artifacts.
- Use generic placeholders in docs and examples such as `/path/to/repo`, `org/repo`, and `local-dev`.
- Avoid personal usernames, private repository names, and absolute local paths in tracked files unless they are intentionally illustrative and already anonymized.
- Prefer examples that another contributor can understand without access to your machine or private services.

## Pull Requests

Pull requests are easiest to review when they include:

- a short problem statement
- the chosen approach
- verification notes
- any known follow-up work or risks

If a change affects operator workflows, include the exact commands or screens that changed.

## Documentation

If you change public behavior, update the relevant docs:

- [`README.md`](README.md) for user-facing setup and usage
- [`CLAUDE.md`](CLAUDE.md) for contributor onboarding and key project rules
- [`docs/`](docs) for durable design and architecture decisions

## Questions And Proposals

If you are unsure whether a change fits the product direction, start with an issue or draft pull request. Early alignment is better than a large surprise diff.
