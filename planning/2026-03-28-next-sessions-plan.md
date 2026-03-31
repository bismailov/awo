# Next Sessions Plan (March 28, 2026; updated April 1, 2026)

## Purpose

Turn the latest audit findings into a concrete sequence of implementation sessions.

For the sharper post-broker checkpoint execution order, see `planning/2026-03-28-next-stages-execution-plan.md`.

This plan is not another broad roadmap. It is the practical continuation plan from the current checkpoint.

As of April 1, 2026, the practical direction has changed:

1. build the embedded terminal workspace on macOS/Linux
2. keep broker/session architecture aligned with that richer TUI
3. freeze Windows feature scope at the current parity baseline
4. preserve releaseability while the richer TUI grows

Status update after the current execution pass:
- Session 3 broker hardening follow-through is complete for the bounded local-product scope
- Session 4 Windows parity completion is now complete on a native Windows 10 machine
- Session 5 runtime usage truth is complete for the current honest-adapter slice
- Session 6 hardening/CI safety is complete
- Session 7 release finalization is complete for the current platform
- Session 8 release-path and artifact strategy is now complete
- Session 9 repeatable cross-platform smoke coverage is now complete
- the first tagged release is now cut and published as `v0.1.0`
- the remaining material work is no longer release-finalization; it is the macOS/Linux embedded terminal workspace

## Current Starting Point

The local product is already strong:

- lead-session orchestration exists
- task-card planning exists
- immutable recovery exists
- review/diff/cleanup flows exist
- configurable storage roots and pruning exist
- runtime recovery messaging exists
- the workspace is green under `fmt`, `clippy`, and `test`

The old “final release observation” blocker is closed.

The new primary product opportunity is:

- turn the TUI from a strong operator dashboard into a stronger in-TUI terminal workspace

Windows is explicitly **not** the place to expand that feature first. Windows should stay stable at the current parity baseline while macOS/Linux carry the next UX wave.

## Session Order

### Session 1: Terminal Workspace Contract And Unix Feature Gate

**Goal:** define the embedded terminal workspace architecture before broad implementation begins.

Why first:
- the current TUI is already useful, but its own docs still state that embedded terminals are unfinished
- input routing, PTY attach semantics, and scrollback behavior need a contract before UI expansion
- Windows should be frozen now, not dragged through an unstable redesign

Target scope:
- define how embedded terminal panes bind to supervised sessions
- define attach/detach/reconnect behavior
- separate live PTY attachment from durable log viewing
- add an explicit Unix-first feature gate and document the Windows freeze policy

