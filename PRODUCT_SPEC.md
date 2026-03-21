# Product Spec: Agent Workspace Orchestrator

## 1. Summary
The product is a macOS-centric orchestration tool for parallel AI coding. Its purpose is to make multi-agent work safe and cheap by managing isolated worktrees, warm workspace pools, and AI CLI sessions behind one repository-aware control plane.

This is not primarily a chatbot shell or IDE. It is a workspace operations product for AI-assisted development.

## 2. Product Wedge
Most existing tools focus on one of two layers:
- session control: run many agents, view transcripts, coordinate prompts
- source control primitives: manually create branches and worktrees

This product's wedge is the layer between them:
- acquire a safe, ready workspace
- attach the right agent runtime
- inject project context and standards
- enforce repo-level guardrails
- recycle the workspace when done

## 3. Problem
Parallel AI coding breaks down in four recurring ways:

1. Workspace collision
Multiple write-capable agents operate in the same checkout or branch and overwrite each other's assumptions.

2. Setup friction
In large repos, a "new worktree" is cheap but a "ready worktree" is expensive because dependencies, caches, generated files, ports, and local services must be prepared.

3. Runtime fragmentation
AI CLIs differ in whether they need a PTY, whether they are persistent or one-shot, how they handle prompts, and how they expose structured output.

4. Context drift
As more agents participate, standards, constraints, and early decisions get lost, creating remediation work later.

## 4. Product Principles
- Workspace-first: the unit of orchestration is a repo-aware slot, not a chat pane.
- Warm-path optimized: the common case should be reuse, not rebuild.
- Guardrails by default: dangerous workflows should require explicit override.
- Repo-aware: behavior should be shaped by a repo profile rather than generic assumptions.
- Context-preserving: every new session should start from the same architectural baseline.
- Layered: core orchestration logic should be independent from the UI shell.

## 5. Target Users
### Primary
- Individual developers running Codex, Claude Code, Cursor Agent, Gemini CLI, or similar tools in parallel on one machine.

### Secondary
- Small teams building repeatable agent workflows for bug backlogs, refactors, audits, or code generation.

### Best-Fit Repos
- JavaScript/TypeScript monorepos with expensive dependency hydration
- Polyglot repos with multiple local services
- Repos where branch isolation matters more than IDE integration

## 6. Core Concepts
### Repository
The canonical source repo and its Git metadata.

### Repo Profile
A durable configuration describing how this repo behaves:
- default base branch
- branch naming rules
- lockfiles and dependency fingerprints
- bootstrap/refresh commands
- env and port allocation rules
- context files to inject
- protected persistent worktrees

### Slot
An isolated workspace that may be:
- fresh: created on demand
- warm: pre-initialized and recyclable
- persistent: named and protected from recycling

Each slot has a state:
- `idle`
- `active`
- `dirty`
- `stale`
- `refreshing`
- `error`

### Session
A runtime instance attached to a slot, including:
- selected adapter/runtime
- process state
- transcript/log pointers
- task brief
- timing and exit metadata

### Context Pack
The durable project memory and standards shared with every session. At minimum:
- `PROJECT.md`
- `CONVENTIONS.md`
- task-specific brief

### Context Library
A discoverable set of repo knowledge sources, grouped by type:
- entrypoint files such as `AGENTS.md`, `PROJECT.md`, `CLAUDE.md`, `GEMINI.md`
- standards docs
- architecture and deployment docs
- optional analysis reports and remediation history

The product should treat this as a library to route from, not as a giant blob to preload blindly.

### Skill Catalog
Portable `SKILL.md`-based workflows discovered from shared locations such as `.agents/skills/` and, when needed, client-native skill directories.

### Tool Layer
Shared MCP configuration and server metadata for tools, resources, and prompts.

### Machine Target
Local machine or remote execution target that can host slots and sessions.

## 7. Primary Jobs To Be Done
- "Give me a ready workspace for this task."
- "Launch the right agent in that workspace."
- "Make sure the agent starts with the right context and rules."
- "Show me what is active, idle, stale, dirty, or blocked."
- "Let me recycle this workspace safely when the task is done."
- "Warn me before I do something likely to create conflicts or drift."

## 8. Core Workflows
### 8.1 Add Repository
1. Select repository root.
2. Detect Git root and current branches.
3. Create repo profile.
4. Detect ecosystem hints:
   - `yarn.lock`, `pnpm-lock.yaml`, `package-lock.json`
   - `Cargo.lock`
   - `poetry.lock`, `uv.lock`, `requirements.txt`
5. Ask for or infer:
   - base branch
   - worktree directory
   - warm slot count
   - bootstrap command
   - refresh command
   - protected worktree names
   - context files

### 8.2 Acquire Slot
1. Choose task type and target branch/base.
2. Decide slot strategy:
   - use fresh slot
   - reuse oldest clean warm slot
   - allocate persistent named slot
3. Compare dependency fingerprint between current slot state and target branch.
4. Run bootstrap or skip based on repo profile rules.
5. Mark slot `active`.

### 8.3 Launch Agent
1. Pick runtime adapter.
2. Build task brief and context pack.
3. Allocate env overlay and ports.
4. Launch process in slot directory.
5. Persist session metadata.

### 8.4 Review Active Work
The UI should surface:
- active slots
- runtime/session state
- dirty/clean status
- branch and base branch
- last activity
- dependency freshness
- high-risk file overlap warnings

