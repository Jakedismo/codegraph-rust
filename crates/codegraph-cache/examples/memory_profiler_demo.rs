/// Memory Profiler Demo - Comprehensive example showcasing all profiler features
///
/// This example demonstrates:
/// 1. Memory allocation tracking with detailed categorization
/// 2. Real-time leak detection and pattern analysis
/// 3. Optimization recommendation engine
/// 4. Live dashboard with WebSocket monitoring
/// 5. Integration with existing cache system
use codegraph_cache::{
    AllocationType, CacheConfig, CacheOptimizedHashMap, MemoryDashboard, MemoryManager,
    MemoryProfiler, ProfilerConfig, MEMORY_PROFILER,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};
use tracing_subscriber;

/// Custom allocator that integrates with the memory profiler
struct DemoAllocator;

unsafe impl GlobalAlloc for DemoAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            // Categorize allocation based on size (simplified heuristic)
            let category = match layout.size() {
                s if s < 1024 => AllocationType::String,
                s if s < 64 * 1024 => AllocationType::Buffer,
                s if s < 1024 * 1024 => AllocationType::Cache,
                _ => AllocationType::Vector,
            };

            MEMORY_PROFILER.record_allocation(ptr, layout, category);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Note: In a real implementation, you'd need to track ptr -> allocation_id mapping
        MEMORY_PROFILER.record_deallocation(0, layout.size());
        System.dealloc(ptr, layout);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("debug").init();

    info!("🚀 Starting Memory Profiler Demo");

    // Initialize the memory profiler with custom configuration
    let config = ProfilerConfig {
        enabled: true,
        stack_trace_depth: 16,
        leak_detection_interval: Duration::from_secs(10),
        history_retention: Duration::from_hours(1),
        memory_limit_bytes: 250 * 1024 * 1024, // 250MB target
        sampling_rate: 1.0,                    // Profile all allocations
        real_time_monitoring: true,
        enable_stack_traces: false, // Disabled for demo performance
    };

    MEMORY_PROFILER.initialize(config)?;
    info!("✅ Memory profiler initialized");

    // Start the dashboard server in a background task
    let dashboard = MemoryDashboard::new();
    let dashboard_handle = tokio::spawn(async move {
        if let Err(e) = dashboard.start_server(8080).await {
            error!("Dashboard server error: {}", e);
        }
    });

    info!("🌐 Dashboard server started on http://localhost:8080");
    info!("   Open your browser to view real-time memory metrics!");

    // Start monitoring task
    let monitoring_handle = tokio::spawn(monitor_memory_profiler());

    // Run memory workload simulation
    tokio::spawn(simulate_memory_workload());
    tokio::spawn(simulate_cache_operations());
    tokio::spawn(simulate_vector_operations());
    tokio::spawn(simulate_memory_leaks());

    info!("📊 Memory workload simulation started");
    info!("⏱️  Running demo for 5 minutes... Press Ctrl+C to stop");

    // Run for 5 minutes
    sleep(Duration::from_secs(300)).await;

    info!("🏁 Demo completed. Generating final report...");

    // Generate final analysis report
    generate_final_report().await;

    // Clean shutdown
    MEMORY_PROFILER.stop();
    dashboard_handle.abort();
    monitoring_handle.abort();

    info!("👋 Memory Profiler Demo finished");
    Ok(())
}

/// Monitor the memory profiler and log key events
async fn monitor_memory_profiler() {
    let mut event_receiver = MEMORY_PROFILER.start_monitoring();
    let mut report_interval = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            Some(event) = event_receiver.recv() => {
                match event {
                    codegraph_cache::ProfilerEvent::MemoryPressure { level, current_usage, limit } => {
                        warn!("Memory pressure: {:?} - {}/{} bytes", level, current_usage, limit);
                    }
                    codegraph_cache::ProfilerEvent::LeakDetected { leak } => {
                        error!("🚨 Memory leak detected: {} bytes, age: {}s",
                               leak.size, leak.age.as_secs());
                    }
                    codegraph_cache::ProfilerEvent::RecommendationGenerated { recommendation } => {
                        info!("💡 Optimization recommendation: {} - estimated savings: {} bytes",
                              recommendation.description, recommendation.estimated_savings);
                    }
                    _ => {}
                }
            }

            _ = report_interval.tick() => {
                let metrics = MEMORY_PROFILER.get_metrics();
                info!("📈 Current usage: {} MB, Peak: {} MB, Allocations: {}",
                      metrics.current_usage / (1024 * 1024),
                      metrics.peak_usage / (1024 * 1024),
                      metrics.allocation_count);
            }
        }
    }
}

