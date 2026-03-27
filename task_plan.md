# Task Plan: Lead-Agent Task-Card Orchestration Implementation

## Goal
Plan the next implementation wave after the current orchestration checkpoint, with the immediate focus on immutable task recovery, review diff/consolidation depth, planning-to-task-card flow, and runtime-usage recovery improvements.

## Current Phase
Phase 17

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
- [x] Prepare the final handoff summary
- **Status:** complete

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
- [x] Prepare the final handoff summary
- **Status:** complete

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
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 9: Next Iterations Planning
- [x] Re-read the current orchestration checkpoint and master roadmap
- [x] Identify the highest-value remaining local-product gaps
- [x] Convert those gaps into an implementation order
- [x] Write a focused next-iterations plan document
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 10: Immutable Recovery And Review Diff Follow-Through
- [x] Add immutable task recovery states and manifest linkage for superseded task cards
- [x] Add command-backed `team.task.cancel` and `team.task.supersede`
- [x] Reject cancel/supersede when a task card still has live sessions bound to its slot
- [x] Surface cancel/supersede in CLI and TUI
- [x] Preserve closed-task cleanup visibility for cancelled/superseded task cards
- [x] Add bounded `review.diff` and expose it through CLI and the TUI
- [x] Update focused docs and manual scenarios for immutable recovery and diff inspection
- [x] Run formatting, clippy, and the full test suite
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 11: Planning-To-Task-Card Flow
- [x] Add durable plan-item schema support to `TeamManifest`
- [x] Add command-backed `team.plan.add`, `team.plan.approve`, and `team.plan.generate`
- [x] Surface plan items in CLI output and `team show`
- [x] Add Team Dashboard plan-pane support with add, approve, and generate actions
- [x] Add focused manifest, TUI, dispatch, and operator-flow tests for plan-item workflows
- [x] Update team-manifest, control-surface, and manual-test docs for plan items
- [x] Run formatting, clippy, and the full test suite
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 12: Consolidation Cockpit Depth And Runtime Recovery Truth
- [x] Make task-card review, cleanup, and history queue roles explicit in the Team Dashboard
- [x] Add quick navigation between actionable review and cleanup task cards
- [x] Extend team reports and text output with queue-aware sections and counts
- [x] Add runtime usage notes and recovery hints to session snapshots and operator surfaces
- [x] Extend runtime capability output with honest budget and session-lifetime support signals
- [x] Add focused tests for queue navigation, report sections, and recovery guidance rendering
- [x] Update docs and manual scenarios for the richer consolidation cockpit and recovery language
- [x] Run formatting, clippy, and the full test suite
- [x] Prepare the final handoff summary
- **Status:** complete

### Phase 13: Audit, Plan Refresh, And Release-Readiness Review
- [x] Re-read the current architecture and roadmap docs before auditing
- [x] Audit the codebase for command-layer parity, product drift, and documentation hygiene
- [x] Fix audit findings that are small, concrete, and safe to resolve in this slice
- [x] Write a dated audit report with strengths, residual risks, and recommendations
- [x] Refresh the development and finalization plans to reflect the actual current checkpoint
- [x] Run formatting, clippy, and the full test suite
- [x] Commit and push the checkpoint
- **Status:** complete

### Phase 14: Post-Audit Next-Sessions Planning
- [x] Re-read the current next-iterations plan and the March 28 audit report
- [x] Convert the audit risks into a concrete session-by-session execution sequence
- [x] Capture recommended worktree/delegation lanes for the next implementation wave
- [x] Update the planning trace with the new continuation plan
- **Status:** complete

### Phase 15: TUI Responsiveness And Event-Bus Hardening
- [x] Move periodic snapshot refresh work off the TUI render loop and into bounded background refreshes
- [x] Preserve Team Dashboard selection across background refresh application
- [x] Split dialog/form/confirm workflow handling out of `crates/awo-app/src/tui/action_router.rs`
- [x] Harden `EventBus` mutex/condvar handling so poisoned synchronization primitives recover instead of panicking immediately
- [x] Add focused regression coverage for snapshot refresh selection preservation and event-bus poison recovery
- [x] Run formatting, clippy, targeted tests, and the full workspace test suite
- **Status:** complete

### Phase 16: Broker Hardening Follow-Through
- [x] Strengthen daemon health probing so “healthy” requires a successful RPC response, not only a socket connect
- [x] Add degraded-state coverage for sockets that accept connections but never answer RPC health checks
- [x] Add more broker-mode visibility in operator surfaces where it materially helps
- [x] Re-run formatting, clippy, targeted daemon/handler tests, and the full workspace test suite
- [ ] Deepen live-client event delivery beyond poll/long-poll
- **Status:** in_progress