### 8.5 Release Slot
1. Check for uncommitted changes.
2. Offer review/open-terminal/open-diff actions.
3. Detach or reset slot according to slot type.
4. Optionally delete merged branch.
5. Return slot to `idle` if safe.

### 8.6 Refresh Warm Pool
1. Periodically or manually update stale warm slots.
2. Refresh from base branch.
3. Re-run dependency bootstrap if fingerprint changed.
4. Preserve protected persistent slots.

## 9. Safety Model
### Hard Rules
- One write-capable session per slot.
- One branch per active task by default.
- No silent slot reuse when dirty.
- No recycle of protected persistent slots.
- No cleanup that destroys user changes without explicit confirmation.

### Soft Warnings
- Multiple slots touching high-risk files:
  - lockfiles
  - migrations
  - deployment config
  - shared schema/DTO packages
- stale base branch
- dependency fingerprint mismatch
- missing context files

### Repo-Level Guardrails
- verify current repo before actions
- prefer stable machine-readable Git output
- keep action logs for slot lifecycle events

## 10. Context And Quality Features
The meta-analysis materials point to a missing layer in many agent workflows: standards enforcement and context preservation.

The product should support:
- required context files per repo
- discoverable optional context packs
- runtime-neutral entrypoint detection
- portable skill discovery
- MCP config discovery
- task brief templates
- quality gate checklist before launch
- handoff notes between sessions
- audit trail for workspace lifecycle changes

Suggested quality gate prompts:
- definition of done
- required tests
- file ownership hints
- forbidden shortcuts
- review checklist

## 11. Runtime Adapter Model
Each adapter should declare capabilities, not force a fake universal interface.

Suggested capability fields:
- `launch_mode`: `persistent` or `oneshot`
- `needs_pty`
- `supports_stdin`
- `supports_interrupt`
- `supports_resume`
- `supports_structured_output`
- `supports_remote`
- `approval_bypass_flags`

Suggested adapter operations:
- detect runtime
- prepare command
- launch session
- send prompt or task
- interrupt
- terminate
- parse output events
- summarize completion metadata

## 12. Repo Profile Strategy
Repo profiles are the core product differentiator.

They should encode:
- lockfiles that define dependency freshness
- commands for bootstrap and refresh
- generated directories to ignore or rebuild
- local services and default ports
- environment overlays
- protected named slots
- context files and standards docs

Examples:
- Node monorepo: warm slots, `yarn install --immutable`, compare `yarn.lock`
- Rust repo: likely fresh or light-warm strategy, compare `Cargo.lock`
- Python repo: virtualenv or uv environment strategy, compare lockfile plus Python version

## 13. UX Direction
### Recommendation
V1 should prioritize control and visibility over fancy transcript rendering.

The best staged path is:
1. Rust orchestration core
2. TUI shell as the primary interactive surface
3. Command layer shared by the TUI and future CLI automation paths
4. External terminal launch for agent interaction when embedded terminals are not essential
5. Optional macOS GUI later if it adds value beyond the TUI

### Why
- This product's wedge is workspace safety, not markdown transcript polish.
- Embedded terminal emulation adds significant complexity.
- OpenSquirrel already shows that session-pane UX can be built later if needed.
- A TUI gets the product into daily use faster while preserving a strong path to later GUI wrappers.

### UI Shape
Whether TUI or GUI, the top-level information architecture should be:
- repositories
- slots
- sessions
- warnings
- actions

## 14. Differentiation From OpenSquirrel
OpenSquirrel is a valuable reference for runtime integration and multi-agent UI, but its stated scope is different.

This product differs by centering:
- repo profiles
- worktree lifecycle
- warm slot economics
- dependency readiness
- context/standards injection
- safe release/recycle flows

OpenSquirrel can be thought of as session-first.
This product should be workspace-first.

## 15. V1 Scope
- Add and configure repositories
- Create and manage worktree directories
- Support fresh and warm slot strategies
- Launch multiple AI CLI runtimes via adapters
- Persist slot/session state
- Inject context files and task briefs
- Provide list/status/release/refresh actions
- Warn on dirty slots, stale slots, and high-risk overlap

## 16. V1 Non-Goals
- Full Git client replacement
- Rich merge conflict UI
- Source code editor
- Team collaboration backend
- Plugin marketplace
- Full remote daemon story unless local workflows are already solid

## 17. Build Sequence
### Phase A
Define domain model and repo profile format.

### Phase B
Build orchestration core around:
- Git worktree lifecycle
- slot state machine
- dependency fingerprinting
- runtime adapters
- persistence

### Phase C
Build user-facing control surface:
- CLI/TUI first, or thin macOS controller app
- launch into external terminals if needed

### Phase D
Add higher-order features:
- warm pool auto-refresh
- remote machine targets
- richer review surfaces
- embedded terminal panes

## 18. Key Risks
- Overbuilding session UI before solving workspace readiness
- Underestimating repo-specific variability
- Treating all AI runtimes as equivalent
- Hiding too much Git behavior behind abstraction
- Letting recycle/cleanup flows become destructive

## 19. Open Design Questions
- Should slot pooling be visible as a first-class concept in the UI, or mostly automatic?
- Should task briefs be plain text templates, markdown files, or structured YAML plus rendered text?
- How much of the session transcript should the product own in V1?
- Should remote execution wait until after the local-slot model is proven?
