//! Zero-copy data structures and serialization for CodeGraph
//!
//! This crate provides zero-copy patterns for efficient data handling:
//! - Zero-copy serialization with rkyv
//! - Memory-mapped file access patterns
//! - Buffer management without copying
//! - Shared memory optimization

pub mod archived;
pub mod buffer;
pub mod mmap;
pub mod serialization;
pub mod shared_memory;

// Re-export key types for convenience
pub use archived::*;
pub use buffer::*;
pub use mmap::*;
pub use serialization::*;
pub use shared_memory::*;

// Re-export rkyv types for external use
pub use rkyv::{
    api::{access, access_unchecked, deserialize, from_bytes, from_bytes_unchecked, to_bytes},
    Archive, Deserialize, Serialize,
};

use thiserror::Error;

/// Zero-copy specific errors
#[derive(Error, Debug)]
pub enum ZeroCopyError {
    #[error("Serialization failed: {0}")]
    Serialization(#[from] rkyv::rancor::Error),
    
    #[error("Validation failed: {0}")]
    Validation(String),
    
    #[error("Memory mapping failed: {0}")]
    MemoryMapping(#[from] std::io::Error),
    
    #[error("Buffer operation failed: {0}")]
    Buffer(String),
    
    #[error("Shared memory operation failed: {0}")]
    SharedMemory(String),
    
    #[error("Archive access failed: {0}")]
    ArchiveAccess(String),
}

pub type ZeroCopyResult<T> = Result<T, ZeroCopyError>;

/// Constants for zero-copy operations
pub mod constants {
    /// Default alignment for zero-copy structures
    pub const DEFAULT_ALIGNMENT: usize = 64;
    
    /// Default page size for memory mapping
    pub const DEFAULT_PAGE_SIZE: usize = 4096;
    
    /// Buffer size for streaming operations
    pub const STREAM_BUFFER_SIZE: usize = 64 * 1024;
    
    /// Maximum shared memory segment size
    pub const MAX_SHARED_MEMORY_SIZE: usize = 1024 * 1024 * 1024; // 1GB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_serialization() {
        use rkyv::{Archive, Deserialize, Serialize};
        
        #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
        struct TestData {
            id: u64,
            name: String,
            values: Vec<i32>,
        }
        
        let data = TestData {
            id: 42,
            name: "test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };
        
        let bytes = to_bytes::<rkyv::rancor::Error>(&data).unwrap();
        let archived = from_bytes::<TestData, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.id, 42);
        assert_eq!(archived.name, "test");
        assert_eq!(archived.values.len(), 5);
    }
}