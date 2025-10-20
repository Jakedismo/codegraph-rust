//! Shared memory optimization patterns
//!
//! This module provides inter-process shared memory patterns for zero-copy
//! data sharing between processes.

use crate::{ZeroCopyError, ZeroCopyResult};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::{Mutex, RwLock};
use rkyv::api::high::HighValidator;
use rkyv::{access, access_unchecked, Archive};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    marker::PhantomData,
    path::Path,
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};
use tracing::{debug, error, instrument, trace, warn};

/// Shared memory segment for inter-process communication
pub struct SharedMemorySegment {
    mmap: MmapMut,
    size: usize,
    name: String,
    header: *mut SharedMemoryHeader,
}

/// Header structure for shared memory segments
#[repr(C, align(64))]
struct SharedMemoryHeader {
    magic: u64,
    version: u32,
    size: u32,
    readers: AtomicUsize,
    writers: AtomicUsize,
    generation: AtomicU64,
    write_lock: AtomicBool,
    data_offset: u32,
    data_size: u32,
    checksum: u64,
}

const SHARED_MEMORY_MAGIC: u64 = 0x5A45524F434F5059; // "ZEROCOPY"
const SHARED_MEMORY_VERSION: u32 = 1;
const HEADER_SIZE: usize = std::mem::size_of::<SharedMemoryHeader>();

impl SharedMemorySegment {
    /// Create a new shared memory segment
    #[instrument(skip(name))]
    pub fn create<P: AsRef<Path>>(name: P, size: usize) -> ZeroCopyResult<Self> {
        let name = name.as_ref().to_string_lossy().to_string();
        let total_size = size + HEADER_SIZE;

        // Create backing file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&name)?;

        file.set_len(total_size as u64)?;

        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        // Initialize header
        let header = mmap.as_mut_ptr() as *mut SharedMemoryHeader;
        unsafe {
            (*header) = SharedMemoryHeader {
                magic: SHARED_MEMORY_MAGIC,
                version: SHARED_MEMORY_VERSION,
                size: total_size as u32,
                readers: AtomicUsize::new(0),
                writers: AtomicUsize::new(0),
                generation: AtomicU64::new(0),
                write_lock: AtomicBool::new(false),
                data_offset: HEADER_SIZE as u32,
                data_size: size as u32,
                checksum: 0,
            };
        }

        debug!(
            "Created shared memory segment: {}, size: {} bytes",
            name, total_size
        );

        Ok(Self {
            mmap,
            size: total_size,
            name,
            header,
        })
    }

    /// Open an existing shared memory segment
    #[instrument(skip(name))]
    pub fn open<P: AsRef<Path>>(name: P) -> ZeroCopyResult<Self> {
        let name = name.as_ref().to_string_lossy().to_string();

        let file = File::open(&name)?;
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        let header = mmap.as_ptr() as *mut SharedMemoryHeader;

        // Validate header
        unsafe {
            if (*header).magic != SHARED_MEMORY_MAGIC {
                return Err(ZeroCopyError::SharedMemory(
                    "Invalid shared memory magic number".to_string(),
                ));
            }

            if (*header).version != SHARED_MEMORY_VERSION {
                return Err(ZeroCopyError::SharedMemory(format!(
                    "Unsupported shared memory version: {}",
                    (*header).version
                )));
            }
        }

        let size = unsafe { (*header).size as usize };

        debug!(
            "Opened shared memory segment: {}, size: {} bytes",
            name, size
        );

        Ok(Self {
            mmap,
            size,
            name,
            header,
        })
    }

    /// Get a reader handle for this shared memory segment
    pub fn reader(&self) -> SharedMemoryReader {
        unsafe {
            (*self.header).readers.fetch_add(1, Ordering::AcqRel);
        }

        SharedMemoryReader {
            segment: self,
            _phantom: PhantomData,
        }
    }

    /// Get a writer handle for this shared memory segment
    pub fn writer(&self) -> ZeroCopyResult<SharedMemoryWriter> {
        // Try to acquire write lock
        unsafe {
            if (*self.header)
                .write_lock
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
            {
                return Err(ZeroCopyError::SharedMemory(
                    "Shared memory segment is already locked for writing".to_string(),
                ));
            }

            (*self.header).writers.fetch_add(1, Ordering::AcqRel);
        }

        Ok(SharedMemoryWriter {
            segment: self,
            _phantom: PhantomData,
        })
    }

