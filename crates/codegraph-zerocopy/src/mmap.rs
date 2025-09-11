//! Memory-mapped file access patterns for zero-copy I/O
//!
//! This module provides safe abstractions over memory-mapped files for high-performance
//! data access without copying.

use crate::{ZeroCopyError, ZeroCopyResult};
use memmap2::{Advice, Mmap, MmapMut, MmapOptions};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use rkyv::{access, access_unchecked, Archive};
use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{debug, error, instrument, warn};

/// Memory-mapped file reader with zero-copy access
pub struct MmapReader {
    mmap: Mmap,
    path: PathBuf,
}

impl MmapReader {
    /// Open a file for memory-mapped reading
    #[instrument(skip(path))]
    pub fn open<P: AsRef<Path>>(path: P) -> ZeroCopyResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        debug!(
            "Opened memory-mapped file: {:?}, size: {} bytes",
            path,
            mmap.len()
        );

        Ok(Self { mmap, path })
    }

    /// Get the raw bytes of the memory-mapped file
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Get the size of the mapped file
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if the mapped file is empty
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Access archived data directly from the memory-mapped file
    #[instrument(skip(self))]
    pub fn access_archived<T>(&self) -> ZeroCopyResult<&T::Archived>
    where
        T: Archive,
        T::Archived: for<'a> bytecheck::CheckBytes<
            bytecheck::rancor::Strategy<bytecheck::DefaultValidator, rkyv::rancor::Failure>,
        >,
    {
        access::<T::Archived, rkyv::rancor::Failure>(&self.mmap).map_err(|_e| {
            ZeroCopyError::ArchiveAccess("Failed to access archived data".to_string())
        })
    }

    /// Access archived data without validation (faster but unsafe)
    #[instrument(skip(self))]
    pub fn access_archived_unchecked<T>(&self) -> &T::Archived
    where
        T: Archive,
        T::Archived: rkyv::Portable,
    {
        unsafe { access_unchecked::<T::Archived>(&self.mmap) }
    }

    /// Get a slice of the mapped data
    pub fn slice(&self, start: usize, len: usize) -> ZeroCopyResult<&[u8]> {
        if start + len > self.mmap.len() {
            return Err(ZeroCopyError::Buffer(format!(
                "Slice bounds out of range: {}..{} > {}",
                start,
                start + len,
                self.mmap.len()
            )));
        }

        Ok(&self.mmap[start..start + len])
    }

    /// Advise the kernel about memory access patterns
    pub fn advise(&self, advice: Advice) -> ZeroCopyResult<()> {
        self.mmap.advise(advice)?;
        Ok(())
    }

    /// Get the path of the mapped file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Memory-mapped file writer with zero-copy writes
pub struct MmapWriter {
    mmap: MmapMut,
    path: PathBuf,
    file: File,
}

impl MmapWriter {
    /// Create a new memory-mapped file for writing
    #[instrument(skip(path))]
    pub fn create<P: AsRef<Path>>(path: P, size: usize) -> ZeroCopyResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        // Set the file size
        file.set_len(size as u64)?;

        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        debug!(
            "Created memory-mapped file: {:?}, size: {} bytes",
            path, size
        );

