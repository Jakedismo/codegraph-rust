use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::time::{SystemTime, Duration, Instant};
use std::thread;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::alloc::{GlobalAlloc, Layout, System};
use parking_lot::RwLock;
use dashmap::DashMap;
use backtrace::Backtrace;
use serde::{Serialize, Deserialize};
use tracing::{debug, warn, info, error};
use tokio::sync::mpsc;

use crate::memory::{MemoryManager, MemoryPressure, OptimizationRecommendation};
use crate::cache_optimized::PaddedAtomicUsize;

/// Global memory profiler instance
pub static MEMORY_PROFILER: once_cell::sync::Lazy<Arc<MemoryProfiler>> = 
    once_cell::sync::Lazy::new(|| Arc::new(MemoryProfiler::new()));

/// Memory allocation tracking data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationInfo {
    pub size: usize,
    pub timestamp: SystemTime,
    pub thread_id: u64,
    pub allocation_id: u64,
    pub stack_trace: Vec<String>,
    pub is_freed: bool,
    pub free_timestamp: Option<SystemTime>,
    pub alignment: usize,
    pub category: AllocationType,
}

/// Types of memory allocations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllocationType {
    Cache,
    Vector,
    Graph,
    Parser,
    String,
    Buffer,
    Index,
    Temp,
    Unknown,
}

/// Memory leak detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLeak {
    pub allocation_id: u64,
    pub size: usize,
    pub age: Duration,
    pub stack_trace: Vec<String>,
    pub category: AllocationType,
    pub estimated_impact: LeakImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeakImpact {
    Low,     // < 1MB
    Medium,  // 1-10MB
    High,    // 10-100MB
    Critical, // > 100MB
}

/// Memory usage pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePattern {
    pub category: AllocationType,
    pub peak_usage: usize,
    pub average_usage: usize,
    pub allocation_rate: f64, // allocations per second
    pub deallocation_rate: f64,
    pub fragmentation_ratio: f64,
    pub lifetime_distribution: Vec<Duration>,
}

/// Real-time memory metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub timestamp: SystemTime,
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub active_allocations: usize,
    pub fragmentation_ratio: f64,
    pub memory_pressure: MemoryPressure,
    pub categories: HashMap<AllocationType, CategoryMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMetrics {
    pub allocated: usize,
    pub freed: usize,
    pub current: usize,
    pub count: u64,
    pub average_size: usize,
    pub peak_size: usize,
}

/// Optimization recommendation from profiler analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerRecommendation {
    pub category: AllocationType,
    pub severity: RecommendationSeverity,
    pub description: String,
    pub estimated_savings: usize,
    pub implementation_difficulty: Difficulty,
    pub action: RecommendedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,    // < 1 day
    Medium,  // 1-3 days
    Hard,    // > 3 days
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendedAction {
    ReduceAllocations,
    EnableCompression,
    IncreaseBufferSize,
    OptimizeDataStructure,
    AddCaching,
    FixMemoryLeak,
    ReduceFragmentation,
}

/// Core memory profiler implementation
pub struct MemoryProfiler {
    /// Active allocations tracking
    allocations: Arc<RwLock<HashMap<u64, AllocationInfo>>>,
    
    /// Real-time metrics
    metrics: Arc<RwLock<MemoryMetrics>>,
    
    /// Historical data for pattern analysis
    history: Arc<RwLock<BTreeMap<SystemTime, MemoryMetrics>>>,
    
    /// Usage patterns by category
    patterns: Arc<RwLock<HashMap<AllocationType, UsagePattern>>>,
    
    /// Detected memory leaks
    leaks: Arc<RwLock<Vec<MemoryLeak>>>,
    
    /// Optimization recommendations
    recommendations: Arc<RwLock<Vec<ProfilerRecommendation>>>,
    
    /// Configuration
    config: ProfilerConfig,
    
    /// Atomic counters for performance
    next_allocation_id: AtomicU64,
    total_allocated: PaddedAtomicUsize,
    total_freed: PaddedAtomicUsize,
    peak_usage: PaddedAtomicUsize,
    allocation_count: PaddedAtomicUsize,
    
    /// Event channel for real-time monitoring
    event_sender: Arc<Mutex<Option<mpsc::UnboundedSender<ProfilerEvent>>>>,
    
    /// Background analysis task handle
    analysis_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Pointer mapping for accurate deallocation: ptr -> (allocation_id, size)
    ptr_map: DashMap<usize, (u64, usize)>,
}

