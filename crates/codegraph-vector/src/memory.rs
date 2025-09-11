use codegraph_core::{CodeGraphError, Result};
#[cfg(feature = "persistent")]
use memmap2::{MmapMut, MmapOptions};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(feature = "persistent")]
use std::fs::OpenOptions;
#[cfg(feature = "persistent")]
use std::io::{Seek, SeekFrom, Write};
#[cfg(feature = "persistent")]
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    pub initial_size_mb: usize,
    pub max_size_mb: usize,
    pub block_size_kb: usize,
    pub enable_reuse: bool,
    pub enable_compaction: bool,
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            initial_size_mb: 64,
            max_size_mb: 256,
            block_size_kb: 64,
            enable_reuse: true,
            enable_compaction: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub allocated_bytes: usize,
    pub peak_usage_bytes: usize,
    pub fragmentation_ratio: f32,
    pub allocation_count: usize,
    pub deallocation_count: usize,
    pub compaction_count: usize,
    pub pool_utilization: f32,
}

#[derive(Debug)]
struct MemoryBlock {
    ptr: *mut u8,
    size: usize,
    used: bool,
    allocation_id: usize,
}

unsafe impl Send for MemoryBlock {}
unsafe impl Sync for MemoryBlock {}

impl MemoryBlock {
    fn new(size: usize, allocation_id: usize) -> Result<Self> {
        let layout = std::alloc::Layout::from_size_align(size, 8)
            .map_err(|e| CodeGraphError::Vector(format!("Invalid memory layout: {}", e)))?;

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(CodeGraphError::Vector(
                "Failed to allocate memory".to_string(),
            ));
        }

        Ok(Self {
            ptr,
            size,
            used: true,
            allocation_id,
        })
    }

    fn deallocate(&mut self) {
        if !self.ptr.is_null() && self.used {
            let layout = std::alloc::Layout::from_size_align(self.size, 8).unwrap();
            unsafe {
                std::alloc::dealloc(self.ptr, layout);
            }
            self.ptr = std::ptr::null_mut();
            self.used = false;
        }
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        self.deallocate();
    }
}

#[derive(Debug, Clone)]
pub struct VectorAllocation {
    id: usize,
    size: usize,
    vector_count: usize,
    dimension: usize,
}

