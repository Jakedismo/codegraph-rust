use crossbeam_queue::ArrayQueue;
use thiserror::Error;
use std::sync::Arc;

#[derive(Debug, Error)]
pub enum MpmcError {
    #[error("queue is full")] Full,
    #[error("queue is empty")] Empty,
}

/// Lock-free bounded MPMC queue based on crossbeam's ArrayQueue.
/// Provides fast, concurrent multi-producer multi-consumer semantics.
pub struct LockFreeMpmcQueue<T> {
    inner: Arc<ArrayQueue<T>>,
}

impl<T> Clone for LockFreeMpmcQueue<T> {
    fn clone(&self) -> Self { Self { inner: self.inner.clone() } }
}

impl<T> LockFreeMpmcQueue<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Arc::new(ArrayQueue::new(capacity)) }
    }

    #[inline]
    pub fn try_push(&self, value: T) -> Result<(), MpmcError> {
        self.inner.push(value).map_err(|_| MpmcError::Full)
    }

    #[inline]
    pub fn try_pop(&self) -> Result<T, MpmcError> {
        self.inner.pop().map_err(|_| MpmcError::Empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn mpmc_basic() {
        let q = LockFreeMpmcQueue::with_capacity(4);
        q.try_push(1).unwrap();
        q.try_push(2).unwrap();
        assert_eq!(q.try_pop().unwrap(), 1);
        assert_eq!(q.try_pop().unwrap(), 2);
    }

    #[test]
    fn mpmc_concurrent() {
        let q = LockFreeMpmcQueue::with_capacity(1024);
        let q1 = q.clone();
        let q2 = q.clone();
        let prod = thread::spawn(move || {
            for i in 0..10_000 {
                loop { if q1.try_push(i).is_ok() { break; } }
            }
        });
        let prod2 = thread::spawn(move || {
            for i in 10_000..20_000 {
                loop { if q2.try_push(i).is_ok() { break; } }
            }
        });
        let mut seen = 0usize;
        while seen < 20_000 {
            if let Ok(_v) = q.try_pop() { seen += 1; } else { thread::yield_now(); }
        }
        prod.join().unwrap();
        prod2.join().unwrap();
    }
}