/// Simulate cache operations with various allocation patterns
async fn simulate_cache_operations() {
    let cache = Arc::new(CacheOptimizedHashMap::<String, Vec<u8>>::new(Some(8)));
    let mut interval = interval(Duration::from_millis(100));

    for i in 0..1000 {
        interval.tick().await;

        // Create cache entries of varying sizes
        let size = match i % 10 {
            0..=6 => 1024,      // Small entries (70%)
            7..=8 => 64 * 1024, // Medium entries (20%)
            _ => 1024 * 1024,   // Large entries (10%)
        };

        let key = format!("cache_key_{}", i);
        let value = vec![0u8; size];

        cache.insert(key.clone(), value, size);

        // Occasionally access old entries
        if i > 10 && i % 7 == 0 {
            let old_key = format!("cache_key_{}", i - 10);
            let _ = cache.get(&old_key);
        }

        // Occasionally remove entries to simulate eviction
        if i > 20 && i % 13 == 0 {
            let old_key = format!("cache_key_{}", i - 20);
            let _ = cache.remove(&old_key);
        }
    }

    info!("✅ Cache operations simulation completed");
}

/// Simulate vector operations (embeddings, search indices)
async fn simulate_vector_operations() {
    let mut interval = interval(Duration::from_millis(200));

    for i in 0..500 {
        interval.tick().await;

        // Simulate embedding vectors
        let dimension = match i % 3 {
            0 => 384,  // Small embeddings
            1 => 768,  // Medium embeddings
            _ => 1536, // Large embeddings
        };

        let _embedding: Vec<f32> = (0..dimension).map(|_| fastrand::f32()).collect();

        // Simulate search index
        if i % 10 == 0 {
            let index_size = 1024 * 1024; // 1MB search index
            let _index: Vec<u8> = vec![0; index_size];
        }

        // Simulate batch operations
        if i % 25 == 0 {
            let batch_size = 100;
            let _batch: Vec<Vec<f32>> = (0..batch_size)
                .map(|_| (0..dimension).map(|_| fastrand::f32()).collect())
                .collect();
        }
    }

    info!("✅ Vector operations simulation completed");
}

/// Simulate graph operations (nodes, edges, traversal)
async fn simulate_graph_operations() {
    let mut interval = interval(Duration::from_millis(150));

    for i in 0..300 {
        interval.tick().await;

        // Simulate graph nodes
        let node_count = 1000;
        let _nodes: Vec<(u64, String)> = (0..node_count)
            .map(|j| (j, format!("node_{}_{}", i, j)))
            .collect();

        // Simulate adjacency matrix
        if i % 20 == 0 {
            let matrix_size = 500 * 500 * std::mem::size_of::<f32>();
            let _adjacency_matrix: Vec<f32> = vec![0.0; matrix_size / std::mem::size_of::<f32>()];
        }

        // Simulate traversal results
        let traversal_size = 50;
        let _traversal_result: Vec<u64> = (0..traversal_size).collect();
    }

    info!("✅ Graph operations simulation completed");
}