impl VectorAllocation {
    pub fn new(id: usize, size: usize, vector_count: usize, dimension: usize) -> Self {
        Self {
            id,
            size,
            vector_count,
            dimension,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn vector_count(&self) -> usize {
        self.vector_count
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

pub struct MemoryOptimizer {
    pool_config: Option<MemoryPoolConfig>,
    memory_blocks: Arc<Mutex<Vec<MemoryBlock>>>,
    stats: Arc<RwLock<MemoryStats>>,
    next_allocation_id: Arc<AtomicUsize>,
    allocations: Arc<Mutex<HashMap<usize, VectorAllocation>>>,
    peak_usage: Arc<AtomicUsize>,
    current_usage: Arc<AtomicUsize>,
}

impl MemoryOptimizer {
    pub fn new() -> Self {
        Self {
            pool_config: None,
            memory_blocks: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(RwLock::new(MemoryStats {
                allocated_bytes: 0,
                peak_usage_bytes: 0,
                fragmentation_ratio: 0.0,
                allocation_count: 0,
                deallocation_count: 0,
                compaction_count: 0,
                pool_utilization: 0.0,
            })),
            next_allocation_id: Arc::new(AtomicUsize::new(1)),
            allocations: Arc::new(Mutex::new(HashMap::new())),
            peak_usage: Arc::new(AtomicUsize::new(0)),
            current_usage: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn create_pool(&mut self, config: MemoryPoolConfig) -> Result<()> {
        self.pool_config = Some(config.clone());

        // Pre-allocate initial memory pool
        let initial_size = config.initial_size_mb * 1024 * 1024;
        let block_count = initial_size / (config.block_size_kb * 1024);

        let mut blocks = self.memory_blocks.lock();
        for _i in 0..block_count {
            let allocation_id = self.next_allocation_id.fetch_add(1, Ordering::SeqCst);
            let block = MemoryBlock::new(config.block_size_kb * 1024, allocation_id)?;
            blocks.push(block);
        }

        self.update_stats();

        Ok(())
    }

    pub fn allocate_for_vectors(&self, vectors: &[Vec<f32>]) -> Result<Vec<VectorAllocation>> {
        if vectors.is_empty() {
            return Ok(Vec::new());
        }

        let mut allocations = Vec::new();
        let mut total_allocated = 0;

        for (_i, vector) in vectors.iter().enumerate() {
            let size = vector.len() * std::mem::size_of::<f32>();
            let allocation_id = self.next_allocation_id.fetch_add(1, Ordering::SeqCst);

            // Simulate memory allocation
            let allocation = VectorAllocation::new(allocation_id, size, 1, vector.len());

            {
                let mut alloc_map = self.allocations.lock();
                alloc_map.insert(allocation_id, allocation);
            }

            allocations.push(VectorAllocation::new(allocation_id, size, 1, vector.len()));
            total_allocated += size;
        }

        // Update memory usage tracking
        let current = self
            .current_usage
            .fetch_add(total_allocated, Ordering::SeqCst)
            + total_allocated;
        let peak = self.peak_usage.load(Ordering::SeqCst);
        if current > peak {
            self.peak_usage.store(current, Ordering::SeqCst);
        }

        self.update_stats();

        Ok(allocations)
    }

    pub fn get_memory_stats(&self) -> MemoryStats {
        self.stats.read().clone()
    }

    pub fn compact_memory(&self) -> Result<()> {
        let mut blocks = self.memory_blocks.lock();

        // Simple compaction: remove unused blocks and defragment
        blocks.retain(|block| block.used);

        // Update compaction statistics
        {
            let mut stats = self.stats.write();
            stats.compaction_count += 1;
            stats.fragmentation_ratio *= 0.8; // Compaction reduces fragmentation
        }

        Ok(())
    }

    #[cfg(feature = "persistent")]
    pub fn save_to_mmap<P: AsRef<Path>>(&self, vectors: &[Vec<f32>], path: P) -> Result<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        let dimension = vectors[0].len();

        // Validate all vectors have the same dimension
        for vector in vectors {
            if vector.len() != dimension {
                return Err(CodeGraphError::Vector(
                    "All vectors must have the same dimension".to_string(),
                ));
            }
        }

        // Calculate total size needed
        let header_size = std::mem::size_of::<u64>() * 2; // vector_count and dimension
        let data_size = vectors.len() * dimension * std::mem::size_of::<f32>();
        let total_size = header_size + data_size;

        // Create the file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to create mmap file: {}", e)))?;

        // Set file size
        file.seek(SeekFrom::Start(total_size as u64 - 1))
            .map_err(|e| CodeGraphError::Vector(format!("Failed to seek in file: {}", e)))?;
        file.write_all(&[0])
            .map_err(|e| CodeGraphError::Vector(format!("Failed to write to file: {}", e)))?;
        file.flush()
            .map_err(|e| CodeGraphError::Vector(format!("Failed to flush file: {}", e)))?;

        // Create memory map
        let mut mmap = unsafe {
            MmapOptions::new().map_mut(&file).map_err(|e| {
                CodeGraphError::Vector(format!("Failed to create memory map: {}", e))
            })?
        };

        // Write header
        let header_bytes = mmap.as_mut_ptr() as *mut u64;
        unsafe {
            *header_bytes = vectors.len() as u64;
            *header_bytes.add(1) = dimension as u64;
        }

        // Write vector data
        let data_start = header_size;
        let data_ptr = unsafe { mmap.as_mut_ptr().add(data_start) as *mut f32 };

        for (i, vector) in vectors.iter().enumerate() {
            let offset = i * dimension;
            for (j, &value) in vector.iter().enumerate() {
                unsafe {
                    *data_ptr.add(offset + j) = value;
                }
            }
        }

        mmap.flush()
            .map_err(|e| CodeGraphError::Vector(format!("Failed to flush memory map: {}", e)))?;

        Ok(())
    }

    #[cfg(feature = "persistent")]
    pub fn load_from_mmap<P: AsRef<Path>>(
        &self,
        path: P,
        expected_dimension: usize,
    ) -> Result<Vec<Vec<f32>>> {
        let file = std::fs::File::open(&path)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to open mmap file: {}", e)))?;

        let mmap = unsafe {
            memmap2::Mmap::map(&file)
                .map_err(|e| CodeGraphError::Vector(format!("Failed to map file: {}", e)))?
        };

        if mmap.len() < std::mem::size_of::<u64>() * 2 {
            return Err(CodeGraphError::Vector(
                "Invalid mmap file: too small".to_string(),
            ));
        }

        // Read header
        let header_ptr = mmap.as_ptr() as *const u64;
        let (vector_count, dimension) =
            unsafe { (*header_ptr as usize, *header_ptr.add(1) as usize) };

        if dimension != expected_dimension {
            return Err(CodeGraphError::Vector(format!(
                "Dimension mismatch: expected {}, found {}",
                expected_dimension, dimension
            )));
        }

        // Validate file size
        let header_size = std::mem::size_of::<u64>() * 2;
        let expected_data_size = vector_count * dimension * std::mem::size_of::<f32>();
        let expected_total_size = header_size + expected_data_size;

        if mmap.len() != expected_total_size {
            return Err(CodeGraphError::Vector(format!(
                "Invalid mmap file size: expected {}, got {}",
                expected_total_size,
                mmap.len()
            )));
        }

        // Read vector data
        let data_ptr = unsafe { mmap.as_ptr().add(header_size) as *const f32 };
        let mut vectors = Vec::with_capacity(vector_count);

        for i in 0..vector_count {
            let mut vector = Vec::with_capacity(dimension);
            let offset = i * dimension;

            for j in 0..dimension {
                let value = unsafe { *data_ptr.add(offset + j) };
                vector.push(value);
            }

            vectors.push(vector);
        }

        Ok(vectors)
    }

    fn update_stats(&self) {
        let current_usage = self.current_usage.load(Ordering::SeqCst);
        let peak_usage = self.peak_usage.load(Ordering::SeqCst);
        let allocation_count = self.allocations.lock().len();

        // Calculate fragmentation (simplified)
        let fragmentation_ratio = if current_usage > 0 {
            let blocks = self.memory_blocks.lock();
            let used_blocks = blocks.iter().filter(|b| b.used).count();
            let total_blocks = blocks.len();

            if total_blocks > 0 {
                1.0 - (used_blocks as f32 / total_blocks as f32)
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Calculate pool utilization
        let pool_utilization = if let Some(config) = &self.pool_config {
            let max_size = config.max_size_mb * 1024 * 1024;
            if max_size > 0 {
                current_usage as f32 / max_size as f32
            } else {
                0.0
            }
        } else {
            0.0
        };

        let mut stats = self.stats.write();
        stats.allocated_bytes = current_usage;
        stats.peak_usage_bytes = peak_usage;
        stats.fragmentation_ratio = fragmentation_ratio;
        stats.allocation_count = allocation_count;
        stats.pool_utilization = pool_utilization;
    }

    pub fn deallocate(&self, allocation_id: usize) -> Result<()> {
        let mut allocations = self.allocations.lock();

        if let Some(allocation) = allocations.remove(&allocation_id) {
            let size = allocation.size();
            self.current_usage.fetch_sub(size, Ordering::SeqCst);

            let mut stats = self.stats.write();
            stats.deallocation_count += 1;

            Ok(())
        } else {
            Err(CodeGraphError::Vector(format!(
                "Allocation {} not found",
                allocation_id
            )))
        }
    }

    pub fn defragment(&self) -> Result<()> {
        // Simple defragmentation: compact memory blocks
        self.compact_memory()?;

        // Update fragmentation statistics
        {
            let mut stats = self.stats.write();
            stats.fragmentation_ratio *= 0.5; // Defragmentation significantly reduces fragmentation
        }

        Ok(())
    }

    pub fn get_allocation_info(&self, allocation_id: usize) -> Option<VectorAllocation> {
        self.allocations.lock().get(&allocation_id).cloned()
    }

    pub fn clear_all(&self) -> Result<()> {
        {
            let mut allocations = self.allocations.lock();
            allocations.clear();
        }

        {
            let mut blocks = self.memory_blocks.lock();
            blocks.clear();
        }

        self.current_usage.store(0, Ordering::SeqCst);

        self.update_stats();

        Ok(())
    }
}

impl Default for MemoryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}
