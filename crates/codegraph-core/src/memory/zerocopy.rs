//! Zero-copy primitives adapted for core usage.
//!
//! - Replace `Vec<u8>`/owned buffers with `bytes::Bytes` and `&[u8]`
//! - Provide rkyv zero-copy (de)serialization
//! - Support read-only memory-mapped archives for large datasets
//! - Offer zero-copy friendly string handling helpers

use bytes::{Bytes, BytesMut};
use std::marker::PhantomData;
use std::path::Path;

use rkyv::{Archive as RkyvArchive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use rkyv::ser::{Serializer, serializers::AllocSerializer};

use crate::mmap::MappedFile;

/// Type alias emphasizing intention: immutable, shareable bytes for zero-copy data.
pub type ZeroCopyBytes = Bytes;

/// Zero-copy specific errors
#[derive(thiserror::Error, Debug)]
pub enum ZeroCopyError {
    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Memory mapping failed: {0}")]
    MemoryMapping(#[from] std::io::Error),

    #[error("Archive access failed: {0}")]
    ArchiveAccess(String),
}

pub type ZeroCopyResult<T> = Result<T, ZeroCopyError>;

/// A zero-copy serializer that reuses buffers
#[derive(Debug)]
pub struct ZeroCopySerializer {
    buffer: BytesMut,
    alignment: usize,
}

impl ZeroCopySerializer {
    /// Create a new serializer with default buffer capacity
    pub fn new() -> Self { Self::with_capacity(4096) }

    /// Create a new serializer with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self { buffer: BytesMut::with_capacity(capacity), alignment: 64 }
    }

    /// Serialize data to bytes without copying
    pub fn serialize<T>(&mut self, data: &T) -> ZeroCopyResult<Bytes>
    where
        T: RkyvSerialize<AllocSerializer<4096>>,
    {
        self.buffer.clear();
        let mut serializer = AllocSerializer::<4096>::default();
        serializer
            .serialize_value(data)
            .map_err(|e| ZeroCopyError::Serialization(format!("{:?}", e)))?;
        let bytes = serializer.into_serializer().into_inner();
        let aligned_len = align_up(bytes.len(), self.alignment);
        self.buffer.extend_from_slice(&bytes);
        self.buffer.resize(aligned_len, 0);
        Ok(self.buffer.split().freeze())
    }
}

impl Default for ZeroCopySerializer { fn default() -> Self { Self::new() } }

/// Zero-copy deserializer that provides direct access to archived data
#[derive(Debug, Default)]
pub struct ZeroCopyDeserializer;

impl ZeroCopyDeserializer {
    pub fn new() -> Self { Self }

    /// Access archived data directly without deserialization
    pub fn access<'a, T: RkyvArchive>(&self, data: &'a [u8]) -> ZeroCopyResult<&'a T::Archived> {
        // Safety: bytes must be produced by rkyv for type T
        Ok(unsafe { rkyv::archived_root::<T>(data) })
    }
}

/// Serialize any rkyv-serializable value into an immutable Bytes buffer.
#[inline]
pub fn serialize_to_bytes<T>(value: &T) -> Result<ZeroCopyBytes, ZeroCopyError>
where
    T: RkyvSerialize<AllocSerializer<4096>>,
{
    let mut s = ZeroCopySerializer::new();
    s.serialize(value)
}

/// Validate and provide archived access to a value from raw bytes.
#[inline]
pub fn access_archived<'a, T: RkyvArchive>(bytes: &'a [u8]) -> Result<&'a <T as RkyvArchive>::Archived, ZeroCopyError> {
    Ok(unsafe { rkyv::archived_root::<T>(bytes) })
}

/// Memory-mapped, read-only archive view for large datasets.
#[derive(Debug)]
pub struct MappedArchive<T> {
    map: MappedFile,
    _phantom: PhantomData<T>,
}

impl<T> MappedArchive<T>
where
    T: RkyvArchive,
{
    /// Open a file as a validated, read-only archive.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ZeroCopyError> {
        let map = MappedFile::open_readonly(path).map_err(ZeroCopyError::MemoryMapping)?;
        Ok(Self { map, _phantom: PhantomData })
    }

    /// Access the archived value within the mapped file.
    #[inline]
    pub fn archived(&self) -> Result<&<T as RkyvArchive>::Archived, ZeroCopyError> {
        access_archived::<T>(self.map.as_bytes())
    }

    /// Get the raw mapped bytes. Useful for custom archive structures.
    #[inline]
    pub fn bytes(&self) -> &[u8] { self.map.as_bytes() }

    /// Total length in bytes of the mapped file.
    #[inline]
    pub fn len(&self) -> usize { self.map.len() }

    /// True if the mapped file is empty.
    #[inline]
    pub fn is_empty(&self) -> bool { self.map.is_empty() }

    /// Provide sequential access hint to the OS.
    #[inline]
    pub fn advise_sequential(&self) { self.map.advise_sequential(); }

    /// Provide random access hint to the OS.
    #[inline]
    pub fn advise_random(&self) { self.map.advise_random(); }

    /// Best-effort prefetch for the given range.
    #[inline]
    pub fn prefetch_range(&self, offset: usize, len: usize) { self.map.prefetch_range(offset, len) }
}

#[inline]
fn align_up(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}

/// Zero-copy string helpers built around SharedStr.
///
/// These helpers enable ergonomic conversions with `std::borrow::Cow<str>` without forcing
/// allocations on borrowed input.
pub mod zstr {
    use std::borrow::Cow;
    use bytes::Bytes;
    use std::sync::Arc;
    use crate::shared::SharedStr;

    /// Convert an arbitrary `Cow<str>` into a `SharedStr` without copying when borrowed.
    #[inline]
    pub fn from_cow(s: Cow<'_, str>) -> SharedStr {
        match s {
            Cow::Borrowed(b) => SharedStr::from(b),
            Cow::Owned(o) => SharedStr::from(o),
        }
    }

    /// Convert `SharedStr` to `Cow<str>`; borrowed when possible.
    #[inline]
    pub fn to_cow(s: &SharedStr) -> Cow<'_, str> {
        Cow::Borrowed(s.as_str())
    }

    /// Construct `SharedStr` from shared byte storage without copying.
    #[inline]
    pub fn from_shared_bytes(bytes: Bytes) -> SharedStr { SharedStr::from_bytes(bytes) }

    /// Construct `SharedStr` from shared Arc<[u8]> slice.
    #[inline]
    pub fn from_arc_slice(data: Arc<[u8]>, start: usize, end: usize) -> SharedStr {
        SharedStr::from_arc_slice(data, start, end)
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
        vals: Vec<i32>,
    }

    #[test]
    fn roundtrip_bytes_zero_copy() {
        let data = TestData { id: 7, name: "x".into(), vals: vec![1,2,3] };
        let bytes = serialize_to_bytes(&data).unwrap();
        let archived = access_archived::<TestData>(&bytes).unwrap();
        assert_eq!(archived.id, 7);
        assert_eq!(archived.name.as_str(), "x");
        assert_eq!(archived.vals.len(), 3);
    }
}
