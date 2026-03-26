# Awo Manual Testing Guide

Welcome back! The system has been hardened and verified with an end-to-end smoke test. You can now reliably use `awo` manually.

## 1. Quick Start (Smoke Test)
Run the automated smoke test to see everything in action (repo registration, daemon auto-start, team creation, and task execution):
```bash
./smoke_test.sh
```

## 2. Manual TUI Exploration
Launch the TUI to manage your workspace:
```bash
./target/debug/awo
```
**Keybindings:**
- `T` (Shift+T): Toggle the **Team Dashboard** (New!).
- `Tab`: Cycle through panels (Repos, Teams, Slots, Sessions).
- `a`: Register the current directory as a repository.
- `s`: (In Teams panel) Start the next pending task.
- `s`: (In Team Dashboard) Start the specifically selected task.
- `r`: Force-refresh the review state.
- `Enter`: (In Sessions panel) View logs for the selected session.
- `Esc`: Return to main view or clear filter.
- `/`: Enter filter mode to search for specific items.

## 3. CLI Power Usage
You can use the CLI while the TUI or Daemon is running. They all share the same state via the daemon.

**Try Delegating a Task:**
```bash
awo team task delegate <team_id> <task_id> <member_id> --notes "Lead notes here" --focus-file "src/lib.rs"
```

## 4. Platform Status
- **macOS/Linux**: Full support including auto-starting background daemons.
- **Windows**: Core orchestration and PTY supervision (ConPTY) are implemented. Daemon support uses the same logic, though verified primarily on Unix.

## Recent Stability Improvements
- **JSON-RPC 2.0 Contract**: Fully stabilized and verified with exhaustive round-trip tests.
- **Unified Overlap Detection**: Direct and soft (directory-level) overlaps are now detected and surfaced during review.
- **Path Consistency**: The daemon and CLI now explicitly synchronize their configuration and data directories.
