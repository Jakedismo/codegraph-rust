// ABOUTME: Caches exact submitted texts with model/task/tokenizer identity and single-flight misses.
// ABOUTME: Bounds inference by rows, tokens, bytes and provider-wide concurrency.
use codegraph_core::{
    CodeGraphError, Result,
    artifact_cache::{ArtifactCache, fingerprint},
};
use futures::{StreamExt, future::BoxFuture, stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    created: u64,
    vector: Vec<f32>,
}

pub(crate) struct SubmittedCache {
    namespace: String,
    dimension: usize,
    artifacts: Option<ArtifactCache>,
    memory: parking_lot::Mutex<lru::LruCache<String, Entry>>,
    locks: parking_lot::Mutex<BTreeMap<String, Weak<Mutex<()>>>>,
    inference: Semaphore,
    rows: usize,
    tokens: usize,
    bytes: usize,
    ttl: u64,
    pub hits: AtomicU64,
    pub inferred: AtomicU64,
    pub submitted_tokens: AtomicU64,
}

impl SubmittedCache {
    pub fn new(
        namespace: String,
        dimension: usize,
        root: Option<std::path::PathBuf>,
        local: bool,
        rows: usize,
        input_tokens: usize,
    ) -> Self {
        let setting = |name, default| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(default)
        };
        let cache = Self {
            namespace,
            dimension,
            artifacts: root.map(|root| ArtifactCache::new(root, "submitted-v1")),
            memory: parking_lot::Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(
                    (64 * 1024 * 1024 / (dimension.max(1) * 4 + 128)).max(1),
                )
                .unwrap(),
            )),
            locks: parking_lot::Mutex::new(BTreeMap::new()),
            inference: Semaphore::new(setting(
                "CODEGRAPH_PROVIDER_CONCURRENCY",
                if local { 1 } else { 4 },
            )),
            rows: rows.max(1),
            tokens: setting(
                "CODEGRAPH_EMBEDDING_BATCH_TOKENS",
                (if local { 8192 } else { 32768 }).max(input_tokens),
            ),
            bytes: setting("CODEGRAPH_EMBEDDING_BATCH_BYTES", 1024 * 1024),
            ttl: if std::env::var_os("CODEGRAPH_MODEL_REVISION").is_some() {
                u64::MAX
            } else {
                setting("CODEGRAPH_EMBEDDING_CACHE_TTL_SECONDS", 3600) as u64
            },
            hits: AtomicU64::new(0),
            inferred: AtomicU64::new(0),
            submitted_tokens: AtomicU64::new(0),
        };
        tracing::info!(
            "Embedding inference limits: {} rows, {} tokens, {} bytes per request; {} concurrent requests",
            cache.rows,
            cache.tokens,
            cache.bytes,
            cache.inference.available_permits()
        );
        cache
    }
    fn valid(&self, entry: &Entry) -> bool {
        entry.vector.len() == self.dimension
            && entry.vector.iter().all(|v| v.is_finite())
            && SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(entry.created)
                <= self.ttl
    }
    pub async fn embed<'a, F>(
        &'a self,
        texts: &[String],
        count: impl Fn(&str) -> usize + Sync,
        generate: F,
    ) -> Result<Vec<Vec<f32>>>
    where
        F: Fn(Vec<String>) -> BoxFuture<'a, Result<Vec<Vec<f32>>>> + Sync,
    {
        let mut unique = BTreeMap::new();
        for text in texts {
            let key = fingerprint(&(&self.namespace, text))
                .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            unique.entry(key).or_insert_with(|| text.clone());
        }
        let keys: Vec<_> = texts
            .iter()
            .map(|text| fingerprint(&(&self.namespace, text)).unwrap())
            .collect();
        let locks: Vec<_> = {
            let mut registry = self.locks.lock();
            registry.retain(|_, lock| lock.strong_count() > 0);
            unique
                .keys()
                .map(|key| {
                    let lock = registry
                        .get(key)
                        .and_then(Weak::upgrade)
                        .unwrap_or_else(|| Arc::new(Mutex::new(())));
                    registry.insert(key.clone(), Arc::downgrade(&lock));
                    lock
                })
                .collect()
        };
        // Canonical acquisition order prevents deadlocks across overlapping requests.
        let mut guards = Vec::new();
        for lock in locks {
            guards.push(lock.lock_owned().await);
        }
        let artifacts = self.artifacts.clone();
        let disk_keys: Vec<_> = {
            let memory = self.memory.lock();
            unique
                .keys()
                .filter(|key| !memory.peek(*key).is_some_and(|entry| self.valid(entry)))
                .cloned()
                .collect()
        };
        let disk: BTreeMap<String, Entry> = tokio::task::spawn_blocking(move || {
            disk_keys
                .into_iter()
                .filter_map(|key| {
                    artifacts
                        .as_ref()
                        .and_then(|cache| cache.get(&key))
                        .map(|value| (key, value))
                })
                .collect()
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
        let mut output = BTreeMap::new();
        let mut misses = Vec::new();
        for (key, text) in unique {
            let memory = self
                .memory
                .lock()
                .get(&key)
                .filter(|entry| self.valid(entry))
                .cloned();
            if let Some(entry) = memory
                .or_else(|| disk.get(&key).cloned())
                .filter(|entry| self.valid(entry))
            {
                self.memory.lock().put(key.clone(), entry.clone());
                output.insert(key, entry.vector);
            } else {
                let tokens = count(&text);
                misses.push((key, text, tokens));
            }
        }
        self.hits
            .fetch_add((texts.len() - misses.len()) as u64, Ordering::Relaxed);
        // Similar lengths reduce padding on local backends. Restore original order below.
        misses.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
        let mut batches = Vec::new();
        let mut batch = Vec::new();
        let mut tokens = 0;
        let mut bytes = 0;
        for item in misses {
            if item.2 > self.tokens || item.1.len() > self.bytes {
                return Err(CodeGraphError::Vector(format!(
                    "One text ({} tokens, {} bytes) exceeds inference batch limits ({} tokens, {} bytes). Increase CODEGRAPH_EMBEDDING_BATCH_TOKENS/CODEGRAPH_EMBEDDING_BATCH_BYTES or lower CODEGRAPH_CHUNK_MAX_TOKENS.",
                    item.2,
                    item.1.len(),
                    self.tokens,
                    self.bytes
                )));
            }
            if !batch.is_empty()
                && (batch.len() >= self.rows
                    || tokens + item.2 > self.tokens
                    || bytes + item.1.len() > self.bytes)
            {
                batches.push(std::mem::take(&mut batch));
                tokens = 0;
                bytes = 0;
            }
            tokens += item.2;
            bytes += item.1.len();
            batch.push(item);
        }
        if !batch.is_empty() {
            batches.push(batch);
        }
        let generate = &generate;
        let mut pending = stream::iter(batches.into_iter().map(|batch| async move {
            let _permit = self
                .inference
                .acquire()
                .await
                .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            let texts = batch.iter().map(|(_, text, _)| text.clone()).collect();
            let vectors = generate(texts).await?;
            if vectors.len() != batch.len()
                || vectors
                    .iter()
                    .any(|v| v.len() != self.dimension || v.iter().any(|v| !v.is_finite()))
            {
                return Err(CodeGraphError::Vector(
                    "Provider returned invalid vector cardinality, dimension or values".into(),
                ));
            }
            self.inferred
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
            self.submitted_tokens.fetch_add(
                batch.iter().map(|item| item.2 as u64).sum::<u64>(),
                Ordering::Relaxed,
            );
            let entries: Vec<_> = batch
                .into_iter()
                .zip(vectors)
                .map(|((key, _, _), vector)| {
                    (
                        key,
                        Entry {
                            created: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            vector,
                        },
                    )
                })
                .collect();
            let artifacts = self.artifacts.clone();
            let disk_entries = entries.clone();
            tokio::task::spawn_blocking(move || {
                if let Some(cache) = artifacts {
                    for (key, entry) in disk_entries {
                        if let Err(error) = cache.put(&key, &entry) {
                            tracing::debug!("Embedding cache write failed: {error}");
                        }
                    }
                }
            })
            .await
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            Ok::<_, CodeGraphError>(entries)
        }))
        .buffer_unordered(self.inference.available_permits().max(1));
        while let Some(entries) = pending.next().await {
            for (key, entry) in entries? {
                self.memory.lock().put(key.clone(), entry.clone());
                output.insert(key, entry.vector);
            }
        }
        Ok(keys.into_iter().map(|key| output[&key].clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn duplicate_and_concurrent_misses_share_inference_and_persist() {
        let root = tempfile::tempdir().unwrap();
        let cache = SubmittedCache::new(
            "model/task/tokenizer".into(),
            2,
            Some(root.path().into()),
            true,
            8,
            512,
        );
        let texts = vec!["same".to_owned(), "same".to_owned()];
        let generate = |batch: Vec<String>| {
            Box::pin(async move { Ok(batch.iter().map(|_| vec![1.0f32, 2.0f32]).collect()) })
                as BoxFuture<'_, Result<Vec<Vec<f32>>>>
        };
        let (a, b) = tokio::join!(
            cache.embed(&texts, str::len, generate),
            cache.embed(&texts, str::len, generate)
        );
        assert_eq!(a.unwrap(), b.unwrap());
        assert_eq!(cache.inferred.load(Ordering::Relaxed), 1);
        let warm = SubmittedCache::new(
            "model/task/tokenizer".into(),
            2,
            Some(root.path().into()),
            true,
            8,
            512,
        );
        assert_eq!(
            warm.embed(&texts, str::len, |_| Box::pin(async {
                panic!("warm cache inferred")
            }))
            .await
            .unwrap()
            .len(),
            2
        );
        let changed = SubmittedCache::new(
            "different-task".into(),
            2,
            Some(root.path().into()),
            true,
            8,
            512,
        );
        changed.embed(&texts, str::len, generate).await.unwrap();
        assert_eq!(changed.inferred.load(Ordering::Relaxed), 1);
    }
    #[tokio::test]
    async fn invalid_provider_outputs_never_enter_cache() {
        let cache = SubmittedCache::new("test".into(), 2, None, true, 8, 512);
        assert!(
            cache
                .embed(&["text".into()], str::len, |_| Box::pin(async {
                    Ok(vec![vec![f32::NAN, 1.]])
                }))
                .await
                .is_err()
        );
        assert_eq!(cache.inferred.load(Ordering::Relaxed), 0);
    }
}
