# Next Sessions Plan (March 28, 2026)

## Purpose

Turn the latest audit findings into a concrete sequence of implementation sessions.

This plan is not another broad roadmap. It is the practical continuation plan from the current checkpoint, ordered around the real remaining risks:

1. TUI responsiveness and decomposition
2. broker hardening
3. Windows completion
4. runtime usage truth
5. hardening and CI safety
6. release finalization

## Current Starting Point

The local product is already strong:

- lead-session orchestration exists
- task-card planning exists
- immutable recovery exists
- review/diff/cleanup flows exist
- configurable storage roots and pruning exist
- runtime recovery messaging exists
- the workspace is green under `fmt`, `clippy`, and `test`

The audit conclusion is that the project is no longer blocked on foundational features. It is blocked on **finish-line quality and parity**.

## Session Order

### Session 1: TUI Responsiveness And Decomposition

**Goal:** keep the operator surface fast and maintainable as orchestration state grows.

Why first:
- the external audit confirmed that `core.snapshot()` work still happens on the UI thread
- `action_router.rs` is already large enough that further feature work there will raise change risk sharply
- this is the highest-value finish-line quality improvement that is feasible in the current environment

Target scope:
- reduce or offload full `snapshot()` blocking from the TUI thread
- keep reconciliation/runtime sync semantics honest while avoiding frame-loop stalls
- split `crates/awo-app/src/tui/action_router.rs` into bounded modules such as:
  - key routing
  - form submission
  - confirm/review actions
  - background operations
- add targeted render/state coverage for the refactored structure where practical

