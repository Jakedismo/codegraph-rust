//! Memory subsystem: arenas, interning, temporary allocators and tracking.
//!
//! Components:
//! - `arena`: Paged arena for long-lived data + bump arena wrapper
//! - `string_interner`: Global interner to deduplicate strings
//! - `temp_bump`: Scoped bump allocator for parsing and short-lived data
//! - `debug`: Memory tracker to record usage by category
//! - `compact_map`: HashMap alternative using hashbrown + Fx hasher

pub mod arena;
pub mod string_interner;
pub mod temp_bump;
pub mod debug;
pub mod compact_map;

pub use arena::*;
pub use string_interner::*;
pub use temp_bump::*;
pub use debug::*;
pub use compact_map::*;