Likely files:
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/`
- `crates/awo-core/src/runtime/`
- `docs/core-architecture.md`
- `docs/v1-control-surface.md`
- `docs/platform-strategy.md`

Definition of done:
- the repo has a precise contract for the first embedded-terminal slice
- Windows scope is clearly documented as “hold steady, fix regressions only”

### Session 2: Single Embedded Session Pane MVP

**Goal:** make one selected macOS/Linux session interactive inside the TUI.

Why second:
- it is the smallest slice that changes the product in the desired direction
- it proves the input, PTY, and resize path before more elaborate workspace UX

Target scope:
- attach to a selected PTY-backed supervised session
- forward keyboard input into the embedded pane
- render live terminal output inside the TUI
- support safe detach back to dashboard mode

Definition of done:
- operators can actually work inside one embedded terminal pane on macOS/Linux
- quitting the TUI does not kill or corrupt a healthy attached session unintentionally

### Session 3: Reattach, Scrollback, And Recovery

**Goal:** make the embedded pane trustworthy instead of merely impressive.

Why third:
- a one-shot pane without recovery will feel fragile and operationally risky

Target scope:
- reconnect to running supervised sessions
- support bounded scrollback
- make “live attached” versus “historical log” state obvious
- handle dead/stale/unavailable attach targets cleanly

Definition of done:
- operators can leave and return to live sessions confidently

### Session 4: Pane Layout And Workspace Navigation

**Goal:** make the TUI feel like a workspace rather than a launcher plus modal views.

Target scope:
- split-pane layouts for dashboard, terminal, logs, and review
- predictable focus movement
- terminal-first and ops-first layout modes

Definition of done:
- the TUI supports sustained multi-surface operator work without feeling cramped

### Session 5: Terminal Ergonomics And Operator Comfort

**Goal:** close the biggest UX gap between “technical success” and “daily usability.”

Target scope:
- copy/search mode
- improved scrollback ergonomics
- stronger terminal status chrome and escape hatches
- cleaner attach/detach prompts and operator cues

Definition of done:
- the embedded workspace is pleasant enough for real daily use on macOS/Linux

### Session 6: Team/Review Integration For The Richer TUI

**Goal:** integrate the embedded terminal workspace back into Awo’s orchestration strengths.

Target scope:
- jump from task cards to live sessions
- move from review states to terminal context cleanly
- preserve slot and team invariants while the UI becomes richer

Definition of done:
- the terminal workspace strengthens the orchestration model instead of bypassing it

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
- the TUI now reacts to event-bus wakeups for command-driven changes and only falls back to a slower periodic refresh for non-evented runtime-state reconciliation
- the MCP facade now advertises resource subscriptions and emits `notifications/resources/updated` for subscribed broker resources after command-driven changes

Remaining scope:
- no blocking local-product work remains in this slice; future broker work is now optional follow-on enrichment

### Session 4: Windows Parity Completion

**Goal:** finish the local-platform story honestly.

Why third:
- Windows remains an explicit release blocker
- the Unix story is now ahead enough that platform parity is the main gap in product completeness

Target scope:
- validate the current ConPTY workflow end to end on Windows
- validate the implemented Named Pipe daemon transport on Windows
- verify the same operator workflows that already work on Unix:
  - repo add
  - slot acquire/release/delete/prune
  - session start/cancel/log
  - team task start/delegate
  - TUI basic operation

Definition of done:
- Windows behavior is no longer “partial support”
- known platform limitations are small, explicit, and documented

Outcome:
- Windows Named Pipe daemon transport is implemented and validated for explicit daemon start/status/stop flows
- Windows `DaemonClient` support is implemented
- Windows JSON CLI isolation now removes inherited `AWO_*` env vars so the serialized suite passes under redirected output
- ordinary Windows CLI commands now stay in direct mode unless `awod` is already running explicitly
- the checked-in March 31 Windows report records passing fmt, clippy, serialized tests, repo/slot/session flows, daemon lifecycle, team lifecycle, and TUI quit smoke

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

Current progress:
- runtime capability output for Claude, Codex, and Gemini now marks usage/capacity telemetry as `planned` instead of permanently `unknown`
- provider-specific usage notes now point operators at Anthropic, OpenAI, or Google truth sources when the current CLI adapter cannot surface spend directly
- targeted operator-flow regression investigation showed the earlier `team_init_creates_manifest_and_shows_it` hang signal was not reproducible in isolation

Remaining scope:
- future adapter-fed spend/quota ingestion remains optional follow-on work

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
- no remaining scope in this slice

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

Current progress:
- manuals, README limitations, platform docs, and release-oriented wording were refreshed
- isolated CLI and TUI smoke workflows passed in temporary Awo roots
- a fresh release-quality audit was written in `planning/2026-03-28-release-finalization-audit.md`

## Post-Windows Next Steps

### Session 8: Release Path And Artifact Strategy

**Goal:** decide how this release candidate should actually ship and be consumed.

Why now:
- Windows parity is no longer the blocker
- the release workflow needed to move from implied maintainer knowledge into versioned tooling and docs

Target scope:
- decide which artifacts should be published for macOS, Linux, and Windows
- document the expected release workflow and ownership
- tighten install/update guidance around explicit daemon use, Windows defaults, and smoke expectations

Outcome:
- added `docs/release-process.md` as the maintainer-facing source of truth for packaging and release execution
- added `scripts/package_release.py` so packaging is reusable outside GitHub Actions
- added `.github/workflows/release.yml` so tag pushes and manual dispatches build, smoke, package, and publish consistently

### Session 9: Repeatable Cross-Platform Smoke Coverage

**Goal:** preserve the new Windows confidence with something more trustworthy than an aging ad hoc harness.

Why next:
- `windows_live_check.ps1` is now explicitly a stale reference artifact
- the new Windows report is strong, but it should be easier to rerun and harder to regress silently

Target scope:
- replace or refresh the Windows checklist harness
- decide what belongs in CI versus documented manual smoke
- keep the smoke matrix aligned across Unix and Windows operator flows

Outcome:
- added `scripts/awo_smoke.py` as the cross-platform smoke runner for repo, slot, session, daemon, team, and TUI validation
- refreshed `windows_live_check.ps1` into a thin wrapper around the shared smoke runner
- wired smoke validation into `.github/workflows/ci.yml` so the core operator loop is exercised on macOS, Linux, and Windows

### Session 10: Optional Final Product Enrichment

**Goal:** choose whether one more pre-release engineering slice is worth it before broader release packaging.

Why last:
- runtime telemetry and richer broker live delivery are now product improvements, not blockers

Target scope:
- decide whether adapter-fed usage/capacity truth deserves another bounded slice before release
- decide whether any broker live-delivery follow-through is still required for the local release candidate

Definition of done:
- the pre-release backlog is explicitly split into “ship now” and “post-release enrichment”

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
- `codex/release-path-and-packaging`
- `codex/windows-smoke-repeatability`
- `codex/runtime-usage-truth`

## Session Guardrails

- Prefer command-backed changes over shell-layer convenience patches.
- Keep TUI mutations thin; push behavior downward into `awo-core`.
- Do not broaden scope into remote execution or transcript-product ambitions.
- Keep open-source safety in mind for every planning and docs update.
- Preserve honest `unknown` / `unsupported` states instead of papering them over.

## Best Immediate Next Session

If we start one implementation session next, it should be:

**Cut the first tagged release candidate and watch it end to end**

The release path and smoke automation now exist. The highest-value next move is exercising the real tag-driven workflow and confirming the produced artifacts and automation behave the way the docs now promise.
