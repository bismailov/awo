use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotStrategy {
    Fresh,
    Warm,
}

impl SlotStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Warm => "warm",
        }
    }
}

impl FromStr for SlotStrategy {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "fresh" => Ok(Self::Fresh),
            "warm" => Ok(Self::Warm),
            _ => Err(format!("unsupported slot strategy `{value}`")),
        }
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
