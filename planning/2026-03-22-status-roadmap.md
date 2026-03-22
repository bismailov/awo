# Awo Project Status & Strategic Roadmap (March 22, 2026)

## 1. Executive Summary
`awo` has transitioned from a design scaffold into a hardened, cross-platform workspace orchestrator. We have successfully implemented a tiered review engine capable of detecting both direct and structural conflicts between parallel agents. Infrastructure for Windows-native session supervision (ConPTY) is now in place, and we are moving toward a "Middleware-First" architecture that will expose `awo` as a virtual super-agent backend.

## 2. Product Vision & Thesis
The core thesis remains: **Orchestrate isolated, ready-to-use workspaces, not just transcripts.** 
`awo` provides the operational layer between an LLM and a repository, ensuring safety through Git worktree isolation, context injection, and strict lifecycle management.

## 3. Current State of the Union (What's Done)

### 3.1. Core Stability & Type Safety ("Iron Core")
- **Typed State Engine**: Replaced string-literal state tracking with robust enums (`SessionStatus`, `SlotStatus`, `FingerprintStatus`).
- **SQLite Resilience**: Hardened the persistence layer with versioned migrations and WAL mode.
- **Negative-Path Coverage**: Added exhaustive tests for repository discovery failures, corrupt team manifests, and malformed database paths.
- **Test Baseline**: 158+ unit and integration tests passing across all modules.

### 3.2. Review Intelligence (The "Detection" Tier)
- **Risky Overlap Detection**: Identifies when multiple slots modify the same file (O(N^2) intersection check).
- **Soft Overlap Detection**: Identifies when multiple slots modify different files within the same directory/module (file-class grouping).
- **Automated De-duplication**: Prevents redundant reporting by prioritizing direct matches while still surfacing structural risks.

### 3.3. Multi-Runtime & Platform Readiness
- **Unix (tmux)**: Robust PTY supervision for long-running sessions.
- **Windows (ConPTY)**: Added `portable-pty` dependency and integrated the `Conpty` variant into the `SessionSupervisor` enum. Initial stubs for launch/sync/kill are implemented.
- **Shell Hardening**: Implemented `.ps1` and `.sh` prompt materialization to prevent raw CLI argument injection.

## 4. Work in Progress

### 4.1. Windows ConPTY Maturity
- **Goal**: Full parity with Unix `tmux` supervision.
- **Status**: Infrastructure and trait integration complete.
- **Next Steps**: Implementation of the PTY master process, IO threading for log capture, and sidecar (PID/Exit) management.

### 4.2. Middleware Foundation (The "Dispatcher" Refactor)
- **Goal**: Decouple orchestration logic from the CLI front-end.
- **Status**: Design approved (see `docs/middleware-design.md`).
- **Next Steps**: Refactor `crates/awo-core/src/commands.rs` to introduce a unified `Dispatcher` that can handle headless execution from a future daemon (`awod`).

## 5. Strategic Roadmap (The Next Waves)

### Wave 1: Headless Orchestration (The Daemon)
- **awod**: Create a long-running broker process that manages slots and supervisors via a JSON-RPC 2.0 API over Unix Domain Sockets or Named Pipes.
- **State Locking**: Ensure single-writer safety for the SQLite database when accessed by both the daemon and the TUI.

### Wave 2: MCP Interoperability
- **awo-mcp**: Build an MCP facade that exposes `awo`'s capabilities (slots, review, context) as standardized tools for external agents.
- **Context Resources**: Expose repository context and live diffs as MCP resources.

### Wave 3: Intelligent Routing
- **Routing Engine**: Introduce a policy module that recommends runtimes (Codex, Claude, Gemini) based on task requirements, cost tiers, and repo-specific capabilities.

## 6. Maintenance & Quality Gates
- **Zero anyhow in Core**: Continue replacing `anyhow` with specialized `AwoError` variants in deeper internals.
- **WASI Exploration**: Begin research into running `awo` runtime adapters in WASM for even tighter security and portability.
- **TUI Navigability**: Enhance the operator console to allow deep inspection of "Soft Overlap" file sets and active PTY logs.