/// Profiler configuration
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    pub enabled: bool,
    pub stack_trace_depth: usize,
    pub leak_detection_interval: Duration,
    pub history_retention: Duration,
    pub memory_limit_bytes: usize,
    pub sampling_rate: f64, // 0.0 to 1.0
    pub real_time_monitoring: bool,
    pub enable_stack_traces: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stack_trace_depth: 32,
            leak_detection_interval: Duration::from_secs(30),
            history_retention: Duration::from_secs(24 * 3600),
            memory_limit_bytes: 250 * 1024 * 1024, // 250MB target
            sampling_rate: 1.0, // Profile all allocations by default
            real_time_monitoring: true,
            enable_stack_traces: true,
        }
    }
}

/// Events for real-time monitoring
#[derive(Debug, Clone, Serialize)]
pub enum ProfilerEvent {
    Allocation {
        id: u64,
        size: usize,
        category: AllocationType,
        timestamp: SystemTime,
    },
    Deallocation {
        id: u64,
        size: usize,
        lifetime: Duration,
    },
    MemoryPressure {
        level: MemoryPressure,
        current_usage: usize,
        limit: usize,
    },
    LeakDetected {
        leak: MemoryLeak,
    },
    RecommendationGenerated {
        recommendation: ProfilerRecommendation,
    },
}

