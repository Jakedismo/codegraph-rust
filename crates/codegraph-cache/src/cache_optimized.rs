use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use std::ptr;

/// Cache line size for current architecture (typically 64 bytes)
pub const CACHE_LINE_SIZE: usize = 64;

/// Padded atomic counter to prevent false sharing
#[repr(align(64))]
#[derive(Debug)]
pub struct PaddedAtomicUsize {
    value: AtomicUsize,
    _padding: [u8; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
}

impl PaddedAtomicUsize {
    pub fn new(val: usize) -> Self {
        Self {
            value: AtomicUsize::new(val),
            _padding: [0; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
        }
    }

    #[inline(always)]
    pub fn load(&self, order: Ordering) -> usize {
        self.value.load(order)
    }

    #[inline(always)]
    pub fn store(&self, val: usize, order: Ordering) {
        self.value.store(val, order);
    }

    #[inline(always)]
    pub fn fetch_add(&self, val: usize, order: Ordering) -> usize {
        self.value.fetch_add(val, order)
    }

    #[inline(always)]
    pub fn fetch_sub(&self, val: usize, order: Ordering) -> usize {
        self.value.fetch_sub(val, order)
    }

    #[inline(always)]
    pub fn compare_exchange(&self, current: usize, new: usize, success: Ordering, failure: Ordering) -> Result<usize, usize> {
        self.value.compare_exchange(current, new, success, failure)
    }
}

/// Thread-local cache statistics to avoid contention
#[repr(align(64))]
pub struct ThreadCacheStats {
    pub hits: PaddedAtomicUsize,
    pub misses: PaddedAtomicUsize,
    pub evictions: PaddedAtomicUsize,
    pub insertions: PaddedAtomicUsize,
}

impl ThreadCacheStats {
    pub fn new() -> Self {
        Self {
            hits: PaddedAtomicUsize::new(0),
            misses: PaddedAtomicUsize::new(0),
            evictions: PaddedAtomicUsize::new(0),
            insertions: PaddedAtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_insertion(&self) {
        self.insertions.fetch_add(1, Ordering::Relaxed);
    }
}

/// Structure of Arrays for cache entries to improve spatial locality
/// Instead of Array of Structures (AoS), we use SoA for better cache performance
pub struct CacheEntriesSoA<V> {
    /// Keys stored contiguously for better cache locality during scans
    keys: Vec<String>,
    /// Values stored contiguously
    values: Vec<V>,
    /// Access times stored contiguously for LRU calculations
    access_times: Vec<AtomicU64>,
    /// Access counts for frequency-based eviction
    access_counts: Vec<AtomicUsize>,
    /// Entry sizes for memory accounting
    sizes: Vec<usize>,
    /// Validity flags (1 byte each for compact representation)
    valid: Vec<bool>,
    /// Current capacity
    capacity: usize,
    /// Current size
    size: AtomicUsize,
}

impl<V> CacheEntriesSoA<V> {
    pub fn new(capacity: usize) -> Self {
        let mut entries = Self {
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            access_times: Vec::with_capacity(capacity),
            access_counts: Vec::with_capacity(capacity),
            sizes: Vec::with_capacity(capacity),
            valid: Vec::with_capacity(capacity),
            capacity,
            size: AtomicUsize::new(0),
        };

        // Pre-allocate all vectors to avoid reallocations during operation
        for _ in 0..capacity {
            entries.keys.push(String::new());
            entries.values.push(unsafe { std::mem::zeroed() });
            entries.access_times.push(AtomicU64::new(0));
            entries.access_counts.push(AtomicUsize::new(0));
            entries.sizes.push(0);
            entries.valid.push(false);
        }

        entries
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<&V> {
        // Sequential scan optimized for cache prefetching
        for i in 0..self.capacity {
            if self.valid[i] && self.keys[i] == key {
                // Prefetch next few entries while we have this cache line loaded
                if i + 1 < self.capacity {
                    unsafe {
                        ptr::prefetch_read_data(&self.keys[i + 1] as *const String as *const u8, 1);
                        ptr::prefetch_read_data(&self.valid[i + 1] as *const bool as *const u8, 1);
                    }
                }
                
                // Update access time and count
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                self.access_times[i].store(now, Ordering::Relaxed);
                self.access_counts[i].fetch_add(1, Ordering::Relaxed);
                
                return Some(&self.values[i]);
            }
        }
        None
    }

    pub fn insert(&mut self, key: String, value: V, size: usize) -> bool {
        let current_size = self.size.load(Ordering::Relaxed);
        
        // Find first invalid slot or replace oldest entry
        let mut slot_idx = None;
        let mut oldest_time = u64::MAX;
        let mut oldest_idx = 0;

        for i in 0..self.capacity {
            if !self.valid[i] {
                slot_idx = Some(i);
                break;
            } else {
                let access_time = self.access_times[i].load(Ordering::Relaxed);
                if access_time < oldest_time {
                    oldest_time = access_time;
                    oldest_idx = i;
                }
            }
        }

        let idx = slot_idx.unwrap_or(oldest_idx);
        
        // Insert new entry
        self.keys[idx] = key;
        self.values[idx] = value;
        self.sizes[idx] = size;
        self.valid[idx] = true;
        
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.access_times[idx].store(now, Ordering::Relaxed);
        self.access_counts[idx].store(1, Ordering::Relaxed);

        if slot_idx.is_some() && current_size < self.capacity {
            self.size.fetch_add(1, Ordering::Relaxed);
        }

        true
    }

    pub fn remove(&mut self, key: &str) -> bool {
        for i in 0..self.capacity {
            if self.valid[i] && self.keys[i] == key {
                self.valid[i] = false;
                self.keys[i].clear();
                self.sizes[i] = 0;
                self.access_times[i].store(0, Ordering::Relaxed);
                self.access_counts[i].store(0, Ordering::Relaxed);
                self.size.fetch_sub(1, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    pub fn memory_usage(&self) -> usize {
        let mut total = 0;
        for i in 0..self.capacity {
            if self.valid[i] {
                total += self.sizes[i];
            }
        }
        total
    }

    /// Optimize memory layout by compacting valid entries
    pub fn compact(&mut self) {
        let mut write_idx = 0;
        
        for read_idx in 0..self.capacity {
            if self.valid[read_idx] && write_idx != read_idx {
                // Move entry to compact position
                self.keys.swap(read_idx, write_idx);
                self.values.swap(read_idx, write_idx);
                self.sizes.swap(read_idx, write_idx);
                self.valid.swap(read_idx, write_idx);
                
                let read_time = self.access_times[read_idx].load(Ordering::Relaxed);
                let read_count = self.access_counts[read_idx].load(Ordering::Relaxed);
                
                self.access_times[write_idx].store(read_time, Ordering::Relaxed);
                self.access_counts[write_idx].store(read_count, Ordering::Relaxed);
                
                self.access_times[read_idx].store(0, Ordering::Relaxed);
                self.access_counts[read_idx].store(0, Ordering::Relaxed);
                
                write_idx += 1;
            } else if self.valid[read_idx] {
                write_idx += 1;
            }
        }
        
        // Mark remaining slots as invalid
        for i in write_idx..self.capacity {
            self.valid[i] = false;
        }
    }

    /// Batch prefetch operation for predictable access patterns
    pub fn prefetch_keys(&self, start_idx: usize, count: usize) {
        let end_idx = std::cmp::min(start_idx + count, self.capacity);
        for i in start_idx..end_idx {
            unsafe {
                ptr::prefetch_read_data(&self.keys[i] as *const String as *const u8, 1);
                ptr::prefetch_read_data(&self.valid[i] as *const bool as *const u8, 1);
            }
        }
    }
}

/// Cache-line optimized hash map for high-performance lookups
#[repr(align(64))]
pub struct CacheOptimizedHashMap<K, V> {
    /// Use multiple smaller hash maps to reduce contention
    shards: Vec<RwLock<HashMap<K, CacheEntry<V>>>>,
    /// Statistics per shard to avoid false sharing
    stats: Vec<ThreadCacheStats>,
    /// Shard count (power of 2 for fast modulo)
    shard_count: usize,
    /// Shard mask for fast modulo operation
    shard_mask: usize,
}

#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    pub value: V,
    pub created_at: SystemTime,
    pub last_accessed: AtomicU64,
    pub access_count: AtomicUsize,
    pub size_bytes: usize,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V, size_bytes: usize) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
            
        Self {
            value,
            created_at: SystemTime::now(),
            last_accessed: AtomicU64::new(now),
            access_count: AtomicUsize::new(1),
            size_bytes,
        }
    }

    #[inline(always)]
    pub fn touch(&self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_accessed.store(now, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> CacheOptimizedHashMap<K, V> {
    pub fn new(shard_count: Option<usize>) -> Self {
        let shard_count = shard_count.unwrap_or_else(|| {
            // Use number of CPU cores as default, but ensure it's a power of 2
            let cores = num_cpus::get();
            cores.next_power_of_two()
        });

        let shard_mask = shard_count - 1;
        let mut shards = Vec::with_capacity(shard_count);
        let mut stats = Vec::with_capacity(shard_count);

        for _ in 0..shard_count {
            shards.push(RwLock::new(HashMap::new()));
            stats.push(ThreadCacheStats::new());
        }

        Self {
            shards,
            stats,
            shard_count,
            shard_mask,
        }
    }

    #[inline(always)]
    fn shard_index(&self, key: &K) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) & self.shard_mask
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let shard_idx = self.shard_index(key);
        let shard = &self.shards[shard_idx];
        let stats = &self.stats[shard_idx];

        if let Some(guard) = shard.try_read() {
            if let Some(entry) = guard.get(key) {
                entry.touch();
                stats.record_hit();
                Some(entry.value.clone())
            } else {
                stats.record_miss();
                None
            }
        } else {
            // Fallback to blocking read if try_read fails
            let guard = shard.read();
            if let Some(entry) = guard.get(key) {
                entry.touch();
                stats.record_hit();
                Some(entry.value.clone())
            } else {
                stats.record_miss();
                None
            }
        }
    }

    pub fn insert(&self, key: K, value: V, size_bytes: usize) {
        let shard_idx = self.shard_index(&key);
        let shard = &self.shards[shard_idx];
        let stats = &self.stats[shard_idx];

        let mut guard = shard.write();
        let entry = CacheEntry::new(value, size_bytes);
        guard.insert(key, entry);
        stats.record_insertion();
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        let shard_idx = self.shard_index(key);
        let shard = &self.shards[shard_idx];

        let mut guard = shard.write();
        guard.remove(key).map(|entry| entry.value)
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| shard.read().len()).sum()
    }

    pub fn memory_usage(&self) -> usize {
        self.shards.iter()
            .map(|shard| {
                shard.read().values()
                    .map(|entry| entry.size_bytes)
                    .sum::<usize>()
            })
            .sum()
    }

    pub fn get_stats(&self) -> (usize, usize, usize, usize) {
        let mut total_hits = 0;
        let mut total_misses = 0;
        let mut total_evictions = 0;
        let mut total_insertions = 0;

        for stats in &self.stats {
            total_hits += stats.hits.load(Ordering::Relaxed);
            total_misses += stats.misses.load(Ordering::Relaxed);
            total_evictions += stats.evictions.load(Ordering::Relaxed);
            total_insertions += stats.insertions.load(Ordering::Relaxed);
        }

        (total_hits, total_misses, total_evictions, total_insertions)
    }
}

impl<K, V> Default for CacheOptimizedHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padded_atomic_counter() {
        let counter = PaddedAtomicUsize::new(0);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        
        counter.fetch_add(5, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_cache_entries_soa() {
        let mut entries = CacheEntriesSoA::<i32>::new(10);
        
        assert!(entries.insert("key1".to_string(), 42, 4));
        assert_eq!(entries.get("key1"), Some(&42));
        assert_eq!(entries.get("nonexistent"), None);
        
        assert_eq!(entries.len(), 1);
        assert!(entries.remove("key1"));
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_cache_optimized_hashmap() {
        let cache = CacheOptimizedHashMap::new(Some(4));
        
        cache.insert("key1".to_string(), 42, 4);
        cache.insert("key2".to_string(), 84, 4);
        
        assert_eq!(cache.get(&"key1".to_string()), Some(42));
        assert_eq!(cache.get(&"key2".to_string()), Some(84));
        assert_eq!(cache.get(&"key3".to_string()), None);
        
        assert_eq!(cache.len(), 2);
        
        let (hits, misses, _, insertions) = cache.get_stats();
        assert_eq!(insertions, 2);
        assert_eq!(hits, 2);
        assert_eq!(misses, 1);
    }

    #[test]
    fn test_cache_line_alignment() {
        let counter = PaddedAtomicUsize::new(0);
        let ptr = &counter as *const PaddedAtomicUsize;
        assert_eq!(ptr.align_offset(CACHE_LINE_SIZE), 0);
    }
}