/// Simulate memory leaks for leak detection testing
async fn simulate_memory_leaks() {
    let mut interval = interval(Duration::from_secs(15));

    for i in 0..10 {
        interval.tick().await;

        // Simulate a memory leak by allocating without freeing
        let leak_size = match i {
            0..=3 => 1024 * 1024,     // 1MB leaks
            4..=6 => 5 * 1024 * 1024, // 5MB leaks
            _ => 10 * 1024 * 1024,    // 10MB leaks
        };

        let layout = Layout::from_size_align(leak_size, 8).unwrap();
        let allocation_id =
            MEMORY_PROFILER.record_allocation(std::ptr::null_mut(), layout, AllocationType::Temp);

        warn!(
            "🐛 Simulated memory leak: {} bytes (ID: {})",
            leak_size, allocation_id
        );

        // Don't call record_deallocation to simulate the leak
    }

    info!("⚠️  Memory leak simulation completed");
}

/// Generate a comprehensive analysis report
async fn generate_final_report() {
    info!("📊 Generating comprehensive memory analysis report...");

    let metrics = MEMORY_PROFILER.get_metrics();
    let leaks = MEMORY_PROFILER.detect_leaks();
    let patterns = MEMORY_PROFILER.analyze_patterns();
    let recommendations = MEMORY_PROFILER.generate_recommendations();

    println!("\n" + "=".repeat(80));
    println!("🧠 CODEGRAPH MEMORY PROFILER - FINAL REPORT");
    println!("=".repeat(80));

    // Overall Memory Metrics
    println!("\n📈 OVERALL MEMORY METRICS");
    println!("─".repeat(40));
    println!(
        "Total Allocated:     {} MB",
        metrics.total_allocated / (1024 * 1024)
    );
    println!(
        "Total Freed:         {} MB",
        metrics.total_freed / (1024 * 1024)
    );
    println!(
        "Current Usage:       {} MB",
        metrics.current_usage / (1024 * 1024)
    );
    println!(
        "Peak Usage:          {} MB",
        metrics.peak_usage / (1024 * 1024)
    );
    println!("Active Allocations:  {}", metrics.active_allocations);
    println!("Memory Pressure:     {:?}", metrics.memory_pressure);
    println!("Target Limit:        250 MB");

    let usage_percentage = (metrics.current_usage as f64 / (250.0 * 1024.0 * 1024.0)) * 100.0;
    println!("Usage vs Target:     {:.1}%", usage_percentage);

    // Memory by Category
    println!("\n🎯 MEMORY USAGE BY CATEGORY");
    println!("─".repeat(40));
    for (category, cat_metrics) in &metrics.categories {
        println!(
            "{:12} - Current: {:>8} MB, Peak: {:>8} MB, Count: {:>6}",
            format!("{:?}", category),
            cat_metrics.current / (1024 * 1024),
            cat_metrics.peak_size / (1024 * 1024),
            cat_metrics.count
        );
    }

    // Memory Leaks
    println!("\n🚨 DETECTED MEMORY LEAKS");
    println!("─".repeat(40));
    if leaks.is_empty() {
        println!("✅ No memory leaks detected!");
    } else {
        let total_leaked = leaks.iter().map(|l| l.size).sum::<usize>();
        println!(
            "Total Leaks: {} ({} MB)",
            leaks.len(),
            total_leaked / (1024 * 1024)
        );

        for leak in &leaks[..std::cmp::min(5, leaks.len())] {
            println!(
                "  • {} bytes - Age: {}s - Category: {:?} - Impact: {:?}",
                leak.size,
                leak.age.as_secs(),
                leak.category,
                leak.estimated_impact
            );
        }

        if leaks.len() > 5 {
            println!("  ... and {} more leaks", leaks.len() - 5);
        }
    }

    // Usage Patterns
    println!("\n📊 USAGE PATTERNS ANALYSIS");
    println!("─".repeat(40));
    for (category, pattern) in &patterns {
        println!(
            "{:12} - Avg: {:>6} KB, Peak: {:>8} KB, Fragmentation: {:.1}%",
            format!("{:?}", category),
            pattern.average_usage / 1024,
            pattern.peak_usage / 1024,
            pattern.fragmentation_ratio * 100.0
        );
    }

    // Optimization Recommendations
    println!("\n💡 OPTIMIZATION RECOMMENDATIONS");
    println!("─".repeat(40));
    if recommendations.is_empty() {
        println!("✅ No optimization recommendations at this time!");
    } else {
        for (i, rec) in recommendations.iter().enumerate().take(5) {
            println!("{}. {:?} - {}", i + 1, rec.severity, rec.description);
            println!(
                "   Category: {:?}, Savings: {} MB, Difficulty: {:?}",
                rec.category,
                rec.estimated_savings / (1024 * 1024),
                rec.implementation_difficulty
            );
        }
    }

    // Performance Assessment
    println!("\n⚡ PERFORMANCE ASSESSMENT");
    println!("─".repeat(40));

    let performance_score = calculate_performance_score(&metrics, &leaks, usage_percentage);
    let grade = match performance_score {
        90..=100 => "A+ (Excellent)",
        80..=89 => "A (Very Good)",
        70..=79 => "B (Good)",
        60..=69 => "C (Fair)",
        50..=59 => "D (Poor)",
        _ => "F (Critical)",
    };

    println!("Overall Score:       {}/100 ({})", performance_score, grade);
    println!("Memory Efficiency:   {:.1}%", 100.0 - usage_percentage);
    println!(
        "Leak Impact:         {}",
        if leaks.is_empty() { "None" } else { "Detected" }
    );
    println!(
        "Target Compliance:   {}",
        if usage_percentage < 80.0 {
            "✅ Compliant"
        } else {
            "⚠️ Exceeds target"
        }
    );

    println!("\n" + "=".repeat(80));
    println!("📋 Report completed. Dashboard available at: http://localhost:8080");
    println!("=".repeat(80) + "\n");
}

