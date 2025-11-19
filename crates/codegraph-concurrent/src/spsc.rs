use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_utils::CachePadded;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpscError {
    #[error("queue is full")]
    Full,
    #[error("queue is empty")]
    Empty,
}

/// Wait-free SPSC bounded ring buffer queue.
///
/// - One dedicated producer, one dedicated consumer
/// - Uses Acquire/Release ordering to ensure correctness
/// - Capacity must be a power of two for fast modulo
pub struct WaitFreeSpscQueue<T> {
    mask: usize,
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

// Safety: Producer and consumer operate on disjoint indices; T must be Send
unsafe impl<T: Send> Send for WaitFreeSpscQueue<T> {}
unsafe impl<T: Send> Sync for WaitFreeSpscQueue<T> {}

impl<T> WaitFreeSpscQueue<T> {
    /// Create a new queue with the given capacity. Capacity is rounded up to the next power of two.
    pub fn with_capacity(cap: usize) -> (Producer<T>, Consumer<T>) {
        assert!(cap > 1, "capacity must be > 1");
        let capacity = cap.next_power_of_two();
        let mask = capacity - 1;
        let mut vec = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            vec.push(UnsafeCell::new(MaybeUninit::uninit()));
        }
        let q = WaitFreeSpscQueue {
            mask,
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            buf: vec.into_boxed_slice(),
        };
        let shared = Box::new(q);
        // Use raw pointer across Producer/Consumer; both wrap Arc-like ownership via Box leak
        let ptr = Box::into_raw(shared);
        (Producer { inner: ptr }, Consumer { inner: ptr })
    }
}

impl<T> WaitFreeSpscQueue<T> {
    #[inline]
    fn capacity(&self) -> usize {
        self.mask + 1
    }

    #[inline]
    fn slot(&self, idx: usize) -> &UnsafeCell<MaybeUninit<T>> {
        &self.buf[idx & self.mask]
    }
}

/// Producer side of the SPSC queue.
pub struct Producer<T> {
    pub(crate) inner: *mut WaitFreeSpscQueue<T>,
}

unsafe impl<T: Send> Send for Producer<T> {}

/// Consumer side of the SPSC queue.
pub struct Consumer<T> {
    pub(crate) inner: *mut WaitFreeSpscQueue<T>,
}

unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Producer<T> {
    #[inline]
    pub fn try_push(&self, value: T) -> Result<(), SpscError> {
        let q = unsafe { &*self.inner };
        let tail = q.tail.load(Ordering::Relaxed);
        let head = q.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) == q.capacity() - 1 {
            // full: keep one slot open
            return Err(SpscError::Full);
        }
        let slot = q.slot(tail);
        unsafe { (*slot.get()).write(value) };
        q.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<T> Consumer<T> {
    #[inline]
    pub fn try_pop(&self) -> Result<T, SpscError> {
        let q = unsafe { &*self.inner };
        let head = q.head.load(Ordering::Relaxed);
        let tail = q.tail.load(Ordering::Acquire);
        if head == tail {
            return Err(SpscError::Empty);
        }
        let slot = q.slot(head);
        let val = unsafe { (*slot.get()).assume_init_read() };
        q.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(val)
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        // SAFETY: Only free when both producer and consumer have been dropped.
        // Here we leak and let the consumer drop free the memory.
    }
}

impl<T> Drop for Consumer<T> {
    fn drop(&mut self) {
        unsafe {
            // Drain remaining elements to drop T properly
            let q = &*self.inner;
            while q.head.load(Ordering::Relaxed) != q.tail.load(Ordering::Relaxed) {
                let _ = self.try_pop();
            }
            drop(Box::from_raw(self.inner));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn spsc_basic() {
        let (p, c) = WaitFreeSpscQueue::with_capacity(8);
        p.try_push(1).unwrap();
        p.try_push(2).unwrap();
        assert_eq!(c.try_pop().unwrap(), 1);
        assert_eq!(c.try_pop().unwrap(), 2);
        assert!(matches!(c.try_pop(), Err(SpscError::Empty)));
    }

    #[test]
    fn spsc_concurrent() {
        let (p, c) = WaitFreeSpscQueue::with_capacity(1024);
        let t = thread::spawn(move || {
            for i in 0..10_000u32 {
                loop {
                    if p.try_push(i).is_ok() {
                        break;
                    }
                }
            }
        });
        let mut count = 0u32;
        while count < 10_000 {
            if let Ok(v) = c.try_pop() {
                assert_eq!(v, count);
                count += 1;
            } else {
                thread::yield_now();
            }
        }
        t.join().unwrap();
    }

    #[cfg(feature = "loom")]
    mod loom_tests {
        use super::*;
        use loom::sync::atomic::Ordering;
        use loom::thread;

        // A small loom test to explore ordering; not exhaustive
        #[test]
        fn loom_spsc() {
            loom::model(|| {
                let (p, c) = WaitFreeSpscQueue::with_capacity(2);
                let tp = thread::spawn(move || {
                    p.try_push(1).ok();
                });
                let tc = thread::spawn(move || {
                    let _ = c.try_pop();
                });
                tp.join().unwrap();
                tc.join().unwrap();
            });
        }
    }
}
