# Task Plan: Lead-Agent Task-Card Orchestration Implementation

## Goal
Extend the current orchestration slice with three operator-facing follow-ons: task-card model overrides, configurable clone/worktree roots, and bulk pruning of released retained slots, while preserving the verified lead/output/review foundations from Job Cards X, Y, and Z.

## Current Phase
Phase 8

## Phases

### Phase 1: Product Direction Audit
- [x] Re-read `project.md` and the current master finalization roadmap
- [x] Reconcile the user's desired workflow with the existing team/task/slot model
- [x] Check how teardown is currently defined so the plan uses existing semantics correctly
- **Status:** complete

### Phase 2: Orchestration Model Definition
- [x] Define Awo as the broker/control plane and the lead session as the current orchestrator
- [x] Standardize on `task card` terminology
- [x] Fold in the new product constraints: replaceable lead, lead-as-worker, configurable roots, explicit worktree deletion, capacity awareness
- **Status:** complete

### Phase 3: Planning Package Authoring
- [x] Write a dedicated orchestration plan document
- [x] Create implementation job cards for the next orchestration slices
- [x] Link the new package back into the master finalization plan
- **Status:** complete

### Phase 4: Implementation
- [x] Add durable `current_lead_member_id` and `current_lead_session_id` support to `TeamManifest`
- [x] Add `AppCore` support for current-lead replacement
- [x] Add command-backed current-lead replacement
- [x] Bind lead-owned sessions when the current lead starts a task
- [x] Clear stale lead-session state during reconciliation/reset
- [x] Surface current-lead state in CLI/TUI output
- [x] Surface current-lead session health and handoff-needed hints in the TUI
- [x] Add TUI operator control to promote the selected member to current lead
- [x] Standardize operator-facing `task card` wording in TUI/docs
- **Status:** complete

### Phase 5: Verification, Trace, And Handoff
- [x] Update `task_plan.md`
- [x] Update `findings.md`
- [x] Update `progress.md`
- [x] Add focused core/TUI tests for the new lead behavior
- [x] Run formatting, linting, targeted tests, and full workspace tests
- [ ] Prepare the final handoff summary
- **Status:** in progress

### Phase 6: Job Card Y Design And Implementation
- [x] Add session end-reason and capacity-state models to runtime/store/snapshot
- [x] Add structured task-card output ingestion fields for result session id and handoff note
- [x] Update reconciliation to populate review-ready task-card data from terminal sessions
- [x] Surface end reasons and capacity state in CLI/TUI output
- [x] Add focused tests for timeout, cancellation, output ingestion, and operator rendering
- [x] Update `task_plan.md`
- [x] Update `findings.md`
- [x] Update `progress.md`
- [x] Run formatting, linting, targeted tests, and full workspace tests
- [ ] Prepare the final handoff summary
- **Status:** in progress

### Phase 7: Job Card Z Review Closeout And Retention Controls
- [x] Re-read Job Card Z and inspect current review/consolidation and retention code paths
- [x] Add command-backed task-card accept/rework actions
- [x] Add explicit slot delete support for released worktrees
- [x] Surface review/consolidation counts and cleanup actions in the TUI Team Dashboard
- [x] Add focused core and TUI tests for review closeout and slot deletion
- [x] Update operator docs and manual scenarios for release-vs-delete choices
- [x] Research official provider and MCP usage/capacity interfaces from primary sources
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 8: Model Routing, Storage Roots, And Prune Controls
- [x] Add task-card model overrides across core, CLI, TUI, and MCP
- [x] Let TUI team creation set explicit lead runtime/model
- [x] Add configurable clone/worktree roots through settings and env overrides
- [x] Surface default worktree-root visibility in the TUI/docs
- [x] Add `slot prune` for bulk cleanup of released retained slots
- [x] Add focused tests for task-card model routing, storage-root precedence, and prune behavior
- [x] Update operator docs/manual scenarios for model overrides, configurable roots, and prune
- [x] Run formatting, linting, and full workspace tests
- [ ] Prepare the final handoff summary
- **Status:** in progress

