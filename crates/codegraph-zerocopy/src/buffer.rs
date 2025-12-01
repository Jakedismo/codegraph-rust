//! Zero-copy buffer management patterns
//!
//! This module provides efficient buffer management without unnecessary copying,
//! including ring buffers, pooled buffers, and shared memory buffers.

use crate::{ZeroCopyError, ZeroCopyResult};
use arc_swap::ArcSwap;
use bytes::{Bytes, BytesMut};
use crossbeam_queue::{ArrayQueue, SegQueue};
use parking_lot::RwLock;
use std::{
    alloc::{alloc, dealloc, Layout},
    ptr::NonNull,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tracing::{debug, instrument, trace, warn};

/// Alignment for zero-copy buffers
#[allow(dead_code)]
const CACHE_LINE_SIZE: usize = 64;

/// Pool of reusable buffers to minimize allocations
pub struct BufferPool {
    buffers: SegQueue<BytesMut>,
    buffer_size: usize,
    max_buffers: usize,
    allocated_count: AtomicUsize,
    hit_count: AtomicUsize,
    miss_count: AtomicUsize,
}

impl BufferPool {
    /// Create a new buffer pool
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        Self {
            buffers: SegQueue::new(),
            buffer_size,
            max_buffers,
            allocated_count: AtomicUsize::new(0),
            hit_count: AtomicUsize::new(0),
            miss_count: AtomicUsize::new(0),
        }
    }

    /// Get a buffer from the pool, creating a new one if necessary
    #[instrument(skip(self))]
    pub fn get(&self) -> BytesMut {
        if let Some(mut buffer) = self.buffers.pop() {
            buffer.clear();
            self.hit_count.fetch_add(1, Ordering::Relaxed);
            trace!("Buffer pool hit, reusing buffer");
            buffer
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            let buffer = BytesMut::with_capacity(self.buffer_size);
            trace!("Buffer pool miss, allocating new buffer");
            buffer
        }
    }

    /// Return a buffer to the pool
    #[instrument(skip(self, buffer))]
    pub fn put(&self, buffer: BytesMut) {
        let current_count = self.allocated_count.load(Ordering::Relaxed);

        if current_count < self.max_buffers
            && buffer.capacity() >= self.buffer_size / 2
            && buffer.capacity() <= self.buffer_size * 2
        {
            self.buffers.push(buffer);
            self.allocated_count.fetch_add(1, Ordering::Relaxed);
            trace!("Returned buffer to pool");
        } else {
            trace!("Dropping oversized or when pool full");
            // Buffer will be dropped, freeing memory
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            buffer_size: self.buffer_size,
            max_buffers: self.max_buffers,
            allocated_count: self.allocated_count.load(Ordering::Relaxed),
            hit_count: self.hit_count.load(Ordering::Relaxed),
            miss_count: self.miss_count.load(Ordering::Relaxed),
            available_count: self.buffers.len(),
        }
    }
}

/// Statistics for buffer pool performance monitoring
#[derive(Debug, Clone, Default)]
pub struct BufferPoolStats {
    pub buffer_size: usize,
    pub max_buffers: usize,
    pub allocated_count: usize,
    pub hit_count: usize,
    pub miss_count: usize,
    pub available_count: usize,
}

impl BufferPoolStats {
    /// Calculate hit rate percentage
    pub fn hit_rate(&self) -> f64 {
        if self.hit_count + self.miss_count == 0 {
            0.0
        } else {
            (self.hit_count as f64) / ((self.hit_count + self.miss_count) as f64) * 100.0
        }
    }
}

