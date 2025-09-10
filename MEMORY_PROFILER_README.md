# CodeGraph Memory Profiler

## Overview

The CodeGraph Memory Profiler is a comprehensive memory analysis system designed to help achieve the <250MB memory target while preventing memory leaks and optimizing performance. It provides real-time monitoring, detailed allocation tracking, leak detection, and optimization recommendations.

## Features

### 🔍 Detailed Allocation Tracking
- **Real-time monitoring** of all memory allocations and deallocations
- **Categorized tracking** by allocation type (Cache, Vector, Graph, etc.)
- **Stack trace capture** for debugging memory issues
- **Thread-aware tracking** with multi-threaded safety
- **Sampling support** for reduced overhead in production

### 🚨 Advanced Leak Detection
- **Automatic leak detection** based on allocation age and patterns
- **Impact assessment** (Low, Medium, High, Critical)
- **Category-specific analysis** to identify leak sources
- **Real-time alerts** for immediate action
- **Historical leak tracking** for trend analysis

### 📊 Usage Pattern Analysis
- **Allocation pattern recognition** by category
- **Fragmentation analysis** to identify memory waste
- **Lifetime distribution** analysis for optimization
- **Rate analysis** (allocations/deallocations per second)
- **Peak usage tracking** for capacity planning

### 💡 Optimization Recommendation Engine
- **Intelligent recommendations** based on usage patterns
- **Estimated savings calculations** for each recommendation
- **Implementation difficulty assessment** (Easy, Medium, Hard)
- **Priority ranking** by impact and feasibility
- **Actionable suggestions** with specific optimization strategies

### 🌐 Real-time Monitoring Dashboard
- **Web-based interface** with live metrics
- **Interactive charts** for memory usage over time
- **WebSocket integration** for real-time updates
- **Export capabilities** (JSON, CSV, Prometheus)
- **Mobile-responsive design** for monitoring on any device

## Architecture

### Core Components

1. **MemoryProfiler** - Central profiling engine
2. **AllocationTracker** - Records and categorizes allocations
3. **LeakDetector** - Identifies and analyzes memory leaks
4. **PatternAnalyzer** - Analyzes usage patterns and trends
5. **RecommendationEngine** - Generates optimization suggestions
6. **Dashboard** - Web interface for monitoring and analysis

### Memory Categories

- `Cache` - Cache system allocations
- `Vector` - Embedding and vector operations
- `Graph` - Graph structure and traversal data
- `Parser` - Code parsing and AST operations
- `String` - String allocations and text processing
- `Buffer` - Temporary buffers and I/O operations
- `Index` - Search indices and data structures
- `Temp` - Temporary allocations
- `Unknown` - Uncategorized allocations

## Usage

### Basic Setup

```rust
use codegraph_cache::{MemoryProfiler, ProfilerConfig, AllocationType, MEMORY_PROFILER};

// Initialize with custom configuration
let config = ProfilerConfig {
    enabled: true,
    memory_limit_bytes: 250 * 1024 * 1024, // 250MB target
    sampling_rate: 1.0, // Profile all allocations
    real_time_monitoring: true,
    leak_detection_interval: Duration::from_secs(30),
    ..Default::default()
};

MEMORY_PROFILER.initialize(config)?;
```

### Recording Allocations

```rust
// Record a memory allocation
let layout = Layout::from_size_align(1024, 8)?;
let allocation_id = MEMORY_PROFILER.record_allocation(
    ptr, 
    layout, 
    AllocationType::Cache
);

// Record deallocation
MEMORY_PROFILER.record_deallocation(allocation_id, 1024);
```

### Monitoring and Analysis

