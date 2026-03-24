# Job Card E: Negative-Path Test Coverage

## Objective

Add failure-path tests for the three core modules with the thinnest coverage relative to their complexity: `store.rs`, `commands/`, and `snapshot.rs`. The happy path is well-tested (356+ tests). The failure path is thin — corrupt state, invalid inputs, missing resources, and broken filesystem state are barely exercised.

## Scope

**Files to modify** (tests only — no production code changes):
- `crates/awo-core/src/store/tests.rs` — currently 11 tests for 676 LOC of production code
- `crates/awo-core/src/commands/` — currently 0 tests for 1450 LOC across 6 submodules
- `crates/awo-core/src/snapshot.rs` — currently 11 tests, but no failure-path tests

**Do NOT modify any production code.** This card is test-only.

## What to build

### 1. Store negative-path tests (`store/tests.rs`)

Add tests for:

- **Missing or corrupt DB**: Open a store with a path to a non-existent directory. Verify error is `AwoError` (not a panic).
- **Get nonexistent records**: `get_repository("nonexistent")` returns `Ok(None)`, not an error.
- **Delete nonexistent slot**: `delete_slot("nonexistent")` should not panic or error.
- **Duplicate repo registration**: Insert the same repo twice — verify the second call either updates or returns a meaningful error.
- **Query with empty DB**: All listing functions (`list_repositories`, `list_slots`, `list_sessions`) return empty vecs on a fresh store.

Target: 5-8 new tests.

### 2. Command layer tests (`commands/`)

The command layer (`commands.rs` + 6 submodules: `repo.rs`, `slot.rs`, `session.rs`, `review.rs`, `context.rs`, `skills.rs`) has **zero tests**. Commands are tested indirectly through `app/tests.rs`, but the command runners themselves have no unit tests.

Create a new file `crates/awo-core/src/commands/tests.rs` and add the `#[cfg(test)] mod tests;` to `commands.rs`.

Add tests for:

- **SlotAcquire with nonexistent repo**: Should return `AwoError::UnknownRepo`.
- **SlotRelease with nonexistent slot**: Should return an appropriate error.
- **SessionStart with nonexistent slot**: Should return an appropriate error.
- **SessionCancel with nonexistent session**: Should return an appropriate error.
- **SessionLog with nonexistent session**: Should return an appropriate error.
- **RepoAdd with nonexistent path**: Should return an appropriate error.
- **ContextDoctor with nonexistent repo**: Should return `AwoError::UnknownRepo`.
- **SkillsDoctor with nonexistent repo**: Should return `AwoError::UnknownRepo`.

Each test should construct a `CommandRunner` with a fresh temp-dir store and verify the error variant. Follow the pattern in `app/tests.rs` for test setup.

Target: 8-12 new tests.

### 3. Snapshot failure-path tests (`snapshot.rs`)

Add tests for:

- **Slot with missing worktree path**: A slot record pointing to a path that doesn't exist on disk. `build_review_summary` should produce a warning, not panic.
- **DirtyFileCache with expired entry**: Insert a cache entry, advance past TTL, verify `get_or_refresh` calls git again.
- **DirtyFileCache retain_slots**: Insert entries for 3 slots, retain only 2, verify the third is gone.
- **DirtyFileCache invalidate**: Insert an entry, invalidate it, verify next call refreshes.

Target: 4-6 new tests.

## Constraints

- **Do NOT modify any production code** — only add test files and `#[cfg(test)]` module declarations.
- Do NOT add any new crate dependencies.
- All tests must use `AwoResult` or `anyhow::Result` return types (not `.unwrap()` in assertions on fallible operations).
- Use `tempfile::TempDir` for filesystem isolation (already a dev-dependency).
- Follow the existing test patterns in `app/tests.rs` and `store/tests.rs` for setup helpers.
- Tests must pass on all three CI platforms (macOS, Ubuntu, Windows).

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

All 356+ existing tests must continue to pass. The new tests should add 17-26 tests.

## What NOT to do

- Do not refactor production code
- Do not add integration/e2e tests — focus on unit tests
- Do not add async test infrastructure
- Do not modify `app/tests.rs` — write new tests in the appropriate module test files
- Do not add test utilities or shared test helper crates
