# Runtime Adapter Spec

## Purpose
Define a stable contract for integrating multiple AI CLIs without pretending they all behave the same way.

## Adapter Philosophy
The product should not have one fake universal "chat runtime" abstraction. It should have:
- a common lifecycle contract
- explicit capability flags
- runtime-specific launch preparation
- a normalized event stream for the orchestration layer

That lets the orchestrator stay generic while respecting real runtime differences.

## V1 Adapter Responsibilities
- detect whether the runtime is installed and callable
- describe capabilities
- construct the correct command and flags
- launch in a specific slot directory
- expose output as normalized events
- support stop/interrupt when possible
- return completion metadata

## Capability Model
Each adapter should expose a descriptor similar to:

```toml
id = "codex"
display_name = "Codex"
launch_mode = "oneshot"
needs_pty = true
supports_stdin = false
supports_interrupt = false
supports_resume = false
supports_structured_output = false
supports_remote = true
supports_read_only_hint = true
```

Key fields:
- `launch_mode`
  - `persistent`: one long-lived process receives multiple prompts
  - `oneshot`: one process per task or prompt
- `needs_pty`
  - true when the CLI expects terminal semantics
- `supports_stdin`
  - true when prompts can be sent programmatically after launch
- `supports_interrupt`
  - true when there is a meaningful stop/cancel signal
- `supports_resume`
  - true when session continuation is real rather than simulated
- `supports_structured_output`
  - true when machine-readable events are available
- `supports_remote`
  - true when the adapter can reasonably run on remote targets

## Core Adapter Interface
Conceptually, each adapter should implement:

### `detect()`
Returns:
- installed or not
- resolved binary path
- version info if available

### `describe()`
Returns capability metadata and human-facing labels.

### `prepare_launch(request)`
Input:
- slot path
- machine target
- model
- task brief
- context pack
- env overlay
- safety mode

Output:
- executable
- args
- env additions
- working directory
- launch strategy:
  - PTY
  - piped process
  - remote wrapper

### `launch(prepared)`
Starts the runtime and returns a session handle.

### `send(handle, input)`
Only valid for persistent runtimes that support stdin or similar request channels.

### `interrupt(handle)`
Requests graceful interruption if supported.

### `terminate(handle)`
Forcibly ends the process if needed.

### `parse_output(chunk)`
Turns runtime-specific output into normalized events.

### `finalize(handle)`
Returns completion metadata:
- exit code
- duration
- final status
- summary hints if available

## Normalized Event Model
The orchestration core should not depend on raw terminal text alone.

Suggested event types:
- `session_started`
- `session_status`
- `session_stdout`
- `session_stderr`
- `session_message`
- `session_warning`
- `session_error`
- `session_complete`
- `session_interrupted`
- `session_metadata`

### Event Payload Examples
```json
{ "type": "session_started", "session_id": "sess_123" }
{ "type": "session_status", "status": "working" }
{ "type": "session_stdout", "text": "Running tests..." }
{ "type": "session_complete", "exit_code": 0, "status": "succeeded" }
```

If a runtime offers structured output, `session_message` may carry typed content:
- assistant text
- markdown
- diff summary
- tool invocation summary

If it does not, the adapter can downgrade to raw stdout/stderr events.

## Launch Request Model
The adapter launch request should include:
- `repo_id`
- `slot_id`
- `slot_path`
- `branch`
- `machine_target`
- `runtime_id`
- `model`
- `task_brief_path`
- `context_files`
- `env_overlay`
- `port_assignments`
- `read_only`
- `approval_mode`

This keeps orchestration decisions outside the adapter while giving the adapter enough information to launch correctly.

## Safety And Approval Semantics
Different runtimes expose different approval-bypass behaviors. The orchestrator should model approval policy explicitly:
- `strict`
- `interactive`
- `workspace-write`
- `dangerous`

The adapter maps these modes to runtime-specific flags where possible.

Important:
- The adapter should never invent unsupported safety modes.
- If a requested mode is unsupported, launch should fail with a clear explanation.

## Persistent Vs One-Shot Behavior
### Persistent runtimes
Examples:
- long-lived stdin-based CLIs

Design implications:
- keep session handle alive
- allow multiple sends
- persist transcript/session cursor
- support resume when real

### One-shot runtimes
Examples:
- run a full task per invocation

Design implications:
- each task is a new process
- session history is product-managed, not runtime-managed
- "resume" means relaunching with previous context, not true continuation

This distinction must be visible in the UX so users are not misled.

## Remote Execution Model
Adapters should not own remote orchestration. The core should.

Meaning:
- machine targeting
- SSH or daemon transport
- remote working directory resolution
- remote environment setup

Should be handled by the orchestration layer.

The adapter simply prepares and launches within the environment it is given.

## V1 Adapter Set
Recommended initial adapters:
- Codex
- Claude Code
- Cursor Agent

Optional later:
- Gemini CLI
- OpenCode
- custom local agents

Recommendation:
- V1 should ship with a small number of well-supported adapters rather than a wide, shallow compatibility matrix.

## Testing Strategy For Adapters
Adapter tests should cover:
- runtime detection
- command construction
- capability declarations
- output parsing
- unsupported mode failures
- interruption behavior where available

Use fixture-based output samples for parsers and smoke tests for installed runtimes.

## Why This Contract Matters
- It avoids overfitting the entire product to one runtime.
- It makes mixed-runtime orchestration honest rather than hand-wavy.
- It supports later UI changes without rewriting runtime integrations.
