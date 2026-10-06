// ABOUTME: Versioned, content-addressed cache for rebuildable indexing artifacts.
// ABOUTME: Uses atomic replacement and bounded compressed entries; corrupt data is a cache miss.

use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ArtifactCache {
    root: PathBuf,
    max_entry_bytes: usize,
}

pub fn fingerprint(value: &impl Serialize) -> std::result::Result<String, serde_json::Error> {
    fn canonicalize(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                object.sort_keys();
                for value in object.values_mut() {
                    canonicalize(value);
                }
            }
            serde_json::Value::Array(array) => {
                for value in array {
                    canonicalize(value);
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(value)?;
    canonicalize(&mut value);
    Ok(blake3::hash(&serde_json::to_vec(&value)?)
        .to_hex()
        .to_string())
}

impl ArtifactCache {
    pub fn new(root: impl AsRef<Path>, namespace: &str) -> Self {
        // Namespace never controls a filesystem path: hashing keeps arbitrary callers scoped.
        Self {
            root: root
                .as_ref()
                .join(fingerprint(&namespace).expect("string fingerprint")),
            max_entry_bytes: 64 * 1024 * 1024,
        }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root
            .join(format!("{}.zst", blake3::hash(key.as_bytes()).to_hex()))
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let result = (|| -> anyhow::Result<T> {
            let file = std::fs::File::open(self.path(key))?;
            if file.metadata()?.len() > self.max_entry_bytes as u64 {
                anyhow::bail!("Oversized cache entry");
            }
            let decoder = zstd::stream::read::Decoder::new(file)?;
            let mut bytes = Vec::new();
            decoder
                .take(self.max_entry_bytes as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > self.max_entry_bytes {
                anyhow::bail!("Oversized decompressed entry");
            }
            Ok(serde_json::from_slice(&bytes)?)
        })();
        result.ok()
    }

    pub fn put(&self, key: &str, value: &impl Serialize) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() > self.max_entry_bytes {
            anyhow::bail!("Artifact exceeds cache entry budget");
        }
        let compressed = zstd::stream::encode_all(bytes.as_slice(), 1)?;
        std::fs::create_dir_all(&self.root)?;
        let path = self.path(key);
        let temporary = self.root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&compressed)?;
            // Rebuildable artifacts need atomic visibility, not an fsync per source file.
            drop(file);
            std::fs::rename(&temporary, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        Ok(result?)
    }

    pub fn remove(&self, key: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Bound all derived namespaces together; pending jobs are durable work, not artifacts.
    pub fn prune_tree(root: &Path, maximum_bytes: u64, protected: &[&str]) -> std::io::Result<()> {
        let excluded: Vec<_> = protected
            .iter()
            .map(|name| Self::new(root, name).root)
            .collect();
        let namespaces = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let mut files = Vec::new();
        let mut total = 0u64;
        for namespace in namespaces {
            let namespace = namespace?;
            if !namespace.file_type()?.is_dir() || excluded.contains(&namespace.path()) {
                continue;
            }
            for entry in std::fs::read_dir(namespace.path())? {
                let entry = entry?;
                if !entry.file_type()?.is_file()
                    || entry.path().extension().and_then(|value| value.to_str()) != Some("zst")
                {
                    continue;
                }
                let metadata = entry.metadata()?;
                total = total.saturating_add(metadata.len());
                files.push((metadata.modified()?, entry.path(), metadata.len()));
            }
        }
        files.sort();
        for (_, path, size) in files {
            if total <= maximum_bytes {
                break;
            }
            match std::fs::remove_file(path) {
                Ok(()) => total = total.saturating_sub(size),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Evict oldest artifacts at a run boundary, avoiding directory scans per write.
    pub fn prune(&self, maximum_bytes: u64) -> std::io::Result<()> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let mut files = Vec::new();
        let mut total = 0u64;
        for entry in entries {
            let entry = entry?;
            if entry.path().extension().and_then(|ext| ext.to_str()) != Some("zst") {
                continue;
            }
            let metadata = entry.metadata()?;
            total = total.saturating_add(metadata.len());
            files.push((metadata.modified()?, entry.path(), metadata.len()));
        }
        files.sort();
        for (_, path, size) in files {
            if total <= maximum_bytes {
                break;
            }
            match std::fs::remove_file(path) {
                Ok(()) => total = total.saturating_sub(size),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_artifacts_are_scoped_and_corruption_is_rebuildable() {
        let directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(directory.path(), "parser-v1");
        cache.put("key", &vec!["source", "data"]).unwrap();
        assert_eq!(
            cache.get::<Vec<String>>("key").unwrap(),
            vec!["source", "data"]
        );
        assert!(
            ArtifactCache::new(directory.path(), "parser-v2")
                .get::<Vec<String>>("key")
                .is_none()
        );
        std::fs::write(cache.path("key"), b"broken").unwrap();
        assert!(cache.get::<Vec<String>>("key").is_none());
        cache.put("key", &vec!["repaired"]).unwrap();
        cache.prune(0).unwrap();
        assert!(cache.get::<Vec<String>>("key").is_none());
    }

    #[test]
    fn aggregate_eviction_preserves_pending_work() {
        let root = tempfile::tempdir().unwrap();
        let artifact = ArtifactCache::new(root.path(), "derived");
        let job = ArtifactCache::new(root.path(), "pending-inference-v1");
        artifact.put("a", &"bytes").unwrap();
        job.put("job", &"resume").unwrap();
        ArtifactCache::prune_tree(root.path(), 0, &["pending-inference-v1"]).unwrap();
        assert!(artifact.get::<String>("a").is_none());
        assert_eq!(job.get::<String>("job").unwrap(), "resume");
    }
    #[test]
    fn fingerprints_include_task_identity_and_ignore_map_insertion_order() {
        let fingerprint = fingerprint(&("parser-v1", "model", "task", 384)).unwrap();
        assert_eq!(fingerprint.len(), 64);
        let a = std::collections::HashMap::from([("a", 1), ("b", 2)]);
        let b = std::collections::HashMap::from([("b", 2), ("a", 1)]);
        assert_eq!(
            super::fingerprint(&a).unwrap(),
            super::fingerprint(&b).unwrap()
        );
        assert_ne!(
            fingerprint,
            super::fingerprint(&("parser-v1", "model", "query", 384)).unwrap()
        );
    }
}
