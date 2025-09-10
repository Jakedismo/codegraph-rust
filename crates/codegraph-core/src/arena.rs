use bumpalo::Bump;

/// Simple wrapper around bumpalo::Bump for scoped arena allocations.
///
/// Typical usage: allocate many short-lived strings and slices during
/// a single parsing/processing pass, then drop the arena to free all.
pub struct Arena {
    bump: Bump,
}

impl Arena {
    pub fn new() -> Self {
        Self { bump: Bump::new() }
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Self { bump: Bump::with_capacity(bytes) }
    }

    pub fn alloc_str<'a>(&'a self, s: &str) -> &'a str {
        self.bump.alloc_str(s)
    }

    pub fn alloc_slice_clone<'a, T: Clone>(&'a self, slice: &[T]) -> &'a [T] {
        self.bump.alloc_slice_clone(slice)
    }

    pub fn clear(&mut self) {
        self.bump.reset();
    }
}