/// Lock-free ring buffer for high-performance single-producer single-consumer scenarios
pub struct RingBuffer {
    buffer: Box<[u8]>,
    capacity: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl RingBuffer {
    /// Create a new ring buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        // Ensure capacity is power of 2 for efficient modulo operations
        let capacity = capacity.next_power_of_two();
        let buffer = vec![0u8; capacity].into_boxed_slice();

        Self {
            buffer,
            capacity,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Write data to the ring buffer
    /// Returns the number of bytes written
    pub fn write(&self, data: &[u8]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        let available = self.available_write_space(head, tail);
        let write_len = data.len().min(available);

        if write_len == 0 {
            return 0;
        }

        // Calculate write positions considering wrap-around
        let mask = self.capacity - 1;
        let start_pos = head & mask;
        let end_pos = (head + write_len) & mask;

        if start_pos < end_pos {
            // No wrap-around
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.buffer.as_ptr().add(start_pos) as *mut u8,
                    write_len,
                );
            }
        } else {
            // Handle wrap-around
            let first_chunk = self.capacity - start_pos;
            let second_chunk = write_len - first_chunk;

            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.buffer.as_ptr().add(start_pos) as *mut u8,
                    first_chunk,
                );
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_chunk),
                    self.buffer.as_ptr() as *mut u8,
                    second_chunk,
                );
            }
        }

        self.head.store(head + write_len, Ordering::Release);
        write_len
    }

    /// Read data from the ring buffer
    /// Returns the number of bytes read
    pub fn read(&self, output: &mut [u8]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        let available = self.available_read_data(head, tail);
        let read_len = output.len().min(available);

        if read_len == 0 {
            return 0;
        }

        // Calculate read positions considering wrap-around
        let mask = self.capacity - 1;
        let start_pos = tail & mask;
        let end_pos = (tail + read_len) & mask;

        if start_pos < end_pos {
            // No wrap-around
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.buffer.as_ptr().add(start_pos),
                    output.as_mut_ptr(),
                    read_len,
                );
            }
        } else {
            // Handle wrap-around
            let first_chunk = self.capacity - start_pos;
            let second_chunk = read_len - first_chunk;

            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.buffer.as_ptr().add(start_pos),
                    output.as_mut_ptr(),
                    first_chunk,
                );
                std::ptr::copy_nonoverlapping(
                    self.buffer.as_ptr(),
                    output.as_mut_ptr().add(first_chunk),
                    second_chunk,
                );
            }
        }

        self.tail.store(tail + read_len, Ordering::Release);
        read_len
    }

    /// Get the amount of data available for reading
    pub fn available_read(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        self.available_read_data(head, tail)
    }

    /// Get the amount of space available for writing
    pub fn available_write(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        self.available_write_space(head, tail)
    }

    fn available_read_data(&self, head: usize, tail: usize) -> usize {
        head.wrapping_sub(tail)
    }

    fn available_write_space(&self, head: usize, tail: usize) -> usize {
        self.capacity - 1 - self.available_read_data(head, tail)
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Shared buffer that can be safely shared between threads
pub struct SharedBuffer {
    data: Arc<ArcSwap<Bytes>>,
    version: AtomicUsize,
}

impl SharedBuffer {
    /// Create a new shared buffer
    pub fn new(initial_data: Bytes) -> Self {
        Self {
            data: Arc::new(ArcSwap::from_pointee(initial_data)),
            version: AtomicUsize::new(0),
        }
    }

    /// Update the buffer with new data
    #[instrument(skip(self, new_data))]
    pub fn update(&self, new_data: Bytes) {
        self.data.store(Arc::new(new_data));
        self.version.fetch_add(1, Ordering::Release);
        debug!(
            "Updated shared buffer, new version: {}",
            self.version.load(Ordering::Acquire)
        );
    }

    /// Get a reference to the current data
    pub fn load(&self) -> Arc<Bytes> {
        self.data.load_full()
    }

    /// Get the current version number
    pub fn version(&self) -> usize {
        self.version.load(Ordering::Acquire)
    }

    /// Check if the buffer has been updated since the given version
    pub fn is_updated_since(&self, version: usize) -> bool {
        self.version.load(Ordering::Acquire) > version
    }
}

/// Aligned buffer allocator for zero-copy operations
pub struct AlignedBuffer {
    ptr: NonNull<u8>,
    layout: Layout,
    capacity: usize,
}

impl AlignedBuffer {
    /// Allocate a new aligned buffer
    pub fn new(size: usize, alignment: usize) -> ZeroCopyResult<Self> {
        let layout = Layout::from_size_align(size, alignment)
            .map_err(|e| ZeroCopyError::Buffer(format!("Invalid layout: {}", e)))?;

        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(ZeroCopyError::Buffer(
                "Failed to allocate memory".to_string(),
            ));
        }

        Ok(Self {
            ptr: NonNull::new(ptr).unwrap(),
            layout,
            capacity: size,
        })
    }

    /// Get a mutable slice to the buffer
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
    }

    /// Get a slice to the buffer
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.capacity) }
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the buffer alignment
    pub fn alignment(&self) -> usize {
        self.layout.align()
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

// Safety: AlignedBuffer can be sent between threads
unsafe impl Send for AlignedBuffer {}
unsafe impl Sync for AlignedBuffer {}

/// Multi-producer multi-consumer buffer queue
pub struct MPMCBufferQueue {
    queue: ArrayQueue<Bytes>,
    total_enqueued: AtomicUsize,
    total_dequeued: AtomicUsize,
}

impl MPMCBufferQueue {
    /// Create a new MPMC buffer queue
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
            total_enqueued: AtomicUsize::new(0),
            total_dequeued: AtomicUsize::new(0),
        }
    }

    /// Push a buffer to the queue
    pub fn push(&self, buffer: Bytes) -> Result<(), Bytes> {
        match self.queue.push(buffer) {
            Ok(()) => {
                self.total_enqueued.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(buffer) => Err(buffer),
        }
    }

    /// Pop a buffer from the queue
    pub fn pop(&self) -> Option<Bytes> {
        match self.queue.pop() {
            Some(buffer) => {
                self.total_dequeued.fetch_add(1, Ordering::Relaxed);
                Some(buffer)
            }
            None => None,
        }
    }

    /// Get the current length of the queue
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }

    /// Get queue statistics
    pub fn stats(&self) -> MPMCBufferQueueStats {
        MPMCBufferQueueStats {
            capacity: self.queue.capacity(),
            length: self.queue.len(),
            total_enqueued: self.total_enqueued.load(Ordering::Relaxed),
            total_dequeued: self.total_dequeued.load(Ordering::Relaxed),
        }
    }
}

