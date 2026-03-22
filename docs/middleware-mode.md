# Middleware Mode

## Goal

Evolve `awo` from a local operator console into a middleware layer that can present itself as one virtual coding agent while hiding the complexity of workspace orchestration and multi-model routing.

## Desired Shape

The future external caller should be able to talk to one logical agent while `awo` decides:

- which model to use
- whether one or several workspaces are needed
- which context packs and skills to attach
- when to fan work out in parallel
- how to reconcile results back into a safe Git workflow

## Conceptual Layers

1. Facade agent
   Accepts a task from a caller such as Antigravity or another orchestration tool.

2. Policy and routing layer
   Chooses Codex, Claude, Gemini, or a combination based on task type, cost, risk, and repo policy.

3. Workspace orchestration layer
   Acquires or reuses slots, applies repo rules, and tracks lifecycle.

4. Runtime adapter layer
   Launches the selected CLI runtime with the right prompt, context, and skills strategy.

5. Result consolidation layer
   Normalizes outputs, diffs, logs, and status into one coherent response.

## Why `awo` Is A Good Base

The product already owns the right hard parts:

- isolated worktree lifecycle
- repo-specific context discovery
- runtime-aware skills policy
- session supervision
- review guardrails

Those are exactly the capabilities a facade agent would need underneath.

## Future Integration Shapes

### MCP-facing facade
- expose `awo` as an MCP server
- let external agents request slot acquisition, launch, review, and result summaries

### CLI broker
- run `awo` as a local broker process with a stable JSON command surface
- treat this as the canonical contract that the TUI, automations, and future middleware all share

### Daemon mode
- long-lived process that coordinates multiple local or remote workspaces and runtimes

## Interface Direction: "JSON Inside, MCP Outside"

The best contract stack follows a **"JSON inside, MCP outside"** pattern:

1. stable JSON CLI first (the inside layer, token-efficient, highly predictable)
2. optional broker or daemon around the same schema
3. MCP facade on top for external tool interoperability (the outside layer, standards-compliant)

This keeps `awo` efficient and scriptable locally while still letting it present itself as a standardized tool surface to outside agents later.

## Core Requirement For This Evolution

The orchestration core must stay separable from the TUI. The TUI should remain one client of the core, not the place where orchestration logic lives.

## Team Mode Direction

Vendor-native subagents should be treated as adapter capabilities, not as the product's core abstraction. The portable orchestration layer should continue to think in terms of:

- lead agent
- owned subtasks
- slots and branches
- context packs and skills
- verification and integration

That lets `awo` work with Claude-style subagents and agent teams where available while still supporting runtimes that only expose single-agent CLI execution.

## Near-Term Work To Enable Middleware Mode

1. Normalize session outputs and exit status more explicitly
2. Add machine-readable command results
3. Introduce a supervisor abstraction instead of hard-wiring tmux semantics
4. Add a routing/policy module that can recommend one runtime or a multi-runtime plan
