use crate::{AiCache, CacheEntry, CacheKey};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, NodeId, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::{interval, Interval};
use tracing::{debug, info, warn};

/// Cache invalidation strategies
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationStrategy {
    /// Time-to-live based invalidation
    Ttl(Duration),
    /// Manual invalidation by key
    Manual,
    /// Content-based invalidation (when source code changes)
    ContentBased,
    /// Dependency-based invalidation (when dependencies change)
    DependencyBased,
    /// LRU-based eviction
    Lru,
    /// Size-based eviction
    SizeBased,
}

/// Invalidation event types
#[derive(Debug, Clone)]
pub enum InvalidationEvent {
    /// File content changed
    FileChanged {
        file_path: String,
        modified_at: SystemTime,
    },
    /// Node updated
    NodeUpdated {
        node_id: NodeId,
        updated_at: SystemTime,
    },
    /// Dependency changed
    DependencyChanged { dependency: String, version: String },
    /// Manual invalidation
    Manual { keys: Vec<String>, reason: String },
    /// Time-based expiration
    Expired { expired_at: SystemTime },
}

/// Cache invalidation manager
pub struct InvalidationManager {
    /// Active invalidation strategies
    strategies: Vec<InvalidationStrategy>,
    /// File to cache key mappings
    file_mappings: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Node to cache key mappings
    node_mappings: Arc<RwLock<HashMap<NodeId, HashSet<String>>>>,
    /// Dependency to cache key mappings
    dependency_mappings: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Cleanup interval timer
    cleanup_interval: Option<Interval>,
    /// Invalidation event listeners
    event_listeners: Vec<Box<dyn InvalidationListener>>,
}

/// Trait for handling invalidation events
#[async_trait]
pub trait InvalidationListener: Send + Sync {
    async fn on_invalidation(&self, event: InvalidationEvent) -> Result<()>;
}

/// Cache invalidation policies
#[derive(Debug, Clone)]
pub struct InvalidationPolicy {
    /// Maximum age before automatic invalidation
    pub max_age: Option<Duration>,
    /// Whether to cascade invalidations to dependent entries
    pub cascade_invalidation: bool,
    /// Whether to use lazy or eager invalidation
    pub lazy_invalidation: bool,
    /// Batch size for bulk invalidations
    pub batch_size: usize,
}

impl Default for InvalidationPolicy {
    fn default() -> Self {
        Self {
            max_age: Some(Duration::from_secs(24 * 60 * 60)), // 24 hours
            cascade_invalidation: true,
            lazy_invalidation: false,
            batch_size: 100,
        }
    }
}

impl InvalidationManager {
    pub fn new(strategies: Vec<InvalidationStrategy>) -> Self {
        Self {
            strategies,
            file_mappings: Arc::new(RwLock::new(HashMap::new())),
            node_mappings: Arc::new(RwLock::new(HashMap::new())),
            dependency_mappings: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval: None,
            event_listeners: Vec::new(),
        }
    }

    /// Start automatic cleanup with specified interval
    pub fn start_cleanup(&mut self, interval_duration: Duration) {
        let mut cleanup_interval = interval(interval_duration);
        self.cleanup_interval = Some(cleanup_interval);
    }

    /// Register a file dependency for cache invalidation
    pub async fn register_file_dependency(&self, file_path: String, cache_key: String) {
        let mut mappings = self.file_mappings.write().await;
        mappings
            .entry(file_path)
            .or_insert_with(HashSet::new)
            .insert(cache_key);
    }

    /// Register a node dependency for cache invalidation
    pub async fn register_node_dependency(&self, node_id: NodeId, cache_key: String) {
        let mut mappings = self.node_mappings.write().await;
        mappings
            .entry(node_id)
            .or_insert_with(HashSet::new)
            .insert(cache_key);
    }

    /// Register a dependency for cache invalidation
    pub async fn register_dependency(&self, dependency: String, cache_key: String) {
        let mut mappings = self.dependency_mappings.write().await;
        mappings
            .entry(dependency)
            .or_insert_with(HashSet::new)
            .insert(cache_key);
    }

    /// Add an invalidation event listener
    pub fn add_listener(&mut self, listener: Box<dyn InvalidationListener>) {
        self.event_listeners.push(listener);
    }