    /// Get the data section of the shared memory
    fn data_section(&self) -> &[u8] {
        let data_offset = unsafe { (*self.header).data_offset as usize };
        let data_size = unsafe { (*self.header).data_size as usize };

        &self.mmap[data_offset..data_offset + data_size]
    }

    /// Get mutable access to the data section
    fn data_section_mut(&mut self) -> &mut [u8] {
        let data_offset = unsafe { (*self.header).data_offset as usize };
        let data_size = unsafe { (*self.header).data_size as usize };

        &mut self.mmap[data_offset..data_offset + data_size]
    }

    /// Get the current generation number
    pub fn generation(&self) -> u64 {
        unsafe { (*self.header).generation.load(Ordering::Acquire) }
    }

    /// Get segment statistics
    pub fn stats(&self) -> SharedMemoryStats {
        unsafe {
            SharedMemoryStats {
                name: self.name.clone(),
                total_size: self.size,
                data_size: (*self.header).data_size as usize,
                readers: (*self.header).readers.load(Ordering::Acquire),
                writers: (*self.header).writers.load(Ordering::Acquire),
                generation: (*self.header).generation.load(Ordering::Acquire),
                is_write_locked: (*self.header).write_lock.load(Ordering::Acquire),
            }
        }
    }
}

/// Statistics for shared memory segment
#[derive(Debug, Clone)]
pub struct SharedMemoryStats {
    pub name: String,
    pub total_size: usize,
    pub data_size: usize,
    pub readers: usize,
    pub writers: usize,
    pub generation: u64,
    pub is_write_locked: bool,
}

/// Reader handle for shared memory segment
pub struct SharedMemoryReader<'a> {
    segment: &'a SharedMemorySegment,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> SharedMemoryReader<'a> {
    /// Read the data as raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.segment.data_section()
    }

    /// Access archived data directly from shared memory
    #[instrument(skip(self))]
    pub fn access_archived<T>(&self) -> ZeroCopyResult<&T::Archived>
    where
        T: Archive,
        T::Archived: for<'b> bytecheck::CheckBytes<HighValidator<'b, rkyv::rancor::Failure>>,
    {
        let data = self.segment.data_section();
        access::<T::Archived, rkyv::rancor::Failure>(data).map_err(|e| {
            ZeroCopyError::ArchiveAccess(format!("Failed to access archived data: {:?}", e))
        })
    }

    /// Access archived data without validation (faster but unsafe)
    #[instrument(skip(self))]
    pub fn access_archived_unchecked<T>(&self) -> &T::Archived
    where
        T: Archive,
        T::Archived: rkyv::Portable,
    {
        let data = self.segment.data_section();
        unsafe { access_unchecked::<T::Archived>(data) }
    }

    /// Get the generation when this data was written
    pub fn generation(&self) -> u64 {
        self.segment.generation()
    }

    /// Check if the data has been updated since the given generation
    pub fn is_updated_since(&self, generation: u64) -> bool {
        self.segment.generation() > generation
    }
}

