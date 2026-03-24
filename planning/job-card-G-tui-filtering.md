# Job Card G: TUI Filtering & Search

## Objective
Add the ability to filter items in all TUI panels using a search query. This makes the TUI scalable for users with dozens of repositories, teams, or slots.

## Scope
**One file**: `crates/awo-app/src/tui.rs`

No changes to `awo-core`.

## What to build

### 1. Add Filter State to `TuiState`
Update `TuiState` to include:
```rust
filter_query: Option<String>,
```

### 2. Implement `/` Key for Search
When in `Normal` mode and `/` is pressed:
- Switch to `TextInput` mode with `prompt_label: "Filter: "`.
- On submit, set `state.filter_query = Some(input)`.
- If input is empty, set `state.filter_query = None`.

### 3. Apply Filter to Visibility Helpers
Update `visible_repos`, `visible_teams`, `visible_slots`, and `visible_sessions` to respect the `filter_query`.
- Match against ID or Name (case-insensitive).
- If `filter_query` is `None`, show all (current behavior).

### 4. Update UI to Show Active Filter
- In each panel's title, if a filter is active, append `(filter: "...")`.
- Add `Esc` or `Backspace` (when empty) to clear the current filter.

### 5. Keybinding Hint
Update the help bar to include `/ search`.

## Verification
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- Manual test: `awo tui`, press `/`, type a partial repo name, verify only matching repos (and their associated teams/slots) are shown.

## Constraints
- Do NOT change `awo-core`.
- Maintain the current panel focus and navigation logic.
- Ensure that if a filter results in an empty list, the selection indices are clamped to 0 correctly (the existing `clamp_selection` should handle this if visibility helpers are updated).
