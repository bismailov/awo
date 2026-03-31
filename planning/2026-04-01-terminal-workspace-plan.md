# Embedded Terminal Workspace Plan (April 1, 2026)

## Purpose

Define the next major implementation direction after `v0.1.0`.

The new direction is explicit:

- turn the TUI into a stronger embedded terminal workspace on macOS/Linux
- freeze Windows feature scope at the current released state for now
- treat the existing operator dashboard as the control-plane foundation, not the final TUI shape

This document is the implementation plan for that shift.

## April 1 Execution Checkpoint

The first Unix-first execution wave is now implemented in the repo.

Completed in this checkpoint:

- Milestone 1: terminal workspace contract
- Milestone 2: single embedded session pane MVP
- Milestone 3: session reattach, scrollback, and recovery
- Milestone 4: pane layout and workspace navigation
- Milestone 5: terminal ergonomics
- Milestone 6: team and review integration

What that means concretely:

- supervised `tmux`-backed macOS/Linux sessions can be attached in the TUI
- the terminal pane supports live capture, input forwarding, scrollback, search, follow mode, and layout switching
- task/session launches can auto-open the richer terminal workspace when the session supports it
- log and review diff escape hatches remain available from the terminal workspace
- Windows remains frozen at the current released baseline for this feature area

## Product Decision

Awo should move from:

- operator dashboard with launch/log/review controls

toward:

- operator dashboard plus embedded interactive session workspace

The target is not “make the TUI prettier.”
The target is “make the TUI a place where operators can both supervise and stay inside the work itself.”

## Platform Policy

### macOS and Linux

Advance immediately.

They are the primary implementation platforms for the embedded terminal workspace because:

- PTY support is already stronger
- detached supervision and terminal behavior are easier to reason about
- the architecture can mature there before we generalize further

### Windows

Freeze feature scope at the current release baseline.

That means:

- keep the current Windows functionality working
- accept bug fixes, regressions, and release-maintenance work
- do not start a full embedded-terminal UX expansion on Windows yet

Rationale:

- Windows parity is finally real and should not be destabilized immediately
- embedded-terminal behavior is materially harder on Windows
- the Unix path should settle first so Windows later ports a mature design, not a moving target

## What “Done” Means For This Direction

The embedded terminal workspace should eventually provide:

- a real interactive terminal pane inside the TUI for running sessions
- attach/detach/reconnect behavior for supervised sessions
- scrollback and log review that feel native instead of bolted on
- pane and focus management that support multi-session operator work
- preserved Awo strengths: slot safety, review state, team flow, and lifecycle control

It does **not** need to become a full IDE.

## Non-Goals For The First Wave

- full cross-platform parity for the new terminal UI
- replacing external terminals completely
- a full tmux clone
- mouse-first UI redesign
- plugin/theme ecosystem work

## Implementation Milestones

### Milestone 1: Terminal Workspace Contract

Goal:
- define the embedded terminal architecture before broad UI work begins

Scope:
- choose how terminal panes bind to `SessionRecord` and supervisor state
- define attach/detach/reconnect semantics
- define what is interactive PTY content versus what remains durable log content
- define Unix-only gating for the new feature path

Likely files:
- `crates/awo-core/src/runtime/`
- `crates/awo-core/src/session/`
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/`
- `docs/core-architecture.md`
- `docs/v1-control-surface.md`

Definition of done:
- the repo has a clear contract for “embedded terminal pane” versus “session log panel”
- Windows freeze policy is documented in product/planning docs

### Milestone 2: Single Embedded Session Pane MVP

Goal:
- make one selected Unix session interactive inside the TUI

Scope:
- PTY-backed session attachment for macOS/Linux
- keyboard input forwarding into the embedded pane
- basic resize propagation
- safe detach back to dashboard mode
- fallback to current log behavior where PTY attach is unavailable

Definition of done:
- an operator can select a supported session and interact with it directly inside the TUI
- quitting the TUI does not corrupt the supervised session

Rough effort:
- about 1 to 2 focused weeks

### Milestone 3: Session Reattach, Scrollback, And Recovery

Goal:
- make the embedded pane operationally trustworthy

Scope:
- reconnect to an existing supervised session
- bounded scrollback model
- clear “live attached” versus “historical log” state in the UI
- better failure surfaces when the session is gone, stale, or no longer attachable

Definition of done:
- attach/detach/reconnect feels normal rather than fragile
- operators do not lose situational awareness when returning to a running session

Rough effort:
- about 1 to 2 more weeks after the MVP

### Milestone 4: Pane Layout And Workspace Navigation

Goal:
- support real multi-surface operator work inside the TUI

Scope:
- split views between dashboard, session pane, log pane, and review pane
- layout switching for “ops first” versus “terminal first”
- focus routing that remains predictable under load

Definition of done:
- the TUI feels like a workspace, not just a modal inspector

Rough effort:
- about 1 to 2 weeks

### Milestone 5: Terminal Ergonomics

Goal:
- close the biggest UX gap between “works” and “feels good”

Scope:
- copy/search mode
- better scrollback handling
- obvious status chrome for attached sessions
- safer escape hatches and operator prompts

Definition of done:
- the embedded pane is pleasant enough for daily use, not only demos

Rough effort:
- about 1 to 3 weeks

### Milestone 6: Team And Review Integration

Goal:
- connect the richer terminal UI back into Awo’s orchestration strengths

Scope:
- jump from task card to live attached session
- better review-to-terminal transitions
- preserve slot/session safety while allowing richer in-TUI work

Definition of done:
- the terminal workspace strengthens orchestration instead of bypassing it

## Overall Effort Bands

Honest rough estimate:

- single embedded terminal pane MVP: 1 to 2 weeks
- solid usable workspace: 4 to 8 weeks
- polished “bells and whistles” experience: 2 to 4 months

These are product-engineering estimates, not just coding estimates. Cross-platform UX, resize behavior, focus/input bugs, and operational reliability will dominate the schedule more than raw feature coding.

## Major Risks

### 1. Input Routing Complexity

Once the TUI embeds a real terminal pane, keyboard routing becomes much trickier.
The product must clearly distinguish:

- global TUI shortcuts
- pane-local terminal input
- dashboard navigation

### 2. Session Lifecycle Drift

The terminal pane must never become a second source of truth.
Session state still belongs to the orchestration core.

### 3. Scrollback And Persistence Ambiguity

Interactive PTY output and durable session logs are related but not identical.
The architecture must avoid pretending they are the same thing.

### 4. Windows Temptation

There will be pressure to “just make Windows match.”
That should be resisted until the Unix design is stable.

## Recommended Execution Order

1. Architecture contract and Unix-only feature gate
2. Single embedded session pane MVP
3. Reattach/recovery/scrollback
4. Pane layout and navigation
5. Terminal ergonomics
6. Team/review integration
7. Re-evaluate Windows port timing only after the Unix workspace feels stable

## Release Policy During This Work

During this implementation wave:

- `main` should remain releasable
- Windows should stay at parity baseline, not accumulate speculative terminal UX work
- every terminal-workspace slice should land with focused Unix validation and no Windows regression

## Immediate Next Session

The next implementation session should be:

### Session A: Terminal Workspace Contract And Unix Feature Gate

Deliverables:
- architecture notes for embedded-session panes
- documented macOS/Linux-first policy and Windows freeze
- first bounded code slice that introduces the new terminal-workspace seams without trying to finish the whole experience at once