        Ok(Self { mmap, path, file })
    }

    /// Open an existing file for memory-mapped writing
    #[instrument(skip(path))]
    pub fn open<P: AsRef<Path>>(path: P) -> ZeroCopyResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().read(true).write(true).open(&path)?;

        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        debug!(
            "Opened memory-mapped file for writing: {:?}, size: {} bytes",
            path,
            mmap.len()
        );

        Ok(Self { mmap, path, file })
    }

    /// Get mutable access to the mapped bytes
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.mmap
    }

    /// Get the size of the mapped file
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if the mapped file is empty
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Write data at a specific offset
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> ZeroCopyResult<()> {
        if offset + data.len() > self.mmap.len() {
            return Err(ZeroCopyError::Buffer(format!(
                "Write bounds out of range: {}..{} > {}",
                offset,
                offset + data.len(),
                self.mmap.len()
            )));
        }

        self.mmap[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Flush changes to disk
    #[instrument(skip(self))]
    pub fn flush(&self) -> ZeroCopyResult<()> {
        self.mmap.flush()?;
        Ok(())
    }

    /// Flush changes to disk asynchronously
    #[instrument(skip(self))]
    pub fn flush_async(&self) -> ZeroCopyResult<()> {
        self.mmap.flush_async()?;
        Ok(())
    }

    /// Advise the kernel about memory access patterns
    pub fn advise(&self, advice: Advice) -> ZeroCopyResult<()> {
        self.mmap.advise(advice)?;
        Ok(())
    }

    /// Get the path of the mapped file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resize the mapped file
    #[instrument(skip(self))]
    pub fn resize(&mut self, new_size: usize) -> ZeroCopyResult<()> {
        // Resize the underlying file
        self.file.set_len(new_size as u64)?;

        // Create a new mapping (old mapping will be dropped automatically)
        self.mmap = unsafe { MmapOptions::new().map_mut(&self.file)? };

        debug!(
            "Resized memory-mapped file: {:?}, new size: {} bytes",
            self.path, new_size
        );

        Ok(())
    }
}

/// Thread-safe memory-mapped file with read-write locking
pub struct ThreadSafeMmap {
    mmap: Arc<RwLock<MmapMut>>,
    path: PathBuf,
}

impl ThreadSafeMmap {
    /// Create a new thread-safe memory-mapped file
    #[instrument(skip(path))]
    pub fn create<P: AsRef<Path>>(path: P, size: usize) -> ZeroCopyResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        file.set_len(size as u64)?;
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        Ok(Self {
            mmap: Arc::new(RwLock::new(mmap)),
            path,
        })
    }

    /// Get a read lock on the mapped data
    pub fn read(&self) -> ThreadSafeMmapReadGuard {
        ThreadSafeMmapReadGuard {
            guard: self.mmap.read(),
        }
    }

    /// Get a write lock on the mapped data
    pub fn write(&self) -> ThreadSafeMmapWriteGuard {
        ThreadSafeMmapWriteGuard {
            guard: self.mmap.write(),
        }
    }

    /// Get the path of the mapped file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Read guard for thread-safe memory-mapped file
pub struct ThreadSafeMmapReadGuard<'a> {
    guard: RwLockReadGuard<'a, MmapMut>,
}

impl<'a> ThreadSafeMmapReadGuard<'a> {
    /// Get the mapped bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.guard
    }

    /// Access archived data
    pub fn access_archived<T>(&self) -> ZeroCopyResult<&T::Archived>
    where
        T: Archive,
        T::Archived: for<'b> bytecheck::CheckBytes<
            bytecheck::rancor::Strategy<bytecheck::DefaultValidator, rkyv::rancor::Failure>,
        >,
    {
        access::<T::Archived, rkyv::rancor::Failure>(&self.guard).map_err(|_e| {
            ZeroCopyError::ArchiveAccess("Failed to access archived data".to_string())
        })
    }
}

/// Write guard for thread-safe memory-mapped file
pub struct ThreadSafeMmapWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, MmapMut>,
}

