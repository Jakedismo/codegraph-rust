//! Zero-copy serialization patterns using rkyv
//!
//! This module provides high-performance serialization without data copying.

use crate::{ZeroCopyError, ZeroCopyResult};
use bytes::{Bytes, BytesMut};
use rkyv::{
    api::{access, access_unchecked, deserialize, from_bytes, from_bytes_unchecked, to_bytes}, 
    Archive, Deserialize, Serialize,
};
use std::sync::Arc;
use tracing::{debug, instrument, trace};

/// A zero-copy serializer that reuses buffers
pub struct ZeroCopySerializer {
    buffer: BytesMut,
    alignment: usize,
}

impl ZeroCopySerializer {
    /// Create a new serializer with default buffer capacity
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }
    
    /// Create a new serializer with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
            alignment: crate::constants::DEFAULT_ALIGNMENT,
        }
    }
    
    /// Serialize data to bytes without copying
    #[instrument(skip(self, data))]
    pub fn serialize<T>(&mut self, data: &T) -> ZeroCopyResult<Bytes>
    where
        T: for<'a> Serialize<rkyv::rancor::Strategy<rkyv::ser::Serializer<rkyv::util::AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, rkyv::ser::sharing::Share>, rkyv::rancor::Error>>,
    {
        self.buffer.clear();
        
        let bytes = to_bytes::<rkyv::rancor::Error>(data)
            .map_err(ZeroCopyError::Serialization)?;
        
        // Ensure proper alignment
        let aligned_len = align_up(bytes.len(), self.alignment);
        self.buffer.extend_from_slice(&bytes);
        self.buffer.resize(aligned_len, 0);
        
        Ok(self.buffer.split().freeze())
    }
    
    /// Serialize with validation
    #[instrument(skip(self, data))]
    pub fn serialize_validated<T>(&mut self, data: &T) -> ZeroCopyResult<Bytes>
    where
        T: Serialize<rkyv::rancor::Error> + Archive,
        T::Archived: for<'a> bytecheck::CheckBytes<bytecheck::rancor::Strategy>,
    {
        let bytes = self.serialize(data)?;
        
        // Validate the serialized data
        let _archived = access::<T, rkyv::rancor::Error>(&bytes)
            .map_err(|e| ZeroCopyError::Validation(format!("Validation failed: {:?}", e)))?;
        
        Ok(bytes)
    }
}

impl Default for ZeroCopySerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-copy deserializer that provides direct access to archived data
pub struct ZeroCopyDeserializer {
    // Buffer to hold shared references
    _buffers: Vec<Arc<Bytes>>,
}

impl ZeroCopyDeserializer {
    pub fn new() -> Self {
        Self {
            _buffers: Vec::new(),
        }
    }
    
    /// Access archived data directly without deserialization
    #[instrument(skip(data))]
    pub fn access<T>(&self, data: &[u8]) -> ZeroCopyResult<&T::Archived>
    where
        T: Archive,
        T::Archived: for<'a> bytecheck::CheckBytes<bytecheck::rancor::Strategy>,
    {
        access::<T, rkyv::rancor::Error>(data)
            .map_err(|e| ZeroCopyError::ArchiveAccess(format!("Access failed: {:?}", e)))
    }
    
    /// Access archived data without validation (unsafe but fast)
    #[instrument(skip(data))]
    pub fn access_unchecked<T>(&self, data: &[u8]) -> &T::Archived
    where
        T: Archive,
    {
        unsafe { access_unchecked::<T>(data) }
    }
    
    /// Deserialize to owned data when necessary
    #[instrument(skip(self, data))]
    pub fn deserialize<T>(&self, data: &[u8]) -> ZeroCopyResult<T>
    where
        T: Archive,
        T::Archived: Deserialize<T, rkyv::rancor::Error> 
            + for<'a> bytecheck::CheckBytes<bytecheck::rancor::Strategy>,
    {
        let archived = self.access::<T>(data)?;
        deserialize::<T, rkyv::rancor::Error>(archived)
            .map_err(ZeroCopyError::Serialization)
    }
}

impl Default for ZeroCopyDeserializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared buffer pool for zero-copy operations
pub struct BufferPool {
    buffers: crossbeam_queue::SegQueue<BytesMut>,
    default_capacity: usize,
}

impl BufferPool {
    pub fn new(default_capacity: usize) -> Self {
        Self {
            buffers: crossbeam_queue::SegQueue::new(),
            default_capacity,
        }
    }
    
    /// Get a buffer from the pool or create a new one
    pub fn get(&self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.default_capacity))
    }
    
    /// Return a buffer to the pool
    pub fn put(&self, mut buffer: BytesMut) {
        buffer.clear();
        if buffer.capacity() <= self.default_capacity * 2 {
            self.buffers.push(buffer);
        }
        // Drop oversized buffers to prevent memory bloat
    }
}

/// Zero-copy container that can hold either owned or borrowed data
pub enum ZeroCopyData<T> {
    Owned(T),
    Archived(Bytes, std::marker::PhantomData<T>),
}