### Phase 17: Hardening And CI Safety
- [x] Replace the remaining high-risk production panic paths in JSON output handling
- [x] Add `cargo audit` and `cargo deny` steps to GitHub Actions
- [x] Add a baseline `deny.toml`
- [x] Validate `cargo audit` locally and record dependency findings
- [x] Record the temporary policy for `RUSTSEC-2017-0008` in `deny.toml`
- [ ] Validate `cargo deny` locally
- **Status:** in_progress

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
| Prioritize immutable task recovery before native planning flow | Recovery is the bigger operational gap and should solidify the task model before more orchestration layers depend on it |
| Put diff/consolidation depth ahead of planning-to-task-card UX | The review loop is closer to operator-critical readiness than planning ergonomics right now |
| Model immutable recovery with `cancelled` and `superseded` task-card states plus `superseded_by_task_id` | This preserves history without adding edit/delete semantics |
| Treat `complete` as “all remaining task cards are closed” rather than strictly “all done” | Cancelled and superseded cards should not keep a finished team permanently out of `complete` |
| Keep review diff bounded and text-first | A status/stat/patch summary is enough for the TUI cockpit without turning Awo into a full terminal pager |
| Add a lightweight `plan item` layer instead of overloading task cards for both planning and execution | This keeps lead planning durable without losing the executable task-card model |
| Make task-card generation command-backed rather than a TUI-only convenience | CLI, TUI, and future MCP/operator surfaces should share the same planning mutation path |
| Put planning directly into the Team Dashboard instead of adding a second planning screen | The dashboard already has the right operator context and keeps the workflow single-surface |
| Show queue roles explicitly instead of forcing operators to infer them from raw task state | The review/consolidation cockpit becomes faster to scan and easier to operate |
| Keep runtime usage messaging advisory and capability-based | We can provide useful operator guidance today without pretending to have universal token telemetry |
| Add direct navigation between actionable task cards | The cockpit should optimize for “what needs my attention now,” not only generic list browsing |
| Treat command-surface parity as a release-level architecture objective, not a background cleanup | It directly affects daemon/direct consistency and the credibility of the core mutation rule |
| Keep roadmap docs aligned with the real checkpoint after major slices land | Drift now hurts contributor onboarding more than feature ideation helps |
| Use a session-by-session continuation plan after major audits, not just a broad roadmap | The remaining work is now mostly finish-line sequencing rather than feature discovery |

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
| The backlog was broad enough to encourage jumping between unrelated concerns | 1 | Wrote a narrower next-iterations plan that orders the work into four concrete local-product slices |
| Bulk fixture update for the new task-card recovery field added duplicate initializers in two operator-surface files | 1 | Re-ran a compile-guided sweep and cleaned the duplicated `superseded_by_task_id` assignments by hand |
| First full `cargo test` pass after the new CLI operator-flow coverage failed because the test did not provide an owner for generated task-card creation | 1 | Added `--owner-id lead` to the operator-flow test so the generation path matches the command contract |
| The first actionable-task navigation test attached a cleanup candidate to a fake slot id and reconciliation stripped the binding immediately | 1 | Acquired a real slot in the test and used that id so the cleanup candidate remains actionable |
| The new queue-navigation helper initially tangled borrow scopes and empty-list edge cases | 1 | Reworked the helper to build an actionable id list first, then mutate the selected index afterward |
| `cargo test` still emits noisy `fatal: cannot change to ...` and `r2d2 unable to open database file` lines from intentional negative-path coverage | 1 | Verified the suites still finish green and left those existing diagnostics untouched in this slice |
| The master finalization plan had machine-specific absolute links checked into the repo | 1 | Replaced them with portable relative links during the audit pass |
| Several operator flows were still bypassing the dispatcher even when matching public commands already existed | 1 | Routed the easy cases back through dispatch and recorded the remaining command-surface gaps in the roadmap |
| The older “next iterations” plan had drifted behind the actual checkpoint and still described already-finished work as upcoming | 1 | Wrote a fresh post-audit next-sessions plan centered on the current residual risks instead of the older backlog |
| `TeamMemberUpdate` used nested `Option<Option<_>>` fields that lost clear-intent when serialized through daemon/JSON transport | 1 | Replaced those fields with explicit clear flags plus single-layer optional payloads so direct and daemon-backed updates behave identically |
| A stale `cargo test` process from an older session kept the package/build lock and made the fresh verification pass look hung | 1 | Located and terminated the orphaned cargo/test child processes, then reran the full suite |
| `handlers.rs` was still using `convert_case` without a workspace dependency, which broke the first combined clippy/test pass | 1 | Replaced the dynamic case conversion with a tiny explicit daemon-issue-code helper |
| The new failing-serializer test helper triggered an unused-variable warning under `-D warnings` | 1 | Renamed the serializer parameter to `_serializer` |
| `cargo-deny` local installation stalled repeatedly on this machine | 1 | Kept the CI wiring and baseline config in place, validated `cargo-audit` locally, and recorded local `cargo-deny` validation as the remaining gap |

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
- The March 28 command-parity sweep is now complete:
  - `team.member.update`
  - `team.member.remove`
  - `team.member.assign_slot`
  - `team.task.bind_slot`
  - CLI and TUI flows now route those mutations through dispatch
  - regression coverage exists in both `crates/awo-core/tests/command_flows.rs` and `crates/awo-app/tests/operator_flows.rs`
- A later external audit was incorporated selectively rather than copied literally:
  - valid remaining issues: TUI-thread snapshot blocking, `action_router.rs` size, CI security checks, and production event-bus panic hardening
  - outdated examples: the specific command-parity gaps it named were already closed by the March 28 parity sweep