    /// Handle file change event
    pub async fn handle_file_change(&self, file_path: String) -> Result<Vec<String>> {
        let mappings = self.file_mappings.read().await;
        let keys_to_invalidate = mappings
            .get(&file_path)
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_else(Vec::new);

        if !keys_to_invalidate.is_empty() {
            let event = InvalidationEvent::FileChanged {
                file_path,
                modified_at: SystemTime::now(),
            };
            self.notify_listeners(event).await?;
        }

        Ok(keys_to_invalidate)
    }

    /// Handle node update event
    pub async fn handle_node_update(&self, node_id: NodeId) -> Result<Vec<String>> {
        let mappings = self.node_mappings.read().await;
        let keys_to_invalidate = mappings
            .get(&node_id)
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_else(Vec::new);

        if !keys_to_invalidate.is_empty() {
            let event = InvalidationEvent::NodeUpdated {
                node_id,
                updated_at: SystemTime::now(),
            };
            self.notify_listeners(event).await?;
        }

        Ok(keys_to_invalidate)
    }

    /// Handle dependency change event
    pub async fn handle_dependency_change(
        &self,
        dependency: String,
        version: String,
    ) -> Result<Vec<String>> {
        let mappings = self.dependency_mappings.read().await;
        let keys_to_invalidate = mappings
            .get(&dependency)
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_else(Vec::new);

        if !keys_to_invalidate.is_empty() {
            let event = InvalidationEvent::DependencyChanged {
                dependency,
                version,
            };
            self.notify_listeners(event).await?;
        }

        Ok(keys_to_invalidate)
    }

    /// Manual invalidation of specific keys
    pub async fn invalidate_keys(&self, keys: Vec<String>, reason: String) -> Result<()> {
        let event = InvalidationEvent::Manual { keys, reason };
        self.notify_listeners(event).await
    }

    /// Cleanup expired entries based on TTL
    pub async fn cleanup_expired_entries<T>(
        &self,
        cache: &mut dyn AiCache<String, T>,
    ) -> Result<usize>
    where
        T: Clone + Send + Sync,
    {
        let mut removed_count = 0;

        // This is a simplified implementation
        // In a real implementation, we'd need access to cache internals
        // or the cache would need to provide an expiration cleanup method

        debug!("Starting expired entry cleanup");

        // For now, we'll just call the cache's stats to trigger any internal cleanup
        let _stats = cache.stats().await;

        info!("Cleanup completed, {} entries removed", removed_count);
        Ok(removed_count)
    }

    /// Notify all event listeners
    async fn notify_listeners(&self, event: InvalidationEvent) -> Result<()> {
        for listener in &self.event_listeners {
            if let Err(e) = listener.on_invalidation(event.clone()).await {
                warn!("Invalidation listener failed: {:?}", e);
            }
        }
        Ok(())
    }

    /// Remove mapping when cache entry is deleted
    pub async fn remove_file_mapping(&self, file_path: &str, cache_key: &str) {
        let mut mappings = self.file_mappings.write().await;
        if let Some(keys) = mappings.get_mut(file_path) {
            keys.remove(cache_key);
            if keys.is_empty() {
                mappings.remove(file_path);
            }
        }
    }

    /// Remove node mapping when cache entry is deleted
    pub async fn remove_node_mapping(&self, node_id: NodeId, cache_key: &str) {
        let mut mappings = self.node_mappings.write().await;
        if let Some(keys) = mappings.get_mut(&node_id) {
            keys.remove(cache_key);
            if keys.is_empty() {
                mappings.remove(&node_id);
            }
        }
    }

    /// Get statistics about invalidation mappings
    pub async fn get_mapping_stats(&self) -> InvalidationStats {
        let file_mappings = self.file_mappings.read().await;
        let node_mappings = self.node_mappings.read().await;
        let dependency_mappings = self.dependency_mappings.read().await;

        InvalidationStats {
            file_mappings_count: file_mappings.len(),
            node_mappings_count: node_mappings.len(),
            dependency_mappings_count: dependency_mappings.len(),
            total_tracked_keys: file_mappings.values().map(|keys| keys.len()).sum::<usize>()
                + node_mappings.values().map(|keys| keys.len()).sum::<usize>()
                + dependency_mappings
                    .values()
                    .map(|keys| keys.len())
                    .sum::<usize>(),
        }
    }
}

/// Statistics about invalidation tracking
#[derive(Debug, Clone)]
pub struct InvalidationStats {
    pub file_mappings_count: usize,
    pub node_mappings_count: usize,
    pub dependency_mappings_count: usize,
    pub total_tracked_keys: usize,
}

