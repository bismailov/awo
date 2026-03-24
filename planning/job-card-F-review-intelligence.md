# Job Card F: Review Intelligence — Changed-File Overlap Detection

## Objective

Upgrade the review engine to detect when two active slots have modified the same files, producing actionable overlap warnings. Currently the review engine detects "risky overlap" by slot-level heuristics (same repo, both dirty), but it cannot tell the operator *which files* conflict. This card adds file-level overlap detection.

## Scope

**Files to modify**:
- `crates/awo-core/src/snapshot.rs` — add file-level overlap analysis to review summary building
- `crates/awo-core/src/snapshot.rs` (or a new `crates/awo-core/src/snapshot/overlap.rs`) — overlap detection logic

**Files to read** (not modify):
- `crates/awo-core/src/git.rs` — `dirty_files()` already returns `Vec<String>` of changed file paths per slot
- `crates/awo-core/src/snapshot.rs` — `DirtyFileCache`, `build_review_summary()`, `ReviewWarning`

## What to build

### 1. File-level overlap detection

After `build_review_summary()` collects dirty files per slot (already cached via `DirtyFileCache`), compare file lists across all active slots within the same repo. If two or more slots have modified the same file, emit a `ReviewWarning`.

```rust
// Pseudocode for the overlap check
fn detect_file_overlaps(slots: &[SlotReviewEntry]) -> Vec<ReviewWarning> {
    // Group slots by repo_id
    // For each repo, build a map: file_path -> Vec<slot_id>
    // For each file touched by 2+ slots, emit a warning
}
```

### 2. New warning variant

Add a new `ReviewWarning` message pattern for file overlaps:

```
"File overlap: `src/main.rs` modified in slots `slot-abc`, `slot-def`"
```

Group by file, not by slot pair — if 3 slots touch the same file, emit one warning listing all 3.

### 3. Integrate into `build_review_summary()`

Call the overlap detection after collecting per-slot dirty files. Append any overlap warnings to the existing `warnings` vec in `ReviewSummary`.

### 4. Add tests

Add tests in `snapshot.rs` (existing test module):

- **No overlap**: Two slots in the same repo with different dirty files → no overlap warning.
- **Single file overlap**: Two slots both modified `src/lib.rs` → one overlap warning mentioning both slot IDs.
- **Multiple file overlaps**: Two slots share 3 files → 3 overlap warnings.
- **Cross-repo no overlap**: Two slots in *different* repos with the same filename → no warning (different repos are independent).
- **Three-way overlap**: Three slots all touch the same file → one warning listing all 3 slot IDs.

Target: 5-7 new tests.

## Design notes

- The dirty file data is already available: `DirtyFileCache::get_or_refresh()` returns `Vec<String>` per slot, and `build_review_summary()` already iterates all active slots.
- The overlap check happens *after* dirty file collection, so it adds no extra git calls.
- Keep the overlap detection as a pure function that takes `&[(slot_id, repo_id, &[String])]` and returns warnings. This makes it easy to test without a full store/snapshot setup.
- The existing `ReviewWarning` struct has a `message: String` field and a `severity: WarningSeverity` enum. File overlaps should use `WarningSeverity::Risk` (same as existing risky-overlap warnings).

## Constraints

- Do NOT add any new crate dependencies.
- Do NOT modify `git.rs` — use the existing `dirty_files()` output.
- Do NOT modify `tui.rs` — the TUI already renders `ReviewWarning` messages.
- Do NOT modify the `DirtyFileCache` — it already provides what you need.
- Keep the overlap logic as a pure function for testability.
- The detection must work with the cached file lists (no extra git calls).

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

All existing tests must continue to pass. New tests should add 5-7 tests.

## What NOT to do

- Do not add semantic/AST-level conflict detection — just filename-level overlap
- Do not add git merge conflict detection
- Do not modify the TUI rendering
- Do not add file watching or change tracking beyond what `git status --porcelain` provides
- Do not add async or background processing for the overlap check
