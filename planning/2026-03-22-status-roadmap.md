# Awo Project Status & Roadmap (Updated March 24, 2026 - Late)

## 1. Current State

`awo` is now a **high-reliability orchestration substrate**. It has transitioned from a local-only CLI tool into a multi-interface system (CLI, TUI, Daemon, MCP) with deep safety guardrails and automated verification. **CI is green on all three platforms** (macOS, Ubuntu, Windows). Total test count is **433+** with extensive negative-path coverage.

### What's Built (Recently Added in **Bold**)

**Core (`awo-core`)**
- Typed state engine: `SessionStatus`, `SlotStatus`, `FingerprintStatus`, `TeamStatus`, `TaskCardState` enums.
- SQLite persistence: **Version 5 schema** with WAL mode and **Session Timeouts/StartedAt tracking**.
- Review engine: **Multi-tiered overlap detection** (Risky, Soft, and **File-level** grouping across repos).
- Team orchestration: **Automatic task verification** (executing `verification_command` like `cargo test` on completion), **Result consolidation** (capturing logs/summaries into TaskCards), and **Markdown report generation**.
- Hardening: **433+ tests** including **exhaustive negative-path tests** for store, commands, and snapshots.
- Cross-platform: **Normalized path canonicalization** (via `dunce`), **authoritative process group/tree cancellation** (Unix `kill -9` / Windows `taskkill /F /T`).

**Daemon (`awod`)**
- JSON-RPC 2.0 over Unix Domain Socket.
- Headless execution support for all orchestration commands.

**MCP Server (`awo-mcp`)**
- Fully synchronized with core: Supports **Session Timeouts** and **Team Task Start** with automatic context.
- Exposes orchestration as tools and resources for external agents (Claude, etc.).

**CLI & TUI (`awo-app`)**
- **Panel-wide Filtering**: `/` search across Repos, Teams, Slots, and Sessions.
- **Live Log Tailing**: Auto-refreshing log viewer with `[running]` status and scroll tracking.
- **Background Operations**: Slow Git tasks (Acquire/Release/Add) use background threads to keep TUI responsive.
- **Interactive Team Control**: `t` to start next task, `R` to generate team reports.

## 2. Milestone D Completion Report

### Team Execution Depth & Result Consolidation
- **Done**: TaskCards now carry `result_summary`, `output_log_path`, and `verification_command`.
- **Done**: `awo team report <id>` generates a full history of the team's mission, status, and outcomes.
- **Done**: Automated verification blocks tasks from entering `Review` if quality gates fail.
- **Done**: Routing transparency via `routing_reason` in domain events.

## 3. Next Wave: Wave 4 (Middleware & Ecosystem)

Now that the core is hardened and the local UX is highly efficient, we move toward **Headless Brokerage and External Integration**.

### Milestone A: Stable Daemon & RPC (The "Broker" Mode)
- **Persistence Layer Scaling**: Transition from `Mutex<Connection>` to a connection pool (`r2d2`) or `tokio-rusqlite` to support higher concurrency in Daemon mode.
- **Daemon Lifecycle**: Implement `awod status`, `awod stop`, and automatic startup on CLI invocation.
- **Named Pipe Support**: Finalize Windows-native Daemon transport.

### Milestone B: MCP Expansion
- **Resource Subscriptions**: Enable long-polling or event-driven updates for MCP clients.
- **Context Pack Auto-gen**: Tooling to automatically derive context packs from repository structure.

### Milestone C: Orchestration Intelligence
- **Lead/Worker Handoff**: Dedicated commands for "Lead" agents to delegate sub-tasks to "Worker" slots with automatic context passing.
- **WASI Sandboxing**: Research running runtime adapters in WASM for zero-trust execution.

## 4. Current Workstream Assignments

### Lane 1: Reliability/Core (Active)
- Database connection pooling.
- Completion of Windows ConPTY master/slave logic (currently a stub).

### Lane 2: Middleware/Daemon (Upcoming)
- JSON-RPC event bus (push notifications for TUI/MCP).
- Stable `awod` lifecycle management.

### Lane 3: Intelligent Flows (Upcoming)
- Multi-agent handoff state machine.
- Automatic result synthesis (LLM-assisted report summaries).
