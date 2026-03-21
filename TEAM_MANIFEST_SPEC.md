# Team Manifest Spec

## Purpose

Define the first durable team-orchestration artifacts for `awo`.

## Operator Model

The default product model should be a single-window controller, not "one giant pane per agent" by default.

### Default UX

- one main `awo` window or TUI session
- background agents attached to slots and sessions
- list/detail panes for:
  - repositories
  - teams
  - slots
  - sessions
  - review
- focused detail for the selected session, including status, logs, and ownership

This means `awo` behaves more like an orchestrator dashboard than a tiled wall of full terminals.

### Why

- most operators need control and visibility more than constant transcript watching
- team state, slot ownership, and review warnings are more important than showing every token stream at once
- a single controller scales better across runtimes with very different UI semantics

### Future UX

Later, `awo` can add:
- split-pane live session views
- detachable session windows
- external terminal attachment
- multi-window team dashboards

But that should layer on top of the controller model rather than replace it.

## Team Manifest Shape

The first implementation lives in [team.rs](crates/awo-core/src/team.rs).

The manifest stores:
- team identity
- repo identity
- shared objective
- lead agent
- member roster
- task cards
- status

## Example

```toml
version = 1
team_id = "team-alpha"
repo_id = "bat-c6342dcc61cb"
objective = "Ship a safe parallel implementation"
status = "planning"

[lead]
member_id = "lead"
role = "lead"
runtime = "claude"
model = "sonnet"
execution_mode = "external_slots"
read_only = true
write_scope = []
context_packs = ["architecture"]
skills = ["planning-with-files"]

[[members]]
member_id = "worker-a"
role = "implementer"
runtime = "codex"
execution_mode = "external_slots"
slot_id = "slot-1"
branch_name = "awo/worker-a"
read_only = false
write_scope = ["crates/awo-core/src/runtime.rs"]
context_packs = ["architecture"]
skills = ["rust-skills"]
notes = "Owns runtime changes"

[[tasks]]
task_id = "task-1"
title = "Implement running-session persistence"
summary = "Persist the session before one-shot completion."
owner_id = "worker-a"
runtime = "codex"
slot_id = "slot-1"
branch_name = "awo/worker-a"
read_only = false
write_scope = ["crates/awo-core/src/runtime.rs"]
deliverable = "A tested runtime/session patch"
verification = ["cargo test"]
depends_on = []
state = "todo"
```

## Execution Modes

- `external_slots`
  - the portable default
  - one write-capable worker per worktree slot
- `inline_subagents`
  - runtime-native subagents inside one parent session
- `multi_session_team`
  - runtime-native teammate sessions managed by that runtime

## Task Card Rules

Every task card should define:
- owner
- write scope
- deliverable
- verification
- dependencies

That keeps parallel work explicit and makes merge/review safer.

## Team Lifecycle: Archive and Reset

### Archive

`awo team archive <team_id>` transitions a team to the `archived` status.

**Safety requirements:**
- All tasks must be in a terminal state (`done` or `blocked`).
- Tasks in `todo`, `in_progress`, or `review` block archival.
- A team that is already archived cannot be archived again.

Archive is an explicit operator action. It signals that the team's work is complete (or intentionally abandoned where blocked) and the manifest should no longer be actively used.

An archived manifest is preserved on disk for auditability. Use `team reset` followed by re-planning to revive an archived team.

### Reset

`awo team reset <team_id> [--force]` returns a team to the `planning` state.

**What reset does:**
- Sets all task states back to `todo`.
- Clears all slot and branch bindings on tasks, members, and the lead.
- Sets team status to `planning`.

**Safety:**
- Without `--force`, reset previews what will be discarded (non-todo tasks, bound members) and asks the operator to confirm.
- With `--force`, reset proceeds immediately.

Reset makes alpha-stage cleanup practical: when a team run goes sideways the operator can wipe slate and re-plan without deleting the team definition, members, or task structure.

**Important:** Reset does not release worktree slots or cancel running sessions. The operator should handle those independently via `awo slot release` and `awo session cancel` before or after reset.

### Status Values

| Status     | Meaning                                           |
|------------|---------------------------------------------------|
| `planning` | No tasks started (initial or post-reset)          |
| `running`  | Some tasks in progress                            |
| `blocked`  | Some tasks blocked                                |
| `complete` | All tasks done                                    |
| `archived` | Operator has explicitly archived the team         |

## Recommendation

Use the single-window controller as the product default, with background agents and structured ownership. Let runtime-native panes or team UIs remain optional views, not the core organizing principle.
