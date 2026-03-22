use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[derive(Debug, Clone)]
pub struct SlotRecord {
    pub id: String,
    pub repo_id: String,
    pub task_name: String,
    pub slot_path: String,
    pub branch_name: String,
    pub base_branch: String,
    pub strategy: String,
    pub status: String,
    pub fingerprint_hash: Option<String>,
    pub fingerprint_status: String,
    pub dirty: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl SlotRecord {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn is_released(&self) -> bool {
        self.status == "released"
    }

    pub fn is_missing(&self) -> bool {
        self.status == "missing"
    }

    pub fn uses_warm_strategy(&self) -> bool {
        self.strategy == SlotStrategy::Warm.as_str()
    }

    pub fn fingerprint_is_ready(&self) -> bool {
        self.fingerprint_status == "ready"
    }

    pub fn fingerprint_is_stale(&self) -> bool {
        self.fingerprint_status == "stale"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display, EnumString, IntoStaticStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SlotStrategy {
    Fresh,
    Warm,
}

impl SlotStrategy {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

pub fn build_slot_id(repo_id: &str, task_name: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}-{suffix}", repo_id, slugify(task_name))
}

pub fn build_branch_name(task_name: &str, slot_id: &str) -> String {
    let short_id = slot_id.chars().rev().take(6).collect::<String>();
    let short_id = short_id.chars().rev().collect::<String>();
    format!("awo/{}/{}", slugify(task_name), short_id)
}

pub fn build_slot_path(worktree_root: &Path, task_name: &str, slot_id: &str) -> PathBuf {
    let short_id = slot_id.chars().rev().take(6).collect::<String>();
    let short_id = short_id.chars().rev().collect::<String>();
    worktree_root.join(format!("{}-{}", slugify(task_name), short_id))
}

fn slugify(input: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    if output.is_empty() {
        "task".to_string()
    } else {
        output.trim_matches('-').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_slot(status: &str, strategy: &str, fingerprint_status: &str) -> SlotRecord {
        SlotRecord {
            id: "slot-1".to_string(),
            repo_id: "repo-1".to_string(),
            task_name: "task".to_string(),
            slot_path: "/tmp/slot".to_string(),
            branch_name: "awo/task/slot-1".to_string(),
            base_branch: "main".to_string(),
            strategy: strategy.to_string(),
            status: status.to_string(),
            fingerprint_hash: None,
            fingerprint_status: fingerprint_status.to_string(),
            dirty: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn slot_record_status_helpers_classify_known_states() {
        let active = sample_slot("active", "warm", "ready");
        assert!(active.is_active());
        assert!(active.uses_warm_strategy());
        assert!(active.fingerprint_is_ready());
        assert!(!active.is_released());
        assert!(!active.is_missing());
        assert!(!active.fingerprint_is_stale());

        let released = sample_slot("released", "fresh", "stale");
        assert!(released.is_released());
        assert!(released.fingerprint_is_stale());
        assert!(!released.is_active());
        assert!(!released.uses_warm_strategy());
    }
}
