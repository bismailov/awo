# Agent-Neutral Context And Skills Strategy

## Purpose
Define a practical interoperability strategy for `awo` so project context, reusable skills, and tool integrations do not need to be rebuilt separately for every agent runtime.

## The Core Problem
There is still no single universal "skill installer" across Codex, Claude Code, Gemini CLI, and similar tools. Different clients expose different native mechanisms:
- project instruction files
- client-specific memory files
- skill catalogs
- MCP tools, resources, and prompts

The way out is not to pick one vendor format. It is to separate the problem into layers and keep the durable content in neutral formats.

## Recommended Neutral Stack

### 1. Repo Entry Point
Use a repo-level agent entry point as the predictable starting file.

Recommended shape:
- `AGENTS.md` as the ecosystem-facing entry point
- `project.md` as the detailed project brain
- thin vendor wrappers only where a client requires them, such as `CLAUDE.md` or `GEMINI.md`

Practical recommendation for repos that already centralize project instructions:
- keep `project.md` as the long-form project brain
- keep `AGENTS.md` thin and explicit: "read `project.md` first"
- keep `CLAUDE.md` and `GEMINI.md` as thin compatibility files when needed

## 2. Portable Skills
Keep reusable workflows in Agent Skills format:
- one skill directory per capability
- `SKILL.md` as the entry point
- optional `scripts/`, `references/`, and `assets/`

Recommended shared path:
- `.agents/skills/`

## 3. Tool And Resource Interop
Use MCP for tools, resources, and prompts.

This is a different layer from skills:
- Skills encode reusable workflows and procedural expertise.
- MCP exposes live tools, remote systems, data resources, and prompt entry points.

Recommended shared path:
- project `.mcp.json` for team-shared MCP servers

## 4. Context Library
Treat durable analysis and architecture material as a discoverable library, not as always-on prompt text.

Recommended classes:
- repo brain: `project.md`, `AGENTS.md`, `README.md`
- standards: design system, coding guides, testing guides
- domain docs: architecture, deployment, product docs
- analysis library: audits, remediation reports, refactor proposals, investigation notes
- task brief: the current task-specific plan or handoff note

This matters because `analysis/` is often valuable but too heavy to preload blindly.

## Recommended `awo` Product Behavior

### Discovery
`awo` should discover:
- `AGENTS.md`
- `project.md`
- `CLAUDE.md`
- `GEMINI.md`
- `.agents/skills/*/SKILL.md`
- `.mcp.json`
- optional knowledge libraries such as `docs/` and `analysis/`

### Normalization
`awo` should build a runtime-neutral view of a repo:
- entrypoint files
- required standards docs
- optional context packs
- portable skills catalog
- MCP config presence

### Runtime Adapters
Adapters should consume the normalized view differently:
- Codex: prefer `AGENTS.md`, plus selected context files and MCP hints
- Claude Code: respect `CLAUDE.md` and project `.mcp.json`
- Gemini CLI: use `context.fileName` support, skills support, and MCP settings

### Fallback For Non-Native Clients
If a client does not natively scan `.agents/skills/`, `awo` should offer:
- symlink or link into the client-native skill directory
- repo-local bootstrap or install scripts
- explicit "pass skill file as context" launch mode

Inference:
- Long term, `awo` could expose repo skills and context packs through a local MCP server, but this should be treated as an advanced compatibility layer, not the initial source of truth.

## A Better Mental Model
Do not ask:
"How do I install skills everywhere?"

Ask:
"Which content belongs in which neutral layer?"

The durable answer is:
- instructions and repo rules -> `AGENTS.md` and `project.md`
- reusable workflows -> `.agents/skills/`
- live tools and data -> `.mcp.json` and MCP servers
- historical analysis and audits -> discoverable context packs

## Proposed `awo` Features

### V1.5
- detect `AGENTS.md`, `project.md` or `PROJECT.md`, `CLAUDE.md`, `GEMINI.md`
- detect `.agents/skills/` and show a skill catalog
- detect `.mcp.json`
- let the user choose a context pack before launch

### V2
- `awo context doctor`
- `awo skills doctor`
- `awo skills link <runtime>`
- `awo skills sync <runtime>`
- `awo context pack build <repo> <task-type>`

### V3
- repo-local compatibility shims generated automatically
- optional local MCP bridge for context packs and skill activation
- organization-level shared skill registries

## Example Repo Fit
A well-structured multi-agent repo already follows this strategy well:
- `project.md` is the real project brain
- `AGENTS.md` and `CLAUDE.md` are thin wrappers
- `.agents/skills/` contains portable skill content
- `skills-lock.json` records provenance and hashes
- `analysis/` contains durable optional context

The main gap is not content design. It is orchestration and distribution.

That makes `awo` a strong fit:
- discover what already exists
- package it consistently per task
- bridge vendor-specific edges only when necessary

## Sources
- [AGENTS.md](https://agents.md/)
- [Agent Skills Overview](https://agentskills.io/)
- [How to add skills support to your agent](https://agentskills.io/integrate-skills)
- [OpenAI Docs MCP](https://developers.openai.com/learn/docs-mcp)
- [Claude Code MCP](https://code.claude.com/docs/en/mcp)
- [Gemini CLI README](https://github.com/google-gemini/gemini-cli)
- [Gemini CLI activate_skill](https://geminicli.com/docs/tools/activate-skill/)
