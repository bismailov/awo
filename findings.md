# Findings & Decisions

## Requirements
- Start implementation step by step using the planning-with-files workflow.
- Begin with Job Card X: lead-session and task-card model.
- Do as much of the first orchestration slice as possible without blocking on later capacity/consolidation work.
- Continue into Job Card Y: structured output ingestion, review-ready task-card state, and honest capacity signaling.
- Continue into Job Card Z: review closeout and explicit worktree retention/deletion controls.
- Research official provider and MCP usage/capacity interfaces in parallel.

## Research Findings
- The existing team model already supports “lead as worker” because task ownership can point at the structural lead member today.
- The missing behavior for Job Card X was not a new task model, but a replaceable “current lead” layer on top of the durable structural lead profile.
- The smallest backward-compatible schema is:
  - keep `lead: TeamMember`
  - add optional `current_lead_member_id`
  - add optional `current_lead_session_id`
- The TUI Team Dashboard was already close enough to support lead promotion with a confirm action instead of a new modal form.
- Reconciliation is the right place to clear dead or terminal lead-session bindings so the operator view does not drift.
- For the “agent ran out of tokens mid-session” problem, the honest product move right now is:
  - treat failed/cancelled/missing lead sessions as handoff-needed states
  - say that token exhaustion or timeout may be one cause
  - let the operator promote another member quickly
- The current runtime/session model already has a natural seam for Job Card Y:
  - `SessionRecord` is the durable place for end reason and capacity state
  - `TaskCard` only needs a small amount of extra review data, not a separate queue object yet
  - reconciliation is already the path that turns terminal sessions into task-card review state
- Because we do not have universal runtime token telemetry, capacity visibility should be:
  - `unsupported` for local runtimes such as `shell`
  - `unknown` for AI CLIs without credible stats
  - `timed_out` or `exhausted` only when we have an explicit session end reason
- A future review queue can be built from task cards if each task persists:
  - `result_session_id`
  - `result_summary`
  - `handoff_note`
  - `output_log_path`
- Best-effort completion ingestion already works well with the current CLI adapters because:
  - one-shot runtimes often write a concise final message to stdout
  - Awo can safely persist that as a handoff note without claiming it is a full transcript
- Explicit timeout and operator-cancel reasons are much more reliable than token exhaustion detection; exhaustion remains heuristic unless adapters expose structured signals.
- The current review-ready task-card model is already enough for a first closeout flow:
  - `TaskCardState::Review` is the review queue
  - `TaskCardState::Done` plus a bound slot is a cleanup/consolidation queue
  - `result_summary`, `handoff_note`, `result_session_id`, and `output_log_path` are enough for the first operator review surface
- Task-card-specific model overrides are the missing economical-routing lever for savvy operators because member-level model defaults already exist but were too coarse for one-off tasks.
- Configurable storage roots fit cleanly at the `AppConfig` layer because repo registration already persists per-repo `worktree_root`; the missing piece was a configurable default root for new repos rather than a repo-schema rewrite.
- Bulk prune is a distinct operator need from `release` and `delete`: release preserves reuse intent, delete targets one slot, and prune clears accumulated retained inventory.
- Retention and deletion are different operator intents and should stay separate:
  - release = retain warm worktrees for reuse, delete fresh worktrees
  - delete = explicitly remove a released slot/worktree now
