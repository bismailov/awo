# V1 Control Surface

## Purpose
Define the user-facing surface for a TUI-first V1 while keeping the command model reusable for later automation and optional GUI shells.

## V1 Product Shape
V1 should behave like a workspace operations console with three access patterns:
- TUI as the primary interactive surface
- CLI-style commands underneath for scripts, shell users, and automation
- external terminal handoff for rich agent interaction when embedded terminals are not yet worth the complexity

This means the command model should be primary, and the UI should mostly expose the same actions visually.

The local broker/daemon should also be legible as part of the control surface:
- `awo daemon status` should distinguish `healthy`, `starting`, `degraded`, and `not running`
- CLI auto-start may use the daemon automatically, but transport fallback to direct mode should be visible in text-mode operator flows

## Top-Level Mental Model
The user should think in this order:
1. Repository
2. Slot
3. Session
4. Review
5. Release

Not:
1. Agent
2. Chat
3. Transcript

## V1 CLI Command Set
Command examples use a placeholder binary name: `awo`.

### Repository Commands
#### `awo repo add <path>`
Registers a repository and initializes its profile.

Expected behaviors:
- validate Git root
- infer ecosystem hints
- suggest worktree root
- prompt for warm-slot strategy
- discover context files if present

#### `awo repo list`
Lists known repositories and summary state:
- active slots
- idle slots
- stale slots
- active sessions
- warnings

#### `awo repo doctor [repo]`
Checks repo configuration and safety conditions:
- base branch health
- worktree directory existence
- bootstrap command availability
- missing context files
- dirty protected slots

### Slot Commands
#### `awo slot acquire <repo> [task-name]`
Acquires a ready slot for a new task.

Common flags:
- `--base <branch>`
- `--branch <name>`
- `--fresh`
- `--warm`
- `--persistent <name>`
- `--runtime <adapter>`
- `--task-file <path>`

Expected output:
- selected slot
- branch created/checked out
- readiness status
- bootstrap action taken or skipped
- next suggested actions

#### `awo slot list [repo]`
Lists slot inventory with:
- slot type
- state
- branch
- last activity
- dirty/clean
- dependency freshness
- assigned session/runtime

#### `awo slot inspect <slot>`
Shows:
- path
- branch/base
- state transitions
- last fingerprint
- context pack
- warnings
- linked session metadata

#### `awo slot go <slot-or-query>`
Prints the path for shell integration or opens the slot in Terminal/iTerm/WezTerm.

Notes:
- shell integration can wrap this for `cd`
- GUI/TUI can map this to "open terminal here"

#### `awo slot release <slot>`
Safely returns a slot to the pool.

Common flags:
- `--delete-branch`
- `--keep-branch`
- `--force` only for non-destructive bypasses

Checks before release:
- dirty state
- active session
- unpushed commits
- protected slot status

#### `awo slot refresh [slot|--all]`
Refreshes stale warm slots from base branch and reruns bootstrap if needed.

### Session Commands
#### `awo session start <slot>`
Starts an AI runtime in a slot.

Common flags:
- `--runtime <adapter>`
- `--model <name>`
- `--task-file <path>`
- `--context-pack <path>`
- `--read-only`
- `--machine <target>`

Expected behavior:
- build prompt/task brief
- inject context references
- allocate env/ports
- launch adapter
- persist state

#### `awo session list [repo]`
Shows live and resumable sessions:
- runtime
- slot
- status
- elapsed time
- last activity
- machine target

#### `awo session stop <session>`
Graceful terminate or interrupt depending on adapter capability.

#### `awo session resume <session>`
Resumes when supported, otherwise relaunches with explicit warning.

#### `awo session logs <session>`
Shows raw or structured session output.

### Review Commands
#### `awo review status [repo]`
Shows a review-oriented snapshot:
- dirty slots
- high-risk overlap
- merged branches eligible for release
- stale warm slots
- failed sessions

#### `awo review diff <slot>`
Opens a diff summary for the slot.

#### `awo review overlap [repo]`
Detects multiple slots modifying risky file classes:
- lockfiles
- migrations
- deploy config
- shared DTO/schema packages

### Context Commands
#### `awo context pack <repo>`
Shows the files and prompt fragments that will be injected into new sessions.

#### `awo context doctor <repo>`
Validates required context files and task template integrity.

## TUI Information Architecture
The TUI should mirror the command model, not invent a competing one.

### Primary Views
#### Repositories View
Table or list of registered repos with:
- repo name
- base branch
- slot counts
- active sessions
- warning badge

#### Repository Detail View
Default operational view for a repo:
- active slots
- idle warm slots
- stale slots
- protected persistent slots
- active sessions
- warnings rail

#### Slot Detail View
Per-slot operational detail:
- slot path and branch
- readiness and fingerprint
- session attachment
- actions: start, open terminal, inspect diff, refresh, release

#### Session Detail View
Minimal in V1:
- runtime metadata
- state
- transcript/log preview
- actions: stop, resume, open external terminal, reveal slot

### Command Palette / Quick Actions
Useful throughout the TUI:
- add repo
- acquire slot
- start session
- open slot in terminal
- release slot
- refresh warm pool
- show overlap warnings

## Default V1 Workflow
### Workflow A: Quick New Task
1. User selects repo.
2. User chooses "Acquire slot".
3. Tool picks warm or fresh strategy.
4. Tool creates branch or checks out target branch.
5. Tool verifies readiness and bootstrap state.
6. User launches runtime.
7. Tool opens external terminal in slot or attaches minimal logs view.

### Workflow B: Parallel Task Board
1. User opens repo detail.
2. Sees 3 active slots, 4 idle slots, 1 stale slot.
3. Sees a warning that two active slots touch a lockfile.
4. Pauses or reassigns one task before conflict grows.

### Workflow C: Release And Recycle
1. User selects completed slot.
2. Tool shows status: clean, merged, branch pushed.
3. User confirms release and optional branch deletion.
4. Slot returns to `idle` or remains protected if persistent.

## Recommended V1 UI Bias
The TUI should prioritize:
- lists
- status chips
- warnings
- lifecycle actions
- open-in-terminal buttons

Over:
- rich transcript rendering
- tiled agent chat panes
- embedded shell emulation

## Why This Surface Works
- It matches the product wedge.
- It keeps automation-friendly commands first-class.
- It supports both solo shell-heavy users and later UI shells.
- It avoids spending V1 complexity budget on the wrong layer.
