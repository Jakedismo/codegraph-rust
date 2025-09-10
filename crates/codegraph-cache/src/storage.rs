use crate::{CacheEntry, CacheStats, CacheSizeEstimator};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, Result};
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, WriteBatch, DB};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::task;
use tracing::{debug, error, info};

/// Persistent storage backend for AI cache
#[derive(Clone)]
pub struct PersistentStorage {
    db: Arc<DB>,
}

/// Column families for different cache types
pub const CF_EMBEDDINGS: &str = "embeddings";
pub const CF_QUERIES: &str = "queries";
pub const CF_METADATA: &str = "metadata";
pub const CF_STATS: &str = "stats";

/// Serializable cache entry for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCacheEntry<T> {
    pub value: T,
    pub created_at: u64, // Unix timestamp
    pub last_accessed: u64,
    pub access_count: u64,
    pub size_bytes: usize,
    pub ttl_secs: Option<u64>,
}

impl<T> From<CacheEntry<T>> for StoredCacheEntry<T> {
    fn from(entry: CacheEntry<T>) -> Self {
        Self {
            value: entry.value,
            created_at: entry.created_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            last_accessed: entry.last_accessed
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            access_count: entry.access_count,
            size_bytes: entry.size_bytes,
            ttl_secs: entry.ttl.map(|d| d.as_secs()),
        }
    }
}

impl<T> From<StoredCacheEntry<T>> for CacheEntry<T> {
    fn from(stored: StoredCacheEntry<T>) -> Self {
        Self {
            value: stored.value,
            created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(stored.created_at),
            last_accessed: SystemTime::UNIX_EPOCH + Duration::from_secs(stored.last_accessed),
            access_count: stored.access_count,
            size_bytes: stored.size_bytes,
            ttl: stored.ttl_secs.map(Duration::from_secs),
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub db_path: String,
    pub enable_compression: bool,
    pub max_open_files: i32,
    pub write_buffer_size: usize,
    pub max_write_buffer_number: u32,
    pub target_file_size_base: u64,
    pub enable_statistics: bool,
    pub enable_wal: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "./cache_db".to_string(),
            enable_compression: true,
            max_open_files: 1000,
            write_buffer_size: 64 * 1024 * 1024, // 64MB
            max_write_buffer_number: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
            enable_statistics: true,
            enable_wal: true,
        }
    }
}

impl PersistentStorage {
    /// Create new persistent storage instance
    pub fn new(config: StorageConfig) -> Result<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_open_files(config.max_open_files);
        db_opts.set_write_buffer_size(config.write_buffer_size);
        db_opts.set_max_write_buffer_number(config.max_write_buffer_number.try_into().unwrap());
        db_opts.set_target_file_size_base(config.target_file_size_base);
        
        if config.enable_compression {
            db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        }

        if !config.enable_wal {
            db_opts.set_use_fsync(false);
        }

