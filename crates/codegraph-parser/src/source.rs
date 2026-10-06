// ABOUTME: Immutable source snapshots shared by parsing and downstream analyzers.
// ABOUTME: Bounds retained source memory by spilling oversized snapshots to temporary files.

use codegraph_core::{CodeGraphError, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug)]
struct SpillDirectory(PathBuf);

impl Drop for SpillDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug)]
enum SourceContents {
    Memory(Arc<str>),
    Disk(PathBuf, Arc<SpillDirectory>),
}

#[derive(Clone, Debug)]
pub struct SourceSnapshot {
    pub path: PathBuf,
    pub content_hash: String,
    pub modified_at: SystemTime,
    pub size: u64,
    pub lines: usize,
    contents: SourceContents,
}

impl SourceSnapshot {
    pub fn contents(&self) -> Result<Arc<str>> {
        match &self.contents {
            SourceContents::Memory(source) => Ok(source.clone()),
            SourceContents::Disk(path, _lifetime) => Ok(std::fs::read_to_string(path)
                .map_err(CodeGraphError::Io)?
                .into()),
        }
    }

    pub async fn contents_async(&self) -> Result<Arc<str>> {
        match &self.contents {
            SourceContents::Memory(source) => Ok(source.clone()),
            SourceContents::Disk(path, _lifetime) => Ok(tokio::fs::read_to_string(path)
                .await
                .map_err(CodeGraphError::Io)?
                .into()),
        }
    }
}

/// A run sees exactly the bytes it hashed, even if an editor changes the working tree.
/// Source strings retained by this set never exceed `memory_budget`; spilled files are
/// private to the run and removed when the last snapshot is dropped.
#[derive(Clone, Debug, Default)]
pub struct SourceSnapshots {
    files: BTreeMap<PathBuf, SourceSnapshot>,
    pub retained_bytes: usize,
    pub read_operations: u64,
    pub read_bytes: u64,
    pub spilled_bytes: u64,
}

impl SourceSnapshots {
    pub async fn capture(
        files: &[(PathBuf, u64)],
        memory_budget: usize,
        workers: usize,
    ) -> Result<Self> {
        use futures::{StreamExt, stream};
        let mut paths: Vec<_> = files.iter().map(|(path, _)| path.clone()).collect();
        paths.sort();
        let spill = Arc::new(SpillDirectory(
            std::env::temp_dir().join(format!("codegraph-sources-{}", uuid::Uuid::new_v4())),
        ));
        let mut out = Self::default();
        // Ordered buffering makes the retained/spilled partition reproducible while reads
        // overlap. At most `workers` additional file buffers are in flight.
        let metrics = Arc::new((
            std::sync::atomic::AtomicU64::new(0),
            std::sync::atomic::AtomicU64::new(0),
        ));
        let read_metrics = metrics.clone();
        let mut reads = stream::iter(paths.into_iter().map(move |path| {
            let metrics = read_metrics.clone();
            async move {
                for _ in 0..3 {
                    let before = tokio::fs::metadata(&path).await?;
                    let source = tokio::fs::read_to_string(&path).await?;
                    metrics.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    metrics
                        .1
                        .fetch_add(source.len() as u64, std::sync::atomic::Ordering::Relaxed);
                    let after = tokio::fs::metadata(&path).await?;
                    if before.len() == after.len() && before.modified()? == after.modified()? {
                        return Ok::<_, std::io::Error>((path, source, after));
                    }
                }
                Err(std::io::Error::other(format!(
                    "Source changed while reading: {}",
                    path.display()
                )))
            }
        }))
        .buffered(workers.max(1));
        while let Some(result) = reads.next().await {
            let (path, source, metadata) = result.map_err(CodeGraphError::Io)?;
            let content_hash = Sha256::digest(source.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let lines = source.lines().count();
            let size = source.len() as u64;
            let contents = if source.len() <= memory_budget.saturating_sub(out.retained_bytes) {
                out.retained_bytes += source.len();
                SourceContents::Memory(source.into())
            } else {
                tokio::fs::create_dir_all(&spill.0)
                    .await
                    .map_err(CodeGraphError::Io)?;
                let spill_path = spill.0.join(&content_hash);
                tokio::fs::write(&spill_path, source.as_bytes())
                    .await
                    .map_err(CodeGraphError::Io)?;
                out.spilled_bytes += size;
                SourceContents::Disk(spill_path, spill.clone())
            };
            out.files.insert(
                path.clone(),
                SourceSnapshot {
                    path,
                    content_hash,
                    modified_at: metadata.modified().map_err(CodeGraphError::Io)?,
                    size,
                    lines,
                    contents,
                },
            );
        }
        out.read_operations = metrics.0.load(std::sync::atomic::Ordering::Relaxed);
        out.read_bytes = metrics.1.load(std::sync::atomic::Ordering::Relaxed);
        Ok(out)
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&SourceSnapshot> {
        self.files.get(path.as_ref())
    }

    /// Reject readiness when a captured file changed or disappeared during indexing.
    pub async fn validate_current(&self) -> Result<()> {
        for source in self.iter() {
            let metadata = tokio::fs::metadata(&source.path)
                .await
                .map_err(CodeGraphError::Io)?;
            if metadata.len() != source.size
                || metadata.modified().map_err(CodeGraphError::Io)? != source.modified_at
            {
                return Err(CodeGraphError::Parse(format!(
                    "Source changed during indexing: {}. Retry reconciliation.",
                    source.path.display()
                )));
            }
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceSnapshot> {
        self.files.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_retains_hashed_bytes_after_file_change_and_spills() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.rs");
        tokio::fs::write(&path, "fn a() {}\n\nfn b() {}\n")
            .await
            .unwrap();
        let snapshots = SourceSnapshots::capture(&[(path.clone(), 0)], 0, 2)
            .await
            .unwrap();
        tokio::fs::write(&path, "changed").await.unwrap();
        let snapshot = snapshots.get(&path).unwrap();
        assert_eq!(
            snapshot.contents().unwrap().as_ref(),
            "fn a() {}\n\nfn b() {}\n"
        );
        assert!(snapshots.validate_current().await.is_err());
        assert_eq!(snapshots.read_operations, 1);
        assert_eq!(snapshots.read_bytes, snapshot.size);
        assert_eq!(snapshot.lines, 3);
        assert_eq!(snapshots.retained_bytes, 0);
        assert_eq!(snapshots.spilled_bytes, snapshot.size);
        let spill_path = match &snapshot.contents {
            SourceContents::Disk(path, _) => path.clone(),
            _ => unreachable!(),
        };
        drop(snapshots);
        assert!(!spill_path.exists());
    }
}
