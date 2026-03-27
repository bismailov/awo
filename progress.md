# Progress Log

## Session: 2026-03-27

### Implementation Session: Broker Hardening Follow-Through
- **Status:** complete for the current slice
- **Started:** 2026-03-28
- Actions taken:
  - Re-read the March 28 continuation plan and the current daemon/client paths before editing.
  - Hardened daemon health classification so `healthy` now requires a successful bounded JSON-RPC health check instead of only a successful socket connect.
  - Added a new degraded issue state for brokers that accept connections but never answer RPC requests:
    - `RpcUnresponsive`
  - Implemented the health probe using a short `events.poll` roundtrip over the daemon socket.
  - Added regression coverage for:
    - a daemon socket that accepts a connection but never responds
    - existing handler-facing daemon status rendering still behaving correctly
  - Re-ran verification and confirmed the workspace still passes cleanly.
- Files created/modified:
  - `crates/awo-core/src/daemon.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo test -q daemon::tests::daemon_status_reports_degraded_when_rpc_health_check_fails`
  - `cargo test -q daemon_status_payload_reports_health_fields`
  - `cargo test -q daemon_status_text_distinguishes_starting_and_healthy`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - note: full `cargo test` still emits expected negative-path `git` and `r2d2` error lines while finishing green

### Implementation Session: TUI Responsiveness And Event-Bus Hardening
- **Status:** complete for the current slice
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the post-audit continuation plan, and the in-progress TUI responsiveness changes before continuing.
  - Finished the TUI background-refresh slice:
    - initial snapshot still loads synchronously for startup correctness
    - later `snapshot()` refreshes now run on a background worker instead of the main render loop
    - Team Dashboard selection is preserved by team id when refreshed data arrives
  - Reduced TUI router maintenance pressure with a bounded structural split:
    - extracted dialog, form submission, confirm handling, and enter-key detail behavior into `crates/awo-app/src/tui/action_router/dialogs.rs`
    - kept key routing and navigation in the main `action_router.rs`
  - Hardened broker synchronization behavior:
    - replaced production `unwrap()`-based `EventBus` mutex/condvar handling with poison-recovery plus warnings
    - added regression coverage proving the event bus still works after a poisoned mutex
  - Ran a full verification pass and reviewed the resulting structure for regressions.
- Files created/modified:
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/action_router/dialogs.rs`
  - `crates/awo-core/src/events.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -q event_bus_recovers_after_poisoned_mutex`
  - `cargo test -q tui::action_router::tests::repo_add_form_submission_dispatches_background_command`
  - `cargo test`
  - note: full `cargo test` still emits expected negative-path `git` and `r2d2` error lines while finishing green

### Planning Session: Post-Audit Next Sessions
- **Status:** complete
- **Started:** 2026-03-28
- Actions taken:
  - Re-read the audit report, the refreshed development plan, and the older next-iterations plan.
  - Converted the current residual risks into a concrete execution sequence for the next work sessions.
  - Wrote a new post-audit continuation plan with:
    - session order
    - per-session goals
    - target scope
    - likely file ownership
    - definitions of done
    - recommended worktree/delegation lanes
  - Updated the planning trace so future sessions can resume from the audit checkpoint without reconstructing priorities from chat history.
- Files created/modified:
  - `planning/2026-03-28-next-sessions-plan.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - planning-only change; no build/test commands run in this turn

### Audit Session: Overall Quality Review And Roadmap Refresh
- **Status:** complete
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the development plan, the master finalization plan, and the architecture rules before auditing.
  - Ran an audit sweep over:
    - command-layer parity vs direct `AppCore` mutation paths
    - roadmap/doc drift vs the implemented orchestration checkpoint
    - open-source safety in checked-in planning docs
    - verification and test noise
  - Fixed audit findings with small safe scope:
    - replaced machine-specific absolute links in the master finalization plan with relative links
    - routed additional operator flows back through command dispatch where public commands already existed
    - added richer command outcome data for archive/reset/delete and dispatcher-backed teardown handling
  - Wrote a dated audit summary document with residual risks and recommendations.
  - Refreshed the general development plan and the master finalization plan to reflect the actual current checkpoint and remaining objectives.
