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
- ConPTY-backed PTY supervision is implemented in the current codebase
- One-shot execution remains the fallback when PTY launch is not selected
- Named Pipe-based daemon transport is implemented
- The remaining gap is deeper workflow validation on a real Windows environment
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

1. Validate the current Windows ConPTY-backed `SessionSupervisor` against the same operator flows used on Unix
Status: implementation landed; real Windows smoke validation is still pending
2. Persist supervisor backend metadata instead of inferring it from log layout
Status: complete
3. Keep CI coverage across macOS, Linux, and Windows healthy
Status: complete for the current matrix
4. Add platform-specific smoke tests for shell runtime, daemon transport, and skills projection
Status: in progress; deeper Windows runtime-specific smoke coverage is still pending
