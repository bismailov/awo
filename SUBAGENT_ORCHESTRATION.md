# Subagent Orchestration

## Goal

Define how `awo` should think about subagents, multi-agent teams, and planning tools without locking itself to one vendor.

## Current Landscape

### Claude Code subagents

Claude Code has first-class subagents that run inside a single parent session. They are configured as Markdown files with YAML frontmatter and can be stored per-user or per-project.

Important characteristics:
- configured in `.claude/agents` or `~/.claude/agents`
- support `name`, `description`, `tools`, `disallowedTools`, `model`, `permissionMode`, `maxTurns`, `skills`, and `memory`
- automatic delegation is driven by the `description` field
- skills can be preloaded directly into the subagent
- subagents do not inherit skills automatically from the parent
- subagents cannot spawn other subagents

Implication for `awo`:
- this is a runtime capability, not the orchestration model itself
- `awo` should be able to launch a parent Claude session that has subagent affordances, but should not make the rest of the product depend on them

### Claude Code agent teams

Claude also has a larger-grain "agent teams" model that coordinates multiple sessions with a lead and teammates.

Important characteristics:
- experimental and disabled by default
- enabled via `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`
- uses a lead plus multiple teammate sessions with separate context windows
- teammates can communicate directly with each other
- best for parallel research, review, and work that can be cleanly partitioned
- Anthropic recommends starting with 3-5 teammates and avoiding file conflicts

Implication for `awo`:
- this is closer to `awo`'s own future team mode than subagents are
- `awo` should model this as a `multi_session_team` capability behind a generic interface

### OpenAI multi-agent patterns

OpenAI's official guidance treats multi-agent systems as an architecture choice, not a default. The main patterns are manager-style orchestration and handoffs between specialized agents.

Implication for `awo`:
- keep the orchestration layer generic
- prefer specialization, handoff, and clear ownership over uncontrolled "more agents"

### Sequential Thinking MCP

The official MCP servers repository includes `src/sequentialthinking`, an MCP server that provides a structured step-by-step reasoning tool. Its purpose is deliberate reasoning inside one agent run, not team coordination across branches or workspaces.

Important characteristics:
- provides a `sequential_thinking` tool
- supports revision, branching, and dynamic thought counts
- can be configured for Claude Desktop, VS Code, and Codex CLI
- useful for hard planning, diagnosis, and analysis tasks

Implication for `awo`:
- treat this as an optional reasoning aid
- do not confuse it with durable planning or multi-agent orchestration

## Sequential Thinking vs Planning With Files

These solve different problems.

### Sequential Thinking MCP

Best for:
- one agent working through a hard problem step by step
- revisable reasoning inside one run
- branching hypotheses and explicit course correction

Weaknesses:
- not durable by default across sessions
- poor as the main shared memory layer for a team
- not a work assignment system

### `planning-with-files`

Best for:
- persistent project memory
- multi-session continuity
- cross-agent coordination through durable artifacts
- explicit phase tracking, findings capture, and recovery after interruption

Weaknesses:
- weaker for rich in-run branching reasoning
- requires discipline to keep files updated

### Practical recommendation

Use them together:

1. `planning-with-files` for durable shared memory and task coordination
2. Sequential Thinking MCP inside individual agents when a subtask needs deliberate internal reasoning

In other words:
- sequential thinking helps an agent think
- planning-with-files helps a team remember

## Recommended `awo` Capability Model

`awo` should not hard-code "Claude subagents" as the product concept. It should model generic runtime capabilities such as:

- `inline_subagents`
- `multi_session_team`
- `skill_preload`
- `persistent_subagent_memory`
- `reasoning_tool_mcp`
- `handoff_or_delegate`

Then each runtime adapter can declare what it supports.

## Planning Several Agents In Parallel

The default team pattern should be:

1. Lead / planner
   - owns `task_plan.md`, decomposition, and file ownership

2. Researchers
   - read-only work
   - investigate architecture, bugs, external docs, or competing hypotheses

3. Implementers
   - one worktree slot each
   - one branch each
   - explicit file ownership

4. Verifier / reviewer
   - checks tests, risk, and integration readiness

5. Integrator
   - merges one branch at a time
   - resolves collisions
   - updates shared project memory

## Team Rules

- one write-capable agent per worktree
- one owner per hot file area
- shared context pack for every agent
- explicit output contract for every subtask
- review and merge happen after worker completion, not during

## What `awo` Should Add Next For Team Mode

### Team manifest

A durable record for:
- objective
- lead agent
- teammates
- slot/branch assignment
- file ownership
- context packs
- skills
- status

### Runtime capability registry

Each runtime should advertise:
- supports inline subagents or not
- supports multi-session teams or not
- supports skills preload or not
- supports persistent agent memory or not
- supports MCP reasoning tools or not

### Task decomposition schema

Each subtask should have:
- task id
- owner
- write scope
- slot id
- branch
- runtime
- deliverable
- verification rule

## Recommendation

For `awo`, the portable foundation should remain:

1. plan in files
2. decompose into owned subtasks
3. assign one slot per write-capable worker
4. let runtime-specific subagent features exist behind adapters

That gives us one stable orchestration model even while Claude, Codex, Gemini, or future runtimes differ dramatically in their native delegation features.

## Sources

- Claude Code subagents: https://docs.anthropic.com/en/docs/claude-code/sub-agents
- Claude Code agent teams: https://code.claude.com/docs/en/agent-teams
- OpenAI Agents multi-agent guide: https://openai.github.io/openai-agents-js/guides/multi-agent/
- OpenAI practical guide to building agents: https://cdn.openai.com/business-guides-and-resources/a-practical-guide-to-building-agents.pdf
- MCP sequential thinking server: https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking
