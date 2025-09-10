#[cfg(test)]
mod memory_tests {
    use crate::memory::{Arena, PagedArena, ArenaIndex, StringInterner, GLOBAL_INTERNER, TempBump, CompactHashMap};
    use crate::memory::debug::{MEMORY_TRACKER, MemoryCategory};

    #[test]
    fn arena_alloc_and_clear() {
        let mut a = Arena::with_capacity(1024);
        let s1 = a.alloc_str("hello");
        let s2 = a.alloc_str("world");
        assert_eq!(s1, "hello");
        assert_eq!(s2, "world");
        a.clear();
        // After clear, previous refs are dangling if used; we just ensure method exists
    }

    #[test]
    fn paged_arena_alloc_get() {
        let mut pa: PagedArena<u32> = PagedArena::with_page_capacity(4, MemoryCategory::Other);
        let idxs: Vec<ArenaIndex> = (0..10).map(|i| pa.alloc(i)).collect();
        assert_eq!(pa.len(), 10);
        for (i, idx) in idxs.into_iter().enumerate() {
            assert_eq!(*pa.get(idx).unwrap(), i as u32);
        }
    }

    #[test]
    fn string_interning_deduplicates() {
        let interner = StringInterner::new();
        let a = interner.intern("identifier_name");
        let b = interner.intern("identifier_name");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(interner.len(), 1);
    }

    use std::sync::Arc;
    #[test]
    fn global_interner_works() {
        let a = GLOBAL_INTERNER.intern("path::to::symbol");
        let b = GLOBAL_INTERNER.intern("path::to::symbol");
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn temp_bump_scope_resets() {
        let tb = TempBump::with_capacity(1024);
        {
            let mut s = tb.scope();
            let a = s.alloc_str("temp");
            assert_eq!(a, "temp");
        } // drop resets
        {
            let mut s2 = tb.scope();
            let b = s2.alloc_str("foo");
            assert_eq!(b, "foo");
        }
    }

    #[test]
    fn compact_hash_map_basic() {
        let mut m: CompactHashMap<&'static str, i32> = CompactHashMap::with_capacity(4);
        m.insert("a", 1);
        m.insert("b", 2);
        assert_eq!(m.get(&"a"), Some(&1));
        assert_eq!(m.remove(&"b"), Some(2));
        assert!(m.get(&"b").is_none());
    }

    #[test]
    fn memory_tracker_records() {
        let before = MEMORY_TRACKER.snapshot();
        let mut pa: PagedArena<u8> = PagedArena::with_page_capacity(2, MemoryCategory::Other);
        pa.alloc(1);
        pa.alloc(2);
        let after = MEMORY_TRACKER.snapshot();
        let b = before.get(&MemoryCategory::Other).map(|s| s.items).unwrap_or(0);
        let a = after.get(&MemoryCategory::Other).map(|s| s.items).unwrap_or(0);
        assert!(a >= b + 2);
    }
}

