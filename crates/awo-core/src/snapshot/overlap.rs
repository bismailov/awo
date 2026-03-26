use crate::snapshot::{ReviewWarning, SlotReviewView};
use std::collections::HashSet;
use std::path::Path;

pub fn detect_overlaps(slots: &[SlotReviewView]) -> Vec<ReviewWarning> {
    let mut warnings = vec![];
    let mut dirty_slots: Vec<&SlotReviewView> = slots
        .iter()
        .filter(|s| s.dirty && !s.dirty_files.is_empty())
        .collect();

    // Sort for deterministic pair comparison
    dirty_slots.sort_by_key(|s| s.id);

    for i in 0..dirty_slots.len() {
        for j in i + 1..dirty_slots.len() {
            let slot_a = dirty_slots[i];
            let slot_b = dirty_slots[j];

            // Only compare within same repo
            if slot_a.repo_id != slot_b.repo_id {
                continue;
            }

            let files_a: HashSet<&str> = slot_a.dirty_files.iter().map(|s| s.as_str()).collect();
            let files_b: HashSet<&str> = slot_b.dirty_files.iter().map(|s| s.as_str()).collect();

            // 1. Direct file overlap (risky-overlap)
            let mut common_files: Vec<&str> = files_a.intersection(&files_b).copied().collect();
            if !common_files.is_empty() {
                common_files.sort();
                warnings.push(ReviewWarning {
                    kind: "risky-overlap".to_string(),
                    slot_id: None,
                    session_id: None,
                    message: format!(
                        "Slots '{}' and '{}' both modified: {}",
                        slot_a.id,
                        slot_b.id,
                        common_files.join(", ")
                    ),
                });
            }

            // 2. Directory-level overlap (soft-overlap)
            let dirs_a: HashSet<String> = get_parent_dirs(&files_a);
            let dirs_b: HashSet<String> = get_parent_dirs(&files_b);

            let mut common_dirs: Vec<String> = dirs_a.intersection(&dirs_b).cloned().collect();
            common_dirs.sort();

            for dir in common_dirs {
                // Check if they modified DIFFERENT files in this directory
                let files_in_dir_a: Vec<&str> = files_a
                    .iter()
                    .filter(|f| is_in_dir(f, &dir))
                    .copied()
                    .collect();
                let files_in_dir_b: Vec<&str> = files_b
                    .iter()
                    .filter(|f| is_in_dir(f, &dir))
                    .copied()
                    .collect();

                let set_a: HashSet<&str> = files_in_dir_a.into_iter().collect();
                let set_b: HashSet<&str> = files_in_dir_b.into_iter().collect();

                // If sets are not identical, it's a soft overlap
                // (if they were identical, it was already reported as risky-overlap for every file)
                if set_a != set_b {
                    warnings.push(ReviewWarning {
                        kind: "soft-overlap".to_string(),
                        slot_id: None,
                        session_id: None,
                        message: format!(
                            "Slots '{}' and '{}' both modified files in module: {}",
                            slot_a.id, slot_b.id, dir
                        ),
                    });
                }
            }
        }
    }

    warnings
}

fn get_parent_dirs(files: &HashSet<&str>) -> HashSet<String> {
    let mut dirs = HashSet::new();
    for file in files {
        if let Some(parent) = Path::new(file).parent()
            && let Some(parent_str) = parent.to_str()
            && !parent_str.is_empty()
        {
            dirs.insert(parent_str.to_string());
        }
    }
    dirs
}

fn is_in_dir(file: &str, dir: &str) -> bool {
    Path::new(file)
        .parent()
        .and_then(|p| p.to_str())
        .is_some_and(|p| p == dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::SlotReviewView;

    fn view<'a>(id: &'a str, repo_id: &'a str, dirty_files: Vec<&'a str>) -> SlotReviewView<'a> {
        SlotReviewView {
            id,
            repo_id,
            is_active: true,
            is_released: false,
            is_missing: false,
            dirty: true,
            dirty_files: dirty_files.into_iter().map(|s| s.to_string()).collect(),
            uses_warm_strategy: false,
            fingerprint_is_ready: true,
            fingerprint_is_stale: false,
        }
    }

    #[test]
    fn detects_risky_overlap() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs", "src/error.rs"]),
            view("slot2", "repo1", vec!["src/main.rs", "src/error.rs"]),
        ];
        let warnings = detect_overlaps(&slots);
        assert!(warnings.iter().any(|w| w.kind == "risky-overlap"));
        assert!(warnings[0].message.contains("src/error.rs"));
    }

    #[test]
    fn detects_soft_overlap() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/runtime/executor.rs"]),
            view("slot2", "repo1", vec!["src/runtime/supervisor.rs"]),
        ];
        let warnings = detect_overlaps(&slots);
        assert!(warnings.iter().any(|w| w.kind == "soft-overlap"));
        assert!(warnings[0].message.contains("src/runtime"));
    }

    #[test]
    fn no_double_reporting_for_identical_files_in_dir() {
        // If they both modified exactly the same files in a dir, it's risky but not soft?
        // Actually if they are IDENTICAL sets in a dir, we don't report soft.
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs"]),
            view("slot2", "repo1", vec!["src/lib.rs"]),
        ];
        let warnings = detect_overlaps(&slots);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "risky-overlap");
    }
}