impl<'a> Drop for SharedMemoryReader<'a> {
    fn drop(&mut self) {
        unsafe {
            (*self.segment.header)
                .readers
                .fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Writer handle for shared memory segment
pub struct SharedMemoryWriter<'a> {
    segment: &'a SharedMemorySegment,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> SharedMemoryWriter<'a> {
    /// Write data to the shared memory segment
    #[instrument(skip(self, data))]
    pub fn write(&mut self, data: &[u8]) -> ZeroCopyResult<()> {
        let data_section = unsafe {
            let segment_ptr = self.segment as *const _ as *mut SharedMemorySegment;
            (*segment_ptr).data_section_mut()
        };

        if data.len() > data_section.len() {
            return Err(ZeroCopyError::SharedMemory(format!(
                "Data size {} exceeds segment capacity {}",
                data.len(),
                data_section.len()
            )));
        }

        // Copy data
        data_section[..data.len()].copy_from_slice(data);

        // Update generation
        unsafe {
            (*self.segment.header)
                .generation
                .fetch_add(1, Ordering::Release);
        }

        trace!("Wrote {} bytes to shared memory segment", data.len());
        Ok(())
    }

    /// Write serialized data to shared memory
    #[instrument(skip(self, data))]
    pub fn write_serialized(&mut self, data: &[u8]) -> ZeroCopyResult<()> {
        self.write(data)
    }

    /// Get mutable access to the raw data section
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            let segment_ptr = self.segment as *const _ as *mut SharedMemorySegment;
            (*segment_ptr).data_section_mut()
        }
    }

    /// Flush any pending writes (no-op for memory-mapped files)
    pub fn flush(&self) -> ZeroCopyResult<()> {
        // Memory-mapped files are automatically flushed by the OS
        Ok(())
    }
}

impl<'a> Drop for SharedMemoryWriter<'a> {
    fn drop(&mut self) {
        unsafe {
            (*self.segment.header)
                .writers
                .fetch_sub(1, Ordering::AcqRel);
            (*self.segment.header)
                .write_lock
                .store(false, Ordering::Release);
        }
    }
}

/// Manager for multiple shared memory segments
pub struct SharedMemoryManager {
    segments: RwLock<std::collections::HashMap<String, Arc<SharedMemorySegment>>>,
    base_path: std::path::PathBuf,
}

impl SharedMemoryManager {
    /// Create a new shared memory manager
    pub fn new<P: AsRef<Path>>(base_path: P) -> ZeroCopyResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path)?;

        Ok(Self {
            segments: RwLock::new(std::collections::HashMap::new()),
            base_path,
        })
    }

    /// Create or get a shared memory segment
    #[instrument(skip(self, name))]
    pub fn get_or_create(
        &self,
        name: &str,
        size: usize,
    ) -> ZeroCopyResult<Arc<SharedMemorySegment>> {
        let segment_path = self.base_path.join(format!("{}.shm", name));

        // Check if segment already exists in manager
        {
            let segments = self.segments.read();
            if let Some(segment) = segments.get(name) {
                return Ok(segment.clone());
            }
        }

        // Try to open existing segment, otherwise create new one
        let segment = if segment_path.exists() {
            SharedMemorySegment::open(&segment_path)?
        } else {
            SharedMemorySegment::create(&segment_path, size)?
        };

        let segment = Arc::new(segment);

        // Store in manager
        {
            let mut segments = self.segments.write();
            segments.insert(name.to_string(), segment.clone());
        }

        debug!("Added shared memory segment to manager: {}", name);
        Ok(segment)
    }

    /// Remove a shared memory segment
    #[instrument(skip(self, name))]
    pub fn remove(&self, name: &str) -> ZeroCopyResult<()> {
        let segment_path = self.base_path.join(format!("{}.shm", name));

        // Remove from manager
        {
            let mut segments = self.segments.write();
            segments.remove(name);
        }

        // Remove backing file
        if segment_path.exists() {
            std::fs::remove_file(&segment_path)?;
        }

        debug!("Removed shared memory segment: {}", name);
        Ok(())
    }

    /// List all managed segments
    pub fn list_segments(&self) -> Vec<String> {
        let segments = self.segments.read();
        segments.keys().cloned().collect()
    }

    /// Get statistics for all segments
    pub fn all_stats(&self) -> Vec<SharedMemoryStats> {
        let segments = self.segments.read();
        segments.values().map(|s| s.stats()).collect()
    }

    /// Cleanup unused segments
    #[instrument(skip(self))]
    pub fn cleanup_unused(&self) -> ZeroCopyResult<usize> {
        let mut removed_count = 0;
        let segment_names: Vec<String> = {
            let segments = self.segments.read();
            segments.keys().cloned().collect()
        };

        for name in segment_names {
            let should_remove = {
                let segments = self.segments.read();
                if let Some(segment) = segments.get(&name) {
                    let stats = segment.stats();
                    stats.readers == 0 && stats.writers == 0
                } else {
                    false
                }
            };

            if should_remove {
                self.remove(&name)?;
                removed_count += 1;
            }
        }

        debug!("Cleaned up {} unused shared memory segments", removed_count);
        Ok(removed_count)
    }
}

