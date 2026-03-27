# Next Iterations Plan (March 27, 2026)

## Purpose

Define the immediate implementation sequence after the current orchestration checkpoint:

- lead session foundation complete
- output ingestion and honest capacity state complete
- review closeout / delete / prune controls complete
- task-card model overrides and configurable storage roots complete

This plan is intentionally narrower than the master roadmap. It answers:

**what should we build next, in what order, and why?**

## Current Product Position

The product now has a credible local orchestration substrate:

- command-backed lead replacement
- task-card execution and delegation
- review-ready task-card state
- accept/rework closeout
- explicit release/delete/prune cleanup
- configurable clone/worktree roots
- task-level runtime/model steering

The biggest remaining gaps in the local operator loop are:

1. immutable task recovery is still incomplete
2. review/consolidation still lacks real diff inspection
3. planning-to-task-card flow is still manual
4. runtime usage/capacity telemetry is still mostly heuristic

## Iteration Order

### Iteration 1: Immutable Task Recovery

**Goal:** make immutable task cards practical under real planning churn.

Deliverables:
- core support for `task cancel`
- core support for `task supersede`
- manifest linkage from superseded task -> replacement task
- TUI actions for cancel and supersede
- CLI commands for cancel and supersede
- reporting/TUI state that clearly distinguishes:
  - `todo`
  - `in_progress`
  - `review`
  - `done`
  - `blocked`
  - `cancelled`
  - `superseded`

Why first:
- this is the biggest missing operational safety valve
- it completes the immutable-task model before more orchestration layers depend on it

Definition of done:
- operators can correct task plans without edit/delete
- TUI and reports preserve task history clearly

### Iteration 2: Review Diff And Consolidation Cockpit V2

**Goal:** make completed task cards reviewable without dropping to manual Git inspection.

Deliverables:
- `review diff` or equivalent command-backed diff helper
- TUI action to inspect the selected task-card diff
- better distinction in the Team Dashboard between:
  - review queue
  - accepted-but-not-cleaned-up queue
  - superseded/cancelled history
- clearer post-review actions:
  - accept
  - rework
  - supersede
  - release
  - delete
  - prune retained inventory

Why second:
- the product already closes tasks semantically, but review still leans too hard on logs
- this is the missing bridge between “task finished” and “I trust the work”

Definition of done:
- a lead can inspect diff/log/result from the TUI and make a closeout decision confidently

### Iteration 3: Planning-To-Task-Card Flow

**Goal:** make the lead-session planning workflow native instead of external/manual.

Deliverables:
- plan-item model for a team
- convert plan items into task cards
- TUI workflow to:
  - capture plan items
  - edit/approve them
  - generate task cards
- preserve freeform notes plus structured routing/scope fields

Why third:
- the current substrate is finally strong enough for this layer
- this is where Awo starts to feel like the orchestration console you actually want, not just a broker plus task list

Definition of done:
- a lead can go from broad objective -> approved plan items -> task cards inside Awo

### Iteration 4: Runtime Usage And Recovery Upgrades

**Goal:** improve the honesty and usefulness of capacity handling.

Deliverables:
- adapter-level capability matrix for:
  - usage telemetry support
  - budget-limit support
  - session-lifetime support
- normalized surface for structured usage where available
- stronger handoff guidance in TUI when:
  - a lead times out
  - a worker exhausts budget
  - usage is unknown
- better operator summaries that separate:
  - explicit timeout
  - explicit cancel
  - likely exhaustion
  - unknown failure

Why fourth:
- it builds on the already-landed end-reason model
- it avoids inventing fake precision before the review/planning loop is stronger

Definition of done:
- Awo gives better recovery guidance without pretending all runtimes expose token stats

## Recommended Sequencing Strategy

1. finish immutable recovery first
2. finish diff/review cockpit second
3. then add native planning flow
4. then deepen runtime usage/capacity truth

This keeps the local operator loop coherent:

- create/plan work
- execute work
- review work
- recover from mistakes and limits

## Explicit Deferrals

These should stay deferred during the next iterations unless they become blocking:

- remote/distributed execution
- full transcript archive ownership
- fully automatic merge orchestration
- large settings-management UI beyond the current storage-root controls
- Windows-only parity work unless it blocks local iteration confidence on the main platform

## Suggested Job Cards

If we continue using the job-card model, the next cards should be:

- `job-card-AB-immutable-task-recovery.md`
- `job-card-AC-review-diff-and-consolidation-v2.md`
- `job-card-AD-planning-to-task-card-flow.md`
- `job-card-AE-runtime-usage-and-recovery-upgrades.md`

## Recommendation

The best next implementation move is:

**Iteration 1: Immutable Task Recovery**

That is the highest-value gap still open in the local orchestration model, and it will make every later planning/review flow safer and more honest.
