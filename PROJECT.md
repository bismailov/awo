# Project Brain: Agent Workspace Orchestrator

## Mission
Build a tool that makes parallel AI-assisted coding safe, fast, and repeatable by orchestrating isolated, ready-to-use workspaces rather than letting multiple write-capable agents collide in one checkout.

## Working Product Definition
This product is a desktop-first agent workspace orchestrator that should grow cleanly across macOS, Linux, and Windows. It manages Git worktrees, warm workspace pools, AI CLI sessions, and the context bundle each agent needs to work without drifting from project standards.

The default UX should be a single controller surface with background agents, not a mandatory wall of full-sized terminals. Multi-pane or multi-window session views can come later as optional operator views.

## Problem Statement
Running multiple AI coding agents in one repository is attractive but risky:
- Shared checkout editing causes file clobbering and confusion.
- Fresh worktree creation is not the real bottleneck in large repos; dependency hydration often is.
- Different AI CLIs have incompatible runtime semantics.
- Multi-agent work suffers from context loss, uneven standards, and expensive cleanup if guardrails are weak.

## Product Thesis
The winning abstraction is not "spawn another agent." It is:

1. Acquire a safe workspace.
2. Ensure it is ready for the repo's dependency/runtime shape.
3. Inject the right context and quality rules.
4. Launch the right agent in the right mode.
5. Track, review, recycle, and merge safely.

## Primary Users
- Solo developers who run multiple AI coding CLIs in parallel on one machine.
- Power users in large monorepos where fresh worktrees are cheap to create but expensive to hydrate.
- Small teams experimenting with "agent pipelines" that need repo-level safety and repeatability.

## Core Entities
- Repository: the source repo being orchestrated
- Repo Profile: repo-specific rules, lockfiles, bootstrap commands, env/port conventions
- Worktree Slot: an isolated workspace, either fresh or warm/recycled
- Session: a live or resumable AI runtime attached to a slot
- Runtime Adapter: integration contract for a specific AI CLI
- Task Brief: what the agent should do, with context and guardrails
- Context Pack: shared files and prompts every agent should receive
- Machine Target: local or remote execution environment

## Design Principles
- Workspace-first, not transcript-first
- Safe defaults over clever shortcuts
- Reuse warm slots when repo economics demand it
- Separate orchestration core from UI
- Preserve context between agents by default
- Treat quality gates as part of orchestration, not an afterthought
- Support mixed runtime semantics without pretending all CLIs behave the same

## Recommended Architecture Direction
- Rust core for worktree lifecycle, PTY/process supervision, persistence, repo profiling, and adapter execution
- `git` CLI for V1 worktree lifecycle management
- PTY-oriented session layer for interactive CLIs
- TUI as the primary interactive shell in V1, backed by the same core command layer as CLI actions
- V1 optimized around workspace orchestration and launch control, not embedded terminal rendering
- Keep the orchestration core separable enough to become a future middleware or virtual-agent facade

## Implementation Status
- Milestone 0 scaffold is complete.
- Milestone 1 repository registration is complete.
- The repository now contains a Rust workspace with `awo-core` and `awo-app`.
- The app boots, resolves config/data paths, initializes SQLite state, registers Git repositories, writes local repo overlays, dispatches commands through the core, and renders a minimal TUI with a repositories pane.

## Key Research Inputs
- Git worktree docs: script-friendly lifecycle operations and porcelain output
- Dave Schumaker article: warm/recycled worktree slots are essential in heavy JS monorepos
- OpenSquirrel: strong reference for multi-agent session UX and runtime adapters, but not for Git/workspace safety
- Meta-analysis materials: context preservation and explicit standards must be first-class or AI workflows drift into remediation
- CLI-Anything and mcp2cli: strong evidence that CLI plus structured output is a powerful agent interface, but best used as a complement to MCP rather than a replacement for it
- Public disposable repos such as `sharkdp/bat` are better first orchestration testbeds than the user's live `sales-erp` product
- Claude Code subagents and agent teams: useful proof that vendor-native delegation exists, but `awo` still needs a runtime-agnostic team model above any one vendor's feature set
- Sequential Thinking MCP: useful for in-run reasoning, but complementary to file-based planning rather than a replacement for durable team memory

## Current Product Wedge
The product should own the layer that OpenSquirrel intentionally does not:
- worktree lifecycle
- warm slot pooling
- dependency readiness checks
- repo-specific safety rules
- context/standards injection for new sessions
- release/recycle/refresh workflows
- runtime-aware skills policy across Codex, Claude, and Gemini
- a portable team-orchestration layer that can sit above vendor-specific subagent features

## Naming Direction
- Keep `awo` as the internal codename and binary for now
- Use `Switchyard` as the leading external product-name candidate for the future middleware-facing form factor

## Non-Goals For Initial Design
- Full IDE/editor replacement
- Arbitrary Git porcelain replacement
- Team collaboration platform
- Generic plugin marketplace
- Complex merge UI in V1

## Open Questions
- How much transcript/session UX should live inside the product versus delegating to external terminals?
- Which repo ecosystems should receive first-class warm-slot support in V1: Node, Rust, Python, or a more generic command-based model?
- When should a macOS GUI shell follow the TUI, if at all?