```rust
// Get current metrics
let metrics = MEMORY_PROFILER.get_metrics();
println!("Current usage: {} MB", metrics.current_usage / (1024 * 1024));

// Detect memory leaks
let leaks = MEMORY_PROFILER.detect_leaks();
for leak in leaks {
    println!("Leak detected: {} bytes, age: {}s", leak.size, leak.age.as_secs());
}

// Analyze usage patterns
let patterns = MEMORY_PROFILER.analyze_patterns();
for (category, pattern) in patterns {
    println!("{:?}: avg={} KB, peak={} KB", 
             category, pattern.average_usage / 1024, pattern.peak_usage / 1024);
}

// Get optimization recommendations
let recommendations = MEMORY_PROFILER.generate_recommendations();
for rec in recommendations {
    println!("💡 {}: estimated savings {} MB", 
             rec.description, rec.estimated_savings / (1024 * 1024));
}
```

### Dashboard Integration

```rust
use codegraph_cache::MemoryDashboard;

// Start the dashboard server
let dashboard = MemoryDashboard::new();
dashboard.start_server(8080).await?;
```

Access the dashboard at `http://localhost:8080` for real-time monitoring.

### Real-time Event Monitoring

```rust
// Subscribe to real-time events
let mut event_receiver = MEMORY_PROFILER.start_monitoring();

while let Some(event) = event_receiver.recv().await {
    match event {
        ProfilerEvent::MemoryPressure { level, current_usage, limit } => {
            println!("Memory pressure: {:?}", level);
        }
        ProfilerEvent::LeakDetected { leak } => {
            println!("🚨 Leak detected: {} bytes", leak.size);
        }
        ProfilerEvent::RecommendationGenerated { recommendation } => {
            println!("💡 New recommendation: {}", recommendation.description);
        }
        _ => {}
    }
}
```

## Configuration

### ProfilerConfig Options

```rust
pub struct ProfilerConfig {
    pub enabled: bool,                    // Enable/disable profiling
    pub stack_trace_depth: usize,         // Stack trace capture depth
    pub leak_detection_interval: Duration, // How often to check for leaks
    pub history_retention: Duration,       // How long to keep historical data
    pub memory_limit_bytes: usize,        // Memory usage target/limit
    pub sampling_rate: f64,               // 0.0-1.0 sampling rate
    pub real_time_monitoring: bool,       // Enable real-time events
    pub enable_stack_traces: bool,        // Capture stack traces
}
```

### Environment Variables

- `MEMORY_PROFILER_ENABLED` - Enable/disable profiling
- `MEMORY_PROFILER_LIMIT_MB` - Memory limit in MB
- `MEMORY_PROFILER_SAMPLING_RATE` - Sampling rate (0.0-1.0)
- `MEMORY_PROFILER_DASHBOARD_PORT` - Dashboard server port

## Performance Impact

The memory profiler is designed to have minimal performance impact:

- **Sampling support** - Profile only a percentage of allocations
- **Lock-free atomic operations** for counters
- **Efficient data structures** optimized for cache performance
- **Configurable overhead** - Disable features not needed
- **Background processing** - Analysis runs in separate threads

### Overhead Measurements

| Configuration | CPU Overhead | Memory Overhead |
|---------------|--------------|-----------------|
| Full profiling | ~2-5% | ~5-10MB |
| 50% sampling | ~1-2% | ~3-5MB |
| 10% sampling | ~0.5-1% | ~1-2MB |
| Disabled | ~0% | ~0MB |

## Integration Examples

### With Existing Cache System

```rust
use codegraph_cache::{CacheOptimizedHashMap, MEMORY_PROFILER};

// Cache operations are automatically tracked
let cache = CacheOptimizedHashMap::new(Some(8));
cache.insert(key, value, size_bytes); // Allocation tracked automatically
```

### Custom Allocator Integration

```rust
use std::alloc::{GlobalAlloc, Layout, System};

struct ProfilingAllocator;

unsafe impl GlobalAlloc for ProfilingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            MEMORY_PROFILER.record_allocation(ptr, layout, AllocationType::Unknown);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        MEMORY_PROFILER.record_deallocation(0, layout.size());
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOC: ProfilingAllocator = ProfilingAllocator;
```

## Dashboard Features

### Real-time Metrics
- Current memory usage and trends
- Memory pressure indicators
- Allocation/deallocation rates
- Category-wise breakdown

### Interactive Charts
- Memory usage over time
- Category distribution pie chart
- Allocation rate trends
- Leak detection timeline

### Export Capabilities
- JSON export for detailed analysis
- CSV export for spreadsheet analysis
- Prometheus metrics for monitoring integration
- Raw data export for custom analysis

### Alert System
- Memory pressure alerts
- Leak detection notifications
- Threshold breach warnings
- Performance degradation alerts

## Best Practices

### For Development
1. **Enable full profiling** during development for comprehensive analysis
2. **Use category-specific tracking** to identify allocation sources
3. **Monitor dashboard regularly** to catch issues early
4. **Act on recommendations** to optimize memory usage
5. **Set up CI integration** to prevent memory regressions

### For Production
1. **Use sampling** (10-50%) to reduce overhead
2. **Monitor memory pressure** for early warning
3. **Set up alerting** for critical thresholds
4. **Regular leak detection** to prevent accumulation
5. **Export metrics** to monitoring systems

### Memory Optimization Tips
1. **Pool allocations** for frequently used objects
2. **Use compression** for large data structures
3. **Implement lazy loading** for non-critical data
4. **Regular cleanup** of temporary allocations
5. **Monitor fragmentation** and compact when needed

## Troubleshooting

### Common Issues

**High Memory Usage**
```rust
// Check current usage vs limit
let metrics = MEMORY_PROFILER.get_metrics();
let usage_ratio = metrics.current_usage as f64 / limit as f64;
if usage_ratio > 0.8 {
    // Apply memory pressure protocols
}
```

**Memory Leaks**
```rust
// Detect and analyze leaks
let leaks = MEMORY_PROFILER.detect_leaks();
for leak in leaks {
    // Focus on high-impact leaks first
    if matches!(leak.estimated_impact, LeakImpact::High | LeakImpact::Critical) {
        // Investigate stack trace and category
        println!("Critical leak in {:?}: {}", leak.category, leak.size);
    }
}
```

**Performance Impact**
```rust
// Reduce sampling rate if overhead is too high
let config = ProfilerConfig {
    sampling_rate: 0.1, // Profile only 10% of allocations
    enable_stack_traces: false, // Disable expensive stack traces
    ..default_config
};
```

## Integration with CI/CD

### Memory Regression Tests
```rust
#[test]
fn test_memory_usage_within_limits() {
    let initial_usage = MEMORY_PROFILER.get_current_usage();
    
    // Run your test workload
    run_test_workload();
    
    let final_usage = MEMORY_PROFILER.get_current_usage();
    let increase = final_usage.saturating_sub(initial_usage);
    
    assert!(increase < MEMORY_REGRESSION_THRESHOLD, 
            "Memory usage increased by {} bytes", increase);
}
```

### Leak Detection in Tests
```rust
#[test]
fn test_no_memory_leaks() {
    let initial_leaks = MEMORY_PROFILER.detect_leaks().len();
    
    // Run test
    run_feature_test();
    
    // Wait for allocations to settle
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    let final_leaks = MEMORY_PROFILER.detect_leaks().len();
    assert_eq!(initial_leaks, final_leaks, "Memory leaks detected during test");
}
```

## Future Enhancements

- **Machine learning** for pattern prediction
- **Automatic optimization** based on recommendations
- **Integration with profiling tools** (perf, valgrind)
- **Cloud monitoring** integration
- **Advanced visualization** with 3D memory maps
- **Predictive leak detection** using ML models

## Support and Contributing

For issues, feature requests, or contributions, please see the main CodeGraph repository. The memory profiler is actively maintained and welcomes community contributions.

## License

The CodeGraph Memory Profiler is licensed under MIT OR Apache-2.0, same as the main CodeGraph project.