impl<'a> ThreadSafeMmapWriteGuard<'a> {
    /// Get mutable access to the mapped bytes
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.guard
    }

    /// Write data at a specific offset
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> ZeroCopyResult<()> {
        if offset + data.len() > self.guard.len() {
            return Err(ZeroCopyError::Buffer(format!(
                "Write bounds out of range: {}..{} > {}",
                offset,
                offset + data.len(),
                self.guard.len()
            )));
        }

        self.guard[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Flush changes to disk
    pub fn flush(&self) -> ZeroCopyResult<()> {
        self.guard.flush()?;
        Ok(())
    }
}

/// Memory-mapped circular buffer for streaming data
pub struct MmapCircularBuffer {
    mmap: MmapMut,
    head: Arc<parking_lot::Mutex<usize>>,
    tail: Arc<parking_lot::Mutex<usize>>,
    capacity: usize,
    path: PathBuf,
}

impl MmapCircularBuffer {
    /// Create a new memory-mapped circular buffer
    #[instrument(skip(path))]
    pub fn create<P: AsRef<Path>>(path: P, capacity: usize) -> ZeroCopyResult<Self> {
        let path = path.as_ref().to_path_buf();

        // Add space for metadata (head and tail pointers)
        let total_size = capacity + std::mem::size_of::<usize>() * 2;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        file.set_len(total_size as u64)?;
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        Ok(Self {
            mmap,
            head: Arc::new(parking_lot::Mutex::new(0)),
            tail: Arc::new(parking_lot::Mutex::new(0)),
            capacity,
            path,
        })
    }

    /// Write data to the circular buffer
    pub fn write(&mut self, data: &[u8]) -> ZeroCopyResult<usize> {
        let mut head = self.head.lock();
        let tail = *self.tail.lock();

        let available = if *head >= tail {
            self.capacity - *head + tail
        } else {
            tail - *head
        };

        if available < data.len() {
            return Err(ZeroCopyError::Buffer(
                "Insufficient space in circular buffer".to_string(),
            ));
        }

        let metadata_offset = std::mem::size_of::<usize>() * 2;
        let write_size = data.len().min(available);

        // Handle wrap-around
        if *head + write_size <= self.capacity {
            // No wrap-around needed
            self.mmap[metadata_offset + *head..metadata_offset + *head + write_size]
                .copy_from_slice(&data[..write_size]);
        } else {
            // Split write due to wrap-around
            let first_part = self.capacity - *head;
            self.mmap[metadata_offset + *head..metadata_offset + self.capacity]
                .copy_from_slice(&data[..first_part]);
            self.mmap[metadata_offset..metadata_offset + write_size - first_part]
                .copy_from_slice(&data[first_part..write_size]);
        }

        *head = (*head + write_size) % self.capacity;
        Ok(write_size)
    }

    /// Read data from the circular buffer
    pub fn read(&mut self, buffer: &mut [u8]) -> ZeroCopyResult<usize> {
        let head = *self.head.lock();
        let mut tail = self.tail.lock();

        let available = if head >= *tail {
            head - *tail
        } else {
            self.capacity - *tail + head
        };

        if available == 0 {
            return Ok(0);
        }

        let metadata_offset = std::mem::size_of::<usize>() * 2;
        let read_size = buffer.len().min(available);

        // Handle wrap-around
        if *tail + read_size <= self.capacity {
            // No wrap-around needed
            buffer[..read_size].copy_from_slice(
                &self.mmap[metadata_offset + *tail..metadata_offset + *tail + read_size],
            );
        } else {
            // Split read due to wrap-around
            let first_part = self.capacity - *tail;
            buffer[..first_part].copy_from_slice(
                &self.mmap[metadata_offset + *tail..metadata_offset + self.capacity],
            );
            buffer[first_part..read_size].copy_from_slice(
                &self.mmap[metadata_offset..metadata_offset + read_size - first_part],
            );
        }

        *tail = (*tail + read_size) % self.capacity;
        Ok(read_size)
    }

    /// Get the amount of data available to read
    pub fn available(&self) -> usize {
        let head = *self.head.lock();
        let tail = *self.tail.lock();

        if head >= tail {
            head - tail
        } else {
            self.capacity - tail + head
        }
    }

    /// Get the amount of space available for writing
    pub fn space_available(&self) -> usize {
        self.capacity - self.available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rkyv::{Archive, Deserialize, Serialize};
    use tempfile::NamedTempFile;

    #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_mmap_reader() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write test data
        let data = TestData {
            id: 42,
            name: "test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&data).unwrap();
        std::fs::write(path, &bytes).unwrap();

        // Test memory-mapped reading
        let reader = MmapReader::open(path).unwrap();
        assert_eq!(reader.len(), bytes.len());
        assert!(!reader.is_empty());

        let archived = reader.access_archived::<TestData>().unwrap();
        assert_eq!(archived.id, 42);
        assert_eq!(archived.name, "test");
    }

    #[test]
    fn test_mmap_writer() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let test_data = b"Hello, memory-mapped world!";

        // Create and write to memory-mapped file
        let mut writer = MmapWriter::create(path, test_data.len()).unwrap();
        writer.write_at(0, test_data).unwrap();
        writer.flush().unwrap();

        // Verify the data was written
        let content = std::fs::read(path).unwrap();
        assert_eq!(content, test_data);
    }

    #[test]
    fn test_thread_safe_mmap() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mmap = ThreadSafeMmap::create(path, 1024).unwrap();

        // Test concurrent access
        {
            let mut write_guard = mmap.write();
            let test_data = b"thread-safe test";
            write_guard.write_at(0, test_data).unwrap();
            write_guard.flush().unwrap();
        }

        {
            let read_guard = mmap.read();
            let bytes = read_guard.as_bytes();
            assert_eq!(&bytes[..16], b"thread-safe test");
        }
    }

    #[test]
    fn test_mmap_circular_buffer() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut buffer = MmapCircularBuffer::create(path, 10).unwrap();

        // Test writing and reading
        let write_data = b"hello";
        let written = buffer.write(write_data).unwrap();
        assert_eq!(written, 5);

        let mut read_buffer = [0u8; 10];
        let read = buffer.read(&mut read_buffer).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&read_buffer[..5], b"hello");

        // Test wrap-around
        let write_data2 = b"worldtest";
        let written2 = buffer.write(write_data2).unwrap();
        assert_eq!(written2, 9);

        let read2 = buffer.read(&mut read_buffer).unwrap();
        assert_eq!(read2, 9);
        assert_eq!(&read_buffer[..9], b"worldtest");
    }
}