- Files created/modified:
  - `crates/awo-core/src/app.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/tui.rs`
  - `planning/2026-03-22-development-plan.md`
  - `planning/2026-03-27-master-finalization-plan.md`
  - `planning/2026-03-28-audit-and-quality-review.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - note: full `cargo test` still emits expected negative-path `git` and `r2d2` error lines while passing
  - checkpoint committed and pushed after this audit pass

### Implementation Session: Consolidation Cockpit Depth And Runtime Recovery Truth
- **Status:** complete for the current slice
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the planning trace, and the current review/runtime surfaces before editing.
  - Deepened the consolidation cockpit in the TUI:
    - explicit task-card queue-role labels
    - quick actionable-task navigation between review and cleanup items with `[` and `]`
    - richer task detail text for review and cleanup work
  - Extended command and reporting output:
    - team reports now include queue-aware sections for plan items, review, cleanup, and history
    - text output now shows queue counts and per-task queue roles
  - Deepened runtime recovery/operator truth:
    - added advisory usage notes per runtime
    - added recovery hints based on runtime kind, session status, end reason, and capacity state
    - extended runtime capability output with budget-guardrail and session-lifetime support signals
    - surfaced those hints in both CLI output and TUI task detail
  - Added focused tests for:
    - actionable task-card navigation between review and cleanup queues
    - team report queue sections
    - session recovery guidance logic
    - TUI task detail usage/recovery rendering
  - Updated operator docs and manuals for the richer review/consolidation cockpit and runtime recovery language.
- Files created/modified:
  - `crates/awo-core/src/capabilities.rs`
  - `crates/awo-core/src/snapshot.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/keymap.rs`
  - `docs/product-spec.md`
  - `docs/v1-control-surface.md`
  - `MANUAL_TEST_SCENARIOS.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - Note: the full test run still emits expected negative-path `git` and `r2d2` error lines while finishing green.

### Implementation Session: Planning-To-Task-Card Flow
- **Status:** complete for the current slice
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the planning trace, and the partial planning-to-task-card changes before continuing.
  - Finished the planning-layer schema in the core:
    - `PlanItem`
    - `PlanItemState`
    - manifest validation and reset behavior
    - command-backed add / approve / generate operations
  - Added CLI support for:
    - `awo team plan add`
    - `awo team plan approve`
    - `awo team plan generate`
  - Extended `team show` / text output so plan items are visible alongside task cards.
  - Finished the TUI Team Dashboard planning workflow:
    - new `Plan` dashboard pane
    - `p` add a plan item
    - `P` approve the selected draft plan item
    - `G` generate a task card from the selected approved plan item
    - plan-item detail rendering and selection behavior
  - Added focused tests for:
    - form defaults for plan add / generate
    - Team Dashboard plan-item add / approve / generate flow
    - manifest-level plan-item lifecycle
    - CLI/operator flow showing generated plan items and task cards
    - command roundtrip coverage for the new plan commands
  - Updated docs/manuals for:
    - plan-item manifest shape
    - new CLI commands
    - new TUI keys and planning workflow
- Files created/modified:
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/team/tests.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/dispatch.rs`
  - `crates/awo-core/src/lib.rs`
  - `crates/awo-app/src/cli.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/forms.rs`
  - `crates/awo-app/src/tui/keymap.rs`
  - `crates/awo-app/tests/operator_flows.rs`
  - `docs/team-manifest-spec.md`
  - `docs/v1-control-surface.md`
  - `MANUAL_TEST_SCENARIOS.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`

### Implementation Session: Immutable Task Recovery And Review Diff
- **Status:** complete for the current slice
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, the planning trace, and the next-iterations plan before editing.
  - Added immutable task recovery to the core team model:
    - new task-card states `cancelled` and `superseded`
    - `superseded_by_task_id` linkage
    - manifest validation and status/archive semantics for the new states
  - Added command-backed task recovery operations:
    - `team.task.cancel`
    - `team.task.supersede`
    - live-session guardrails so active bound work cannot be silently retired
  - Surfaced immutable recovery across the operator surfaces:
    - CLI commands and text output
    - TUI confirm/form flows for cancel and supersede
    - Team Dashboard queue/detail updates for cancelled and superseded history
  - Added a bounded review diff helper:
    - core `review.diff`
    - CLI `awo review diff <slot>`
    - TUI diff inspection in the existing log/detail panel on `v`
  - Updated docs/manual scenarios for:
    - immutable task recovery
    - bounded diff inspection
  - Added focused tests for:
    - manifest-level cancel/supersede behavior
    - command-flow recovery and diff behavior
    - TUI cancel/supersede actions
- Files created/modified:
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/team/reconcile.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/commands/review.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/snapshot.rs`
  - `crates/awo-core/src/dispatch.rs`
  - `crates/awo-core/src/team/tests.rs`
  - `crates/awo-core/src/app/tests.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `crates/awo-app/src/cli.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/forms.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/keymap.rs`
  - `crates/awo-mcp/src/server.rs`
  - `docs/team-manifest-spec.md`
  - `docs/v1-control-surface.md`
  - `MANUAL_TEST_SCENARIOS.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Planning Session: Next Iterations After The Current Orchestration Checkpoint
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Re-read the current task plan and the master/orchestration roadmap documents.
  - Identified the highest-value remaining local-product gaps after the current checkpoint:
    - immutable task recovery
    - diff/review cockpit depth
    - planning-to-task-card workflow
    - runtime usage/recovery upgrades
  - Authored a focused follow-on plan that narrows the next work into four concrete iterations instead of a loose backlog.
  - Updated the planning trace to reflect the new immediate sequencing.
