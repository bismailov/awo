# Platform Strategy

## Current Support Matrix

### macOS
- Full repo, slot, context, and skills workflows
- tmux-backed PTY supervision when `tmux` is installed
- symlink skill projection by default
- shell runtime prefers `zsh`, then falls back to `bash` or `sh`

### Linux
- Same operational model as macOS
- tmux-backed PTY supervision when `tmux` is installed
- symlink skill projection by default
- shell runtime prefers `zsh`, then falls back to `bash` or `sh`

### Windows
- Repo registration, clone/fetch, slot lifecycle, context discovery, skills workflows, daemon lifecycle, and team flows now pass the checked-in Windows smoke checklist
- Shell runtime uses `pwsh`, then falls back to `powershell`
- ConPTY-backed PTY supervision remains in the codebase, but the validated default path currently prefers direct one-shot execution for ordinary shell/team flows
- Named Pipe-based daemon transport is implemented and validated for explicit daemon start/status/stop flows
- The March 31, 2026 Windows checklist report closes the earlier native-validation gap
- Copy mode is the recommended default for skill projection because Windows symlink behavior is often permission-sensitive

## Design Direction

The platform seam should stay explicit:

- workspace orchestration should remain platform-neutral
- session supervision should be backend-driven through `awo-core::runtime::supervisor::SessionSupervisor`
- skill projection should choose sane defaults per platform

The intended backend split is:

- `tmux` supervisor on Unix-like systems
- ConPTY-based supervisor on Windows
- one-shot execution everywhere as the lowest common denominator

## Practical Implications For V1

- Never assume `tmux`
- Never assume `zsh`
- Never assume symlink creation is frictionless
- Default behavior should degrade gracefully rather than fail mysteriously

## Next Platform Work

1. Turn the March 31 Windows smoke checklist into a maintained repeatable workflow
Status: complete via `scripts/awo_smoke.py`, the refreshed `windows_live_check.ps1`, and CI/release workflow wiring
2. Persist supervisor backend metadata instead of inferring it from log layout
Status: complete
3. Keep CI coverage across macOS, Linux, and Windows healthy
Status: complete for the current matrix
4. Add platform-specific smoke tests for shell runtime, daemon transport, and skills projection
Status: complete for the current operator-core smoke matrix; future work is deeper PTY-specific enrichment rather than basic workflow coverage
