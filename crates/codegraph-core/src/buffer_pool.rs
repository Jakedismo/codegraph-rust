use parking_lot::Mutex;
use std::sync::{Arc, Weak};
// Memory tracking disabled in minimal build to avoid pulling heavy deps.

#[derive(Debug, Clone)]
pub struct BufferPoolConfig {
    pub capacity: usize,
    pub buffer_size: usize,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self { capacity: 256, buffer_size: 64 * 1024 }
    }
}

#[derive(Debug)]
struct InnerPool {
    free: Vec<Vec<u8>>,
    buffer_size: usize,
    outstanding: usize,
}

#[derive(Debug, Clone)]
pub struct BufferPool {
    inner: Arc<Mutex<InnerPool>>,
    tracker: Arc<LeakTracker>,
}

impl BufferPool {
    pub fn new(config: BufferPoolConfig) -> Self {
        let mut free = Vec::with_capacity(config.capacity);
        for _ in 0..config.capacity.min(16) { // warm a few by default
            free.push(Vec::with_capacity(config.buffer_size));
            // tracking disabled
        }
        let inner = InnerPool { free, buffer_size: config.buffer_size, outstanding: 0 };
        Self { inner: Arc::new(Mutex::new(inner)), tracker: Arc::new(LeakTracker::new()) }
    }

    pub fn get(&self) -> PooledBuffer {
        let mut inner = self.inner.lock();
        let buf = inner.free.pop().unwrap_or_else(|| Vec::with_capacity(inner.buffer_size));
        let _ = buf.capacity(); // keep branch predictable; tracking disabled
        inner.outstanding += 1;
        drop(inner);
        self.tracker.incr();
        PooledBuffer { buf: Some(buf), pool: Arc::downgrade(&self.inner), tracker: Arc::downgrade(&self.tracker) }
    }

    fn put_back(&self, mut buf: Vec<u8>) {
        let cap = buf.capacity();
        buf.clear();
        let mut inner = self.inner.lock();
        if buf.capacity() != inner.buffer_size {
            // normalize capacity for predictable reuse
            buf = Vec::with_capacity(inner.buffer_size);
        }
        inner.free.push(buf);
        inner.outstanding = inner.outstanding.saturating_sub(1);
        // track retained memory
        let _ = cap; // tracking disabled
    }
}

pub struct PooledBuffer {
    buf: Option<Vec<u8>>,
    pool: Weak<Mutex<InnerPool>>,
    tracker: Weak<LeakTracker>,
}

impl PooledBuffer {
    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        self.buf.as_mut().unwrap()
    }

    pub fn into_inner(mut self) -> Vec<u8> {
        self.tracker.upgrade().map(|t| t.decr());
        self.buf.take().unwrap()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            if let Some(pool) = self.pool.upgrade() {
                if let Some(tracker) = self.tracker.upgrade() { tracker.decr(); }
                // avoid deadlock: construct a BufferPool facade and call put_back
                let pool = BufferPool { inner: pool, tracker: Arc::new(LeakTracker::new()) };
                pool.put_back(buf);
            }
        }
    }
}

#[derive(Debug, Default)]
struct LeakTracker {
    outstanding: Mutex<usize>,
}

impl LeakTracker {
    pub fn new() -> Self { Self { outstanding: Mutex::new(0) } }
    pub fn incr(&self) { let mut g = self.outstanding.lock(); *g += 1; }
    pub fn decr(&self) { let mut g = self.outstanding.lock(); *g = g.saturating_sub(1); }

    #[cfg(debug_assertions)]
    pub fn assert_no_leaks(&self) {
        let g = self.outstanding.lock();
        debug_assert_eq!(*g, 0, "BufferPool leak detected: {} outstanding buffers", *g);
    }
}
