# Open Source Release Checklist

Use this checklist before announcing a public release or opening the repo to wider contributions.

## Repository Hygiene

- Confirm no tracked files contain secrets, tokens, private keys, or credentials.
- Confirm no tracked files contain private repository names, internal-only notes, or personal machine paths.
- Remove transcript dumps, scratch planning files, local editor state, and one-off research artifacts that do not belong in the public history.
- Make sure `.gitignore` covers local planning files, editor state, and generated artifacts.

## Community Health

- `README.md` explains what the project is, how to build it, and how to use it.
- `LICENSE` is present and matches maintainer intent.
- `CONTRIBUTING.md` is present and accurate.
- `CODE_OF_CONDUCT.md` is present.
- `SECURITY.md` is present.

## Build And Quality Gates

- `cargo fmt --all` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test` passes.
- CI is green on supported platforms.

## Documentation Safety

- Public docs use generic placeholders such as `/path/to/repo` and `org/repo`.
- Examples do not assume access to private services or private repositories.
- Onboarding docs explain how to keep future edits open-source-safe.

## Product Framing

- Known limitations are documented honestly.
- Experimental or incomplete platform support is described clearly.
- Any future-looking architecture notes are labeled as direction rather than shipped behavior.

## Maintainer Readiness

- There is a clear issue triage approach.
- Security reports have a documented intake path.
- Release assumptions that are still implicit are written down in docs.
- The current license, contribution model, and project scope reflect maintainer intent.

## Nice-To-Have Before Wider Promotion

- Add issue templates and pull request templates.
- Add release notes or a changelog process.
- Add a simple roadmap or “good first issue” triage label strategy.