/// Cross-process lock using shared memory
pub struct SharedMemoryLock {
    segment: Arc<SharedMemorySegment>,
    lock_offset: usize,
}

impl SharedMemoryLock {
    /// Create a new shared memory lock
    pub fn new(segment: Arc<SharedMemorySegment>, lock_offset: usize) -> Self {
        Self {
            segment,
            lock_offset,
        }
    }

    /// Try to acquire the lock (non-blocking)
    pub fn try_lock(&self) -> bool {
        let data = self.segment.data_section();
        if self.lock_offset + std::mem::size_of::<AtomicBool>() > data.len() {
            return false;
        }

        let lock_ptr = unsafe { data.as_ptr().add(self.lock_offset) as *const AtomicBool };

        unsafe {
            (*lock_ptr)
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        }
    }

    /// Release the lock
    pub fn unlock(&self) {
        let data = self.segment.data_section();
        if self.lock_offset + std::mem::size_of::<AtomicBool>() > data.len() {
            return;
        }

        let lock_ptr = unsafe { data.as_ptr().add(self.lock_offset) as *const AtomicBool };

        unsafe {
            (*lock_ptr).store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rkyv::{Archive, Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_shared_memory_segment() {
        let temp_dir = TempDir::new().unwrap();
        let segment_path = temp_dir.path().join("test_segment.shm");

        // Create segment
        let segment = SharedMemorySegment::create(&segment_path, 4096).unwrap();
        let stats = segment.stats();
        assert_eq!(stats.data_size, 4096);

        // Test reader
        let reader = segment.reader();
        let data = reader.as_bytes();
        assert_eq!(data.len(), 4096);

        // Test writer
        let mut writer = segment.writer().unwrap();
        let test_data = b"Hello, shared memory!";
        writer.write(test_data).unwrap();

        // Verify data was written
        let reader2 = segment.reader();
        let read_data = reader2.as_bytes();
        assert_eq!(&read_data[..test_data.len()], test_data);
    }

    #[test]
    fn test_shared_memory_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SharedMemoryManager::new(temp_dir.path()).unwrap();

        // Create segments
        let segment1 = manager.get_or_create("test1", 1024).unwrap();
        let segment2 = manager.get_or_create("test2", 2048).unwrap();

        assert_eq!(segment1.stats().data_size, 1024);
        assert_eq!(segment2.stats().data_size, 2048);

        // List segments
        let segments = manager.list_segments();
        assert_eq!(segments.len(), 2);
        assert!(segments.contains(&"test1".to_string()));
        assert!(segments.contains(&"test2".to_string()));

        // Get existing segment
        let segment1_again = manager.get_or_create("test1", 1024).unwrap();
        assert_eq!(segment1.stats().data_size, segment1_again.stats().data_size);
    }

    #[test]
    fn test_shared_memory_archived_data() {
        let temp_dir = TempDir::new().unwrap();
        let segment_path = temp_dir.path().join("archived_test.shm");

        let data = TestData {
            id: 42,
            name: "shared test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };

        // Serialize data
        let bytes = rkyv::to_bytes::<rkyv::rancor::Failure>(&data).unwrap();

        // Create segment and write data
        let segment = SharedMemorySegment::create(&segment_path, bytes.len() + 1024).unwrap();
        {
            let mut writer = segment.writer().unwrap();
            writer.write(&bytes).unwrap();
        }

        // Read archived data
        let reader = segment.reader();
        let archived = reader.access_archived::<TestData>().unwrap();

        assert_eq!(archived.id, 42);
        assert_eq!(archived.name, "shared test");
        assert_eq!(archived.values.len(), 5);
    }

    #[test]
    fn test_shared_memory_lock() {
        let temp_dir = TempDir::new().unwrap();
        let segment_path = temp_dir.path().join("lock_test.shm");

        let segment = Arc::new(SharedMemorySegment::create(&segment_path, 1024).unwrap());
        let lock = SharedMemoryLock::new(segment, 0);

        // Test lock acquisition
        assert!(lock.try_lock());
        assert!(!lock.try_lock()); // Should fail second time

        // Test unlock
        lock.unlock();
        assert!(lock.try_lock()); // Should succeed after unlock
        lock.unlock();
    }
}
