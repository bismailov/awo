# Release Process

This document defines the current Awo Console release path.

## What Ships

Each release archive bundles:

- `awo`
- `awod`
- `awo-mcp`
- `README.md`
- `LICENSE`
- `docs/release-process.md`

Archives are named as:

- `awo-<version>-<target>.tar.gz` on macOS and Linux
- `awo-<version>-<target>.zip` on Windows

## Release Workflow

The repository now supports two release-oriented workflow modes:

1. `push` a version tag that starts with `v`
2. run the GitHub Actions `Release` workflow manually

In both cases the workflow:

- checks formatting
- runs clippy with warnings denied
- runs the serialized workspace test suite
- builds the release binaries
- runs the cross-platform smoke workflow from `scripts/awo_smoke.py`
- packages the release archives with `scripts/package_release.py`

When the workflow runs from a version tag, it also publishes a GitHub Release with the packaged archives and smoke reports attached.

When the workflow runs manually, it uploads the same artifacts to the Actions run without publishing a GitHub Release.

## Smoke Validation Contract

The release smoke runner intentionally validates the operator-facing core loop rather than every edge case:

- repo registration
- context and skills doctor
- slot acquire, release, and delete
- shell session start and log inspection
- explicit daemon start, status, repo access, stop, and final status
- team init, planning, generation, task start, supersede, report, teardown, and delete
- TUI startup and quit via non-interactive `q`

The smoke runner isolates all Awo state in temporary directories so it does not depend on the operator's local machine state.

## Maintainer Notes

- If a release run fails in smoke validation, fix the product or the smoke harness before publishing.
- Prefer updating `scripts/awo_smoke.py` over adding one-off debugging scripts.
- If platform behavior changes intentionally, update this document, `README.md`, and the smoke expectations together.
