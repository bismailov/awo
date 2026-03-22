# Design Document: Awo Middleware & Broker Mode

## Status: Draft
## Version: 0.1.0

## 1. Overview
The goal is to transition `awo` from a CLI-driven operator tool into a persistent orchestration substrate. This enables external agents (e.g., via MCP) to use `awo` as a high-level "workspace and runtime provider" while abstracting away the low-level Git and PTY management.

## 2. Architecture: The "JSON Inside, MCP Outside" Stack

### Layer 1: The Awo Core (`awo-core`)
The existing Rust library remains the source of truth for all orchestration logic, state persistence (SQLite), and runtime supervision.

### Layer 2: The Awo Daemon (`awod`)
A new long-running process that:
- Owns the exclusive lock on the SQLite state database.
- Manages the lifecycle of persistent sessions (supervisors).
- Exposes a **JSON-RPC 2.0 API** over a Unix Domain Socket (UDS) on Unix or Named Pipes on Windows.
- Provides a "Broker" service for coordinating multiple parallel tasks across slots.

### Layer 3: The CLI Client (`awo`)
The existing CLI becomes a thin wrapper that:
- Connects to `awod` if it's running.
- Falls back to direct core execution (stateless) if `awod` is absent (preserving current behavior).

### Layer 4: The MCP Facade (`awo-mcp`)
A specialized adapter that translates the `awod` JSON-RPC API into the **Model Context Protocol (MCP)**.
- **Tools**: `acquire_slot`, `start_session`, `get_review_status`, `release_slot`.
- **Resources**: `repo://{id}/context`, `slot://{id}/diff`.

## 3. The JSON-RPC API Contract
We will standardize on a JSON-RPC 2.0 interface. Example request/response:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "slot.acquire",
  "params": {
    "repo_id": "awo-core",
    "strategy": "warm"
  },
  "id": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "ok": true,
    "slot_id": "slot-123",
    "events": [...]
  },
  "id": 1
}
```

## 4. Routing & Policy Engine
A new module `crates/awo-core/src/routing.rs` will be introduced:
- **Inputs**: Task description, repo metadata, runtime capabilities, cost/priority constraints.
- **Output**: A recommended `RuntimeKind` and `SkillRuntime` configuration.
- **Logic**: Initially rule-based (e.g., "use Codex for Rust refactoring"), evolving into LLM-assisted routing.

## 5. Roadmap
1.  **Phase 1: API Standardization**: Refactor `awo-app` handlers to use a shared internal "Command Dispatcher" that can be easily exposed via RPC.
2.  **Phase 2: The Daemon (`awod`)**: Implement the UDS/Named Pipe listener and state locking.
3.  **Phase 3: Routing Engine**: Implement the first version of the runtime recommendation logic.
4.  **Phase 4: MCP Facade**: Build the MCP server using the `mcp-sdk`.

## 6. Security Considerations
- **Local-only**: `awod` will listen only on local sockets by default.
- **Permission Mapping**: Future versions may require token-based auth for external agents connecting to the daemon.
- **Slot Isolation**: Continue to enforce strict FS isolation for all sessions.