/// Calculate an overall performance score based on memory metrics
fn calculate_performance_score(
    metrics: &codegraph_cache::MemoryMetrics,
    leaks: &[codegraph_cache::MemoryLeak],
    usage_percentage: f64,
) -> u32 {
    let mut score = 100u32;

    // Deduct points for high memory usage
    if usage_percentage > 80.0 {
        score = score.saturating_sub(20);
    } else if usage_percentage > 60.0 {
        score = score.saturating_sub(10);
    }

    // Deduct points for memory leaks
    for leak in leaks {
        let deduction = match leak.estimated_impact {
            codegraph_cache::LeakImpact::Critical => 25,
            codegraph_cache::LeakImpact::High => 15,
            codegraph_cache::LeakImpact::Medium => 10,
            codegraph_cache::LeakImpact::Low => 5,
        };
        score = score.saturating_sub(deduction);
    }

    // Deduct points for memory pressure
    match metrics.memory_pressure {
        codegraph_cache::MemoryPressure::Critical => score = score.saturating_sub(30),
        codegraph_cache::MemoryPressure::High => score = score.saturating_sub(15),
        codegraph_cache::MemoryPressure::Medium => score = score.saturating_sub(5),
        codegraph_cache::MemoryPressure::Low => {}
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_profiler_initialization() {
        let config = ProfilerConfig::default();
        assert!(MEMORY_PROFILER.initialize(config).is_ok());
    }

    #[test]
    fn test_performance_scoring() {
        let metrics = codegraph_cache::MemoryMetrics {
            timestamp: std::time::SystemTime::now(),
            total_allocated: 100 * 1024 * 1024,
            total_freed: 50 * 1024 * 1024,
            current_usage: 50 * 1024 * 1024,
            peak_usage: 60 * 1024 * 1024,
            allocation_count: 1000,
            deallocation_count: 500,
            active_allocations: 500,
            fragmentation_ratio: 0.2,
            memory_pressure: codegraph_cache::MemoryPressure::Low,
            categories: std::collections::HashMap::new(),
        };

        let leaks = vec![];
        let usage_percentage = 20.0; // 50MB / 250MB

        let score = calculate_performance_score(&metrics, &leaks, usage_percentage);
        assert!(score >= 90); // Should get high score for low usage and no leaks
    }
}
