// ABOUTME: Complete-project input fingerprints and durable reconciliation catalogs.
// ABOUTME: Cached ASTs supply unchanged callers when definitions move or disappear.

use anyhow::Result;
use codegraph_core::{CodeNode, artifact_cache::fingerprint};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Catalog {
    pub fingerprint: String,
    pub nodes: BTreeMap<String, String>,
    pub edges: BTreeSet<String>,
    pub chunks: BTreeSet<String>,
    #[serde(default)]
    pub chunk_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub file_metadata_hashes: BTreeMap<String, String>,
    pub stats: crate::indexer::IndexStats,
}

pub(crate) fn node_digest(node: &CodeNode) -> Result<String> {
    let mut value = serde_json::to_value(node)?;
    if let Some(metadata) = value.get_mut("metadata").and_then(|v| v.as_object_mut()) {
        metadata.remove("created_at");
        metadata.remove("updated_at");
    }
    Ok(fingerprint(&value)?)
}

/// Stream artifact identity rather than materializing a potentially large compiler index.
pub(crate) fn file_fingerprint(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn support_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    matches!(
        name,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "tsconfig.json"
            | "pyproject.toml"
            | "requirements.txt"
            | "codegraph.boundaries.toml"
            | "config.toml"
    ) || matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("md" | "surql")
    )
}

pub(crate) fn support_fingerprints(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".codegraph" | "target" | "node_modules" | ".venv")
            )
        })
    {
        let entry = entry?;
        if entry.file_type().is_file() && support_file(entry.path()) {
            files.insert(
                entry.path().to_string_lossy().into_owned(),
                file_fingerprint(entry.path())?,
            );
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_and_document_changes_invalidate_inputs_but_cache_files_do_not() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "old").unwrap();
        let before = support_fingerprints(root.path()).unwrap();
        std::fs::create_dir(root.path().join(".codegraph")).unwrap();
        std::fs::write(root.path().join(".codegraph/cache.md"), "cache").unwrap();
        assert_eq!(before, support_fingerprints(root.path()).unwrap());
        std::fs::write(root.path().join("Cargo.toml"), "new").unwrap();
        assert_ne!(before, support_fingerprints(root.path()).unwrap());
        std::fs::write(root.path().join("README.md"), "docs").unwrap();
        assert_eq!(support_fingerprints(root.path()).unwrap().len(), 2);
    }
}
