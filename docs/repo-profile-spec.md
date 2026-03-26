# Repo Profile Spec

## Purpose
Define how the product captures repo-specific orchestration rules without forcing every repository into the same workflow.

## Design Decision
Repo profile data should be split into two layers:

1. Shared repo manifest
- versionable
- lives in the repository
- describes repo behavior that should be consistent for every developer and agent

2. Local machine overlay
- per-user and per-machine
- lives in the app config directory
- describes paths, local runtime details, and personal defaults

This split is important because branch naming rules and context files belong with the repo, while absolute paths, port ranges, and terminal preferences do not.

## Proposed File Locations
### Shared manifest
- `.awo/repo.toml`

### Local overlay
- `~/.config/awo/repos/<repo-id>.toml`

Inference:
- On macOS, a more native app path could be used later, but TOML in a predictable config directory is the easiest V1 shape.

## Repo Identity
Each registered repo needs a stable id derived from:
- canonical Git root path
- Git remote URL if available

This avoids collisions when the same repo exists in multiple locations.

## Shared Manifest Schema
### Core
```toml
version = 1
name = "rentals-js"
default_base_branch = "main"
worktree_dir_policy = "sibling"
```

### Branch Rules
```toml
[branches]
default_prefix = "user"
pattern = "{owner}/{ticket}/{slug}"
allow_direct_checkout = true
```

### Slot Strategy
```toml
[slots]
default_strategy = "warm"
warm_pool_size = 6
protected_names = ["dev", "review"]
recycle_policy = "oldest_clean_idle"
```

### Dependency Fingerprint Rules
```toml
[[fingerprints]]
name = "node-deps"
files = ["yarn.lock", ".yarnrc.yml", "package.json"]
bootstrap = "yarn install --immutable"
refresh = "git pull --ff-only origin main && yarn install --immutable"
strategy = "content-hash"
```

Multiple fingerprint groups should be allowed so polyglot repos can track more than one readiness concern.

### Context Pack
```toml
[context]
entrypoints = ["AGENTS.md", "PROJECT.md", "CLAUDE.md", "GEMINI.md"]
required_files = [
  "PROJECT.md",
  "CONVENTIONS.md"
]
optional_files = [
  "README.md",
  "docs/architecture.md"
]
analysis_globs = ["analysis/*.md"]

[[context.packs]]
name = "audit"
files = [
  "analysis/2026-03-20-consolidated-issue-list.md",
  "analysis/2026-03-20-audit-remediation-report.md"
]

[[context.packs]]
name = "architecture"
files = [
  "docs/architecture.md",
  "analysis/refactor-1.md"
]

task_template = ".awo/task-template.md"
quality_checklist = ".awo/checklist.md"
```

### Skills
```toml
[skills]
shared_paths = [".agents/skills"]
native_compat_paths = [".claude/skills", ".gemini/skills"]
lockfile = "skills-lock.json"
link_mode = "prefer-shared"
```

### MCP
```toml
[mcp]
project_config = ".mcp.json"
discover = true
```

### Risk Rules
```toml
[risks]
high_overlap_globs = [
  "yarn.lock",
  "pnpm-lock.yaml",
  "package-lock.json",
  "migrations/**",
  "infra/**",
  "shared/**/schema/**"
]
```

### Environment Rules
```toml
[env]
env_files = [".env", ".env.local"]
port_strategy = "slot-offset"
base_port = 3000
port_block_size = 20
```

### Launch Defaults
```toml
[launch]
default_runtime = "codex"
open_in_terminal_after_start = true
terminal_app = "wezterm"
```

## Local Overlay Schema
### Paths And Machine Preferences
```toml
repo_id = "rentals-js-abcd1234"
repo_root = "/path/to/workspace/rentals-js"
worktree_root = "/path/to/workspace/rentals-js.worktrees"
terminal_app = "wezterm"
preferred_machine = "local"
```

### Local Slot Overrides
```toml
[slots]
warm_pool_size = 8
auto_refresh_on_open = false
```

### Runtime Defaults
```toml
[runtimes]
preferred = "claude-code"
fallback = "codex"
```

### Local Environment Overrides
```toml
[env]
base_port = 4300
extra_env = { AWO_OPERATOR = "local-dev" }
```

## Merge Rules
The effective repo profile is:

1. product defaults
2. shared repo manifest
3. local machine overlay

Local overlay may override:
- absolute paths
- local terminal app
- warm pool size
- machine target defaults
- local env values

Local overlay must not silently override:
- required context files
- risk glob definitions
- shared task checklist

Those should require explicit opt-out behavior, because they are part of the repo's safety model.

## Required V1 Fields
For V1, the minimum viable shared manifest should include:
- `name`
- `default_base_branch`
- `slots.default_strategy`
- at least one fingerprint group or bootstrap command
- context required files

The minimum viable local overlay should include:
- `repo_root`
- `worktree_root`

## Fingerprint Semantics
Fingerprints determine whether a slot is ready, stale, or requires refresh.

Each fingerprint group should support:
- list of watched files
- optional watched commands
- bootstrap command
- refresh command
- comparison strategy

V1 comparison strategies:
- `content-hash`
- `mtime`
- `git-diff`

Recommendation:
- default to `content-hash` or `git-diff` for lockfile-centric repos

## Slot State Derivation
Slot state should be computed from Git state plus fingerprint state:
- `idle`: detached or assignable, clean, no active session
- `active`: assigned to a branch or live session
- `dirty`: uncommitted changes present
- `stale`: fingerprint mismatch with assigned target
- `refreshing`: bootstrap or refresh in progress
- `error`: last lifecycle action failed

## Why This Schema Matters
- It gives the product real repo awareness.
- It supports heavy monorepo workflows without hard-coding JavaScript assumptions everywhere.
- It allows shared standards and local machine realities to coexist cleanly.
- It gives the product room to model optional analysis packs, portable skills, and MCP-aware repo setup.
- It turns orchestration policy into data, which makes future automation easier.
