# Findings & Decisions

## Requirements
- Start implementation step by step using the planning-with-files workflow.
- Begin with Job Card X: lead-session and task-card model.
- Do as much of the first orchestration slice as possible without blocking on later capacity/consolidation work.
- Continue into Job Card Y: structured output ingestion, review-ready task-card state, and honest capacity signaling.
- Continue into Job Card Z: review closeout and explicit worktree retention/deletion controls.
- Research official provider and MCP usage/capacity interfaces in parallel.
- After the current checkpoint, produce the plan for the next implementation iterations.
- Step back for an overall audit and quality review.
- Update the general development plan and the remaining objectives based on the audit.
- Commit and push the resulting checkpoint.

## Research Findings
- The current roadmap is effectively complete except for the native Windows blocker surfaced by the real checklist run:
  - JSON CLI integration tests were contaminated by inherited `AWO_*` env vars from the outer `cargo test` environment
  - shell runtime works in `oneshot` mode on Windows
  - shell runtime fails in the PTY/ConPTY path on Windows
  - daemon mode accepts Windows named-pipe connections but fails RPC health checks
- The March 30 rerun on the real Windows machine tightened the finish-line picture further:
  - `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -q -- --test-threads=1`, and `cargo build` all pass
  - repo registration, context/skills/runtime inspection, slot lifecycle, and standalone shell session start/log all pass in direct mode
  - daemon mode still fails reproducibly:
    - foreground `awod.exe` starts
    - first `awo.exe daemon status` reports `starting`
    - second `awo.exe daemon status` reports `degraded`
    - the daemon then exits with `0xC0000409`
  - team planning and teardown commands work, but the checklist's Windows task body `pwd && ls` still fails when executed through the team-task shell-script path because the generated `.ps1` is interpreted by a PowerShell variant that rejects `&&`
  - TUI startup still works, but scripted quit via piped input still fails with `Failed to show the cursor ... (os error 232)`
- The bounded broker/MCP completion slice is now effectively closed for the local product:
  - the TUI wakes on broker events
  - MCP supports resource subscriptions
  - no obvious subscribed-resource blind spot remained in the current resource model
- The Windows daemon transport code can be implemented locally, but full Windows validation is still environment-bound:
  - the current macOS machine can compile deep into the Windows target graph
  - bundled `libsqlite3-sys` still fails before a full Rust-level Windows parity check can finish
  - this is a real toolchain/environment blocker, not a reproduced application bug
- Native Windows smoke results sharpen the remaining work further:
  - `session start ... --launch-mode oneshot` succeeds for `shell` with `exit=0`
  - default Windows session start fails only when the PTY path is selected
  - the daemon's named-pipe roundtrip likely breaks in clone-based client/server stream handling because the process stays alive, accepts connections, but never answers the health-check RPC
- The Windows ConPTY implementation had one concrete correctness issue worth fixing now:
  - it was collapsing all non-zero exits to `1` instead of preserving the actual exit code
  - `taskkill /T` is the safer process-tree cancellation shape for supervised Windows sessions
- Runtime/operator truth improves meaningfully when provider-limit failures are separated from token exhaustion:
  - `rate limit` / `quota exceeded` / `insufficient_quota` should not be described as “out of tokens”
  - operators need different recovery guidance for budget/quota pressure than for context exhaustion
- Real local CLI surfaces can sharpen capability truth without inventing adapter telemetry:
  - Claude print mode exposes `--max-budget-usd`, JSON output, and JSON-schema validation
  - Codex `exec` exposes JSON output and JSON-schema-constrained final responses
  - Gemini headless mode exposes `json` and `stream-json` output modes
- Local `cargo-deny` validation was worth doing because CI wiring alone was not enough:
  - the checked-in `deny.toml` had schema drift for the installed `cargo-deny`
  - the new Windows transport dependency chain introduced OSI-approved `0BSD` licenses that needed explicit allowance