        // Define column families
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_EMBEDDINGS, Options::default()),
            ColumnFamilyDescriptor::new(CF_QUERIES, Options::default()),
            ColumnFamilyDescriptor::new(CF_METADATA, Options::default()),
            ColumnFamilyDescriptor::new(CF_STATS, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&db_opts, &config.db_path, cf_descriptors)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database: {}", e)))?;

        info!("Opened persistent storage at: {}", config.db_path);
        
        Ok(Self {
            db: Arc::new(db),
        })
    }

    /// Store a value in the specified column family
    pub async fn store<T>(&self, cf_name: &str, key: &str, entry: CacheEntry<T>) -> Result<()>
    where
        T: Serialize + Send + 'static,
    {
        let stored_entry: StoredCacheEntry<T> = entry.into();
        let serialized = bincode::serialize(&stored_entry)
            .map_err(|e| CodeGraphError::Database(format!("Serialization failed: {}", e)))?;

        let db = self.db.clone();
        let cf_name = cf_name.to_string();
        let key = key.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            db.put_cf(cf, key.as_bytes(), &serialized)
                .map_err(|e| CodeGraphError::Database(format!("Write failed: {}", e)))
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))??;

        Ok(())
    }

    /// Retrieve a value from the specified column family
    pub async fn retrieve<T>(&self, cf_name: &str, key: &str) -> Result<Option<CacheEntry<T>>>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
    {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();
        let key = key.to_string();

        let data = task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            db.get_cf(cf, key.as_bytes())
                .map_err(|e| CodeGraphError::Database(format!("Read failed: {}", e)))
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))??;

        if let Some(bytes) = data {
            let stored_entry: StoredCacheEntry<T> = bincode::deserialize(&bytes)
                .map_err(|e| CodeGraphError::Database(format!("Deserialization failed: {}", e)))?;
            
            Ok(Some(stored_entry.into()))
        } else {
            Ok(None)
        }
    }

    /// Remove a value from the specified column family
    pub async fn remove(&self, cf_name: &str, key: &str) -> Result<bool> {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();
        let key = key.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            // Check if key exists first
            let exists = db.get_cf(cf, key.as_bytes())
                .map_err(|e| CodeGraphError::Database(format!("Read failed: {}", e)))?
                .is_some();

            if exists {
                db.delete_cf(cf, key.as_bytes())
                    .map_err(|e| CodeGraphError::Database(format!("Delete failed: {}", e)))?;
            }

            Ok(exists)
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Batch store multiple entries
    pub async fn batch_store<T>(&self, cf_name: &str, entries: Vec<(String, CacheEntry<T>)>) -> Result<()>
    where
        T: Serialize + Send + 'static,
    {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            let mut batch = WriteBatch::default();
            
            for (key, entry) in entries {
                let stored_entry: StoredCacheEntry<T> = entry.into();
                let serialized = bincode::serialize(&stored_entry)
                    .map_err(|e| CodeGraphError::Database(format!("Serialization failed: {}", e)))?;
                
                batch.put_cf(cf, key.as_bytes(), &serialized);
            }

            db.write(batch)
                .map_err(|e| CodeGraphError::Database(format!("Batch write failed: {}", e)))
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Iterate over all keys in a column family
    pub async fn scan_keys(&self, cf_name: &str) -> Result<Vec<String>> {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut keys = Vec::new();

            for item in iter {
                let (key, _) = item
                    .map_err(|e| CodeGraphError::Database(format!("Iterator failed: {}", e)))?;
                
                let key_str = String::from_utf8_lossy(&key).to_string();
                keys.push(key_str);
            }

            Ok(keys)
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Get approximate number of entries in a column family
    pub async fn count_entries(&self, cf_name: &str) -> Result<u64> {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            let property = format!("rocksdb.estimate-num-keys");
            let count_str = db.property_value_cf(cf, &property)
                .map_err(|e| CodeGraphError::Database(format!("Property read failed: {}", e)))?
                .unwrap_or_else(|| "0".to_string());

            count_str.parse::<u64>()
                .map_err(|e| CodeGraphError::Database(format!("Failed to parse count: {}", e)))
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Cleanup expired entries from a column family
    pub async fn cleanup_expired<T>(&self, cf_name: &str) -> Result<usize>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
    {
        let db = self.db.clone();
        let cf_name = cf_name.to_string();

        task::spawn_blocking(move || {
            let cf = db.cf_handle(&cf_name)
                .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", cf_name)))?;
            
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut expired_keys = Vec::new();
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();

            for item in iter {
                let (key, value) = item
                    .map_err(|e| CodeGraphError::Database(format!("Iterator failed: {}", e)))?;
                
                // Try to deserialize and check expiration
                if let Ok(stored_entry) = bincode::deserialize::<StoredCacheEntry<T>>(&value) {
                    if let Some(ttl_secs) = stored_entry.ttl_secs {
                        if stored_entry.created_at + ttl_secs < now {
                            expired_keys.push(key.to_vec());
                        }
                    }
                }
            }

            // Delete expired keys
            let mut batch = WriteBatch::default();
            for key in &expired_keys {
                batch.delete_cf(cf, key);
            }

            if !expired_keys.is_empty() {
                db.write(batch)
                    .map_err(|e| CodeGraphError::Database(format!("Cleanup batch write failed: {}", e)))?;
            }

            Ok(expired_keys.len())
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Compact the database to reclaim space
    pub async fn compact(&self) -> Result<()> {
        let db = self.db.clone();

        task::spawn_blocking(move || {
            db.compact_range::<&[u8], &[u8]>(None, None);
            Ok(())
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<StorageStats> {
        let db = self.db.clone();

        task::spawn_blocking(move || {
            let mut stats = StorageStats::default();

            // Get approximate database size
            if let Ok(Some(size_str)) = db.property_value("rocksdb.total-sst-files-size") {
                stats.total_size_bytes = size_str.parse().unwrap_or(0);
            }

            // Get number of entries across all column families
            for cf_name in [CF_EMBEDDINGS, CF_QUERIES, CF_METADATA, CF_STATS] {
                if let Some(cf) = db.cf_handle(cf_name) {
                    if let Ok(Some(count_str)) = db.property_value_cf(cf, "rocksdb.estimate-num-keys") {
                        if let Ok(count) = count_str.parse::<u64>() {
                            match cf_name {
                                CF_EMBEDDINGS => stats.embedding_entries = count,
                                CF_QUERIES => stats.query_entries = count,
                                CF_METADATA => stats.metadata_entries = count,
                                CF_STATS => stats.stats_entries = count,
                                _ => {}
                            }
                        }
                    }
                }
            }

            Ok(stats)
        }).await
        .map_err(|e| CodeGraphError::Database(format!("Task failed: {}", e)))?
    }

    /// Store cache statistics
    pub async fn store_stats(&self, stats: &CacheStats) -> Result<()> {
        let key = format!("cache_stats_{}", chrono::Utc::now().timestamp());
        let entry = CacheEntry::new(stats.clone(), stats.estimate_size(), None);
        self.store(CF_STATS, &key, entry).await
    }

    /// Retrieve latest cache statistics
    pub async fn get_latest_stats(&self) -> Result<Option<CacheStats>> {
        let keys = self.scan_keys(CF_STATS).await?;
        
        if let Some(latest_key) = keys.last() {
            if let Some(entry) = self.retrieve::<CacheStats>(CF_STATS, latest_key).await? {
                return Ok(Some(entry.value));
            }
        }
        
        Ok(None)
    }
}

impl CacheSizeEstimator for CacheStats {
    fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Storage performance statistics
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub total_size_bytes: u64,
    pub embedding_entries: u64,
    pub query_entries: u64,
    pub metadata_entries: u64,
    pub stats_entries: u64,
}

/// Trait for persistent cache implementations
#[async_trait]
pub trait PersistentCache<K, V>: Send + Sync {
    /// Load cache from persistent storage
    async fn load_from_storage(&mut self) -> Result<usize>;
    
    /// Save cache to persistent storage
    async fn save_to_storage(&self) -> Result<()>;
    
    /// Enable write-through caching (write to both cache and storage)
    fn enable_write_through(&mut self, enabled: bool);
    
    /// Enable write-behind caching (batch writes to storage)
    fn enable_write_behind(&mut self, enabled: bool, batch_size: usize);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_storage() -> (PersistentStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            db_path: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let storage = PersistentStorage::new(config).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let key = "test_key";
        let value = vec![1.0f32, 2.0, 3.0];
        let entry = CacheEntry::new(value.clone(), value.estimate_size(), None);
        
        storage.store(CF_EMBEDDINGS, key, entry).await.unwrap();
        
        let retrieved = storage.retrieve::<Vec<f32>>(CF_EMBEDDINGS, key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, value);
    }

    #[tokio::test]
    async fn test_remove() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let key = "test_key";
        let value = vec![1.0f32, 2.0, 3.0];
        let entry = CacheEntry::new(value, value.estimate_size(), None);
        
        storage.store(CF_EMBEDDINGS, key, entry).await.unwrap();
        
        let removed = storage.remove(CF_EMBEDDINGS, key).await.unwrap();
        assert!(removed);
        
        let retrieved = storage.retrieve::<Vec<f32>>(CF_EMBEDDINGS, key).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let entries = vec![
            ("key1".to_string(), CacheEntry::new(vec![1.0f32], 4, None)),
            ("key2".to_string(), CacheEntry::new(vec![2.0f32], 4, None)),
            ("key3".to_string(), CacheEntry::new(vec![3.0f32], 4, None)),
        ];
        
        storage.batch_store(CF_EMBEDDINGS, entries).await.unwrap();
        
        let count = storage.count_entries(CF_EMBEDDINGS).await.unwrap();
        assert_eq!(count, 3);
        
        let keys = storage.scan_keys(CF_EMBEDDINGS).await.unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[tokio::test]
    async fn test_stats_storage() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let stats = CacheStats {
            hits: 100,
            misses: 50,
            evictions: 10,
            total_entries: 1000,
            memory_usage_bytes: 1024 * 1024,
            average_access_time_ns: 1000,
            hit_rate: 0.67,
        };
        
        storage.store_stats(&stats).await.unwrap();
        
        let retrieved_stats = storage.get_latest_stats().await.unwrap();
        assert!(retrieved_stats.is_some());
        
        let retrieved = retrieved_stats.unwrap();
        assert_eq!(retrieved.hits, stats.hits);
        assert_eq!(retrieved.misses, stats.misses);
    }
}