impl MemoryProfiler {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(MemoryMetrics::new())),
            history: Arc::new(RwLock::new(BTreeMap::new())),
            patterns: Arc::new(RwLock::new(HashMap::new())),
            leaks: Arc::new(RwLock::new(Vec::new())),
            recommendations: Arc::new(RwLock::new(Vec::new())),
            config: ProfilerConfig::default(),
            next_allocation_id: AtomicU64::new(1),
            total_allocated: PaddedAtomicUsize::new(0),
            total_freed: PaddedAtomicUsize::new(0),
            peak_usage: PaddedAtomicUsize::new(0),
            allocation_count: PaddedAtomicUsize::new(0),
            event_sender: Arc::new(Mutex::new(None)),
            analysis_handle: Arc::new(Mutex::new(None)),
            ptr_map: DashMap::new(),
        }
    }

    /// Initialize the profiler with configuration
    pub fn initialize(&self, config: ProfilerConfig) -> Result<(), Box<dyn std::error::Error>> {
        if !config.enabled {
            return Ok(());
        }

        info!("Initializing memory profiler with config: {:?}", config);

        // Start background analysis task
        self.start_background_analysis();

        info!("Memory profiler initialized successfully");
        Ok(())
    }

    /// Record a memory allocation
    pub fn record_allocation(
        &self, 
        ptr: *mut u8, 
        layout: Layout, 
        category: AllocationType
    ) -> u64 {
        if !self.config.enabled {
            return 0;
        }

        // Sample based on sampling rate
        if fastrand::f64() > self.config.sampling_rate {
            return 0;
        }

        let allocation_id = self.next_allocation_id.fetch_add(1, Ordering::Relaxed);
        let size = layout.size();
        let timestamp = SystemTime::now();
        
        // Update atomic counters
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
        
        // Update peak usage
        let current_usage = self.get_current_usage();
        let peak = self.peak_usage.load(Ordering::Relaxed);
        if current_usage > peak {
            self.peak_usage.store(current_usage, Ordering::Relaxed);
        }

        // Capture stack trace if enabled
        let stack_trace = if self.config.enable_stack_traces {
            self.capture_stack_trace()
        } else {
            Vec::new()
        };

        // Potentially infer category from captured stack if unknown
        let final_category = if category == AllocationType::Unknown && !stack_trace.is_empty() {
            Self::infer_category_from_stack(&stack_trace)
        } else {
            category.clone()
        };

        let allocation_info = AllocationInfo {
            size,
            timestamp,
            thread_id: self.get_thread_id(),
            allocation_id,
            stack_trace,
            is_freed: false,
            free_timestamp: None,
            alignment: layout.align(),
            category: final_category.clone(),
        };

        // Store allocation info
        {
            let mut allocations = self.allocations.write();
            allocations.insert(allocation_id, allocation_info);
        }

        // Map ptr for deallocation tracking
        if !ptr.is_null() {
            self.ptr_map.insert(ptr as usize, (allocation_id, size));
        }

        // Send real-time event
        if self.config.real_time_monitoring {
            self.send_event(ProfilerEvent::Allocation {
                id: allocation_id,
                size,
                category: final_category.clone(),
                timestamp,
            });
        }

        // Update metrics
        self.update_metrics_for_allocation(size, &final_category);

        allocation_id
    }

    /// Record a memory deallocation
    pub fn record_deallocation(&self, allocation_id: u64, size: usize) {
        if !self.config.enabled || allocation_id == 0 {
            return;
        }

        let free_timestamp = SystemTime::now();
        
        // Update atomic counters
        self.total_freed.fetch_add(size, Ordering::Relaxed);

        let lifetime = {
            let mut allocations = self.allocations.write();
            if let Some(allocation) = allocations.get_mut(&allocation_id) {
                allocation.is_freed = true;
                allocation.free_timestamp = Some(free_timestamp);
                
                free_timestamp.duration_since(allocation.timestamp)
                    .unwrap_or_default()
            } else {
                Duration::ZERO
            }
        };

        // Send real-time event
        if self.config.real_time_monitoring {
            self.send_event(ProfilerEvent::Deallocation {
                id: allocation_id,
                size,
                lifetime,
            });
        }

        // Update metrics
        self.update_metrics_for_deallocation(size);
    }

    /// Record a memory deallocation by pointer (for GlobalAlloc integration)
    pub fn record_deallocation_by_ptr(&self, ptr: *mut u8, fallback_size: usize) {
        if !self.config.enabled || ptr.is_null() {
            return;
        }

        if let Some((alloc_id, sz)) = self.ptr_map.remove(&(ptr as usize)).map(|entry| entry.1) {
            let size = sz.max(fallback_size);
            self.record_deallocation(alloc_id, size);
        } else {
            // Unknown pointer; update freed counter conservatively to avoid false leak signals
            self.total_freed.fetch_add(fallback_size, Ordering::Relaxed);
            self.update_metrics_for_deallocation(fallback_size);
        }
    }

    /// Get current memory usage
    pub fn get_current_usage(&self) -> usize {
        let allocated = self.total_allocated.load(Ordering::Relaxed);
        let freed = self.total_freed.load(Ordering::Relaxed);
        allocated.saturating_sub(freed)
    }

    /// Get comprehensive memory metrics
    pub fn get_metrics(&self) -> MemoryMetrics {
        self.metrics.read().clone()
    }

    /// Detect memory leaks
    pub fn detect_leaks(&self) -> Vec<MemoryLeak> {
        let now = SystemTime::now();
        let mut leaks = Vec::new();
        
        let allocations = self.allocations.read();
        for (_, allocation) in allocations.iter() {
            if !allocation.is_freed {
                let age = now.duration_since(allocation.timestamp)
                    .unwrap_or_default();
                
                // Consider allocation a leak if it's older than 5 minutes
                if age > Duration::from_secs(300) {
                    let impact = match allocation.size {
                        s if s < 1024 * 1024 => LeakImpact::Low,
                        s if s < 10 * 1024 * 1024 => LeakImpact::Medium,
                        s if s < 100 * 1024 * 1024 => LeakImpact::High,
                        _ => LeakImpact::Critical,
                    };

                    leaks.push(MemoryLeak {
                        allocation_id: allocation.allocation_id,
                        size: allocation.size,
                        age,
                        stack_trace: allocation.stack_trace.clone(),
                        category: allocation.category.clone(),
                        estimated_impact: impact,
                    });
                }
            }
        }

        // Update stored leaks
        {
            let mut stored_leaks = self.leaks.write();
            *stored_leaks = leaks.clone();
        }

        leaks
    }

    /// Analyze usage patterns
    pub fn analyze_patterns(&self) -> HashMap<AllocationType, UsagePattern> {
        let mut patterns = HashMap::new();
        let allocations = self.allocations.read();
        
        // Group allocations by category
        let mut category_data: HashMap<AllocationType, Vec<&AllocationInfo>> = HashMap::new();
        for allocation in allocations.values() {
            category_data.entry(allocation.category.clone())
                .or_insert_with(Vec::new)
                .push(allocation);
        }

        // Analyze each category
        for (category, allocs) in category_data {
            let mut sizes = Vec::new();
            let mut lifetimes = Vec::new();
            let mut total_size = 0;
            let mut freed_count = 0;

            for allocation in &allocs {
                sizes.push(allocation.size);
                total_size += allocation.size;
                
                if allocation.is_freed {
                    freed_count += 1;
                    if let Some(free_time) = allocation.free_timestamp {
                        let lifetime = free_time.duration_since(allocation.timestamp)
                            .unwrap_or_default();
                        lifetimes.push(lifetime);
                    }
                }
            }

            let count = allocs.len();
            let peak_usage = sizes.iter().max().copied().unwrap_or(0);
            let average_usage = if count > 0 { total_size / count } else { 0 };
            
            // Calculate rates (simplified for now)
            let allocation_rate = count as f64 / 60.0; // per second
            let deallocation_rate = freed_count as f64 / 60.0;
            
            // Simple fragmentation estimate
            let fragmentation_ratio = if count > 0 {
                1.0 - (freed_count as f64 / count as f64)
            } else {
                0.0
            };

            patterns.insert(category.clone(), UsagePattern {
                category,
                peak_usage,
                average_usage,
                allocation_rate,
                deallocation_rate,
                fragmentation_ratio,
                lifetime_distribution: lifetimes,
            });
        }

        // Update stored patterns
        {
            let mut stored_patterns = self.patterns.write();
            *stored_patterns = patterns.clone();
        }

        patterns
    }

    /// Generate optimization recommendations
    pub fn generate_recommendations(&self) -> Vec<ProfilerRecommendation> {
        let mut recommendations = Vec::new();
        let patterns = self.analyze_patterns();
        let current_usage = self.get_current_usage();
        let limit = self.config.memory_limit_bytes;

        // Check overall memory pressure
        if current_usage > (limit as f64 * 0.8) as usize {
            recommendations.push(ProfilerRecommendation {
                category: AllocationType::Unknown,
                severity: RecommendationSeverity::Critical,
                description: "Memory usage approaching limit. Consider immediate optimization.".to_string(),
                estimated_savings: current_usage.saturating_sub(limit / 2),
                implementation_difficulty: Difficulty::Medium,
                action: RecommendedAction::ReduceAllocations,
            });
        }

        // Analyze patterns for each category
        for (category, pattern) in patterns {
            // Check for high fragmentation
            if pattern.fragmentation_ratio > 0.3 {
                recommendations.push(ProfilerRecommendation {
                    category: category.clone(),
                    severity: RecommendationSeverity::Warning,
                    description: format!("High fragmentation detected in {:?} allocations", category),
                    estimated_savings: pattern.peak_usage / 4,
                    implementation_difficulty: Difficulty::Medium,
                    action: RecommendedAction::ReduceFragmentation,
                });
            }

            // Check for large average allocation sizes
            if pattern.average_usage > 1024 * 1024 { // > 1MB
                recommendations.push(ProfilerRecommendation {
                    category: category.clone(),
                    severity: RecommendationSeverity::Info,
                    description: format!("Large average allocation size in {:?}", category),
                    estimated_savings: pattern.average_usage / 2,
                    implementation_difficulty: Difficulty::Easy,
                    action: RecommendedAction::EnableCompression,
                });
            }
        }

        // Check for memory leaks
        let leaks = self.detect_leaks();
        for leak in leaks {
            if matches!(leak.estimated_impact, LeakImpact::High | LeakImpact::Critical) {
                recommendations.push(ProfilerRecommendation {
                    category: leak.category,
                    severity: RecommendationSeverity::Critical,
                    description: format!("Memory leak detected: {} bytes for {} seconds", 
                                       leak.size, leak.age.as_secs()),
                    estimated_savings: leak.size,
                    implementation_difficulty: Difficulty::Hard,
                    action: RecommendedAction::FixMemoryLeak,
                });
            }
        }

        // Update stored recommendations
        {
            let mut stored_recommendations = self.recommendations.write();
            *stored_recommendations = recommendations.clone();
        }

        recommendations
    }

    /// Start real-time monitoring
    pub fn start_monitoring(&self) -> mpsc::UnboundedReceiver<ProfilerEvent> {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        {
            let mut event_sender = self.event_sender.lock().unwrap();
            *event_sender = Some(sender);
        }

        receiver
    }

    /// Stop the profiler
    pub fn stop(&self) {
        info!("Stopping memory profiler");
        
        // Stop background analysis
        if let Some(handle) = self.analysis_handle.lock().unwrap().take() {
            handle.abort();
        }

        // Clear event sender
        {
            let mut event_sender = self.event_sender.lock().unwrap();
            *event_sender = None;
        }
    }

    // Private helper methods

    fn start_background_analysis(&self) {
        let profiler = Arc::downgrade(&MEMORY_PROFILER);
        let interval = self.config.leak_detection_interval;
        
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            loop {
                ticker.tick().await;
                
                if let Some(profiler) = profiler.upgrade() {
                    // Detect leaks
                    let leaks = profiler.detect_leaks();
                    for leak in leaks {
                        profiler.send_event(ProfilerEvent::LeakDetected { leak });
                    }

                    // Generate recommendations
                    let recommendations = profiler.generate_recommendations();
                    for recommendation in recommendations {
                        profiler.send_event(ProfilerEvent::RecommendationGenerated { recommendation });
                    }

                    // Update historical data
                    profiler.update_history();

                    // Check memory pressure
                    let current_usage = profiler.get_current_usage();
                    let limit = profiler.config.memory_limit_bytes;
                    let pressure = if current_usage < (limit as f64 * 0.7) as usize {
                        MemoryPressure::Low
                    } else if current_usage < (limit as f64 * 0.85) as usize {
                        MemoryPressure::Medium
                    } else if current_usage < (limit as f64 * 0.95) as usize {
                        MemoryPressure::High
                    } else {
                        MemoryPressure::Critical
                    };

                    profiler.send_event(ProfilerEvent::MemoryPressure {
                        level: pressure,
                        current_usage,
                        limit,
                    });
                } else {
                    break; // Profiler has been dropped
                }
            }
        });

        {
            let mut analysis_handle = self.analysis_handle.lock().unwrap();
            *analysis_handle = Some(handle);
        }
    }

    fn send_event(&self, event: ProfilerEvent) {
        if let Some(sender) = self.event_sender.lock().unwrap().as_ref() {
            if let Err(_) = sender.send(event) {
                // Receiver has been dropped, ignore
            }
        }
    }

    fn capture_stack_trace(&self) -> Vec<String> {
        let bt = Backtrace::new();
        let mut out = Vec::with_capacity(self.config.stack_trace_depth);
        let mut taken = 0usize;
        for frame in bt.frames() {
            for sym in frame.symbols() {
                let mut s = String::new();
                if let Some(name) = sym.name() { s.push_str(&name.to_string()); } else { s.push_str("<unknown>"); }
                if let (Some(file), Some(line)) = (sym.filename(), sym.lineno()) {
                    s.push_str(&format!(" ({}:{})", file.display(), line));
                }
                out.push(s);
                taken += 1;
                if taken >= self.config.stack_trace_depth { break; }
            }
            if taken >= self.config.stack_trace_depth { break; }
        }
        out
    }

    fn infer_category_from_stack(stack: &[String]) -> AllocationType {
        for line in stack {
            let l = line.to_lowercase();
            if l.contains("codegraph-graph") || l.contains("codegraph_graph") { return AllocationType::Graph; }
            if l.contains("codegraph-vector") || l.contains("codegraph_vector") { return AllocationType::Vector; }
            if l.contains("codegraph-parser") || l.contains("codegraph_parser") { return AllocationType::Parser; }
            if l.contains("cache") { return AllocationType::Cache; }
            if l.contains("buffer") { return AllocationType::Buffer; }
            if l.contains("index") { return AllocationType::Index; }
            if l.contains("string") { return AllocationType::String; }
        }
        AllocationType::Unknown
    }

    fn get_thread_id(&self) -> u64 {
        // Portable thread id hashing
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let tid_str = format!("{:?}", std::thread::current().id());
        let mut h = DefaultHasher::new();
        tid_str.hash(&mut h);
        h.finish()
    }

    fn update_metrics_for_allocation(&self, size: usize, category: &AllocationType) {
        let mut metrics = self.metrics.write();
        metrics.allocation_count += 1;
        metrics.current_usage = self.get_current_usage();
        metrics.peak_usage = self.peak_usage.load(Ordering::Relaxed);
        
        let category_metrics = metrics.categories.entry(category.clone())
            .or_insert_with(|| CategoryMetrics::new());
        category_metrics.allocated += size;
        category_metrics.current += size;
        category_metrics.count += 1;
        
        if size > category_metrics.peak_size {
            category_metrics.peak_size = size;
        }
        
        if category_metrics.count > 0 {
            category_metrics.average_size = category_metrics.allocated / category_metrics.count as usize;
        }
    }

    fn update_metrics_for_deallocation(&self, size: usize) {
        let mut metrics = self.metrics.write();
        metrics.deallocation_count += 1;
        metrics.current_usage = self.get_current_usage();
    }

    fn update_history(&self) {
        let now = SystemTime::now();
        let metrics = self.get_metrics();
        
        {
            let mut history = self.history.write();
            history.insert(now, metrics);
            
            // Clean up old history
            let cutoff = now.checked_sub(self.config.history_retention)
                .unwrap_or(now);
            history.retain(|&time, _| time >= cutoff);
        }
    }
}

