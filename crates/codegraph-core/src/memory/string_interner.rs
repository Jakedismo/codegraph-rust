use hashbrown::HashMap;
use parking_lot::RwLock;
use rustc_hash::FxHasher;
use std::hash::BuildHasherDefault;
use std::sync::Arc;

use super::debug::{MemoryCategory, MEMORY_TRACKER};

/// Interned string type. Cloning is cheap and deduplicated by content.
pub type InternedStr = Arc<str>;

/// A thread-safe string interner backed by a HashMap with Fx hasher.
/// Reduces duplicate string memory by sharing `Arc<str>` across the process.
#[derive(Debug, Default)]
pub struct StringInterner {
    map: RwLock<HashMap<InternedStr, (), BuildHasherDefault<FxHasher>>>, // set semantics
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::with_hasher(
                BuildHasherDefault::<FxHasher>::default(),
            )),
        }
    }

    /// Interns the provided string slice. If already present, returns a clone
    /// of the existing Arc; otherwise inserts and returns the new Arc.
    pub fn intern(&self, s: &str) -> InternedStr {
        // Fast-path read lock: if present, return it
        {
            let map = self.map.read();
            if let Some((k, _)) = map.get_key_value(s) {
                return k.clone();
            }
        }
        // Upgrade to write lock to insert
        let mut map = self.map.write();
        if let Some((k, _)) = map.get_key_value(s) {
            return k.clone();
        }
        let arc: InternedStr = Arc::<str>::from(s);
        MEMORY_TRACKER.record_bytes(MemoryCategory::StringInterner, arc.len() as i64);
        map.insert(arc.clone(), ());
        arc
    }

    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    pub fn clear(&self) {
        let mut map = self.map.write();
        // Not tracking exact freed bytes here; external snapshotting can infer reductions
        map.clear();
    }
}

/// Global interner for convenience.
use once_cell::sync::Lazy;
pub static GLOBAL_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);