- Files created/modified:
  - `planning/2026-03-27-next-iterations-plan.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Task-Card Model Overrides, Storage Roots, And Prune Controls
- **Status:** complete for the current follow-on slice
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, the planning trace, Job Card AA, and the current task-card/storage-root code paths before editing.
  - Added `model` to `TaskCard` and threaded it through:
    - core routing/execution
    - CLI `team task add --model`
    - TUI task-card add form
    - MCP `team_add_task`
    - operator rendering and prompts
  - Extended the TUI team-init form so the structural lead can be created with an explicit runtime/model profile instead of only CLI defaults.
  - Added configurable storage roots in `AppConfig`:
    - `settings.json` keys for `clones_root` and `worktrees_root`
    - env overrides `AWO_CLONES_DIR` and `AWO_WORKTREES_DIR`
    - snapshot/TUI visibility for the default worktree root
  - Changed new repo registration to derive default worktree roots from the configured global worktrees directory instead of the repo parent directory.
  - Added `slot prune` for bulk cleanup of released/missing retained slots and covered it with command-flow tests.
  - Updated manual scenarios and control-surface docs for:
    - task-card model overrides
    - configurable clone/worktree roots
    - prune-based cleanup
- Files created/modified:
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/app/team_ops.rs`
  - `crates/awo-core/src/config.rs`
  - `crates/awo-core/src/repo.rs`
  - `crates/awo-core/src/snapshot.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/slot.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/app.rs`
  - `crates/awo-core/src/app/tests.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `crates/awo-core/src/runtime/tests.rs`
  - `crates/awo-core/src/team/tests.rs`
  - `crates/awo-core/tests/repo_management.rs`
  - `crates/awo-core/tests/fingerprint.rs`
  - `crates/awo-core/src/commands/tests.rs`
  - `crates/awo-core/tests/negative_paths.rs`
  - `crates/awo-core/src/daemon.rs`
  - `crates/awo-app/src/cli.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/forms.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-mcp/src/server.rs`
  - `MANUAL_TEST_SCENARIOS.md`
  - `docs/v1-control-surface.md`
  - `docs/team-manifest-spec.md`
  - `docs/product-spec.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Job Card Z Review Closeout And Retention Controls
- **Status:** complete for the current Job Card Z slice
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, Job Card Z, the Team Dashboard render/router code, and the slot release/reuse behavior before editing.
  - Added command-backed closeout actions:
    - `team.task.accept`
    - `team.task.rework`
    - `slot.delete`
  - Added core support for task-card accept/rework semantics in the manifest layer and command runner.
  - Added explicit slot deletion semantics for released worktrees, keeping release-vs-delete as a deliberate two-step operator choice.
  - Extended the Team Dashboard with:
    - review/consolidation counts in the mission pane
    - richer task-card detail showing queue role, slot status/path, and cleanup hints
    - key-driven closeout actions for accept, rework, open task-card log, release task-card slot, and delete released slot
  - Added focused tests for:
    - manifest-level accept/rework behavior
    - core slot deletion and active-slot rejection
    - TUI accept and rework actions
  - Updated `MANUAL_TEST_SCENARIOS.md` and `docs/v1-control-surface.md` for the new cleanup semantics and TUI review flow.
  - Researched official usage/capacity interfaces:
    - OpenAI, Anthropic, and Gemini expose official API-layer usage/cost signals
    - MCP does not currently provide a standard token-usage telemetry field in the spec pages reviewed
- Files created/modified:
  - `crates/awo-core/src/store.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/slot.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/app.rs`
  - `crates/awo-core/src/team/tests.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `crates/awo-app/src/cli.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/forms.rs`
  - `crates/awo-app/src/tui/keymap.rs`
  - `MANUAL_TEST_SCENARIOS.md`
  - `docs/v1-control-surface.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Job Card Y Output Ingestion And Capacity State