- Release smoke validation is much cleaner in an isolated temporary Git repo than in the actively changing `chaban` worktree:
  - using the dev repo itself immediately exercised legitimate stale-slot and dirty-slot safety guards
  - that is good product behavior, but not a clean release smoke target
- Two “failures” in the release smoke were actually confirmations of correct guardrails:
  - stale slots on a moving repo correctly refuse session start without refresh or read-only intent
  - dirty task-card slots correctly refuse teardown/release until the worktree is clean
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
- The next highest-value local gap is immutable task recovery (`cancel` / `supersede`), not another round of lower-level plumbing.
- Review diff/consolidation is the next most important cockpit improvement after immutable recovery because the product still leans too heavily on logs for closeout confidence.
- Planning-to-task-card flow should follow recovery and review depth, not precede them, because the task model and review loop need to feel settled first.
- A lightweight `plan item` layer is enough for the first native planning workflow; it does not require a second manifest file or a separate planning store.
- The existing Team Dashboard can absorb planning cleanly by adding a `Plan` pane between team selection and member/task execution panes.
- Plan-item generation works best as a one-way flow:
  - `draft`
  - `approved`
  - `generated`
- Generated plan items should keep a backlink to the produced task card rather than mutating into the task card itself.
- CLI generation still needs the same minimum execution truth as direct task-card creation:
  - owner is required
  - deliverable is required
  - runtime/model can come from plan intent
- The immutable task model lands cleanly as:
  - `cancelled` for retire-without-replacement
  - `superseded` plus `superseded_by_task_id` for retire-in-favor-of-replacement
- `complete` should treat `done`, `cancelled`, and `superseded` as closed states; otherwise immutable recovery would leave teams permanently unfinished.
- Reconciliation must never resurrect `cancelled` or `superseded` task cards back into `review`/`blocked` just because older slot/session artifacts still exist.
- A bounded review diff is enough for the current cockpit layer:
  - `git status --short`
  - `git diff --stat HEAD`
  - truncated `git diff --unified=3 HEAD`
- Reusing the existing log panel for diff output works well if the TUI tags the source (`slot-diff:<slot_id>`) and refreshes accordingly.
- Retention and deletion are different operator intents and should stay separate:
  - release = retain warm worktrees for reuse, delete fresh worktrees
  - delete = explicitly remove a released slot/worktree now
- The Team Dashboard already had the right shell for Job Card Z; the missing pieces were semantic review actions and explicit cleanup controls, not a brand-new screen.
- Review and cleanup become much easier to operate when the TUI renders an explicit queue role for each task card instead of making the operator derive it from state and slot bindings.
- A lightweight “actionable task” navigation model is enough for the current cockpit layer; it does not require a dedicated second review screen yet.
- Runtime recovery guidance is still valuable without provider-native usage telemetry if it stays honest:
  - timeout should recommend restart or handoff
  - likely exhaustion should recommend switching lead/worker or using a cheaper model
  - unsupported runtimes should say so plainly
