# Job Card T — Local Orchestration Intelligence

## Objective

Deepen Awo’s core wedge: local multi-agent coordination with clear lead/worker flows and automatic context passing.

## Why This Matters

This is where Awo differentiates from “just worktrees” and from “just a model launcher.”

## Scope

### In Scope
- lead/worker handoff depth
- automatic context passing on delegation
- better local mission synthesis and reports
- clearer routing and reasoning visibility

### Out Of Scope
- remote workers
- speculative agent-society abstractions
- full transcript analysis products

## Deliverables

### 1. Lead/Worker Handoff
- define the delegated-task lifecycle more clearly
- support context transfer from lead to worker
- make ownership/routing changes explicit

### 2. Context Passing
- attach the right repo/team/task context during delegation
- capture why a worker received the task

### 3. Result Synthesis
- improve task summaries
- improve team report usefulness
- optionally use LLM-assisted summarization where bounded and valuable

### 4. Operator Transparency
- surface routing reasons and delegation reasons cleanly in CLI/TUI/reporting

## Likely Files

- `crates/awo-core/src/app/team_ops.rs`
- `crates/awo-core/src/team.rs`
- `crates/awo-core/src/context.rs`
- `crates/awo-app/src/handlers.rs`
- `crates/awo-app/src/tui.rs`
- `crates/awo-app/src/tui/action_router.rs`
- reporting docs/specs

## Risks

- richer delegation can blur the line between planning state and execution state
- automatic context passing can become opaque if not surfaced

## Mitigations

- keep task/task-owner history explicit
- surface delegation context in reports and task state
- test lead/worker handoff as workflows, not just field changes

## Verification

- team workflow tests
- delegation prompt/context tests
- report-generation tests
- manual multi-task mission runs on real repos

## Definition Of Done

- delegation feels like a first-class orchestration workflow
- the right context follows the task automatically
- reports help the operator understand outcomes without reading raw logs
