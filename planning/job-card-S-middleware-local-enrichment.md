# Job Card S — Middleware Local Enrichment

## Objective

Strengthen Awo’s local automation and integration surfaces without expanding into remote execution.

## Why This Matters

The product already has CLI, daemon, and MCP surfaces. This work makes them coherent and powerful enough to act like a true local middleware layer.

## Scope

### In Scope
- MCP/resource subscription improvements
- local event-driven updates
- context-pack auto-generation
- shared RPC type cleanup
- better CLI/operator help text

### Out Of Scope
- remote orchestration
- cluster/distributed scheduling

## Deliverables

### 1. Live Integration Surface
- subscription or live-update mechanism for MCP and possibly TUI
- clearer event contracts

### 2. Context-Pack Tooling
- auto-generate useful context packs from repo structure
- preserve operator override and curation

### 3. RPC Cleanup
- reduce duplicated request/response types
- document the stable local command contract

### 4. Docs And Help
- improve CLI help descriptions
- improve automation-facing docs

## Likely Files

- `crates/awo-core/src/dispatch.rs`
- `crates/awo-core/src/events.rs`
- `crates/awo-mcp/src/protocol.rs`
- `crates/awo-mcp/src/server.rs`
- `crates/awo-app/src/cli.rs`
- `crates/awo-app/src/output.rs`
- context-pack related docs/specs

## Risks

- integration improvements can sprawl if not bounded by the local-first rule
- auto-generated context packs can become noisy without curation controls

## Mitigations

- keep generated output reviewable and optional
- treat the command contract as primary and adapters as shells over it

## Verification

- MCP tests
- JSON CLI roundtrip tests
- auto-generated context-pack snapshot/workflow tests
- manual automation smoke tests

## Definition Of Done

- local integrations receive useful live updates
- context-pack generation is practical
- RPC/control-surface behavior is cleaner and better documented
