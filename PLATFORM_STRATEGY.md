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
- Repo registration, clone/fetch, slot lifecycle, context discovery, and skills workflows are all expected to work
- Shell runtime uses `pwsh`, then falls back to `powershell`
- One-shot session execution is the current default
- PTY supervision is not implemented yet
- Copy mode is the recommended default for skill projection because Windows symlink behavior is often permission-sensitive

## Design Direction

The platform seam should stay explicit:

- workspace orchestration should remain platform-neutral
- session supervision should be backend-driven through `awo-core::runtime::supervisor::SessionSupervisor`
- skill projection should choose sane defaults per platform

The intended backend split is:

- `tmux` supervisor on Unix-like systems
- future ConPTY-based supervisor on Windows
- one-shot execution everywhere as the lowest common denominator

## Practical Implications For V1

- Never assume `tmux`
- Never assume `zsh`
- Never assume symlink creation is frictionless
- Default behavior should degrade gracefully rather than fail mysteriously

## Next Platform Work

1. Implement a Windows ConPTY-backed `SessionSupervisor`
2. Persist supervisor backend metadata instead of inferring it from log layout
Status: complete in the current tmux-backed implementation
3. Add CI coverage across macOS, Linux, and Windows
4. Add platform-specific smoke tests for shell runtime and skills projection
