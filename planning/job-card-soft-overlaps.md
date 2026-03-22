# Job Card: Implement "Soft Overlap" Detection (File Class Grouping)

## 1. Context & Motivation
We now detect direct file overlaps (e.g., both slots modified `src/error.rs`). However, we also want to catch "soft" overlaps where agents are working in the same module or directory (e.g., Slot A modified `src/runtime/executor.rs` and Slot B modified `src/runtime/supervisor.rs`). These "file class" overlaps are high-signal warnings for potential architectural conflicts.

## 2. Key Files
- `crates/awo-core/src/snapshot.rs`: `build_review_summary_impl` logic.
- `crates/awo-core/src/snapshot.rs`: `ReviewWarning` struct.

## 3. Implementation Steps
1. **Directory Extraction**: In `build_review_summary_impl`, for each dirty slot, derive a set of "file classes" (parent directory paths) from their `dirty_files`.
2. **Intersection Logic**:
   - Compare the directory sets of Slot A and Slot B.
   - If an intersection is found (e.g., both modified files in `crates/awo-core/src/runtime/`):
     - Check if the files involved are different (to avoid redundant reporting with `risky-overlap`).
     - Create a `ReviewWarning` with `kind: "soft-overlap"`.
     - Message: `"Slots '{id_a}' and '{id_b}' both modified files in module: {dir_path}"`.
3. **De-duplication**: Ensure that if two slots share an exact file, they get a `risky-overlap` warning, but only get a `soft-overlap` warning if they *also* share a directory where they modified *different* files.

## 4. Verification
- Add a unit test `detects_soft_overlap_between_modules` in `snapshot.rs`.
- Verify that a direct file overlap and a directory-level overlap can be reported together if they stem from different file sets.

## 5. Constraints
- Maintain the O(N^2) comparison for slots, but keep the directory set calculation efficient (e.g., pre-calculate directory sets once per slot).
- Do not perform any Git CLI calls inside the loop.