impl MemoryMetrics {
    fn new() -> Self {
        Self {
            timestamp: SystemTime::now(),
            total_allocated: 0,
            total_freed: 0,
            current_usage: 0,
            peak_usage: 0,
            allocation_count: 0,
            deallocation_count: 0,
            active_allocations: 0,
            fragmentation_ratio: 0.0,
            memory_pressure: MemoryPressure::Low,
            categories: HashMap::new(),
        }
    }
}

impl CategoryMetrics {
    fn new() -> Self {
        Self {
            allocated: 0,
            freed: 0,
            current: 0,
            count: 0,
            average_size: 0,
            peak_size: 0,
        }
    }
}

/// Custom allocator wrapper for tracking allocations
pub struct ProfilingAllocator<A: GlobalAlloc> {
    inner: A,
}

impl<A: GlobalAlloc> ProfilingAllocator<A> {
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for ProfilingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc(layout);
        if !ptr.is_null() {
            MEMORY_PROFILER.record_allocation(ptr, layout, AllocationType::Unknown);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Use pointer-based deallocation tracking to avoid false leak reports
        MEMORY_PROFILER.record_deallocation_by_ptr(ptr, layout.size());
        self.inner.dealloc(ptr, layout);
    }
}

/// Global profiling allocator
pub type GlobalProfilingAllocator = ProfilingAllocator<System>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_profiler_initialization() {
        let profiler = MemoryProfiler::new();
        let config = ProfilerConfig::default();
        
        assert!(profiler.initialize(config).is_ok());
        assert_eq!(profiler.get_current_usage(), 0);
    }

    #[test]
    fn test_allocation_tracking() {
        let profiler = MemoryProfiler::new();
        let layout = Layout::from_size_align(1024, 8).unwrap();
        
        let id = profiler.record_allocation(
            std::ptr::null_mut(), 
            layout, 
            AllocationType::Cache
        );
        
        assert!(id > 0);
        assert_eq!(profiler.get_current_usage(), 1024);
        
        profiler.record_deallocation(id, 1024);
        assert_eq!(profiler.get_current_usage(), 0);
    }

    #[test]
    fn test_leak_detection() {
        let profiler = MemoryProfiler::new();
        let layout = Layout::from_size_align(1024 * 1024, 8).unwrap(); // 1MB
        
        // Record allocation but don't free it
        profiler.record_allocation(
            std::ptr::null_mut(), 
            layout, 
            AllocationType::Vector
        );
        
        // Sleep to age the allocation
        std::thread::sleep(Duration::from_millis(100));
        
        // For testing, we'll modify the timestamp manually
        {
            let mut allocations = profiler.allocations.write();
            for allocation in allocations.values_mut() {
                allocation.timestamp = SystemTime::now() - Duration::from_secs(400);
            }
        }
        
        let leaks = profiler.detect_leaks();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].size, 1024 * 1024);
    }

    #[test]
    fn test_pattern_analysis() {
        let profiler = MemoryProfiler::new();
        let layout = Layout::from_size_align(1024, 8).unwrap();
        
        // Record multiple allocations of same category
        for _ in 0..5 {
            profiler.record_allocation(
                std::ptr::null_mut(), 
                layout, 
                AllocationType::Cache
            );
        }
        
        let patterns = profiler.analyze_patterns();
        assert!(patterns.contains_key(&AllocationType::Cache));
        
        let cache_pattern = &patterns[&AllocationType::Cache];
        assert_eq!(cache_pattern.peak_usage, 1024);
        assert_eq!(cache_pattern.average_usage, 1024);
    }

    #[test]
    fn test_recommendation_generation() {
        let profiler = MemoryProfiler::new();
        
        // Create high memory usage to trigger recommendations
        for _ in 0..1000 {
            let layout = Layout::from_size_align(1024 * 1024, 8).unwrap(); // 1MB each
            profiler.record_allocation(
                std::ptr::null_mut(), 
                layout, 
                AllocationType::Vector
            );
        }
        
        let recommendations = profiler.generate_recommendations();
        assert!(!recommendations.is_empty());
        
        // Should have a critical recommendation for high memory usage
        let critical_recs: Vec<_> = recommendations.iter()
            .filter(|r| matches!(r.severity, RecommendationSeverity::Critical))
            .collect();
        assert!(!critical_recs.is_empty());
    }
}
