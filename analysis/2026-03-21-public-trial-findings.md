# Public Trial Findings

## Goal

Validate `awo` against disposable public GitHub repositories instead of a live product repository.

## Repos Used

- `sharkdp/bat`
  - repo id: `bat-c6342dcc61cb`
- `jesseduffield/lazygit`
  - repo id: `lazygit-6f32fd09abf5`

## What We Exercised

- managed remote clones already registered in `awo`
- team manifest init/list/show flows
- parallel cross-process team member/task mutation
- team-task-driven slot acquisition
- team-task-driven session launch
- JSON output mode for runtime/team/slot/session/review commands
- real read-only Codex, Gemini, and Claude launches
- deterministic shell runtime launch
- review/status/session reconciliation after interrupted oneshot launchers
- post-trial slot release cleanup

## Real Outputs

### `bat`

- Gemini `docs-scan`
  - result: completed
  - useful findings:
    - syntax-contribution guidance is fragmented across `CONTRIBUTING.md`, `README.md`, `doc/assets.md`, and `src/syntax_mapping/builtins/README.md`
    - local linting requirements enforced by CI are not documented for contributors
    - there is no high-level architecture guide for core feature contributors

- Codex `ci-scan`
  - result: session became a failed recovered oneshot after the launcher path was interrupted during an early live trial
  - product takeaway:
    - stale `running` oneshot sessions must reconcile after launcher interruption

### `lazygit`

- Claude `doc-scan`
  - result: completed
  - useful findings:
    - `CONTRIBUTING.md` lacks a plain non-devcontainer/non-Nix setup path
    - testing guidance is too sparse and too buried
    - there is no newcomer-oriented contribution path or functional code-of-conduct guidance

- Shell `script-scan`
  - result: completed
  - useful output:
    - quickly surfaced contributor-facing docs/workflow references from `Makefile`, `pkg/integration/README.md`, `VISION.md`, `docs/`, and config-schema descriptions

## Product Gaps Found And Fixed

### Fixed during this phase

- cross-process team-manifest mutation could clobber earlier edits
  - fixed with manifest-side file locking
- `team task start` held the manifest lock too long
  - fixed by splitting launch into reservation, slot-binding, and finalize phases
- interrupted oneshot launchers left stale `running` sessions forever
  - fixed with PID and exit-code sidecars plus oneshot sync logic

### Still open after this phase

- team manifests need an explicit archive/reset or reconcile flow after runs finish and slots are released
- true interruption/timeout control for running oneshot sessions is still not implemented
- richer post-run task-state reconciliation from session outcomes back into team manifests would reduce manual cleanup

## Current Recommendation

The public-repo trials are strong enough that `awo` is now testable as a real operator tool.

The next highest-value steps are:

1. team archive/reset and manifest reconciliation
2. middleware-oriented JSON command surface tightening
3. supervisor abstraction and Windows-native PTY backend
