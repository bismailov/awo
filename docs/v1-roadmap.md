# V1 Roadmap

## Goal
Ship a TUI-first V1 that makes parallel AI work safer and faster for local repositories, without getting trapped in UI polish or overbuilt remote features.

## Execution Philosophy
- Move fast on the core product wedge: workspace safety and readiness.
- Keep scope narrow where complexity is high.
- Prefer vertical slices over broad scaffolding.
- Cut aggressively if a milestone threatens the rest of V1.

## Chosen V1 Shape
- TUI-first interactive product
- command model underneath the TUI
- local machine only
- external terminals for deep agent interaction
- small adapter set

## Milestone 0: Skeleton
### Objective
Create the minimum project skeleton that keeps the architecture honest.

### Deliverables
- Rust workspace with `awo-core` and `awo-app`
- config loading bootstrap
- state storage bootstrap
- command dispatch skeleton
- basic logging and event plumbing

### Exit Criteria
- app starts
- config path resolves
- state database initializes
- a no-op command can round-trip through core and back to the shell

### Main Risk
- overengineering crate structure too early

### Mitigation
- keep only two crates initially

## Milestone 1: Repository Registration
### Objective
Make the product repo-aware.

### Deliverables
- `repo add`
- `repo list`
- effective repo profile loading
- Git root detection
- local overlay generation
- TUI repository list screen

### Exit Criteria
- user can register a repo
- repo profile persists
- TUI shows repo summary

### Main Risk
- spending too long on perfect manifest schema

### Mitigation
- support only the minimum required V1 fields first

## Milestone 2: Slot Lifecycle
### Objective
Make workspaces real.

### Deliverables
- worktree discovery and inventory
- fresh slot acquisition
- warm slot acquisition
- release flow
- slot state derivation
- TUI repo detail and slot detail screens

### Exit Criteria
- user can acquire and release a slot safely
- dirty slots block unsafe recycle
- TUI reflects slot states correctly

### Main Risk
- state explosion and edge cases around dirty/protected/stale slots

### Mitigation
- use composed internal state with derived display states

## Milestone 3: Readiness And Fingerprints
### Objective
Make a slot not just isolated, but ready.

### Deliverables
- fingerprint engine
- bootstrap decisioning
- stale slot detection
- refresh flow
- basic readiness indicators in TUI

### Exit Criteria
- product can decide whether a slot is ready or stale
- refresh command works for supported repo profiles

### Main Risk
- overfitting to one ecosystem

### Mitigation
- implement generic fingerprint groups plus one strong Node-oriented profile first

## Milestone 4: Session Engine And Adapters
### Objective
Attach real AI runtimes to slots.

### Deliverables
- session persistence
- process/PTY supervision
- adapter registry
- initial adapters:
  - Codex
  - Claude Code
  - Cursor Agent
- session list and session detail screens

### Exit Criteria
- user can start at least one supported runtime in a slot
- session lifecycle persists across app restarts
- unsupported capability requests fail clearly

### Main Risk
- runtime behavior differences consume too much time

### Mitigation
- ship only a few high-confidence adapters
- defer fancy structured-output parsing when raw events are enough

## Milestone 5: Review, Warnings, And Release Confidence
### Objective
Help the user notice danger before it becomes cleanup work.

### Deliverables
- risky overlap detection
- stale warm-pool warnings
- releasable slot detection
- review status screen
- action log view

### Exit Criteria
- user can see which slots are risky, stale, dirty, or releasable
- review flow feels meaningfully safer than raw manual terminals

### Main Risk
- warning heuristics becoming noisy or misleading

### Mitigation
- start with a small, high-signal warning set

## Milestone 6: Hardening Pass
### Objective
Make V1 trustworthy enough to use daily.

### Deliverables
- failure recovery improvements
- better error surfaces in TUI
- profile validation
- state migration hooks
- smoke-test matrix for core workflows

### Exit Criteria
- fresh task flow works end-to-end
- warm task flow works end-to-end
- release flow works end-to-end
- common failures are recoverable without corrupting state

### Main Risk
- spending too much time polishing the TUI instead of reliability

### Mitigation
- define "hardening" in terms of workflow success and recovery, not aesthetics

## Scope Cuts If Time Gets Tight
Cut in this order:
1. structured output parsing beyond basics
2. multi-ecosystem repo presets beyond Node plus generic fallback
3. rich session log views
4. advanced review/diff presentation
5. remote-target scaffolding beyond interface placeholders

Do not cut:
- safe slot lifecycle
- repo profiles
- session persistence
- dirty-slot protection

## Suggested Fast Sequence
### Week 1
- Milestone 0
- Milestone 1

### Week 2
- Milestone 2
- begin Milestone 3

### Week 3
- finish Milestone 3
- Milestone 4 with one runtime first

### Week 4
- expand adapters
- Milestone 5

### Week 5
- Milestone 6
- cut anything non-essential

This is aggressive but realistic only if V1 discipline holds.

## Definition Of V1 Done
V1 is done when a user can:
1. register a repo
2. acquire a fresh or warm slot
3. launch a supported AI runtime in that slot
4. see slot/session state in the TUI
5. detect obvious conflict risk
6. release or recycle the slot safely

If any of those fail, the product is not done, even if the TUI looks polished.