/// Statistics for MPMC buffer queue
#[derive(Debug, Clone, Default)]
pub struct MPMCBufferQueueStats {
    pub capacity: usize,
    pub length: usize,
    pub total_enqueued: usize,
    pub total_dequeued: usize,
}

/// Buffer manager that coordinates multiple buffer pools and queues
pub struct BufferManager {
    small_pool: BufferPool,
    medium_pool: BufferPool,
    large_pool: BufferPool,
    mpmc_queue: MPMCBufferQueue,
    stats: RwLock<BufferManagerStats>,
}

impl BufferManager {
    /// Create a new buffer manager with default pool sizes
    pub fn new() -> Self {
        Self {
            small_pool: BufferPool::new(4096, 100),   // 4KB buffers
            medium_pool: BufferPool::new(65536, 50),  // 64KB buffers
            large_pool: BufferPool::new(1048576, 10), // 1MB buffers
            mpmc_queue: MPMCBufferQueue::new(1000),
            stats: RwLock::new(BufferManagerStats::default()),
        }
    }

    /// Get an appropriately sized buffer
    #[instrument(skip(self))]
    pub fn get_buffer(&self, size: usize) -> BytesMut {
        let mut stats = self.stats.write();
        stats.total_requests += 1;

        let buffer = if size <= 4096 {
            stats.small_requests += 1;
            self.small_pool.get()
        } else if size <= 65536 {
            stats.medium_requests += 1;
            self.medium_pool.get()
        } else if size <= 1048576 {
            stats.large_requests += 1;
            self.large_pool.get()
        } else {
            stats.oversized_requests += 1;
            BytesMut::with_capacity(size)
        };

        buffer
    }

    /// Return a buffer to the appropriate pool
    #[instrument(skip(self, buffer))]
    pub fn return_buffer(&self, buffer: BytesMut) {
        let capacity = buffer.capacity();

        if capacity <= 8192 {
            self.small_pool.put(buffer);
        } else if capacity <= 131072 {
            self.medium_pool.put(buffer);
        } else if capacity <= 2097152 {
            self.large_pool.put(buffer);
        }
        // Large buffers are dropped
    }

    /// Submit a buffer to the MPMC queue
    pub fn submit_buffer(&self, buffer: Bytes) -> Result<(), Bytes> {
        self.mpmc_queue.push(buffer)
    }

    /// Retrieve a buffer from the MPMC queue
    pub fn retrieve_buffer(&self) -> Option<Bytes> {
        self.mpmc_queue.pop()
    }