- **Status:** complete for the current Job Card Y slice
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, the planning-with-files guidance, and Job Card Y before touching the runtime/session schema.
  - Audited the current flow from runtime execution into the SQLite store, snapshot summaries, team reconciliation, and the TUI Team Dashboard.
  - Confirmed the current data gaps:
    - `SessionRecord` has no explicit end reason
    - task cards only persist `result_summary` and `output_log_path`
    - timeout failures are not distinguishable from generic runtime failures after persistence
    - capacity visibility is not yet represented in the runtime capability matrix
  - Chose the minimal Job Card Y design:
    - add explicit session end reasons
    - derive honest capacity state from runtime capability support plus terminal end reasons
    - persist `result_session_id` and `handoff_note` on task cards
    - keep review-ready state anchored on `TaskCardState::Review`
  - Added `SessionEndReason` and `SessionCapacityStatus` to the runtime layer and persisted `end_reason` in the SQLite sessions table.
  - Added runtime capability flags for usage and capacity reporting support.
  - Updated reconciliation so completed and failed sessions populate:
    - task-card result summaries
    - result session ids
    - handoff notes
    - output log paths
  - Added best-effort token-exhaustion detection from session logs while keeping unsupported/unknown capacity cases explicit.
  - Surfaced end reasons and capacity state in:
    - snapshot session summaries
    - CLI session/runtime output
    - Team Dashboard mission/task detail views
    - team reports
  - Updated product/spec docs for bounded review data and honest capacity reporting.
  - Added focused tests for:
    - timeout end reasons
    - token exhaustion detection from logs
    - result-session persistence
    - handoff-note ingestion
- Files created/modified:
  - `crates/awo-core/src/runtime.rs`
  - `crates/awo-core/src/runtime/tests.rs`
  - `crates/awo-core/src/capabilities.rs`
  - `crates/awo-core/src/store.rs`
  - `crates/awo-core/src/store/tests.rs`
  - `crates/awo-core/src/snapshot.rs`
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/team/reconcile.rs`
  - `crates/awo-core/src/app/team_ops.rs`
  - `crates/awo-core/src/app/tests.rs`
  - `crates/awo-core/src/commands/session.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/dispatch.rs`
  - `crates/awo-core/src/lib.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-mcp/src/server.rs`
  - `docs/product-spec.md`
  - `docs/team-manifest-spec.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Job Card X Lead Session Foundation
- **Status:** complete for the current Job Card X slice
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, the orchestration job cards, and the current team/session/TUI flows before touching the schema.
  - Added optional `current_lead_member_id` and `current_lead_session_id` to `TeamManifest` while keeping the durable structural `lead` member intact.
  - Added helpers for current-lead lookup, promotion, session binding, and current-lead safety checks.
  - Updated member removal rules so the current lead cannot be removed until leadership is handed back.
  - Added `AppCore::replace_team_lead()` and wired current-lead session binding into task execution.
  - Added a first-class `team.lead.replace` command/event so lead replacement is command-backed across operator surfaces.
  - Cleared stale current-lead session bindings during reconciliation and reset.
  - Extended `TeamSummary` plus CLI/TUI rendering so current lead state is visible in snapshots, text output, and the Team Dashboard.
  - Added current-lead session attention hints so failed, cancelled, missing, timed-out, or token-exhausted lead sessions surface as handoff-needed operator conditions.
  - Added a CLI subcommand to promote a member to current lead and added a TUI Team Dashboard confirm flow on `L`.
  - Updated operator-facing terminology toward `task card` in the TUI, CLI help text, report headings, and product docs.
  - Added focused tests for:
    - team manifest promotion/binding rules
    - current-lead persistence
    - current-lead session binding/reconciliation
    - command-backed lead promotion
    - TUI promote/remove flows
    - TUI current-lead rendering
