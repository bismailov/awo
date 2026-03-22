# Rust Core Architecture

## Purpose
Define the internal Rust architecture for a TUI-first V1 while preserving a clean path to later CLI automation and optional GUI shells.

## Chosen Direction
The product should ship as:
- a Rust core library that owns orchestration logic
- a TUI-first application shell for interactive use
- a command interface backed by the same core actions

This keeps the product fast to build without trapping it in a UI-specific architecture.

## Workspace Shape
Recommended initial Rust workspace:

```text
Cargo.toml
crates/
  awo-core/
  awo-app/
```

### `awo-core`
Pure application logic:
- domain types
- config loading and merging
- persistence
- Git/worktree lifecycle
- slot engine
- fingerprint engine
- runtime adapters
- session supervision
- review and warning engine
- event model

### `awo-app`
User-facing shell:
- argument parsing
- TUI event loop and rendering
- command dispatch
- external terminal launch helpers

Rationale:
- fast enough for V1
- clean separation between orchestration and presentation
- easy path to later split into `awo-tui` and `awo-cli` crates if needed

## Core Architectural Rule
All mutations must pass through one command layer in `awo-core`.

The TUI should never mutate state directly.
Instead:
1. user action triggers a command
2. command validates inputs and current state
3. domain service performs the mutation
4. persistence is updated
5. events are emitted
6. TUI refreshes from new snapshots

This is the main guard against state drift and race conditions.

## Suggested `awo-core` Modules
### `domain`
Owns stable entities and ids:
- `RepoId`
- `SlotId`
- `SessionId`
- `MachineId`
- `Repo`
- `Slot`
- `Session`
- `TaskBrief`

### `config`
Loads and merges:
- shared repo manifest
- local overlay
- app defaults

Produces an effective `RepoProfile`.

### `store`
Persists operational state.

Recommended storage split:
- TOML for config
- SQLite for runtime state and action history
- append-only log/transcript files on disk, referenced by SQLite

Tables or collections should cover:
- repositories
- slots
- sessions
- action log
- fingerprint cache
- warnings cache

### `git_ops`
Wraps `git` CLI operations:
- discover repo root
- branch inspection
- worktree list/add/remove/lock/prune/repair
- diff status
- ahead/behind checks

### `fingerprints`
Calculates readiness and staleness using repo profile rules:
- file hashing
- Git-based comparisons
- bootstrap decisioning

### `slots`
Owns slot lifecycle:
- acquire
- reserve
- mark active
- refresh
- release
- protect
- derive display state

### `context`
Builds the context pack for new sessions:
- required files
- optional files
- task brief
- quality checklist
- handoff notes

### `runtime`
Owns adapter registry and capability-based launch behavior:
- runtime detection
- launch preparation
- output parsing
- approval mode mapping

### `sessions`
Supervises running and completed processes:
- launch
- attach metadata
- stop/interrupt
- finalize
- resume semantics

### `review`
Computes warnings and review signals:
- risky overlap
- dirty slot warnings
- stale warm pool
- failed sessions
- releasable merged branches

### `commands`
Public application service layer used by both TUI and CLI flows.

Examples:
- `add_repo`
- `acquire_slot`
- `start_session`
- `release_slot`
- `refresh_slot`
- `review_status`

### `events`
Defines normalized domain and runtime events for UI refresh and audit history.

### `machine`
Encapsulates local vs remote execution targets.

V1 can implement only local behavior while keeping remote as a future-capable abstraction.

## Persistence Strategy
### Why not only JSON?
JSON would be fast for early persistence, but this product already has:
- multiple entity types
- state transitions
- action history
- queries for TUI views

SQLite is a better V1 choice for operational state.

### Recommended pattern
- configs remain human-editable TOML
- state is normalized in SQLite
- large logs remain file-based

This gives:
- durable state
- queryable lists for TUI tables
- easier migration path later

## Concurrency Model
The core should serialize mutating operations per repository.

Suggested rule:
- reads may run concurrently
- writes for the same repo must be serialized through a `RepoCoordinator`

Why:
- prevents two slot acquisitions from racing
- prevents refresh/release collisions
- keeps Git/worktree mutations predictable

This can be implemented later with:
- per-repo async mutex
- per-repo command queue

The important design choice is the invariant, not the exact primitive.

## State Model
Do not model slot state as one giant enum with every combination baked in.
Use composed state internally and derive user-facing labels from it.

## Slot State Machine
### Internal Axes
#### Assignment
- `Unassigned`
- `Reserved`
- `Assigned`
- `Releasing`

#### Readiness
- `Unknown`
- `Ready`
- `Stale`
- `Refreshing`
- `Failed`

#### Cleanliness
- `Clean`
- `Dirty`

#### Protection
- `Pooled`
- `Persistent`

#### Session Attachment
- `None`
- `Starting`
- `Running`
- `Completed`
- `Failed`

### Derived Display States
The TUI should show simplified states:
- `idle`
- `active`
- `dirty`
- `stale`
- `refreshing`
- `error`

### Key Slot Transitions
#### Acquire
`Unassigned + Ready + Clean -> Reserved -> Assigned`

#### Session Start
`Assigned + Ready -> Session Starting -> Session Running`

#### Dirtying
Any assigned slot with file changes:
`Clean -> Dirty`

#### Refresh
`Ready/Stale -> Refreshing -> Ready`
or
`Refreshing -> Failed`

#### Release
`Assigned -> Releasing -> Unassigned`

Protected persistent slots never auto-transition into pooled recycle flows.

## Session State Machine
### Core States
- `Created`
- `Preparing`
- `Starting`
- `Running`
- `AwaitingInput`
- `Interrupting`
- `Stopping`
- `Completed`
- `Failed`
- `Cancelled`

### Notes
- `AwaitingInput` is mainly meaningful for persistent runtimes.
- one-shot runtimes usually go directly from `Running` to `Completed` or `Failed`.
- "resume" is a real state transition only for runtimes that actually support it.

### Key Session Transitions
#### Start
`Created -> Preparing -> Starting -> Running`

#### Persistent Idle Loop
`Running <-> AwaitingInput`

#### Graceful Stop
`Running -> Interrupting -> Stopping -> Cancelled`

#### Normal Completion
`Running -> Completed`

#### Error Path
`Preparing/Starting/Running -> Failed`

## Command Execution State
Every mutating command should conceptually pass through:
- `Received`
- `Validating`
- `Executing`
- `Persisting`
- `Broadcasting`
- `Completed`
or
- `Failed`

This is useful for:
- audit logging
- debug visibility
- future progress UI

## TUI Integration Model
The TUI should render read models, not raw domain internals.

Recommended TUI layers:
- `screens`
- `view_models`
- `keymap`
- `action_router`
- `widgets`

The TUI loop should combine:
- keyboard input
- timer/tick events
- domain/runtime events from the core

## Initial TUI Screens
- repository list
- repository detail
- slot detail
- session detail
- review/warnings view
- command palette

## External Terminal Strategy
Even with TUI chosen, external terminals still matter.

V1 should support:
- open slot path in terminal
- open session terminal
- reveal log file

This avoids forcing transcript rendering to carry the entire workflow.

## Why This Architecture Is Fast
- one core, one TUI shell
- command-first mutation model
- clear module boundaries
- state machines that avoid enum explosion
- SQLite for durable, queryable state from the beginning

## V1 Cut Line
Required in V1:
- local machine target only
- one core library
- one TUI app shell
- command layer
- repo/slot/session persistence

Can wait:
- remote daemon
- embedded terminal emulation
- macOS GUI shell
- advanced merge UI
