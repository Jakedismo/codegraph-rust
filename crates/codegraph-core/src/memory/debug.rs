use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Categories for attributing memory usage and allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryCategory {
    NodeArena,
    EdgeArena,
    StringInterner,
    TempAlloc,
    BufferPool,
    Embeddings,
    Cache,
    Other,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug, Default)]
pub struct MemoryTracker {
    inner: Mutex<HashMap<MemoryCategory, CategoryStats>>,
}

impl MemoryTracker {
    pub fn record_alloc(&self, category: MemoryCategory, items: u64) {
        let mut g = self.inner.lock();
        let e = g.entry(category).or_default();
        e.items = e.items.saturating_add(items);
    }

    pub fn record_bytes(&self, category: MemoryCategory, delta: i64) {
        let mut g = self.inner.lock();
        let e = g.entry(category).or_default();
        if delta >= 0 {
            e.bytes = e.bytes.saturating_add(delta as u64);
        } else {
            let d = (-delta) as u64;
            e.bytes = e.bytes.saturating_sub(d.min(e.bytes));
        }
    }

    pub fn snapshot(&self) -> HashMap<MemoryCategory, CategoryStats> {
        self.inner.lock().clone()
    }
}

pub static MEMORY_TRACKER: Lazy<MemoryTracker> = Lazy::new(MemoryTracker::default);