- Files created/modified:
  - `crates/awo-core/src/team.rs`
  - `crates/awo-core/src/team/reconcile.rs`
  - `crates/awo-core/src/team/tests.rs`
  - `crates/awo-core/src/app/team_ops.rs`
  - `crates/awo-core/src/app/tests.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/snapshot.rs`
  - `crates/awo-app/src/cli.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/forms.rs`
  - `crates/awo-app/src/tui/keymap.rs`
  - `crates/awo-app/tests/operator_flows.rs`
  - `docs/product-spec.md`
  - `docs/team-manifest-spec.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Planning Session: Lead-Agent Task-Card Orchestration Package
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Re-read the planning workflow guidance, the master finalization plan, and the completed Job Card V/W context.
  - Mapped the user's desired workflow onto the current Awo model: Awo as broker/control plane, lead session as orchestrator, workers as task-card executors.
  - Confirmed the current teardown semantics so the new orchestration plan reuses existing cleanup language correctly.
  - Authored a dedicated orchestration plan focused on lead-session orchestration, task cards, output ingestion, consolidation, capacity handling, and storage-root control.
  - Created new implementation job cards for:
    - lead-session and task-card model
    - output ingestion and capacity state
    - consolidation cockpit and retention controls
    - configurable storage roots
  - Updated the master finalization roadmap so Milestone 6 now explicitly links to the new orchestration package.
- Files created/modified:
  - `planning/2026-03-27-lead-agent-task-card-orchestration-plan.md`
  - `planning/job-card-X-lead-session-and-task-card-model.md`
  - `planning/job-card-Y-output-ingestion-and-capacity-state.md`
  - `planning/job-card-Z-consolidation-cockpit-and-retention-controls.md`
  - `planning/job-card-AA-configurable-storage-roots.md`
  - `planning/2026-03-27-master-finalization-plan.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Post-V Broker Event Delivery Slice
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md` and traced the current event path through `EventBus`, command dispatch, `AppCore`, and MCP.
  - Added a bounded `EventBus::wait()` path backed by a shared `Condvar`, keeping the core synchronous while enabling long-poll style broker waiting.
  - Added `Command::EventsWait` and handled it in `AppCore` so integrations can use the normal command layer instead of ad hoc polling behavior.
  - Updated dispatch roundtrip coverage for the new command variant.
  - Exposed the new broker wait path in MCP as a `wait_events` tool and added a lightweight `awo://events` resource.
  - Added focused tests for immediate-return, blocking, and timeout wait behavior in `events.rs`, plus MCP mapping/resource assertions.
  - Updated `docs/interface-strategy.md` to capture the new bounded wait direction for broker event delivery.
  - Kept the write scope intentionally away from Job Card W ownership areas so consolidation later stays simple.
- Files created/modified:
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/app.rs`
  - `crates/awo-core/src/dispatch.rs`
  - `crates/awo-mcp/src/server.rs`
  - `docs/interface-strategy.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Consolidation Session: Job Card W Fingerprint And Reconciliation Lane
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Inspected the external Job Card W worktree in `.claude/worktrees/fingerprint-reconcile-tests`.
  - Verified the reported fingerprint and reconciliation tests in the external lane before consolidation.
  - Lifted the relevant changes into the main workspace manually to avoid trampling the newer broker/event-delivery work.
  - Merged the fingerprint-status fix in `commands/slot.rs`, ensuring slots without fingerprint markers are marked `missing` rather than incorrectly `ready`.
  - Added fingerprint unit tests in `fingerprint.rs`.
  - Added fingerprint workflow/integration tests in `crates/awo-core/tests/fingerprint.rs`.
  - Added reconciliation tests for released slots and verification pass/fail outcomes in `app/tests.rs`.
- Files created/modified:
  - `crates/awo-core/src/commands/slot.rs`
  - `crates/awo-core/src/fingerprint.rs`
  - `crates/awo-core/src/app/tests.rs`
  - `crates/awo-core/tests/fingerprint.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Job Card V Broker Health And Lifecycle Slice
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Re-read Job Card V, the daemon core, CLI handlers, and the broker/control-surface docs.
  - Replaced the old `Running { socket_ok }` daemon model with explicit `Starting`, `Healthy`, and `Degraded` states plus `DaemonHealthIssue`.
  - Added stale pid/socket/lock cleanup when the recorded daemon PID is no longer alive.
  - Tightened `spawn_daemon()` so it treats existing healthy daemons as reusable, waits on legitimate startup, and reports degraded broker state clearly.
  - Updated CLI bootstrap to wait for `starting` daemons, emit visible text-mode fallback notices when direct mode is used, and improve `awo daemon status` output.
  - Added focused daemon tests for transitional, degraded, and stale-artifact cases, plus handler tests for daemon status rendering.
  - Updated broker-facing docs in `docs/interface-strategy.md` and `docs/v1-control-surface.md`.
  - Fixed a stale `TeamCommand::List` handler match that surfaced during verification.
- Files created/modified:
  - `crates/awo-core/Cargo.toml`
  - `Cargo.lock`
  - `crates/awo-core/src/daemon.rs`
  - `crates/awo-app/src/handlers.rs`
  - `docs/interface-strategy.md`
  - `docs/v1-control-surface.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Planning Session: Milestone 0 Contract Lock And Worktree Setup
