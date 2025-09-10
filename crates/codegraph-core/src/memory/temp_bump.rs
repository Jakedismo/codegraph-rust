use bumpalo::Bump;
use super::debug::{MemoryCategory, MEMORY_TRACKER};

/// TempBump provides a scoped bump allocator intended for temporary
/// allocations during parsing/analysis. Dropping a `Scope` frees all
/// allocations performed within that scope.
#[derive(Default)]
pub struct TempBump {
    inner: Bump,
}

impl TempBump {
    pub fn new() -> Self { Self { inner: Bump::new() } }
    pub fn with_capacity(bytes: usize) -> Self { Self { inner: Bump::with_capacity(bytes) } }

    /// Start a new scope. All allocations through the returned `Scope` will
    /// be freed when it is dropped. Not re-entrant: do not nest scopes.
    pub fn scope(&mut self) -> Scope<'_> {
        let bump_mut: *mut Bump = &mut self.inner;
        let bump_ref: &Bump = unsafe { &*bump_mut };
        Scope { bump_ref, bump_mut, bytes_allocated: 0 }
    }
}

pub struct Scope<'a> { bump_ref: &'a Bump, bump_mut: *mut Bump, bytes_allocated: usize }

impl<'a> Scope<'a> {
    #[inline]
    fn bump_ref(&self) -> &Bump { self.bump_ref }

    #[inline]
    pub fn alloc_str<'b>(&'b mut self, s: &str) -> &'b str {
        self.bytes_allocated += s.len();
        self.bump_ref().alloc_str(s)
    }

    #[inline]
    pub fn alloc_slice_clone<'b, T: Clone>(&'b mut self, slice: &[T]) -> &'b [T] {
        self.bytes_allocated += std::mem::size_of::<T>() * slice.len();
        self.bump_ref().alloc_slice_clone(slice)
    }

    #[inline]
    pub fn alloc<'b, T>(&'b mut self, value: T) -> &'b mut T {
        self.bytes_allocated += std::mem::size_of::<T>();
        self.bump_ref().alloc(value)
    }
}

impl<'a> Drop for Scope<'a> {
    fn drop(&mut self) {
        MEMORY_TRACKER.record_bytes(MemoryCategory::TempAlloc, self.bytes_allocated as i64);
        // Reset the entire bump to release memory from this temp scope
        // SAFETY: Scope was created from &mut TempBump (unique), not re-entrant.
        unsafe { (&mut *self.bump_mut).reset() };
    }
}