/// Simple logging invalidation listener
pub struct LoggingInvalidationListener;

#[async_trait]
impl InvalidationListener for LoggingInvalidationListener {
    async fn on_invalidation(&self, event: InvalidationEvent) -> Result<()> {
        match event {
            InvalidationEvent::FileChanged {
                file_path,
                modified_at,
            } => {
                info!(
                    "Cache invalidation: File '{}' changed at {:?}",
                    file_path, modified_at
                );
            }
            InvalidationEvent::NodeUpdated {
                node_id,
                updated_at,
            } => {
                info!(
                    "Cache invalidation: Node '{}' updated at {:?}",
                    node_id, updated_at
                );
            }
            InvalidationEvent::DependencyChanged {
                dependency,
                version,
            } => {
                info!(
                    "Cache invalidation: Dependency '{}' changed to version '{}'",
                    dependency, version
                );
            }
            InvalidationEvent::Manual { keys, reason } => {
                info!(
                    "Cache invalidation: Manual invalidation of {} keys, reason: {}",
                    keys.len(),
                    reason
                );
            }
            InvalidationEvent::Expired { expired_at } => {
                debug!("Cache invalidation: Entries expired at {:?}", expired_at);
            }
        }
        Ok(())
    }
}

/// Cache-aware invalidation listener that performs actual cache operations
pub struct CacheInvalidationListener<T>
where
    T: Clone + Send + Sync,
{
    cache: Arc<RwLock<dyn AiCache<String, T>>>,
}

impl<T> CacheInvalidationListener<T>
where
    T: Clone + Send + Sync,
{
    pub fn new(cache: Arc<RwLock<dyn AiCache<String, T>>>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl<T> InvalidationListener for CacheInvalidationListener<T>
where
    T: Clone + Send + Sync,
{
    async fn on_invalidation(&self, event: InvalidationEvent) -> Result<()> {
        match event {
            InvalidationEvent::Manual { keys, reason: _ } => {
                let mut cache = self.cache.write().await;
                for key in keys {
                    cache.remove(&key).await?;
                }
            }
            _ => {
                // Other event types would need specific handling based on the cache implementation
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestListener {
        call_count: Arc<AtomicUsize>,
    }

    impl TestListener {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let call_count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    call_count: call_count.clone(),
                },
                call_count,
            )
        }
    }

    #[async_trait]
    impl InvalidationListener for TestListener {
        async fn on_invalidation(&self, _event: InvalidationEvent) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_file_dependency_tracking() {
        let manager = InvalidationManager::new(vec![InvalidationStrategy::ContentBased]);

        let file_path = "src/test.rs".to_string();
        let cache_key = "test_key".to_string();

        manager
            .register_file_dependency(file_path.clone(), cache_key.clone())
            .await;

        let keys = manager.handle_file_change(file_path).await.unwrap();
        assert_eq!(keys, vec![cache_key]);
    }

    #[tokio::test]
    async fn test_node_dependency_tracking() {
        let manager = InvalidationManager::new(vec![InvalidationStrategy::DependencyBased]);

        let node_id = NodeId::new_v4();
        let cache_key = "test_key".to_string();

        manager
            .register_node_dependency(node_id, cache_key.clone())
            .await;

        let keys = manager.handle_node_update(node_id).await.unwrap();
        assert_eq!(keys, vec![cache_key]);
    }

    #[tokio::test]
    async fn test_event_listener_notification() {
        let mut manager = InvalidationManager::new(vec![InvalidationStrategy::Manual]);

        let (listener, call_count) = TestListener::new();
        manager.add_listener(Box::new(listener));

        manager
            .invalidate_keys(vec!["key1".to_string()], "test reason".to_string())
            .await
            .unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_mapping_cleanup() {
        let manager = InvalidationManager::new(vec![]);

        let file_path = "test.rs";
        let cache_key = "test_key";

        manager
            .register_file_dependency(file_path.to_string(), cache_key.to_string())
            .await;

        let stats_before = manager.get_mapping_stats().await;
        assert_eq!(stats_before.file_mappings_count, 1);

        manager.remove_file_mapping(file_path, cache_key).await;

        let stats_after = manager.get_mapping_stats().await;
        assert_eq!(stats_after.file_mappings_count, 0);
    }
}