- **Status:** complete
- **Started:** 2026-03-27
- Actions taken:
  - Re-read `project.md`, the master finalization roadmap, and the durable docs most likely to drift.
  - Updated the product spec to encode the final local product contract, automatic-but-transparent slot pooling, hybrid task briefs, bounded history ownership, and explicit remote deferral.
  - Updated the development plan to align the remaining work with the master roadmap and replaced the older "missing work" framing with a clearer finalized-vs-unfinished picture.
  - Added a historical-baseline note to the V1 roadmap so it no longer competes silently with the newer finalization plan.
  - Authored two new execution cards: one for the primary broker implementation lane and one for an independent external test-depth lane.
  - Created two new `codex/` branches and matching worktrees for the next parallel implementation wave.

### Implementation Session: Session 1 Command-Surface Parity Sweep
- **Status:** complete
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the post-audit next-sessions plan, and the remaining direct mutation paths before editing.
  - Added first-class command support for:
    - `team.member.update`
    - `team.member.remove`
    - `team.member.assign_slot`
    - `team.task.bind_slot`
  - Added matching domain events and dispatch roundtrip coverage for the new commands.
  - Routed the remaining CLI and TUI member/task mutation flows through dispatch instead of direct `AppCore` helpers.
  - Added core regression coverage for member update, member-slot assignment, task-slot binding, and member removal in `crates/awo-core/tests/command_flows.rs`.
  - Added operator-flow coverage for assign-slot, bind-slot, remove-member, and member-update clear behaviors in `crates/awo-app/tests/operator_flows.rs`.
  - Found and fixed a daemon/direct parity bug where nested `Option<Option<_>>` command fields lost clear intent when serialized through daemon/JSON transport.
  - Replaced that brittle shape in `team.member.update` with explicit `clear_fallback` and `clear_routing_preferences` booleans plus single-layer optional payloads.
- Files created/modified:
  - `crates/awo-core/src/commands.rs`
  - `crates/awo-core/src/commands/team.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-core/src/dispatch.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/tests/operator_flows.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- Verification:
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - note: the full test run still emits expected negative-path `git` and `r2d2` error lines while passing

### Planning Session: External Audit Reconciliation
- **Status:** complete
- **Started:** 2026-03-28
- Actions taken:
  - Reviewed the external audit against the live codebase instead of copying it through unchanged.
  - Confirmed that the TUI blocking-snapshot concern is still real because `crates/awo-app/src/tui.rs` calls `core.snapshot()` on the UI thread and `snapshot()` still performs runtime sync and reconciliation in `crates/awo-core/src/app.rs`.
  - Confirmed that `crates/awo-app/src/tui/action_router.rs` has grown large enough to justify a bounded structural split.
  - Confirmed that CI still lacks `cargo audit` and `cargo deny`.
  - Rejected the external audit’s concrete command-parity examples as outdated because the March 28 parity sweep already closed those named gaps.
  - Narrowed the panic-risk finding to the real remaining production concern in `crates/awo-core/src/events.rs` rather than treating all `unwrap`/`expect` hits in `awo-core` as equally risky.
  - Updated the durable audit and roadmap docs to reflect that reconciled view.
- Files created/modified:
  - `planning/2026-03-28-audit-and-quality-review.md`
  - `planning/2026-03-22-development-plan.md`
  - `planning/2026-03-28-next-sessions-plan.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### Implementation Session: Session 3 Broker Hardening Follow-Through And Session 6 CI Safety
- **Status:** in_progress
- **Started:** 2026-03-28
- Actions taken:
  - Re-read `project.md`, the next-sessions plan, and the broker/CI hardening notes before editing.
  - Finished the high-value bounded broker hardening work:
    - daemon health now requires a bounded RPC roundtrip instead of a bare socket connect
    - sockets that accept but do not answer are now reported as degraded via `RpcUnresponsive`
    - daemon clients now apply bounded read/write timeouts so sick brokers fail predictably
    - CLI daemon status text/json now carries clearer degraded-state detail and stable issue codes
  - Continued TUI hardening already in flight by:
    - keeping periodic snapshots off the render loop
    - preserving Team Dashboard selection on background refresh
    - extracting dialog/form-confirm handling into `crates/awo-app/src/tui/action_router/dialogs.rs`
  - Hardened the app shell against unexpected JSON serialization failure in output helpers.
  - Added a new `security` CI job with `cargo audit` and `cargo deny`.
  - Added a baseline `deny.toml`.
  - Validated `cargo audit` locally and recorded the current warning about `portable-pty -> serial`.
  - Recorded an explicit temporary ignore for `RUSTSEC-2017-0008` in `deny.toml` so CI policy is intentional.
  - Ran a broader review pass, found a stale `convert_case` usage in daemon-status JSON output, fixed it, and added a focused regression test for `RpcUnresponsive` text output.
  - Discovered stale background `cargo test` processes holding build/package locks, cleaned them up, and reran full verification.