- The Team Dashboard already had the right shell for Job Card Z; the missing pieces were semantic review actions and explicit cleanup controls, not a brand-new screen.
- OpenAI, Anthropic, and Gemini all expose official usage or cost reporting signals at the API layer.
- I did not find a standard MCP token-usage telemetry field in the spec pages reviewed; MCP currently standardizes progress/task notifications more than provider usage accounting.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Model lead replacement as current-lead metadata rather than mutating the structural lead member | Preserves manifest compatibility and limits the write scope |
| Bind current-lead sessions only when the current lead actually starts a real task session | Avoids falsely overwriting lead-session state during dry-runs or no-auto-start delegation |
| Expose current-lead state in both text output and the TUI team detail/member list | Operators need to see who is currently orchestrating before later review/consolidation features land |
| Add a real command plus CLI and TUI controls for lead promotion in the same slice | Replaceable lead behavior should be usable immediately, not just stored invisibly |
| Let promoted current leads become non-removable until lead is handed back | This matches the safety rule already applied to the structural lead |
| Use handoff-needed attention hints for failed/missing lead sessions instead of pretending we can measure tokens directly | Today the system can observe session state reliably, but not universal per-runtime token telemetry |
| Add `SessionEndReason` to the core runtime model and persist it in SQLite | Timeout, operator cancel, and best-effort exhaustion detection should survive snapshot/reconcile cycles |
| Keep usage/capacity reporting capability-based instead of adapter-fiction | The TUI should be able to say `unknown` or `unsupported` plainly |
| Add only `handoff_note` and `result_session_id` to `TaskCard` for this slice | These are enough to make completed work reviewable without a larger review-object rewrite |
| Derive session capacity state from end reason plus runtime kind | This keeps the model honest today and leaves room for true telemetry later |
| Add semantic `team.task.accept` and `team.task.rework` commands for review closeout | This gives the TUI a command-backed review flow instead of generic mutable state toggles |
| Add `slot.delete` as an explicit cleanup command instead of overloading release | Operators need a visible “delete now” choice for retained warm worktrees |
| Keep diff inspection out of this slice | Logs and slot details are ready now; a real diff helper deserves its own bounded follow-up |
| Treat API usage research as guidance for future adapters, not a reason to invent fake live token stats now | Honest `unknown` remains better than fabricated telemetry |
| Add task-card `model` as a first-class field and route `task.model.or(owner.model)` | This gives per-task budget steering while keeping member defaults intact |
| Resolve clone/worktree roots with `env override -> settings.json -> data-dir default` | This gives operators immediate control while staying deterministic and easy to document |
| Add `slot.prune` for released or missing slots only | Bulk cleanup should remain a safe, explicit operation focused on retained inventory |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| `TeamSummary::from` borrowed `value` after partially moving owned fields | Captured current-lead fields before moving the manifest fields into the summary |
| Existing TUI test expected a promoted current lead to remain removable | Updated the test flow to promote the lead back before removing the worker |
| Lead replacement was initially implemented only via direct `AppCore` calls | Added `team.lead.replace` as a first-class command and event so operator surfaces stay command-backed |
| New Team Dashboard closeout actions initially borrowed selected task data too long for Rust’s borrow checker | Clone the needed slot/session ids before mutating TUI state or dispatching log/release actions |
| Process-environment mutation in tests is `unsafe` in this toolchain | Replaced env-mutation coverage with a pure storage-root precedence helper test so the workspace remains `unsafe`-free |
| A naive prune test reused the same warm slot twice because released warm slots are intentionally reusable | Held two warm slots active before release so the prune test verifies real multi-slot inventory cleanup |

## Resources
- `/Users/bismailov/Documents/chaban/project.md`
- `/Users/bismailov/Documents/chaban/planning/job-card-X-lead-session-and-task-card-model.md`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/app/team_ops.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/app/tests.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/commands.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/commands/team.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/events.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/snapshot.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/team.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-core/src/team/reconcile.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/src/cli.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/src/handlers.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/src/output.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/src/tui.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/src/tui/action_router.rs`
- `/Users/bismailov/Documents/chaban/crates/awo-app/tests/operator_flows.rs`
- `/Users/bismailov/Documents/chaban/docs/product-spec.md`
- `/Users/bismailov/Documents/chaban/docs/team-manifest-spec.md`
- [OpenAI Managing costs](https://platform.openai.com/docs/guides/realtime-costs)
- [Anthropic Administration API](https://docs.anthropic.com/en/api/administration-api)
- [Anthropic Messages usage report](https://docs.anthropic.com/en/api/admin-api/usage-cost/get-messages-usage-report)
- [Gemini Live API guide](https://ai.google.dev/gemini-api/docs/live-guide)
- [MCP Overview](https://modelcontextprotocol.io/specification/2025-06-18/basic/index)
- [MCP Progress utility](https://modelcontextprotocol.io/specification/draft/basic/utilities/progress)

## Visual/Browser Findings
- None; this task stayed inside Rust code, CLI behavior, tests, and local docs.
