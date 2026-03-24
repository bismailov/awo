# Awo Next Moves (March 24, 2026)

## Current Status
- **V1 Core Done**: Slot lifecycle, multi-runtime support, reviewed-based safety, and TUI control surface are fully operational.
- **Hardening Completed**: Typed state, SQLite WAL, negative-path tests, and cross-platform (Windows) parity are established.
- **Performance Optimized**: Background Git operations and status caching prevent TUI freezes.
- **Review Intelligence**: Multi-tiered conflict detection (file-level and directory-level) is live.

## Next Objectives: Wave 3 (Reliability & Coordination)

### 1. Robust Session Lifecycle (Timeout & Interruption)
Currently, sessions run until completion or manual cancellation. If an agent hangs, it consumes resources indefinitely. We need explicit timeout controls and cleaner process tree termination.

### 2. Team Execution Depth (Consolidation)
Moving from tracking tasks to consolidating results. Teams need a way to summarize what was achieved across multiple slots.

### 3. TUI Refinement (Filtering & Search)
As repositories and slots grow, navigating the TUI becomes harder. Inline filtering for all panels is the next usability step.

---

## The Plan

### Job Card G: TUI Filtering & Navigation (External Agent)
**Objective**: Add inline filtering to all TUI panels.
**Key Tasks**:
- Add `filter_query: Option<String>` to `TuiState`.
- Implement `/` key to enter filter mode (text input).
- Apply filter to `visible_repos`, `visible_teams`, etc.
- Highlight the filter string in the UI.

### Job Card H: Session Timeout & Interruption (Me)
**Objective**: Implement session-level timeouts and robust process termination.
**Key Tasks**:
- Add `timeout_secs: Option<u64>` to `SessionStart`.
- Update `SessionRecord` to track started_at and timeout.
- Implement timeout enforcement in `sync_session` (marks as Failed if exceeded).
- Improve `SessionCancel` to ensure child process groups are killed, not just the parent.
- Add "kill -9" fallback for stubborn supervisors.
