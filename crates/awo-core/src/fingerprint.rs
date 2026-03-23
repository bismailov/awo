use crate::error::{AwoError, AwoResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const FINGERPRINT_FILES: &[&str] = &[
    "yarn.lock",
    "pnpm-lock.yaml",
    "package-lock.json",
    "Cargo.lock",
    "uv.lock",
    "poetry.lock",
    "package.json",
];

#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub hash: Option<String>,
    pub files: Vec<String>,
}

pub fn fingerprint_for_dir(root: &Path) -> AwoResult<Fingerprint> {
    let mut hasher = Sha256::new();
    let mut files = Vec::new();

    for relative in FINGERPRINT_FILES {
        let path = root.join(relative);
        if path.is_file() {
            hasher.update(relative.as_bytes());
            let contents = fs::read(&path)
                .map_err(|source| AwoError::io("read fingerprint file", &path, source))?;
            hasher.update(contents);
            files.push(relative.to_string());
        }
    }

    let hash = if files.is_empty() {
        None
    } else {
        Some(format!("{:x}", hasher.finalize()))
    };

    Ok(Fingerprint { hash, files })
}
