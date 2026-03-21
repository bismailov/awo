# Interface Strategy

## Question

Should `awo` rely on MCP, a structured JSON CLI, or a "CLI anything" style interface for future integrations?

## Short Answer

Use all three, but at different layers:

1. Internal canonical interface: structured JSON CLI
2. Optional local broker or daemon: same schema over a long-lived process
3. Optional external interop surface: MCP facade

`CLI-Anything` is a useful reference, but it is not a replacement for `awo`'s orchestration API. It is a system for turning other software into agent-friendly CLIs.

## What The Research Says

### MCP

The Model Context Protocol is the strongest current standard for exposing tools, resources, and prompts to AI clients. MCP is especially valuable when `awo` eventually needs to appear as one virtual agent or middleware layer to outside systems.

Strengths:
- standard ecosystem language for tool exposure
- client-driven discovery of tools and schemas
- good fit for "virtual agent" or middleware mode
- natural way to expose slots, launches, reviews, and summaries to external agents

Costs:
- more protocol overhead than a narrow local CLI
- more schema/token surface at runtime
- not the best human operator interface by itself

### Structured JSON CLI

A structured CLI is the best fit for `awo`'s core because the product is already command-centric: repo lifecycle, slot lifecycle, session launch, review, doctor, and sync flows.

Strengths:
- easy to script
- easy to call from other agents
- token-efficient for machine use
- stable local contract for tests and automation
- easy to wrap behind MCP later

Costs:
- needs careful output stability rules
- weaker discovery story than MCP unless `--help`, schemas, and JSON modes are disciplined

### CLI-Anything

CLI-Anything positions itself as a system that can transform a codebase into an AI-agent-ready CLI with `--help` and `--json`, via an automated pipeline. That is interesting for agent-native software in general, and it reinforces the value of CLI as an interface shape. But it targets a different layer than `awo`.

Relevant lessons:
- CLI is a strong universal interface for agents
- `--help` and `--json` are excellent discovery and output conventions
- deterministic command execution is preferable to brittle UI automation

Why it is not a drop-in answer for `awo`:
- `awo` is itself the orchestrator, not a target app to be "CLI-fied"
- `awo` already has a native command model; it does not need codebase-to-CLI generation
- the hard problem for `awo` is safe orchestration and policy, not command-surface generation

### mcp2cli

`mcp2cli` is a more directly relevant reference than CLI-Anything. It turns MCP, OpenAPI, or GraphQL interfaces into a runtime CLI, emphasizes token efficiency, and supports machine-oriented output modes.

The main lesson for `awo`:
- one system can support both standardized tool protocols and CLI ergonomics

## Recommended Contract Stack For `awo`

We are standardizing on a **"JSON inside, MCP outside"** pattern.

### Layer 1: Stable JSON CLI (Inside)

Add machine-readable output to every core operation:
- `repo add/list/clone/fetch`
- `slot acquire/release/refresh/list`
- `session start/cancel/delete/list`
- `context pack/doctor`
- `skills list/doctor/link/sync`
- `review status`

Design goal:
- one command = one structured result envelope with status, identifiers, warnings, and payload

### Layer 2: Optional Broker / Daemon

Expose the same operations through a long-lived local process when we need:
- multi-call workflows
- caching
- long-running supervision
- middleware use by another orchestration system

### Layer 3: MCP Facade (Outside)

Expose the broker or CLI behind MCP when we want third-party agent clients to treat `awo` as a tool provider or virtual agent backend.

## Recommendation

Do not choose between CLI and MCP as if only one can exist.

Build `awo` around a stable JSON CLI first, because it is the best local control plane and the best internal contract. Then add an MCP facade on top when middleware mode becomes real. Treat CLI-Anything as validation that CLI is a strong agent interface, not as the architecture we should copy literally.

## Sources

- CLI Anything official site: https://clianything.org/
- CLI-Anything README: https://github.com/HKUDS/CLI-Anything
- MCP tools spec: https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- mcp2cli repository: https://github.com/knowsuchagency/mcp2cli
