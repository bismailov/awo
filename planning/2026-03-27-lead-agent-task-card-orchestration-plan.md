# Lead-Agent Task-Card Orchestration Plan (March 27, 2026)

## Purpose

Define how Awo Console should evolve from a strong local broker into a **TUI-first multi-agent orchestration console where one live CLI agent acts as the current lead orchestrator**.

This plan extends the local-first finalization roadmap in
`planning/2026-03-27-master-finalization-plan.md`.

It does **not** replace the broker/reliability/platform work already underway. Instead, it explains how the product should use those foundations to support the real operator workflow:

1. human gives a broad goal
2. lead agent plans
3. lead agent creates task cards
4. Awo allocates slots/worktrees and launches worker sessions
5. worker outputs flow back into Awo
6. lead agent reviews, consolidates, and closes the loop

## Product Stance

### Awo's Role
- Awo is the **broker, memory, control plane, and safety layer**
- Awo is **not** the primary "thinking brain"
- Awo owns repos, slots, sessions, task-card state, context, reports, and lifecycle

### Lead Agent's Role
- One current session is the **lead orchestrator**
- The lead agent talks to the human
- The lead agent plans and revises the work
- The lead agent creates, delegates, reviews, and consolidates task cards

### Worker Agent's Role
- Worker sessions execute task cards in isolated slots
- Workers return result summaries, verification notes, and handoff context

### TUI's Role
- The TUI is the main operator cockpit
- The TUI should let the human inspect, steer, pause, replace, and recover the orchestration workflow

## Terms To Standardize

### Use `task card`
Use `task card` as the durable product term.

Reason:
- it matches the current `team` + `task` domain model
- it is close to the existing implementation
- it avoids inventing a parallel "job card" concept that overlaps with the current data model

### Use `lead session`
Use `lead session` for the currently designated orchestrator session for a team.

This is intentionally dynamic rather than tied forever to one runtime or one member.

## Product Decisions Locked For This Plan

### 1. Lead Agent Is Replaceable
- The lead session must be replaceable.
- Token exhaustion, timeout, cost ceilings, or operator choice must not strand the team.
- A team should be able to nominate a different lead session or promote a worker to lead.

### 2. Lead Agent May Also Execute Task Cards
- The lead is not a pure manager-only role.
- The lead may also own and execute a task card when appropriate.
- The product should distinguish:
  - lead responsibilities
  - worker responsibilities
  - without forbidding the same agent from doing both

### 3. Worktree Deletion Must Be Explicitly Supported
- Awo should support retaining worktrees for reuse and explicitly deleting them when desired.
- Space is not treated as scarce by default, so retention can be conservative.
- The product still needs:
  - delete/prune controls
  - retention visibility
  - safe cleanup flows

### 4. Repo Clone Location Must Be Configurable
- Clone and workspace roots must not be hardcoded to platform app-support locations alone.
- Operators should be able to configure:
  - clone root
  - worktree root strategy
  - optional per-repo overrides

### 5. Capacity/Usage Monitoring Is Best-Effort And Runtime-Aware
- Awo should monitor capacity/usage only where runtimes expose it credibly.
- The product should support a capability matrix:
  - `supported`
  - `partially_supported`
  - `unsupported`
- If a CLI runtime does not expose token/budget/session stats, Awo should surface `unknown`, not fake precision.

### 6. Token Exhaustion And Timeout Are First-Class Operational States
- Lead and worker sessions need explicit recovery when they:
  - run out of tokens
  - hit runtime budget limits
  - hit provider-enforced session lifetimes
  - hit local timeouts or operator-configured limits
- These should become normal orchestration states, not ad hoc failures.

## What Exists Already

### Strong Foundations Already Present
- repos, slots, sessions, teams, and tasks already exist
- task start already acquires slots and launches sessions
- delegation already exists
- context discovery and attachment already exist
- reports and teardown already exist
- the TUI already has a Team Dashboard and task/member actions

### This Means
Awo already handles much of the **plumbing** of the workflow.

What is still missing is the **lead-agent orchestration layer** and the **consolidation loop**.

## Gap Analysis

### Gap 1: No First-Class Lead Session
Today the team model has a lead member, but not a durable operational concept of:
- current lead session
- lead handoff
- lead replacement
- lead-capacity failure recovery

### Gap 2: Planning Is Not Yet A Native Workflow
The product can execute tasks, but it does not yet treat:
- planning
- plan approval
- plan-to-task-card conversion
as a first-class operator flow.

### Gap 3: Output Ingestion Is Too Weak
Workers produce outputs, but Awo does not yet have a rich, structured model for:
- result summaries
- handoff notes
- review readiness
- imported/copied worker summaries

### Gap 4: Consolidation Is Not A First-Class Product Workflow
Awo can launch and track work, but it does not yet provide a strong built-in loop for:
- review completed worktrees
- accept/reject/rework
- consolidate changes into the main repo
- decide whether to delete or retain the slot afterward

### Gap 5: Capacity And Runtime-Limit Recovery Is Too Implicit
If an agent runs out of tokens or times out, the operator story is not yet explicit enough.

### Gap 6: Storage Roots Need Operator Control
Repo clone location and workspace placement need to be configurable in a first-class way.

## Target End State

When this plan is complete, the workflow should look like this:

1. User opens Awo in the TUI.
2. A designated lead session is visible for the active team.
3. The user gives the lead a broad goal.
4. The lead drafts a plan and proposes task cards.
5. The user accepts/refines the task cards in the TUI.
6. Awo allocates slots and starts the worker sessions.
7. Worker outputs are collected back into task-card review state.
8. The lead reviews results and decides:
   - done
   - rework
   - supersede
   - consolidate