    /// Get comprehensive buffer manager statistics
    pub fn stats(&self) -> BufferManagerStats {
        let mut stats = self.stats.read().clone();
        stats.small_pool_stats = self.small_pool.stats();
        stats.medium_pool_stats = self.medium_pool.stats();
        stats.large_pool_stats = self.large_pool.stats();
        stats.mpmc_queue_stats = self.mpmc_queue.stats();
        stats
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive statistics for buffer manager
#[derive(Debug, Clone, Default)]
pub struct BufferManagerStats {
    pub total_requests: usize,
    pub small_requests: usize,
    pub medium_requests: usize,
    pub large_requests: usize,
    pub oversized_requests: usize,
    pub small_pool_stats: BufferPoolStats,
    pub medium_pool_stats: BufferPoolStats,
    pub large_pool_stats: BufferPoolStats,
    pub mpmc_queue_stats: MPMCBufferQueueStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024, 10);

        // Test getting and returning buffers
        let buffer1 = pool.get();
        assert_eq!(buffer1.capacity(), 1024);

        pool.put(buffer1);

        let buffer2 = pool.get();
        assert_eq!(buffer2.capacity(), 1024);

        let stats = pool.stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
    }

    #[test]
    fn test_ring_buffer() {
        let ring = RingBuffer::new(64);

        // Test basic write/read
        let data = b"hello world";
        let written = ring.write(data);
        assert_eq!(written, data.len());

        let mut output = [0u8; 20];
        let read = ring.read(&mut output);
        assert_eq!(read, data.len());
        assert_eq!(&output[..read], data);

        // Test wrap-around
        let large_data = [42u8; 100];
        let written = ring.write(&large_data);
        assert!(written < large_data.len()); // Should be limited by capacity
    }

    #[test]
    fn test_shared_buffer() {
        let initial_data = Bytes::from_static(b"initial");
        let shared = SharedBuffer::new(initial_data);

        assert_eq!(shared.version(), 0);

        let data = shared.load();
        assert_eq!(&**data, b"initial");

        shared.update(Bytes::from_static(b"updated"));
        assert_eq!(shared.version(), 1);

        let updated_data = shared.load();
        assert_eq!(&**updated_data, b"updated");
    }

    #[test]
    fn test_aligned_buffer() {
        let mut buffer = AlignedBuffer::new(1024, 64).unwrap();

        assert_eq!(buffer.capacity(), 1024);
        assert_eq!(buffer.alignment(), 64);

        let slice = buffer.as_mut_slice();
        slice[0] = 42;
        slice[1023] = 24;

        let read_slice = buffer.as_slice();
        assert_eq!(read_slice[0], 42);
        assert_eq!(read_slice[1023], 24);
    }

    #[test]
    fn test_mpmc_buffer_queue() {
        let queue = MPMCBufferQueue::new(10);

        // Test single-threaded operations
        let data = Bytes::from_static(b"test data");
        queue.push(data.clone()).unwrap();

        let retrieved = queue.pop().unwrap();
        assert_eq!(retrieved, data);

        // Test queue full
        for i in 0..10 {
            let data = Bytes::from(format!("data {}", i));
            queue.push(data).unwrap();
        }

        let overflow_data = Bytes::from_static(b"overflow");
        assert!(queue.push(overflow_data).is_err());
    }

    #[test]
    fn test_buffer_manager() {
        let manager = BufferManager::new();

        // Test different sized buffer requests
        let small_buf = manager.get_buffer(1024);
        assert!(small_buf.capacity() >= 1024);

        let medium_buf = manager.get_buffer(32768);
        assert!(medium_buf.capacity() >= 32768);

        let large_buf = manager.get_buffer(524288);
        assert!(large_buf.capacity() >= 524288);

        // Return buffers
        manager.return_buffer(small_buf);
        manager.return_buffer(medium_buf);
        manager.return_buffer(large_buf);

        let stats = manager.stats();
        assert!(stats.total_requests >= 3);
    }

    #[test]
    fn test_concurrent_ring_buffer() {
        let ring = Arc::new(RingBuffer::new(1024));
        let ring_reader = ring.clone();
        let ring_writer = ring.clone();

        let writer = thread::spawn(move || {
            for i in 0..100 {
                let data = format!("message {}", i);
                while ring_writer.write(data.as_bytes()) == 0 {
                    thread::yield_now();
                }
            }
        });

        let reader = thread::spawn(move || {
            let mut received = 0;
            let mut buffer = [0u8; 100];

            while received < 100 {
                let read = ring_reader.read(&mut buffer);
                if read > 0 {
                    received += 1;
                }
                thread::yield_now();
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }
}
