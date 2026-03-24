# Job Card A: TUI Input Prompts & Help Overlay

## Objective

Make the TUI usable for real work by adding text input for operator-driven actions and a keybinding help overlay. Currently the TUI hardcodes task names and prompts — the operator cannot enter their own values.

## Scope

**One file**: `crates/awo-app/src/tui.rs` (972 LOC)

No changes to `awo-core`, no changes to CLI handlers, no changes to tests outside of TUI.

## What to build

### 1. Inline text input widget

Add a minimal text input mode to `TuiState`:

```
enum InputMode {
    Normal,
    TextInput { prompt_label: String, buffer: String, on_submit: InputAction },
}

enum InputAction {
    AcquireSlot,        // buffer → task_name
    StartSession,       // buffer → prompt (runtime defaults to Shell)
    StartSessionCustom, // buffer → "runtime:prompt" format
}
```

When `InputMode::TextInput` is active:
- Render a single-line input bar at the bottom of the screen (above the status line)
- `Enter` submits the buffer and dispatches the appropriate `Command`
- `Esc` cancels and returns to `Normal`
- Printable characters append to buffer, `Backspace` removes last char

### 2. Wire input to existing keybindings

- `s` (acquire slot): Instead of dispatching immediately with `"tui-task"`, enter `TextInput` mode with label `"Task name: "` and action `AcquireSlot`. On submit, dispatch `Command::SlotAcquire` with the buffer as `task_name`.

- `Enter` on a slot (start session): Enter `TextInput` mode with label `"Prompt: "` and action `StartSession`. On submit, dispatch `Command::SessionStart` with `RuntimeKind::Shell` and the buffer as `prompt`. If the user wants a different runtime, they can prefix with `claude:`, `codex:`, `gemini:` — parse this in the submit handler.

- `Enter` on a session: Keep current behavior (view log) — no input needed.

### 3. Help overlay

- `?` key toggles a help overlay panel
- The overlay is a centered box listing all keybindings in two columns
- Any key dismisses it

Keybinding reference to show:
```
s       Acquire slot (enter task name)
Enter   Start session / View log
x       Cancel session
X       Release slot
t       Start next team task
a       Add current dir as repo
r       Refresh review / Refresh log
c       Context doctor
d       Skills doctor
Tab     Next panel
Esc     Close panel / Cancel input
q       Quit
?       This help
```

## Constraints

- Do NOT add any new crate dependencies. `crossterm` already supports text input via `KeyCode::Char`.
- Do NOT restructure the render function — add the input bar and help overlay as additional draw calls on top of the existing layout.
- Keep it simple. A single-line input bar is sufficient. No multi-line editor, no tab completion, no history.
- The `TuiState` struct must remain `#[derive(Debug)]`.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Then manual smoke test:
1. `awo repo add .`
2. `awo tui`
3. Press `?` → help overlay appears → press any key to dismiss
4. Press `s` → input bar appears → type task name → Enter → slot acquired
5. Select slot → press `Enter` → input bar appears → type `echo hello` → Enter → session starts
6. Press `Esc` during input → cancels without action

## What NOT to do

- Do not add background threading or async — that's a separate job
- Do not modify `awo-core` or any core commands
- Do not add new CLI subcommands
- Do not refactor the existing render layout
- Do not add session log tailing — that's separate
