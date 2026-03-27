# Implementation Plan: TUI Setup Parity and Immutable Task Workflow

**Date:** 2026-03-26
**Status:** Draft / Proposed
**Objective:** Enable repository, team, member, and task bootstrapping directly within the TUI, so operators can complete initial setup without dropping to the CLI.

## Scope Clarification

This plan targets **initial setup parity**, not full CRUD parity for every team-manifest object.

In scope for this plan:

1. **Repository Management:** `awo repo add`
2. **Team Lifecycle:** `awo team init`
3. **Roster Management:** `awo team member add`, `awo team member remove`, and member updates supported by the current command layer
4. **Task Planning:** `awo team task add`
5. **Task Delegation:** `awo team task delegate`

Explicitly out of scope for this plan:

- task edit
- task delete
- full team lifecycle controls like archive/reset/teardown/delete
- TUI-only business logic that bypasses `awo-core`

The TUI should remain a command-backed shell over the existing orchestration core.

## Task Model Decision

Tasks are **immutable planning records** once created.

That means:

- operators can create tasks
- operators cannot edit tasks in place
- operators cannot delete tasks outright
- if a task is no longer valid, the workflow should preserve history rather than erase it

Chosen recovery model:

- **Immutable + supersede/cancel task**

Practical meaning:

- if a task was planned incorrectly, the operator creates a replacement task
- the obsolete task is marked as cancelled or superseded instead of being edited or deleted
- the active plan remains clear, while historical planning intent is preserved for review/audit

Because the current command layer does not yet expose explicit cancel/supersede task actions, this plan treats that capability as a **follow-up core feature**, not part of immediate TUI parity.

## Design Principles

- Reuse the existing command layer wherever possible.
- Do not introduce TUI mutations that have no matching `awo-core` command path.
- Favor explicit operator workflows over hidden mutation.
- Preserve task history instead of silently rewriting planning artifacts.
- Keep the first milestone small enough to ship and validate quickly.

## Gap Analysis

Currently, the TUI is primarily an execution and monitoring surface. To remove CLI usage during initial setup, it needs:

1. **Repo add by path:** allow registering repos beyond the current implicit "add current directory" shortcut
2. **Team creation:** team init from the Teams view
3. **Member management:** add/remove members and update currently supported member routing/runtime fields
4. **Task creation:** add task cards directly from the Team Dashboard
5. **Task delegation:** reassign a task using the existing delegation flow

## Command-Backed Feature Map

This plan only commits to TUI actions that map to existing supported command/core operations.

| TUI Action | Backing Capability | Notes |
| :--- | :--- | :--- |
| Add repo by path | `repo add` | Extends current TUI support beyond current working directory |
| Create team | `team init` | Main missing bootstrap action |
| Add member | `team member add` | Supported today |
| Remove member | `team member remove` | Supported today |
| Update member runtime/routing | `team member update` | Limited to fields the command layer already supports |
| Add task | `team task add` | Supported today |
| Delegate task | `AppCore::delegate_team_task` | Already supported in core app flow |

Not included because they do not have matching parity support today:

- task edit
- task delete
- task cancel/supersede

## Phased Roadmap

### Phase 1: TUI Input and Action Infrastructure

Before adding new setup flows, the TUI needs reusable form and action plumbing.

- Expand the existing single-line input work into reusable form state.
- Add a centered modal/form component with multi-field navigation and submit/cancel.
- Add select-style inputs for enums and fixed choices such as repo IDs, runtimes, and roles.
- Keep the implementation aligned with the architecture target of separate keymap, action routing, and widgets rather than growing `tui.rs` further.

### Phase 2: Repo and Team Bootstrap

Allow operators to create the basic project structure from the main TUI tabs.

- **Repos tab**
- `a`: open a repo-add form instead of only adding the current working directory
- fields: repository path

- **Teams tab**
- `c`: open a team-init form
- fields: repo ID, team ID, objective
- optional advanced fields can be deferred behind defaults or an "advanced" mode

### Phase 3: Member Management

Bring supported `awo team member` operations into the Team Dashboard.

- Add a Members section or pane in the Team Dashboard
- `m`: add member
- `u`: update selected member runtime/routing preferences
- `d`: remove selected member with confirmation

Important limitation:

- member editing in this phase means **runtime/model/fallback/routing updates only**
- changing member identity or role is not part of the currently supported update surface and should not be implied by the UI

### Phase 4: Task Creation and Delegation

Bring supported task planning actions into the Team Dashboard.

- `n`: add task
- fields: task ID, owner, title, summary, runtime override, read-only flag, write scope, deliverable, verification, dependencies
- `D`: delegate selected task using the existing delegation flow
- delegation UI should capture target member, optional lead notes, optional focus files, and auto-start preference

Important limitation:

- tasks remain immutable after creation
- there is no edit-in-place or delete action in this plan

### Phase 5: Follow-Up Core Work for Immutable Task Recovery

This is not required to remove CLI usage during setup, but it is the next logical step for the immutable task model.

Add explicit core support for one of the following:

- `task cancel`
- `task supersede <old> -> <new>`

Then expose that flow in the TUI as a history-preserving planning correction path.

This phase should only start after the setup-parity slices above are working cleanly.

## Proposed Keybindings

| Context | Key | Action |
| :--- | :--- | :--- |
| Repos tab | `a` | Add repository by path |
| Teams tab | `c` | Create team |
| Team Dashboard | `m` | Add member |
| Team Dashboard | `u` | Update selected member |
| Team Dashboard | `d` | Remove selected member |
| Team Dashboard | `n` | Add task |
| Task list | `D` | Delegate selected task |

## Technical Implementation Notes

**Module organization**

Refactor the TUI toward the documented architecture rather than only splitting out render helpers.

Target structure:

- `crates/awo-app/src/tui/mod.rs`
- `crates/awo-app/src/tui/screens.rs`
- `crates/awo-app/src/tui/view_models.rs`
- `crates/awo-app/src/tui/keymap.rs`
- `crates/awo-app/src/tui/action_router.rs`
- `crates/awo-app/src/tui/forms.rs`
- `crates/awo-app/src/tui/widgets.rs`

**Validation and behavior**

- all form submission should route through existing `awo-core` commands or `AppCore` flows
- the TUI should not duplicate business validation rules
- error messages should surface command validation failures directly to the operator

**Responsiveness**

- keep slow repo and disk operations on the existing background-thread path
- use the same pattern for repo add and any slower form submissions
- lightweight manifest mutations can remain synchronous if they do not harm input responsiveness

**Defaults**

- choose conservative defaults for advanced fields instead of blocking on a large first-form design
- expose advanced options only where they materially affect setup

## Acceptance Criteria

This plan is successful when an operator can:

1. register a repo by path from the TUI
2. create a team from the TUI
3. add and remove members from the TUI
4. update supported member runtime/routing fields from the TUI
5. add tasks from the TUI
6. delegate a task from the TUI
7. complete the above without using CLI for initial setup

This plan is still considered successful even if:

- tasks cannot be edited
- tasks cannot be deleted
- cancelled/superseded task support has not landed yet

Those belong to the next planning slice for immutable task recovery.