## Key Questions
1. What is the smallest durable lead-session model that keeps older team manifests compatible?
2. How should Awo represent “replace the lead” without rewriting the existing structural lead profile?
3. Where should the current-lead/session state be visible so the TUI can become the operator cockpit later?
4. What is the smallest honest capacity model we can ship before any runtime exposes true token telemetry?
5. Which task-card result fields are enough to power review without turning logs into the primary UX?
6. How should Awo express “retain this worktree for reuse” versus “delete it now” without hiding cleanup inside teardown/reset?

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Keep the durable structural `lead` member and add separate current-lead metadata on top | This preserves existing manifests and avoids a wider schema rewrite |
| Track current lead with `current_lead_member_id` and `current_lead_session_id` | This is enough to support lead replacement and lead-session visibility in the first slice |
| Make the current lead replaceable through a real command plus CLI/TUI controls | Lead churn is an operator reality and the operator surfaces should stay command-backed |
| Let reconciliation clear stale current-lead sessions | The current lead pointer should not keep pointing at dead/terminal sessions |
| Keep task ownership semantics unchanged so the lead can already work as a task owner | This minimizes implementation risk while still enabling lead-as-worker behavior |
| Treat failed/missing lead sessions as handoff-needed operator conditions | We cannot reliably detect token exhaustion yet, but we can surface the practical recovery action honestly |
| Model Job Card Y capacity state as `unsupported`, `unknown`, `timed_out`, or `exhausted` | This gives operators actionable truth without faking live token telemetry |
| Persist task-card `result_session_id` and `handoff_note` instead of inventing a separate review object | That is enough to power review-ready task cards and future review queues with minimal schema churn |
| Detect token exhaustion only as a best-effort log heuristic | Current adapters do not provide universal structured token telemetry, so exact detection would be dishonest |
| Model review closeout with semantic `accept` and `rework` commands instead of generic TUI-only state edits | This keeps operator intent explicit and leaves room for richer closeout semantics later |
| Keep retention explicit: release and delete are separate actions | Warm-slot reuse should be visible and operator-controlled rather than an invisible side effect |
| Limit `slot delete` to released or missing slots | This avoids destructive cleanup of active work and preserves release-vs-delete as a deliberate two-step choice |
| Treat provider token usage as adapter-specific and MCP usage as non-standard | OpenAI, Anthropic, and Gemini expose official API-layer usage signals, but MCP does not define a universal token-usage telemetry field today |
| Add task-card `model` separately from member defaults | Savvy operators need per-task cost/performance steering without mutating the assigned member profile |
| Make clone/worktree roots globally configurable via settings/env before adding richer UI config management | This unlocks real operator control now without expanding into a larger settings UX slice |
| Add `slot prune` as bulk cleanup rather than hiding deletion inside release/teardown | Operators should be able to clear retained warm workspace inventory intentionally and at repo scope |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| Borrow-after-partial-move in `snapshot.rs` while deriving current-lead summary fields | 1 | Capture current-lead fields before moving owned manifest fields |
| Existing TUI member-removal test broke after a promoted member became non-removable current lead | 1 | Update the test to hand lead back before removing the worker |
| Command-backed lead replacement required extra enum/dispatch coverage | 1 | Added a dedicated command/event and updated dispatch roundtrip fixtures |
| `store.rs` migration patch missed the current schema tail on the first try | 1 | Re-read the exact migration block and patched the end-reason migration surgically |
| `cargo test` exposed widespread fixture fallout from new `TaskCard` and `SessionRecord` fields | 1 | Updated all direct initializers and added targeted Job Card Y assertions while touching them |
| Verification expectation drifted after making review summaries more explicit | 1 | Updated the tests to match the clearer “Ready for review” wording and schema version bump |
| New Team Dashboard closeout actions hit borrow-checker conflicts in `action_router.rs` | 1 | Clone selected slot/session ids before mutating TUI state so the new release/log flows stay ownership-safe |
| `std::env::set_var`/`remove_var` are unsafe in this toolchain and the workspace forbids unsafe code | 1 | Refactored storage-root precedence into a pure helper and tested that instead of mutating process env in tests |
| New prune test initially reused the same warm slot instead of producing multiple retained slots | 1 | Adjusted the test to hold two active warm slots before release so prune verifies the true multi-slot case |

## Notes
- The orchestration planning package created earlier in this line of work is:
  - `planning/2026-03-27-lead-agent-task-card-orchestration-plan.md`
  - `planning/job-card-X-lead-session-and-task-card-model.md`
  - `planning/job-card-Y-output-ingestion-and-capacity-state.md`
  - `planning/job-card-Z-consolidation-cockpit-and-retention-controls.md`
  - `planning/job-card-AA-configurable-storage-roots.md`
- This implementation slice completed the core of Job Card X’s first step:
  - replaceable current lead metadata
  - current lead promotion
  - current lead session tracking
  - command-backed lead replacement
  - CLI/TUI visibility
  - current lead handoff-needed attention strings for failed/missing sessions
  - TUI promotion control
  - task-card terminology cleanup in the operator surfaces and docs
