# Job Card M — Lead → Worker Task Delegation

## Objective

Add a `team.task.delegate` command that lets the lead agent assign a task to a specific worker with enriched context, optionally auto-starting the session. Currently `team.task.start` resolves the worker via routing preferences; this command allows explicit delegation with additional context the lead has gathered during planning.

## Motivation

The current `team.task.start` flow picks a worker based on routing rules, but a lead agent often knows *which* worker should handle a task and *why*, plus has planning context (file references, approach notes) that should travel with the delegation. This command bridges that gap.

## What to change

### 1. Add `DelegationContext` struct in `crates/awo-core/src/team.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationContext {
    /// The member_id of the worker being delegated to.
    pub target_member_id: String,
    /// Free-form notes from the lead to prepend to the worker's prompt.
    pub lead_notes: Option<String>,
    /// Specific files the lead wants the worker to focus on.
    pub focus_files: Vec<String>,
    /// Whether to auto-start a session after delegation.
    pub auto_start: bool,
}
```

### 2. Add `TeamTaskDelegateOptions` struct in `crates/awo-core/src/team.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskDelegateOptions {
    pub team_id: String,
    pub task_id: String,
    pub delegation: DelegationContext,
    pub strategy: String,
    pub dry_run: bool,
    pub launch_mode: String,
    pub attach_context: bool,
}
```

### 3. Add `Command::TeamTaskDelegate` variant in `crates/awo-core/src/commands.rs`

```rust
#[serde(rename = "team.task.delegate")]
TeamTaskDelegate { options: TeamTaskDelegateOptions },
```

Add matching arms to `method_name()` (`"team.task.delegate"`) and `run()` (delegates to `self.run_team_task_delegate(options)`).

### 4. Implement `run_team_task_delegate` in `crates/awo-core/src/commands/team.rs`

The flow:
1. Load the team manifest.
2. Find the task by `task_id` — error if not found or not in `Todo` state.
3. Find the target member by `delegation.target_member_id` — error if not found.
4. Set task `owner_id` to the target member (if different from current).
5. Set task state to `InProgress`.
6. If `delegation.lead_notes` is set, prepend it to the rendered prompt (use existing `render_task_prompt`).
7. If `delegation.focus_files` is non-empty, append a "Focus files" section to the prompt.
8. If `delegation.auto_start` is true, proceed with the same slot-acquire + session-start flow as `run_team_task_start`.
9. Save the manifest.
10. Emit `TeamTaskDelegated` event.

### 5. Add `TeamTaskDelegated` event in `crates/awo-core/src/events.rs`

```rust
TeamTaskDelegated {
    team_id: String,
    task_id: String,
    target_member_id: String,
    auto_started: bool,
},
```

Add the corresponding `to_message()` arm.

### 6. Add CLI subcommand in `crates/awo-app/src/cli.rs`

Add a `Delegate` variant to `TeamTaskCommand`:

```rust
Delegate {
    team_id: String,
    task_id: String,
    target_member_id: String,
    #[arg(long)]
    notes: Option<String>,
    #[arg(long)]
    focus_file: Vec<String>,
    #[arg(long, default_value = "true")]
    auto_start: bool,
    #[arg(long, default_value = "fresh")]
    strategy: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    launch_mode: Option<String>,
},
```

### 7. Wire CLI → Command in `crates/awo-app/src/handlers.rs`

Map the `Delegate` CLI variant to `Command::TeamTaskDelegate` in the existing team task match block.

### 8. Add MCP tool in `crates/awo-mcp/src/server.rs`

Add `delegate_team_task` tool definition and map it to `Command::TeamTaskDelegate`.

### 9. Add `render_task_prompt` enhancement in `crates/awo-core/src/team.rs`

Add a new function (or extend existing):
```rust
pub fn render_delegated_prompt(
    manifest: &TeamManifest,
    task: &TaskCard,
    delegation: &DelegationContext,
) -> String
```

This calls `render_task_prompt` and prepends lead notes / appends focus files.

## Files touched

| File | Change |
|------|--------|
| `crates/awo-core/src/team.rs` | Add `DelegationContext`, `TeamTaskDelegateOptions`, `render_delegated_prompt` |
| `crates/awo-core/src/commands.rs` | Add `Command::TeamTaskDelegate` variant + method_name + run arm |
| `crates/awo-core/src/commands/team.rs` | Implement `run_team_task_delegate` |
| `crates/awo-core/src/events.rs` | Add `TeamTaskDelegated` event + message |
| `crates/awo-app/src/cli.rs` | Add `Delegate` to `TeamTaskCommand` |
| `crates/awo-app/src/handlers.rs` | Wire `Delegate` → `Command::TeamTaskDelegate` |
| `crates/awo-mcp/src/server.rs` | Add `delegate_team_task` tool |

## Files NOT to touch

- `crates/awo-core/src/daemon.rs` — unrelated
- `crates/awo-core/src/app.rs` — no changes needed (dispatch flows through existing `Dispatcher`)
- `crates/awo-app/src/tui.rs` — no TUI changes in this card
- `crates/awo-core/src/team/reconcile.rs` — reconcile doesn't need to know about delegation
- `crates/awo-core/src/slot.rs` — slot mechanics unchanged

## Constraints

- `unsafe_code = "forbid"` workspace-wide
- Synchronous core — no Tokio, no async
- All state mutations through the command layer
- Follow the existing pattern in `run_team_task_start` for slot acquisition and session start
- The `auto_start: false` path must work cleanly — just delegate ownership and save the manifest without starting a session

## Tests

- Unit test in `commands/team.rs`: delegate assigns owner, sets state, emits event
- Unit test: delegate with `auto_start: false` does not start session
- Unit test: delegate to unknown member returns error
- Unit test: delegate task not in `Todo` state returns error
- MCP mapping test in `server.rs`

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Definition of done

- `team.task.delegate` command exists and is routed end-to-end (CLI → Command → Runner → Event)
- Delegation reassigns task owner and enriches the prompt with lead context
- `auto_start: true` acquires slot and starts session (reuses existing flow)
- `auto_start: false` just updates the manifest
- MCP tool `delegate_team_task` is registered
- All existing tests pass, no new clippy warnings
