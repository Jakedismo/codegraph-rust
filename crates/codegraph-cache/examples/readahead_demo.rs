use codegraph_cache::{ReadAheadIntegration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 CodeGraph Read-Ahead Optimizer Demo");
    println!("=====================================\n");

    // Create the read-ahead integration
    let integration = ReadAheadIntegration::new();

    // Run comprehensive demonstration
    match integration.run_comprehensive_demo().await {
        Ok(()) => {
            println!("\n🎉 Demo completed successfully!");

            // Show final metrics
            let metrics = integration.optimizer().get_metrics().await;
            print_detailed_metrics(&metrics);
        }
        Err(e) => {
            println!("❌ Demo failed: {}", e);
        }
    }

    Ok(())
}

fn print_detailed_metrics(metrics: &codegraph_cache::ReadAheadMetrics) {
    println!("\n📈 Detailed Performance Metrics:");
    println!("┌─────────────────────────────────────┬─────────────┐");
    println!("│ Metric                              │ Value       │");
    println!("├─────────────────────────────────────┼─────────────┤");
    println!(
        "│ Total Predictions                   │ {:11} │",
        metrics.total_predictions
    );
    println!(
        "│ Successful Predictions              │ {:11} │",
        metrics.successful_predictions
    );
    println!(
        "│ Prediction Accuracy                 │ {:8.2}%   │",
        metrics.prediction_accuracy
    );
    println!(
        "│ Cache Hits from Read-ahead          │ {:11} │",
        metrics.cache_hits_from_readahead
    );
    println!(
        "│ Sequential Reads Detected           │ {:11} │",
        metrics.sequential_reads_detected
    );
    println!(
        "│ Cache Warming Events                │ {:11} │",
        metrics.cache_warming_events
    );
    println!(
        "│ Bytes Prefetched                    │ {:11} │",
        metrics.bytes_prefetched
    );
    println!(
        "│ I/O Reduction                       │ {:8.2}%   │",
        metrics.io_reduction_percentage
    );
    println!(
        "│ Average Prediction Time             │ {:8.2}ms  │",
        metrics.average_prediction_time_ms
    );
    println!(
        "│ Pattern Recognition Success Rate    │ {:8.2}%   │",
        metrics.pattern_recognition_success_rate
    );
    println!("└─────────────────────────────────────┴─────────────┘");
}

/// Standalone example showing basic read-ahead functionality
#[cfg(test)]
async fn basic_readahead_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Basic Read-Ahead Example");

    let config = ReadAheadConfig {
        max_pattern_history: 1000,
        prediction_window_size: 20,
        sequential_threshold: 3,
        cache_warming_interval: Duration::from_secs(30),
        prefetch_depth: 10,
        pattern_decay_factor: 0.9,
        min_confidence_threshold: 0.6,
        adaptive_learning_rate: 0.1,
    };

    let optimizer = ReadAheadOptimizer::new(config);

    // Simulate sequential access pattern
    println!("Simulating sequential access pattern...");
    let start_time = Instant::now();

    for i in 0..50 {
        let key = codegraph_cache::CacheKey::Custom(format!("node-{}", 1000 + i));

        if let Some(_data) = optimizer.optimize_read(key).await? {
            // Data retrieved successfully
        }
    }

    let elapsed = start_time.elapsed();
    println!("Sequential access completed in: {:?}", elapsed);

    // Get final metrics
    let metrics = optimizer.get_metrics().await;
    println!("Predictions made: {}", metrics.total_predictions);
    println!(
        "Average prediction time: {:.2}ms",
        metrics.average_prediction_time_ms
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_example() {
        assert!(basic_readahead_example().await.is_ok());
    }
}