Likely files:
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/action_router.rs`
- `crates/awo-app/src/tui/`
- `crates/awo-core/src/app.rs`
- `crates/awo-core/src/snapshot.rs`

Definition of done:
- the TUI no longer waits on full snapshot/reconciliation work in the main render loop in the same way it does today
- TUI control logic is split into smaller bounded modules with clearer ownership

Outcome:
- the TUI now applies periodic `snapshot()` refreshes from a background worker instead of refreshing them on the main render loop
- Team Dashboard selection survives those refreshes by stable team id
- dialog/form/confirm workflow handling was extracted into `crates/awo-app/src/tui/action_router/dialogs.rs`
- remaining follow-up is now optional finish-line polish, not a blocker for moving to broker hardening

### Session 2: Command-Surface Parity Sweep

Status: completed on March 28, 2026.

**Goal:** close the remaining mutation paths that still bypass the dispatcher.

Why first:
- this is the most important residual architecture risk from the audit
- it directly affects daemon/direct consistency
- it is the clearest way to make the “all mutations flow through commands” rule true in practice

Target scope:
- add first-class commands for:
  - `team.member.update`
  - `team.member.remove`
  - `team.member.assign_slot`
  - `team.task.bind_slot`
- route the remaining CLI/TUI flows through dispatch
- add direct-vs-daemon parity coverage for those flows

Likely files:
- `crates/awo-core/src/commands.rs`
- `crates/awo-core/src/commands/team.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-core/src/dispatch.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/tui/action_router.rs`
- `crates/awo-app/tests/operator_flows.rs`

Definition of done:
- no mutating team-management operator flow uses a direct `AppCore` helper when an equivalent command can exist
- daemon/direct parity is stronger and better tested

Outcome:
- added first-class commands for `team.member.update`, `team.member.remove`, `team.member.assign_slot`, and `team.task.bind_slot`
- routed the remaining CLI/TUI member/task mutation flows through dispatch
- added core and operator-flow regression coverage for those paths
- found and fixed a daemon/direct transport bug where nested `Option<Option<_>>` update fields lost clear intent on JSON serialization

### Session 3: Broker Hardening Follow-Through

**Goal:** make daemon mode feel like the normal, trustworthy local operating model.

Why second:
- once command parity is stronger, broker-mode confidence becomes much more meaningful
- this is the next biggest product-level trust issue after parity

Target scope:
- review stale daemon/process/socket handling again under repeated use
- deepen live-client event delivery where polling is still too prominent
- improve broker-mode operator visibility for degraded states
- add concurrency and lifecycle regression coverage

Likely files:
- `crates/awo-core/src/daemon.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-core/src/app.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/tui.rs`

Definition of done:
- repeated CLI/TUI/MCP usage through the daemon feels stable and unsurprising
- degraded states are visible and actionable instead of mysterious

Current progress:
- daemon health is now probed at the RPC layer with a bounded `events.poll` roundtrip instead of a bare socket connect
- sockets that accept connections but never answer RPC requests are now classified as degraded (`RpcUnresponsive`)
- lifecycle and degraded-state regression coverage was extended for that case
- daemon clients now use bounded read/write I/O timeouts instead of waiting forever on a sick broker
- CLI daemon status text/json now carries clearer degraded-state detail and issue codes

Remaining scope:
- deepen live-client event delivery where long-polling still feels too primitive
- decide whether any additional broker status should be surfaced in operator-facing CLI/TUI views

### Session 4: Windows Parity Completion

**Goal:** finish the local-platform story honestly.

Why third:
- Windows remains an explicit release blocker
- the Unix story is now ahead enough that platform parity is the main gap in product completeness

Target scope:
- complete ConPTY workflow validation
- implement/finish Named Pipe daemon transport
- verify the same operator workflows that already work on Unix:
  - repo add
  - slot acquire/release/delete/prune
  - session start/cancel/log
  - team task start/delegate
  - TUI basic operation

Definition of done:
- Windows behavior is no longer “partial support”
- known platform limitations are small, explicit, and documented

### Session 5: Runtime Usage Truth Upgrade

**Goal:** improve runtime usage/capacity visibility without inventing fake precision.

Why fourth:
- advisory recovery messaging is already in place
- the next step is to improve structured truth where adapters allow it
- this becomes more valuable once broker/platform parity are stronger

Target scope:
- add adapter-level capability flags and normalized fields for:
  - usage telemetry support
  - budget-guardrail support
  - session-lifetime support
- ingest structured usage/capacity data where real CLIs/APIs expose it
- keep `unknown` / `unsupported` explicit otherwise
- improve TUI/CLI messaging for timeout vs likely exhaustion vs generic failure

Definition of done:
- operators get more useful capacity guidance without the product overstating certainty

### Session 6: Hardening And CI Safety

**Goal:** reduce avoidable crash and supply-chain risk before the final release pass.

Why fifth:
- this is now clearer after the external audit reconciliation
- the remaining production `unwrap()` risk is smaller than the audit implied, but it is real in synchronization-heavy code
- CI hardening is cheap insurance before broader release preparation

Target scope:
- replace production `unwrap()` lock/condvar handling in `EventBus` with graceful error handling
- evaluate whether any other production panic paths remain outside tests
- add `cargo audit` and `cargo deny` to GitHub Actions
- document the expected failure mode for dependency/security checks

Definition of done:
- a poisoned synchronization primitive does not immediately crash the process without context
- CI covers dependency-security and license/supply-chain checks in addition to fmt/test/clippy

Current progress:
- `EventBus` poison handling now recovers and warns instead of panicking immediately
- JSON output handling no longer panics on unexpected serialization failures
- `.github/workflows/ci.yml` now includes `cargo audit` and `cargo deny`
- `deny.toml` now exists in the repo
- local `cargo audit` validation succeeded, with one known warning: `RUSTSEC-2017-0008` (`serial` via `portable-pty`)
- `deny.toml` now ignores `RUSTSEC-2017-0008` explicitly so CI policy is documented rather than accidental

Remaining scope:
- validate `cargo deny` locally

### Session 7: Release-Finalization Pass

**Goal:** turn the current engineering checkpoint into a coherent local release story.

Why last:
- this depends on the parity/platform/runtime work being settled enough to document honestly

Target scope:
- refresh help text and manuals
- run a full manual release sweep from `MANUAL_TEST_SCENARIOS.md`
- tighten known limitations
- do another codebase audit focused on release quality rather than feature completion
- verify open-source safety across docs/examples one more time

Definition of done:
- the project has a clean contributor/operator story
- the remaining limitations are explicit and reasonable
- the local product feels shippable

## Recommended Worktree / Delegation Pattern

For the next sessions, the best split is:

- main lane: TUI responsiveness/decomposition and broker work
- side lane: Windows-specific work when the environment is available
- side lane: runtime telemetry research/adapter exploration

Good worktree candidates:
- `codex/tui-responsiveness-pass`
- `codex/command-parity-sweep`
- `codex/broker-hardening-pass-2`
- `codex/windows-parity-completion`
- `codex/runtime-usage-truth`

## Session Guardrails

- Prefer command-backed changes over shell-layer convenience patches.
- Keep TUI mutations thin; push behavior downward into `awo-core`.
- Do not broaden scope into remote execution or transcript-product ambitions.
- Keep open-source safety in mind for every planning and docs update.
- Preserve honest `unknown` / `unsupported` states instead of papering them over.

## Best Immediate Next Session

If we start one implementation session next, it should be:

**Session 3: Broker Hardening Follow-Through**

The broker slice now has better status truth and safer failure behavior, but live-event delivery is still the biggest remaining gap before this area feels truly finished.
