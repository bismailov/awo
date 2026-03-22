# Job Card: Implement "Risky Overlap" Detection in Review Engine

## 1. Context & Motivation
Currently, `awo` tracks when slots are "dirty" (contain file changes), but it doesn't analyze **what** changed. In a multi-agent environment, the biggest risk is two agents modifying the same file or closely related files (e.g., `src/lib.rs` and `src/error.rs`) in separate slots.

This task is to implement a "Risky Overlap" detection system that compares dirty files across active slots and surfaces warnings in the `ReviewSummary`.

## 2. Key Files & Symbols
- `crates/awo-core/src/snapshot.rs`: Contains `build_review_summary_impl` and the `ReviewWarning` struct.
- `crates/awo-core/src/slot.rs`: `SlotRecord` struct.
- `crates/awo-core/src/git.rs`: Useful for getting dirty file lists if needed (though we should prefer using existing metadata if possible).

## 3. Implementation Steps

### Phase 1: Data Model Expansion
- Update the `SlotReviewView` internal struct in `snapshot.rs` to include an optional list of dirty files: `dirty_files: Vec<String>`.
- Update `build_review_summary` and `build_review_summary_from_summaries` to populate this field. 
- *Note*: You may need to add a `dirty_files` field to `SlotRecord` or `SlotSummary` first if it's missing. Check `crates/awo-core/src/slot.rs` and `crates/awo-core/src/snapshot.rs`.

### Phase 2: Overlap Detection Logic
In `build_review_summary_impl` (in `snapshot.rs`):
- After processing sessions and slots, implement an O(N^2) comparison between all "dirty" slots.
- For every pair of slots (Slot A, Slot B), check if their `dirty_files` intersect.
- If an intersection is found (e.g., both modified `crates/awo-core/src/error.rs`):
    - Create a `ReviewWarning` with `kind: "risky-overlap"`.
    - Message format: `"Slots '{id_a}' and '{id_b}' both modified: {file_list}"`.

### Phase 3: Classification (Stretch)
- If a file isn't an exact match, but both slots modified files in the same "class" (e.g., both modified files in `crates/awo-core/src/runtime/`), surface a lower-priority warning or group them.

## 4. Verification & Testing
- Add a unit test in `crates/awo-core/src/snapshot.rs` (or a new test file) that:
    1. Creates two `SlotReviewView` objects with overlapping dirty files.
    2. Runs `build_review_summary_impl`.
    3. Asserts that a `"risky-overlap"` warning is present in the resulting `ReviewSummary`.
- Run `cargo test -p awo-core` to ensure no regressions in the review engine.

## 5. Constraints
- **Do not** perform Git CLI calls inside the `build_review_summary_impl` loop. This function must remain a pure "view builder" that operates on snapshots/records already in memory.
- If `SlotRecord` doesn't have the dirty file list yet, you may need to add a small helper in `crates/awo-core/src/git.rs` to fetch it during the *initial* snapshot load, but keep the `ReviewSummary` calculation logic clean.