9. Awo helps the operator inspect diffs/logs and consolidate accepted work.
10. Slots are either retained for warm reuse or explicitly deleted through visible controls.
11. If the lead or a worker hits token/time capacity, the TUI shows the state and supports reassignment or lead replacement.

## Major Workstreams

### Workstream A: Lead Session Model
Add a first-class operational lead concept.

Deliverables:
- current lead session per team
- lead-session metadata and lifecycle
- lead handoff / lead replacement
- operator-visible lead state in TUI
- support for a lead also owning task cards

### Workstream B: Planning To Task-Card Flow
Make planning a first-class workflow.

Deliverables:
- planning session state for the team
- plan items that can become task cards
- task-card generation/approval flow in TUI
- support for structured task-card fields plus freeform notes

### Workstream C: Output Ingestion And Review Queue
Make worker output durable and reviewable.

Deliverables:
- task-card handoff/result fields
- review-ready state
- structured result summary
- copied/imported worker summary support
- review queue in TUI

### Workstream D: Capacity And Session-Limit Handling
Make runtime exhaustion explicit.

Deliverables:
- capacity state model
- session end reasons beyond just `completed` / `failed`
- lead replacement workflow
- worker reassignment / retry flows
- runtime capability matrix for usage/limits reporting

### Workstream E: Consolidation Workflow
Make integration and cleanup a first-class product workflow.

Deliverables:
- inspect diff/logs from completed task cards
- accept/rework/supersede actions
- explicit consolidation queue
- post-consolidation slot decision:
  - retain for reuse
  - release only
  - delete/prune worktree

### Workstream F: Configurable Storage Roots
Make local storage placement operator-controlled.

Deliverables:
- configurable clone root
- configurable default worktree root
- optional per-repo overrides
- TUI/CLI visibility into current storage roots

## Recommended Delivery Order

This sequence keeps the orchestration vision grounded in the already-running finalization work.

### Phase 1: Terminology And Lead Model
- standardize on `task card`
- add first-class lead session model
- allow lead replacement
- allow lead-as-worker behavior

### Phase 2: Storage And Retention Controls
- configurable clone/workspace roots
- explicit delete/prune worktree controls
- TUI visibility for retained vs deletable worktrees

### Phase 3: Planning To Task-Card Flow
- plan capture
- plan item approval
- task-card generation
- lead-driven assignment in the TUI

### Phase 4: Output Ingestion And Review Queue
- worker summary ingestion
- review-ready queue
- TUI review panel

### Phase 5: Capacity And Recovery
- explicit token/budget/time-limit states
- lead replacement / worker reassignment
- capability-aware usage display

### Phase 6: Consolidation Cockpit
- inspect diffs/logs by task card
- accept/rework/supersede
- consolidate into main repo
- decide whether to reuse or delete worktrees

## TUI Shape

The TUI should eventually expose five orchestration panes:

### 1. Mission
- objective
- current lead session
- plan status
- blockers

### 2. Task Cards
- todo
- in_progress
- review
- blocked
- done
- later: cancelled / superseded

### 3. Workers And Slots
- session status
- runtime/model
- slot/worktree path
- reuse state
- dirty/stale/healthy

### 4. Review Queue
- worker summaries
- verification result
- handoff notes
- open diff / logs / slot

### 5. Consolidation Queue
- accepted task cards
- pending integration
- retain/delete decision

## Interaction Model For Capacity And Failure

### Lead Session Failure
If the lead session:
- runs out of tokens
- hits provider/session timeout
- exits unexpectedly
- becomes operator-abandoned

the team should not be stranded.

Required recovery actions:
- promote another member/session to lead
- restart the same lead runtime in a fresh or retained slot
- preserve the current plan and task-card state

### Worker Session Failure
If a worker:
- runs out of tokens
- times out
- exits partially complete

the operator/lead should be able to:
- retry the same task card
- delegate a replacement worker
- mark blocked
- supersede the task card later

## Monitoring And Usage Visibility

### Supported Where Possible
Awo should show usage/capacity data when runtimes expose it reliably, for example:
- token usage
- budget/cost estimate
- session age
- timeout horizon
- provider/session quota hints

### Honest Fallback
If a runtime does not expose these metrics, Awo should show:
- `unknown`
- `not exposed by runtime`
- `unsupported`

Never fabricate token or budget precision.

## Teardown In This Model

Teardown remains the **operational cleanup command**.

In plain terms:
- cancel cancellable sessions
- release bound slots
- reconcile the team
- reset the team back to planning

Teardown is not delete.
Teardown is "stop the active run and clean the workspace state so the team can be planned again."

Delete remains the step after teardown if the team definition itself should disappear.

## Relationship To Existing Finalization Plan

This plan depends on the earlier finalization work:
- Milestone 1 broker hardening
- Milestone 2 reliability/test depth
- Milestone 3 immutable-task recovery
- Milestone 5 middleware enrichment

It primarily sharpens **Milestone 6: Orchestration Intelligence** into an operator-facing product story.

## Definition Of Done For This Orchestration Layer

This orchestration direction counts as successful when:

1. a lead session can be designated and replaced
2. the lead can also execute task cards when appropriate
3. plans can become task cards through a TUI-first workflow
4. worker outputs flow back into reviewable task-card state
5. token exhaustion/time-limit cases are visible and recoverable
6. the lead can review and consolidate completed work
7. clone/workspace roots are configurable
8. worktree retention and deletion are explicit operator controls
9. the TUI feels like the main orchestration cockpit rather than a status monitor
