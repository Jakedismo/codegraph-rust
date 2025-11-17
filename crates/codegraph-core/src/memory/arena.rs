use bumpalo::Bump;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Index, IndexMut};

use crate::memory::debug::{MemoryCategory, MEMORY_TRACKER};

/// Simple wrapper around bumpalo::Bump for scoped arena allocations of
/// short-lived data structures created during parsing and batch transforms.
///
/// Dropping the arena frees all allocations at once.
#[derive(Debug)]
pub struct Arena {
    bump: Bump,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    pub fn new() -> Self {
        Self { bump: Bump::new() }
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            bump: Bump::with_capacity(bytes),
        }
    }

    /// Allocate a string into the arena and return a &'a str borrowed
    /// from the arena's lifetime.
    pub fn alloc_str<'a>(&'a self, s: &str) -> &'a str {
        self.bump.alloc_str(s)
    }

    /// Clone a slice into the arena.
    pub fn alloc_slice_clone<'a, T: Clone>(&'a self, slice: &[T]) -> &'a [T] {
        self.bump.alloc_slice_clone(slice)
    }

    /// Allocate a value by moving it into the arena and returning a &'a mut T.
    pub fn alloc<T>(&self, value: T) -> &mut T {
        self.bump.alloc(value)
    }

    /// Reset the arena, freeing all allocations.
    pub fn clear(&mut self) {
        self.bump.reset();
    }
}

/// A handle into the paged arena. Compact 64-bit encoding of (page, offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArenaIndex(u64);

impl ArenaIndex {
    #[inline]
    pub fn new(page: u32, offset: u32) -> Self {
        Self(((page as u64) << 32) | (offset as u64))
    }
    #[inline]
    pub fn page(self) -> u32 {
        (self.0 >> 32) as u32
    }
    #[inline]
    pub fn offset(self) -> u32 {
        self.0 as u32
    }
}

/// Paged arena for long-lived objects (e.g., nodes/edges) that avoids per-item
/// heap allocation by storing items in fixed-size pages (Vec<T> buckets).
///
/// - Allocation: amortized O(1), push into current page until full.
/// - Access: O(1) via ArenaIndex.
/// - Memory: contiguous pages improve locality and reduce allocator overhead.
#[derive(Debug)]
pub struct PagedArena<T> {
    pages: Vec<Vec<T>>,   // owned pages
    page_capacity: usize, // items per page
    len: usize,           // total items
    category: MemoryCategory,
    _marker: PhantomData<T>,
}

impl<T> PagedArena<T> {
    pub fn with_page_capacity(page_capacity: usize, category: MemoryCategory) -> Self {
        let cap = page_capacity.max(1);
        Self {
            pages: Vec::new(),
            page_capacity: cap,
            len: 0,
            category,
            _marker: PhantomData,
        }
    }

    pub fn new_nodes() -> Self {
        // Default: 4K items per page for nodes
        Self::with_page_capacity(4096, MemoryCategory::NodeArena)
    }

    pub fn new_edges() -> Self {
        // Default: 8K items per page for edges (typically smaller than nodes)
        Self::with_page_capacity(8192, MemoryCategory::EdgeArena)
    }

    #[inline]
    fn ensure_page(&mut self) {
        if self
            .pages
            .last()
            .map(|p| p.len() < self.page_capacity)
            .unwrap_or(false)
        {
            return;
        }
        self.pages.push(Vec::with_capacity(self.page_capacity));
    }

    /// Allocate a new value inside the arena and get its ArenaIndex.
    pub fn alloc(&mut self, value: T) -> ArenaIndex {
        self.ensure_page();
        let page_idx = (self.pages.len() - 1) as u32;
        let page = self.pages.last_mut().unwrap();
        let offset = page.len() as u32;
        page.push(value);
        self.len += 1;
        MEMORY_TRACKER.record_alloc(self.category, 1); // count items; byte estimate is domain-specific
        ArenaIndex::new(page_idx, offset)
    }

    /// Length (number of elements) in the arena.
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Immutable access by index.
    pub fn get(&self, idx: ArenaIndex) -> Option<&T> {
        let page = self.pages.get(idx.page() as usize)?;
        page.get(idx.offset() as usize)
    }

    /// Mutable access by index.
    pub fn get_mut(&mut self, idx: ArenaIndex) -> Option<&mut T> {
        let page = self.pages.get_mut(idx.page() as usize)?;
        page.get_mut(idx.offset() as usize)
    }

    /// Iterate over all elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.pages.iter().flat_map(|p| p.iter())
    }

    /// Iterate mutably over all elements in insertion order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.pages.iter_mut().flat_map(|p| p.iter_mut())
    }
}

impl<T> Index<ArenaIndex> for PagedArena<T> {
    type Output = T;
    fn index(&self, index: ArenaIndex) -> &Self::Output {
        self.get(index).expect("invalid ArenaIndex")
    }
}

impl<T> IndexMut<ArenaIndex> for PagedArena<T> {
    fn index_mut(&mut self, index: ArenaIndex) -> &mut Self::Output {
        self.get_mut(index).expect("invalid ArenaIndex")
    }
}

/// A simple bump-like arena for plain-old-data arrays, backed by pages of
/// uninitialized memory. Useful for building temporary vectors without per-push
/// reallocation, then freezing them.
#[derive(Debug)]
pub struct ChunkArena<T> {
    chunks: Vec<Box<[MaybeUninit<T>]>>, // fixed-size chunks
    chunk_capacity: usize,
    len: usize,
    category: MemoryCategory,
}

impl<T> ChunkArena<T> {
    pub fn with_chunk_capacity(chunk_capacity: usize, category: MemoryCategory) -> Self {
        let cap = chunk_capacity.max(1);
        Self {
            chunks: Vec::new(),
            chunk_capacity: cap,
            len: 0,
            category,
        }
    }

    #[inline]
    fn ensure_chunk(&mut self) {
        if self.len % self.chunk_capacity == 0 {
            let mut vec: Vec<MaybeUninit<T>> = Vec::with_capacity(self.chunk_capacity);
            vec.resize_with(self.chunk_capacity, || MaybeUninit::uninit());
            self.chunks.push(vec.into_boxed_slice());
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Push an element, moving it into the arena.
    pub fn push(&mut self, value: T) {
        self.ensure_chunk();
        let idx = self.len;
        let chunk_idx = idx / self.chunk_capacity;
        let within = idx % self.chunk_capacity;
        self.chunks[chunk_idx][within].write(value);
        self.len += 1;
        MEMORY_TRACKER.record_alloc(self.category, 1);
    }

    /// Freeze the arena into a Vec<T>, moving all initialized elements out.
    pub fn into_vec(mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.len);
        for (chunk_i, chunk) in self.chunks.iter_mut().enumerate() {
            let to_take = if (chunk_i + 1) * self.chunk_capacity <= self.len {
                self.chunk_capacity
            } else {
                self.len % self.chunk_capacity
            };
            for i in 0..to_take {
                // SAFETY: we only read elements we wrote with push
                let val = unsafe { chunk[i].assume_init_read() };
                out.push(val);
            }
        }
        out
    }
}