- Files created/modified:
  - `.github/workflows/ci.yml`
  - `deny.toml`
  - `crates/awo-core/src/daemon.rs`
  - `crates/awo-core/src/events.rs`
  - `crates/awo-app/src/handlers.rs`
  - `crates/awo-app/src/output.rs`
  - `crates/awo-app/src/tui.rs`
  - `crates/awo-app/src/tui/action_router.rs`
  - `crates/awo-app/src/tui/action_router/dialogs.rs`
  - `crates/awo-app/tests/operator_flows.rs`
  - `crates/awo-core/tests/command_flows.rs`
  - `planning/2026-03-22-development-plan.md`
  - `planning/2026-03-28-audit-and-quality-review.md`
  - `planning/2026-03-28-next-sessions-plan.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

## Test Results
| Test | Input | Expected | Actual | Status |
|------|-------|----------|--------|--------|
| Focused CLI operator flow | `cargo test -p awo-app team_member_promote_lead_updates_current_lead_state -- --nocapture` | Promote-lead command updates current lead through the app surface | Passed | ✓ |
| Focused team manifest tests | `cargo test -p awo-core current_lead_can_be_promoted_to_member -- --nocapture` | Current-lead promotion rules pass | Passed | ✓ |
| Focused lead-session bind test | `cargo test -p awo-core start_team_task_for_current_lead_binds_session_until_reconcile -- --nocapture` | Current lead session binds on start and clears on reconcile | Passed | ✓ |
| Focused lead replacement persistence test | `cargo test -p awo-core replace_team_lead_persists_current_lead_pointer -- --nocapture` | Current-lead replacement persists through load | Passed | ✓ |
| Focused TUI promote/remove flow test | `cargo test -p awo-app member_add_update_remove_and_task_add_delegate_forms_work -- --nocapture` | Team Dashboard flow still works with current-lead safety rules | Passed | ✓ |
| Focused TUI render test | `cargo test -p awo-app team_detail_includes_current_lead_summary -- --nocapture` | Team detail renders current-lead state | Passed | ✓ |
| Planning package authoring | Docs/planning edits only | New orchestration plan and job cards are internally consistent with the master roadmap | Completed | ✓ |
| External-lane fingerprint verification | `cargo test -p awo-core fingerprint -- --nocapture` in W worktree | New fingerprint tests pass before consolidation | Passed | ✓ |
| External-lane reconcile verification | `cargo test -p awo-core --lib test_reconcile_ -- --nocapture` in W worktree | New reconciliation tests pass before consolidation | Passed | ✓ |
| Targeted core event tests | `cargo test -p awo-core events -- --nocapture` | New bounded wait tests pass | Passed | ✓ |
| Targeted core dispatch tests | `cargo test -p awo-core dispatch -- --nocapture` | Command roundtrip tests cover `EventsWait` | Passed | ✓ |
| Targeted MCP tests | `cargo test -p awo-mcp -- --nocapture` | New tool/resource mappings pass | Passed | ✓ |
| Targeted daemon tests | `cargo test -p awo-core daemon -- --nocapture` | New broker health/lifecycle tests pass | Passed | ✓ |
| Targeted handler tests | `cargo test -p awo-app handlers -- --nocapture` | New daemon status rendering tests pass | Passed | ✓ |
| Formatting | `cargo fmt --all` | Workspace formatting succeeds | Passed | ✓ |
| Linting | `cargo clippy --all-targets -- -D warnings` | No warnings remain | Passed | ✓ |
| Full workspace tests | `cargo test` | Whole workspace remains green after broker changes | Passed | ✓ |
| Job Card Z strict linting | `cargo clippy --all-targets -- -D warnings` | New closeout/cleanup code introduces no warnings | Passed | ✓ |
| Job Card Z full workspace tests | `cargo test -q` | Whole workspace remains green after closeout/cleanup changes | Passed | ✓ |
| Command-parity targeted core flow | `cargo test -q team_member_and_task_binding_commands_round_trip_through_dispatch --test command_flows` | New dispatch-backed member/task binding commands behave correctly end to end | Passed | ✓ |
| Command-parity targeted operator flow | `cargo test -q team_member_assign_slot_bind_task_slot_and_remove_flow_are_dispatch_backed --test operator_flows` | CLI/operator layer routes member assign-slot, task bind-slot, and member remove through dispatch correctly | Passed | ✓ |
| Clear-fallback regression | `cargo test -q team_member_update_clear_fallback_removes_fields --test operator_flows` | Clear-fallback still works after routing member updates through commands | Passed | ✓ |
| Clear-routing regression | `cargo test -q team_member_update_clear_routing_defaults_removes_prefs --test operator_flows` | Clear-routing-defaults still works after routing member updates through commands | Passed | ✓ |
| Command-parity linting | `cargo clippy --all-targets -- -D warnings` | Workspace remains warning-free after command-surface parity changes | Passed | ✓ |
| Command-parity full workspace tests | `cargo test` | Full workspace stays green after command-surface parity and transport-shape fixes | Passed | ✓ |
| Local dependency audit | `cargo audit` | No vulnerability blockers; document any warnings honestly | Passed with warning (`RUSTSEC-2017-0008` via `portable-pty -> serial`) | ✓ |
| Broker/CI hardening linting | `cargo clippy --all-targets -- -D warnings` | Workspace remains warning-free after daemon/output/CI hardening changes | Passed | ✓ |
| Broker/CI hardening full workspace tests | `cargo test` | Full workspace stays green after broker/status/output/CI wiring changes | Passed | ✓ |

## Error Log
| Timestamp | Error | Attempt | Resolution |
|-----------|-------|---------|------------|
| 2026-03-27 | `snapshot.rs` borrowed the manifest after partially moving owned fields into `TeamSummary` | 1 | Captured current-lead summary fields before moving owned manifest fields |
| 2026-03-27 | TUI test expected the promoted current lead to remain removable | 1 | Updated the test to promote the lead back before removing the worker |
| 2026-03-27 | Lead replacement only existed as a direct core call, not a real command | 1 | Added `team.lead.replace`, corresponding domain event, and command dispatch coverage |
| 2026-03-27 | None during lead-agent orchestration planning | - | Planning-only slice |
| 2026-03-27 | Mistyped targeted cargo invocation passed an unexpected extra argument to `cargo test` | 1 | Re-ran the intended targeted suites with separate `events`, `dispatch`, and `awo-mcp` commands |
| 2026-03-27 | `handlers.rs` failed to compile because `TeamCommand::List` was matched as a unit variant | 1 | Updated the match arm to accept its `repo_id` field and re-ran verification |
| 2026-03-27 | New Team Dashboard closeout actions borrowed selected task data too long for Rust ownership rules | 1 | Cloned the required slot/session ids before mutating TUI state or dispatching log/release actions |
| 2026-03-28 | `team.member.update` lost clear intent under daemon/JSON transport because nested `Option<Option<_>>` fields collapse on `null` | 1 | Replaced the nested option shape with explicit clear flags plus single-layer optional payloads and added regression tests for both clear flows |
| 2026-03-28 | A stale background `cargo test` process kept the package/build lock and made the fresh verification pass look hung | 1 | Identified and terminated the orphaned cargo/test child processes, then reran verification |
| 2026-03-28 | `handlers.rs` still referenced `convert_case` without a dependency after the broker-status work settled | 1 | Replaced the dynamic conversion with an explicit daemon-issue-code helper and reran clippy/test |
| 2026-03-28 | The new failing-serializer test helper triggered an unused-variable warning under `-D warnings` | 1 | Renamed the serializer parameter to `_serializer` |
| 2026-03-28 | Local `cargo-deny` installation repeatedly stalled on this machine | 1 | Kept CI wiring and `deny.toml` in place, validated `cargo-audit`, and recorded local `cargo-deny` validation as remaining follow-through |

## 5-Question Reboot Check
| Question | Answer |
|----------|--------|
| Where am I? | The local orchestration loop is strong and verified; the current active finish-line work is broker/event maturity plus CI/security follow-through |
| Where am I going? | Toward the remaining release blockers: broker live-event delivery, Windows parity, stronger runtime usage truth, and final release documentation/manual validation |
| What's the goal? | Finalize Awo as a local orchestration console where a replaceable lead agent plans, dispatches, reviews, and consolidates task cards through Awo |
| What have I learned? | The biggest remaining gaps are no longer foundational features; they are finish-line trust issues like broker live updates, platform parity, and explicit security/advisory policy |
| What have I done? | Finished the high-value bounded broker hardening slice, tightened panic handling in the shell, added CI security wiring, validated `cargo audit`, and re-verified the workspace with `cargo fmt`, `cargo clippy`, and full `cargo test` |
