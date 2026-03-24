use crate::snapshot::{ReviewWarning, SlotReviewView};
use std::collections::HashMap;

pub fn detect_file_overlaps(slots: &[SlotReviewView]) -> Vec<ReviewWarning> {
    let mut by_repo: HashMap<&str, HashMap<&str, Vec<&str>>> = HashMap::new();
    for slot in slots {
        if !slot.dirty || slot.dirty_files.is_empty() {
            continue;
        }
        let repo_entry = by_repo.entry(slot.repo_id).or_default();
        for file in &slot.dirty_files {
            repo_entry.entry(file.as_str()).or_default().push(slot.id);
        }
    }
    let mut warnings = vec![];
    for (_repo, file_map) in by_repo {
        for (file, mut slot_ids) in file_map {
            if slot_ids.len() >= 2 {
                slot_ids.sort_unstable();
                let slot_list = slot_ids.join(", ");
                warnings.push(ReviewWarning {
                    kind: "file-overlap".to_string(),
                    slot_id: None,
                    session_id: None,
                    message: format!("File overlap: `{}` modified in slots {}", file, slot_list),
                });
            }
        }
    }
    warnings
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
    fn no_overlap_different_files_same_repo() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs"]),
            view("slot2", "repo1", vec!["src/main.rs"]),
        ];
        let warnings = detect_file_overlaps(&slots);
        assert!(warnings.is_empty());
    }

    #[test]
    fn single_file_overlap() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs"]),
            view("slot2", "repo1", vec!["src/lib.rs"]),
        ];
        let warnings = detect_file_overlaps(&slots);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "file-overlap");
        assert!(warnings[0].message.contains("src/lib.rs"));
        assert!(warnings[0].message.contains("slot1"));
        assert!(warnings[0].message.contains("slot2"));
    }

    #[test]
    fn multi_file_overlaps() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs", "src/error.rs"]),
            view("slot2", "repo1", vec!["src/lib.rs", "src/error.rs"]),
        ];
        let warnings = detect_file_overlaps(&slots);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn cross_repo_same_file_no_overlap() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs"]),
            view("slot2", "repo2", vec!["src/lib.rs"]),
        ];
        let warnings = detect_file_overlaps(&slots);
        assert!(warnings.is_empty());
    }

    #[test]
    fn three_way_overlap_one_warning() {
        let slots = vec![
            view("slot1", "repo1", vec!["src/lib.rs"]),
            view("slot2", "repo1", vec!["src/lib.rs"]),
            view("slot3", "repo1", vec!["src/lib.rs"]),
        ];
        let warnings = detect_file_overlaps(&slots);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("slot1"));
        assert!(warnings[0].message.contains("slot2"));
        assert!(warnings[0].message.contains("slot3"));
    }
}