impl<T> ZeroCopyData<T> 
where
    T: Archive,
{
    /// Create from owned data
    pub fn owned(data: T) -> Self {
        Self::Owned(data)
    }
    
    /// Create from archived bytes
    pub fn archived(bytes: Bytes) -> Self {
        Self::Archived(bytes, std::marker::PhantomData)
    }
    
    /// Access the data (either owned or archived)
    pub fn access(&self) -> ZeroCopyResult<ZeroCopyDataRef<'_, T>>
    where
        T::Archived: for<'a> bytecheck::CheckBytes<bytecheck::rancor::Strategy>,
    {
        match self {
            Self::Owned(data) => Ok(ZeroCopyDataRef::Owned(data)),
            Self::Archived(bytes, _) => {
                let archived = access::<T, rkyv::rancor::Error>(bytes)
                    .map_err(|e| ZeroCopyError::ArchiveAccess(format!("Access failed: {:?}", e)))?;
                Ok(ZeroCopyDataRef::Archived(archived))
            }
        }
    }
}

/// Reference to either owned or archived data
pub enum ZeroCopyDataRef<'a, T> {
    Owned(&'a T),
    Archived(&'a T::Archived),
}

/// Helper function to align size up to boundary
fn align_up(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}

/// Streaming serializer for large datasets
pub struct StreamingSerializer {
    serializer: ZeroCopySerializer,
    chunk_size: usize,
}

impl StreamingSerializer {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            serializer: ZeroCopySerializer::with_capacity(chunk_size),
            chunk_size,
        }
    }
    
    /// Serialize items in chunks
    pub fn serialize_chunks<T, I>(&mut self, items: I) -> ZeroCopyResult<Vec<Bytes>>
    where
        T: Serialize<rkyv::rancor::Error>,
        I: IntoIterator<Item = T>,
    {
        let mut chunks = Vec::new();
        let mut chunk = Vec::new();
        let mut current_size = 0;
        
        for item in items {
            let item_bytes = to_bytes::<rkyv::rancor::Error>(&item)
                .map_err(ZeroCopyError::Serialization)?;
            
            if current_size + item_bytes.len() > self.chunk_size && !chunk.is_empty() {
                // Serialize current chunk
                let chunk_bytes = self.serializer.serialize(&chunk)?;
                chunks.push(chunk_bytes);
                chunk.clear();
                current_size = 0;
            }
            
            chunk.push(item);
            current_size += item_bytes.len();
        }
        
        // Serialize remaining items
        if !chunk.is_empty() {
            let chunk_bytes = self.serializer.serialize(&chunk)?;
            chunks.push(chunk_bytes);
        }
        
        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rkyv::{Archive, Deserialize, Serialize};

    #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_zero_copy_serializer() {
        let mut serializer = ZeroCopySerializer::new();
        
        let data = TestData {
            id: 42,
            name: "test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };
        
        let bytes = serializer.serialize(&data).unwrap();
        assert!(!bytes.is_empty());
        
        // Verify we can deserialize
        let deserializer = ZeroCopyDeserializer::new();
        let archived = deserializer.access::<TestData>(&bytes).unwrap();
        
        assert_eq!(archived.id, 42);
        assert_eq!(archived.name, "test");
        assert_eq!(archived.values.len(), 5);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024);
        
        let buf1 = pool.get();
        assert_eq!(buf1.capacity(), 1024);
        
        pool.put(buf1);
        
        let buf2 = pool.get();
        assert_eq!(buf2.capacity(), 1024);
    }

    #[test]
    fn test_zero_copy_data() {
        let data = TestData {
            id: 123,
            name: "example".to_string(),
            values: vec![10, 20, 30],
        };
        
        // Test owned data
        let owned = ZeroCopyData::owned(data.clone());
        let owned_ref = owned.access().unwrap();
        match owned_ref {
            ZeroCopyDataRef::Owned(d) => assert_eq!(d.id, 123),
            _ => panic!("Expected owned data"),
        }
        
        // Test archived data
        let bytes = to_bytes::<rkyv::rancor::Error>(&data).unwrap();
        let archived = ZeroCopyData::<TestData>::archived(Bytes::from(bytes));
        let archived_ref = archived.access().unwrap();
        match archived_ref {
            ZeroCopyDataRef::Archived(d) => assert_eq!(d.id, 123),
            _ => panic!("Expected archived data"),
        }
    }

    #[test]
    fn test_streaming_serializer() {
        let mut streaming = StreamingSerializer::new(1024);
        
        let items: Vec<TestData> = (0..10).map(|i| TestData {
            id: i,
            name: format!("item_{}", i),
            values: vec![i as i32, i as i32 + 1],
        }).collect();
        
        let chunks = streaming.serialize_chunks(items).unwrap();
        assert!(!chunks.is_empty());
        
        // Verify we can access each chunk
        let deserializer = ZeroCopyDeserializer::new();
        for chunk in chunks {
            let _archived = deserializer.access::<Vec<TestData>>(&chunk).unwrap();
        }
    }
}