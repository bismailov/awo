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

The first implementation lives in [team.rs](../crates/awo-core/src/team.rs).

The manifest stores:
- team identity
- repo identity
- shared objective
- durable lead profile
- current lead session/member state
- plan items
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

current_lead_member_id = "lead"

[[plan_items]]
plan_id = "plan-1"
title = "Break out runtime persistence work"
summary = "Convert the high-level implementation idea into one executable task card."
owner_id = "worker-a"
runtime = "codex"
model = "gpt-5.4-mini"
read_only = false
write_scope = ["crates/awo-core/src/runtime.rs"]
deliverable = "A concrete task card ready for execution"
verification = ["cargo test"]
depends_on = []
state = "approved"

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
model = "gpt-5.4-mini"
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

## Plan Item Rules

Plan items sit one step above task cards. They are the durable planning layer a lead uses before committing to executable work.

Every plan item should define:
- title
- summary
- optional owner intent
- optional runtime/model intent
- write scope
- verification intent
- optional notes

Plan-item states:
- `draft`
- `approved`
- `generated`

Generation rules:
- plan items are immutable records; they are not edited into task cards
- a plan item must be `approved` before generation
- generation creates a new task card and stores `generated_task_id` on the plan item
- a generated plan item preserves planning history even after the task card changes state later

Every task card should define:
- owner
- runtime/model intent when it differs from the owner's default
- write scope
- deliverable
- verification
- dependencies

Task cards may also retain bounded review data after execution:
- result summary
- result session id
- handoff note
- output log path
- superseded-by task id when the card is retired in favor of a replacement

Task-card states:
- `todo`
- `in_progress`
- `review`
- `done`
- `blocked`
- `cancelled`
- `superseded`

Immutable recovery uses explicit state transitions instead of edit/delete:
- `cancelled` means the task card is intentionally retired without a replacement
- `superseded` means the task card is intentionally retired in favor of another task card

That keeps parallel work explicit and makes merge/review safer.

## Current Lead Rules

- The manifest keeps a durable structural `lead` profile for defaults and team identity.
- The operator may replace the **current lead** with another member when the active orchestrator runs out of tokens, times out, or otherwise needs handoff.
- The current lead may also own executable task cards directly.
- Replacing the current lead does not rewrite task history; it only changes who is considered the active orchestrator now.
- If the current lead session fails, is cancelled, or goes missing, the TUI should surface that as a handoff-needed operator condition rather than silently pretending orchestration is still active.

## Team Lifecycle: Archive and Reset

### Archive

`awo team archive <team_id>` transitions a team to the `archived` status.

**Safety requirements:**
- All tasks must be in a terminal state (`done`, `blocked`, `cancelled`, or `superseded`).
- Tasks in `todo`, `in_progress`, or `review` block archival.
- A team that is already archived cannot be archived again.
- Bound slots that are still active block archival.
- Non-terminal sessions attached to bound slots block archival.

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

### Teardown

`awo team teardown <team_id> [--force]` is the operational cleanup path.

**What teardown does:**
- Syncs runtime state and reconciles the manifest first.
- Cancels any cancellable sessions still attached to the team's bound slots.
- Releases active bound slots.
- Resets the team back to `planning`.

**Safety:**
- Without `--force`, teardown previews what it will cancel, release, and reset.
- Dirty slots block teardown.
- Running one-shot sessions block teardown because they still cannot be interrupted safely.
- Teardown is intentionally honest about blockers instead of pretending the team is clean.

### Delete

`awo team delete <team_id>` removes the manifest file once the team no longer references live workspace state.

**Safety:**
- Bound slots must already be gone.
- Attached non-terminal sessions must already be gone.
- The intended path is usually `team teardown` first, then `team delete` if the manifest itself is no longer needed.

### Status Values

| Status     | Meaning                                           |
|------------|---------------------------------------------------|
| `planning` | No tasks started (initial or post-reset)          |
| `running`  | Some tasks in progress                            |
| `blocked`  | Some tasks blocked                                |
| `complete` | All remaining task cards are closed               |
| `archived` | Operator has explicitly archived the team         |

## Recommendation

Use the single-window controller as the product default, with background agents and structured ownership. Let runtime-native panes or team UIs remain optional views, not the core organizing principle.