- Runtime capability output needs to separate budget guardrails from session-lifetime support, because those are different truths for local operators.
- Team reports are now important operator artifacts, so planning, review, cleanup, and history sections need to be explicit and stable rather than inferred from one task list.
- OpenAI, Anthropic, and Gemini all expose official usage or cost reporting signals at the API layer.
- I did not find a standard MCP token-usage telemetry field in the spec pages reviewed; MCP currently standardizes progress/task notifications more than provider usage accounting.
- The biggest remaining architecture risk is no longer “missing features,” but partial command-surface parity: some mutating team-management flows still bypass the dispatcher because the command layer does not expose them yet.
- Roadmap drift is now a real maintenance cost: the product has outgrown parts of the older development plan, and contributors would get a less accurate picture if the plan were not refreshed.
- Open-source safety needs active enforcement in planning docs too; roadmap markdown had started to accumulate local absolute filesystem links.
- After the audit, the most useful continuation artifact is no longer another feature backlog; it is a session-by-session execution plan tied to the remaining release risks.

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
| Distinguish `provider_limited` from `exhausted` in runtime/session truth | Rate limits and quota failures require different operator actions than context-window exhaustion |
| Treat real local CLI flags as capability truth even before the adapters ingest their full telemetry | This lets operator surfaces be more accurate without faking end-to-end usage accounting |
| Allow `0BSD` in `deny.toml` | The new Windows transport dependency chain legitimately brings in OSI-approved `0BSD` crates via `interprocess` |
| Order the next local iterations as: immutable recovery -> diff/review cockpit -> planning flow -> runtime-usage upgrades | This sequence best completes the operator loop with the least ambiguity |
| Implement immutable recovery as command-backed `team.task.cancel` and `team.task.supersede` | This keeps CLI, TUI, and future MCP surfaces aligned on one mutation path |
| Keep `review.diff` bounded and command-backed | The lead needs TUI-visible diff inspection now, but not a full embedded pager yet |
| Model planning with `PlanItem` records and `draft` / `approved` / `generated` states | This preserves planning history separately from executable task cards while staying small enough for the current schema |
| Use dedicated Team Dashboard keys for planning (`p`, `P`, `G`) | That keeps plan-item actions explicit and avoids overloading task-card actions with planning semantics |
| Add queue-role labels and actionable navigation inside the existing Team Dashboard | This deepens the cockpit without expanding into a second orchestration screen |
| Put usage notes and recovery hints directly on session summaries | CLI, TUI, and future integrations can all reuse one honest source of operator guidance |
| Expand runtime capability descriptors with budget and session-lifetime support flags | Operators need to know whether a runtime can actually help with cost and capacity management |
| Route operator flows through `Command` dispatch whenever a public command already exists | This keeps daemon/direct behavior aligned and enforces the architectural contract in practice |
| Treat development-plan refresh as part of product finalization work | The roadmap is now one of the main onboarding surfaces for future contributors and agents |
| Plan the next implementation wave as explicit sessions after major audits | The remaining work benefits more from crisp sequencing and scope control than from another generic backlog |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| `TeamSummary::from` borrowed `value` after partially moving owned fields | Captured current-lead fields before moving the manifest fields into the summary |
| Existing TUI test expected a promoted current lead to remain removable | Updated the test flow to promote the lead back before removing the worker |
| Lead replacement was initially implemented only via direct `AppCore` calls | Added `team.lead.replace` as a first-class command and event so operator surfaces stay command-backed |
| New Team Dashboard closeout actions initially borrowed selected task data too long for Rust’s borrow checker | Clone the needed slot/session ids before mutating TUI state or dispatching log/release actions |
| Process-environment mutation in tests is `unsafe` in this toolchain | Replaced env-mutation coverage with a pure storage-root precedence helper test so the workspace remains `unsafe`-free |
| A naive prune test reused the same warm slot twice because released warm slots are intentionally reusable | Held two warm slots active before release so the prune test verifies real multi-slot inventory cleanup |
| The existing plans described the finish line well but not the immediate “what next” slice tightly enough | Added a dedicated next-iterations plan to keep the delivery order concrete |
| A bulk initializer update for the new task-card field created duplicate lines in two files | Cleaned the duplicates manually and then relied on compile-guided sweeps for the remaining fixtures |
| The first end-to-end operator-flow test for `team plan generate` failed because the plan item had no owner intent and the test also omitted `--owner-id` | Updated the test fixture to supply an explicit owner so it matches the real command contract |
| The first queue-navigation test used a fake slot id for the cleanup candidate, so reconciliation removed the binding before navigation ran | Changed the test to acquire a real slot and bind that real slot id |
| The actionable-navigation helper was more reliable once “collect actionable ids” was separated from “mutate selection” | Refactored the helper around a precomputed id list to simplify borrow and edge-case handling |
| Full `cargo test` remains noisy because negative-path store/git coverage intentionally hits missing directories and invalid SQLite paths | Treated the logs as expected noise after verifying the full suite still finishes green |
| The master finalization plan had checked-in `/Users/...` links, which violated the repo’s open-source safety guidance | Replaced them with relative links during the audit pass |
| Several CLI/TUI flows still bypassed the dispatcher despite equivalent public commands already existing | Routed the concrete easy cases back through dispatch and recorded the remaining missing command surfaces in the roadmap |
| The March 27 “next iterations” document no longer matched the actual product checkpoint after the audit | Replaced it for practical planning purposes with a new March 28 next-sessions plan keyed to the audit risks |
| Nested `Option<Option<_>>` command fields are not safe for clear-intent across daemon/JSON transport | `team.member.update` needed explicit `clear_fallback` and `clear_routing_preferences` booleans because JSON `null` collapses the outer `Option` |
| The command-surface parity risk is materially smaller now | Member update/remove/assign-slot and task bind-slot all have first-class commands, domain events, CLI/TUI routing, and regression coverage |
| The external audit was directionally useful but not fully current | Its TUI responsiveness, module-bloat, and CI-hardening findings still apply; its concrete command-parity examples were already closed by the March 28 parity sweep |
| Production panic risk is narrower than the external audit implied | The real remaining concern is mostly `EventBus` mutex/condvar `unwrap()` handling in `crates/awo-core/src/events.rs`, while most other `unwrap`/`expect` hits in `awo-core` are test-only |
| Background snapshot refresh is a good bounded answer to the TUI stutter risk | The TUI now loads its first snapshot synchronously, then applies later `snapshot()` refreshes from a background worker and preserves Team Dashboard selection by team id |
| Dialog/form workflow was the cleanest next seam in `action_router.rs` | Extracting it into `crates/awo-app/src/tui/action_router/dialogs.rs` reduced the main router from 2,329 lines to 1,516 while keeping behavior stable |
| `EventBus` poison recovery can be improved without changing public APIs | Recovering the inner guard and warning is a safer broker failure mode than panicking immediately on poisoned mutex or condvar state |
| Daemon health should be defined at the RPC layer, not just the socket layer | A live socket can still be a bad broker if it never answers JSON-RPC, so the health probe now uses a bounded `events.poll` roundtrip |
| A short RPC health probe is a good degraded-state discriminator | Sockets that accept connections but never answer are now classified as `RpcUnresponsive`, which is more actionable than treating them as healthy |
| Daemon clients also need bounded stream timeouts, not only a healthier status probe | A good status check does not help if an already-connected client can still hang forever on an unresponsive broker |
| The remaining production panic surface in the app shell was small and easy to miss | `print_json_response` and `json_error_string` were still using unconditional JSON serialization success assumptions until this pass |
| CI security hardening is now mostly a policy/config problem rather than a code-architecture problem | The workflow and baseline `deny.toml` are easy to add; the real follow-through is validating `cargo-deny` locally |
| `cargo audit` currently reports a single ecosystem warning rather than a vulnerability blocker | `RUSTSEC-2017-0008` reaches this workspace through `portable-pty -> serial`, and `deny.toml` now records that temporary ignore explicitly so CI behavior is intentional |
| Long-lived orphaned cargo test binaries can distort local verification signal | The apparent “hung test suite” in this session was caused by stale background test processes holding locks, not by the current code under test |
| The TUI does not need to choose between blind polling and perfect push semantics | A hybrid model works well here: wake immediately on event-bus activity for command-driven changes, then keep a slower fallback refresh for off-thread runtime reconciliation that does not publish events yet |
| “Unknown” is too pessimistic when provider telemetry exists but the current adapter does not ingest it yet | Marking Claude/Codex/Gemini usage and capacity reporting as `planned` is a truer operator signal than implying the product has no path to structured truth |
| Runtime usage notes are more actionable when they name the current best truth source | Pointing operators toward Anthropic, OpenAI, or Google usage surfaces is more useful than a generic “check dashboards” note |
| MCP resource subscriptions are the right bounded next step for live integrations | They let external clients react to broker-backed resource changes without forcing a streaming transport rewrite or deeper async refactor |
| The isolated `team_init_creates_manifest_and_shows_it` slowdown signal was environmental noise, not a reproduced product regression | The targeted test passed cleanly once stale background cargo processes were cleared and rerun in isolation |
| The remaining roadmap is now dominated by release blockers rather than missing core product concepts | The practical order is broker completion, Windows parity, runtime telemetry improvement, CI closure, then release finalization |
| Windows parity is the clearest external-agent lane | It is the largest remaining release blocker and benefits the most from a dedicated environment or separate worktree |
| Windows `runtime` commands should stay out of daemon bootstrap | `runtime list/show/route-preview/pressure` are local capability/config operations, and letting them trigger broker bootstrap made `json_cli` hang under redirected output on Windows |
| Windows direct-mode fallback is the safer default when `awod` is not already running | Explicit daemon flows now validate cleanly, while ordinary repo/slot/session/team commands remain reliable and scriptable without silent auto-start side effects |
| The old Windows smoke harness had become a debugging artifact rather than a trustworthy source of truth | `windows_live_check.ps1` still routed `awo.exe` into a stale subprocess shape, so the final Windows report had to be refreshed from clean manual smoke commands and exact serialized test runs |
| The March 31 Windows checklist report closes the repo's last explicit platform release blocker | The checked-in report shows passing fmt, clippy, serialized tests, repo/slot/session flows, daemon lifecycle, team workflows, and TUI quit smoke on a real Windows 10 machine |
| The repo's current-state docs drifted behind the new Windows checkpoint immediately | `README.md`, `docs/platform-strategy.md`, and the continuation plans still described Windows validation as pending even after `windows_checklist_report.md` landed |
| After Windows parity closure, the highest-value next work is operational rather than platform-debugging | The next questions are how to preserve Windows confidence with repeatable smoke coverage and how to package or ship the release candidate cleanly |
| A single shared smoke runner is a better product-quality investment than platform-specific checklist sprawl | `scripts/awo_smoke.py` now validates the core operator loop across macOS, Linux, and Windows with isolated state and machine-readable reports |
| Release packaging should be reusable outside GitHub Actions | `scripts/package_release.py` keeps archive composition deterministic locally and in CI instead of hiding packaging logic inside workflow YAML |
| The release path is only trustworthy if it proves the shipped binaries, not just the dev tree | The new `Release` workflow builds release-profile binaries, runs smoke validation against them, packages the archives, and publishes assets on version tags |

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
- `/Users/bismailov/Documents/chaban/planning/2026-03-22-development-plan.md`
- `/Users/bismailov/Documents/chaban/planning/2026-03-27-master-finalization-plan.md`
- `/Users/bismailov/Documents/chaban/planning/2026-03-28-audit-and-quality-review.md`
- `/Users/bismailov/Documents/chaban/planning/2026-03-28-next-sessions-plan.md`
- `/Users/bismailov/Documents/chaban/windows_checklist_report.md`
- `/Users/bismailov/Documents/chaban/windows_checklist_report.json`
- [OpenAI Managing costs](https://platform.openai.com/docs/guides/realtime-costs)
- [Anthropic Administration API](https://docs.anthropic.com/en/api/administration-api)
- [Anthropic Messages usage report](https://docs.anthropic.com/en/api/admin-api/usage-cost/get-messages-usage-report)
- [Gemini Live API guide](https://ai.google.dev/gemini-api/docs/live-guide)
- [MCP Overview](https://modelcontextprotocol.io/specification/2025-06-18/basic/index)
- [MCP Progress utility](https://modelcontextprotocol.io/specification/draft/basic/utilities/progress)

## Visual/Browser Findings
- None; this task stayed inside Rust code, CLI behavior, tests, and local docs.
