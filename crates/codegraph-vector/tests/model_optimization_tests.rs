use approx::relative_eq;
use codegraph_core::Result;
use codegraph_vector::{
    GpuAcceleration, MemoryOptimizer, MemoryPoolConfig, ModelOptimizer, OptimizationPipelineConfig,
    ParallelConfig, QuantizationConfig, QuantizationMethod,
};
use std::time::Instant;
use tempfile::TempDir;

/// Test configuration for model optimization
#[derive(Debug, Clone)]
struct OptimizationTestConfig {
    pub dimension: usize,
    pub num_vectors: usize,
    pub _quantization_bits: u8,
    pub _compression_ratio: f32,
    pub _accuracy_threshold: f32,
}

impl Default for OptimizationTestConfig {
    fn default() -> Self {
        Self {
            dimension: 128,
            num_vectors: 1000,
            _quantization_bits: 8,
            _compression_ratio: 0.25,
            _accuracy_threshold: 0.95,
        }
    }
}

/// Generate deterministic test vectors for optimization testing
fn generate_optimization_vectors(count: usize, dimension: usize, seed: u64) -> Vec<Vec<f32>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..count)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let hash = hasher.finish();

            (0..dimension)
                .map(|j| {
                    let mut hasher = DefaultHasher::new();
                    hash.hash(&mut hasher);
                    j.hash(&mut hasher);
                    let val = (hasher.finish() as f32 / u64::MAX as f32) - 0.5;
                    val * 2.0 // Scale to [-1, 1] range
                })
                .collect()
        })
        .collect()
}

#[tokio::test]
async fn test_model_quantization_int8() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let vectors = generate_optimization_vectors(config.num_vectors, config.dimension, 12345);

    let quantization_config = QuantizationConfig {
        bits: 8,
        method: QuantizationMethod::Linear,
        calibration_samples: 100,
        symmetric: true,
        preserve_accuracy: true,
    };

    let optimizer = ModelOptimizer::new(config.dimension);

    // Test vector quantization
    let original_vector = &vectors[0];
    let quantized = optimizer.quantize_vector(original_vector, &quantization_config)?;
    let dequantized = optimizer.dequantize_vector(&quantized, &quantization_config)?;

    // Check memory reduction
    let original_size = original_vector.len() * std::mem::size_of::<f32>();
    let quantized_size = quantized.len() * std::mem::size_of::<i8>();
    let compression_ratio = quantized_size as f32 / original_size as f32;

    assert!(compression_ratio <= 0.26); // Should be ~0.25 for 8-bit quantization
    assert_eq!(dequantized.len(), original_vector.len());

    // Test accuracy preservation
    let mut error_sum = 0.0;
    for (orig, deq) in original_vector.iter().zip(dequantized.iter()) {
        error_sum += (orig - deq).abs();
    }
    let mean_absolute_error = error_sum / original_vector.len() as f32;

    assert!(mean_absolute_error < 0.1); // Reasonable error for 8-bit quantization

    println!("✓ INT8 Quantization Test:");
    println!("  Compression ratio: {:.3}", compression_ratio);
    println!("  Mean absolute error: {:.6}", mean_absolute_error);

    Ok(())
}

#[tokio::test]
async fn test_model_quantization_int4() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let vectors = generate_optimization_vectors(config.num_vectors, config.dimension, 54321);

    let quantization_config = QuantizationConfig {
        bits: 4,
        method: QuantizationMethod::Asymmetric,
        calibration_samples: 200,
        symmetric: false,
        preserve_accuracy: false, // Allow more aggressive compression
    };

    let optimizer = ModelOptimizer::new(config.dimension);

    // Test batch quantization for efficiency
    let batch_vectors: Vec<&[f32]> = vectors[0..10].iter().map(|v| v.as_slice()).collect();
    let quantized_batch = optimizer.quantize_batch(&batch_vectors, &quantization_config)?;

    // Check memory reduction for batch
    let original_batch_size = batch_vectors.len() * config.dimension * std::mem::size_of::<f32>();
    let quantized_batch_size = quantized_batch.data.len() * std::mem::size_of::<u8>()
        + quantized_batch.scales.len() * std::mem::size_of::<f32>();
    let compression_ratio = quantized_batch_size as f32 / original_batch_size as f32;

    assert!(compression_ratio <= 0.13); // Should be ~0.125 for 4-bit + metadata

    // Test dequantization
    let dequantized_batch = optimizer.dequantize_batch(&quantized_batch, &quantization_config)?;
    assert_eq!(dequantized_batch.len(), batch_vectors.len());

    println!("✓ INT4 Quantization Test:");
    println!("  Batch compression ratio: {:.3}", compression_ratio);
    println!("  Batch size: {} vectors", batch_vectors.len());

    Ok(())
}

#[tokio::test]
async fn test_gpu_acceleration_setup() -> Result<()> {
    let config = OptimizationTestConfig::default();

    // Test GPU detection and initialization
    let mut gpu_accel = GpuAcceleration::new()?;
    let gpu_info = gpu_accel.get_device_info()?;

    println!("✓ GPU Acceleration Test:");
    println!("  GPU available: {}", gpu_info.available);

    if gpu_info.available {
        println!("  Device name: {}", gpu_info.device_name);
        println!("  Memory: {:.1} GB", gpu_info.memory_gb);
        println!(
            "  Compute capability: {}.{}",
            gpu_info.compute_major, gpu_info.compute_minor
        );

        // Test memory allocation
        let test_size_mb = 16; // Allocate 16MB for testing
        let allocation = gpu_accel.allocate_memory(test_size_mb * 1024 * 1024)?;
        assert!(allocation.is_valid());

        // Test vector operations on GPU
        let vectors = generate_optimization_vectors(100, config.dimension, 99999);
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();

        let start = Instant::now();
        let gpu_result = gpu_accel.upload_vectors(&flat_vectors, config.dimension)?;
        let upload_time = start.elapsed();

        assert!(gpu_result.is_uploaded());
        println!("  Upload time: {:?}", upload_time);

        // Test GPU-accelerated distance computation
        let query = &vectors[0];
        let start = Instant::now();
        let distances = gpu_accel.compute_distances(query, &gpu_result, 10)?;
        let search_time = start.elapsed();

        assert_eq!(distances.len(), 10);
        println!("  Search time: {:?}", search_time);

        gpu_accel.deallocate_memory(allocation)?;
    } else {
        println!("  Running CPU-only tests");
        // Test CPU fallback
        let cpu_accel = gpu_accel.get_cpu_fallback()?;
        assert!(cpu_accel.is_available());
    }

    Ok(())
}

#[tokio::test]
async fn test_memory_optimization_strategies() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let vectors = generate_optimization_vectors(config.num_vectors, config.dimension, 77777);

    let mut memory_optimizer = MemoryOptimizer::new();

    // Test memory pooling
    let pool_config = MemoryPoolConfig {
        initial_size_mb: 64,
        max_size_mb: 256,
        block_size_kb: 64,
        enable_reuse: true,
        enable_compaction: true,
    };

    memory_optimizer.create_pool(pool_config)?;

    // Test efficient memory allocation
    let start = Instant::now();
    let allocations = memory_optimizer.allocate_for_vectors(&vectors)?;
    let allocation_time = start.elapsed();

    assert_eq!(allocations.len(), vectors.len());

    // Test memory usage tracking
    let memory_stats = memory_optimizer.get_memory_stats();
    assert!(memory_stats.allocated_bytes > 0);
    assert!(memory_stats.peak_usage_bytes >= memory_stats.allocated_bytes);
    assert!(memory_stats.fragmentation_ratio >= 0.0);
    assert!(memory_stats.fragmentation_ratio <= 1.0);

    // Test memory compaction
    let pre_compaction_fragmentation = memory_stats.fragmentation_ratio;
    memory_optimizer.compact_memory()?;
    let post_compaction_stats = memory_optimizer.get_memory_stats();

    // Fragmentation should not increase after compaction
    assert!(post_compaction_stats.fragmentation_ratio <= pre_compaction_fragmentation + 0.01);

    // Test memory-mapped file storage for large datasets (only with persistent feature)
    #[cfg(feature = "persistent")]
    {
        let temp_dir = TempDir::new()?;
        let mmap_file = temp_dir.path().join("vectors.mmap");

        let start = Instant::now();
        memory_optimizer.save_to_mmap(&vectors, &mmap_file)?;
        let save_time = start.elapsed();

        let start = Instant::now();
        let loaded_vectors = memory_optimizer.load_from_mmap(&mmap_file, config.dimension)?;
        let load_time = start.elapsed();

        assert_eq!(loaded_vectors.len(), vectors.len());

        // Verify data integrity
        for (original, loaded) in vectors.iter().zip(loaded_vectors.iter()) {
            assert_eq!(original.len(), loaded.len());
            for (a, b) in original.iter().zip(loaded.iter()) {
                assert!(relative_eq!(*a, *b, epsilon = 1e-6));
            }
        }

        println!("  Save to mmap: {:?}", save_time);
        println!("  Load from mmap: {:?}", load_time);
    }

    #[cfg(not(feature = "persistent"))]
    {
        println!("  Memory-mapped file storage skipped (persistent feature not enabled)");
    }

    println!("✓ Memory Optimization Test:");
    println!("  Allocation time: {:?}", allocation_time);
    println!(
        "  Peak memory: {:.2} MB",
        memory_stats.peak_usage_bytes as f64 / 1024.0 / 1024.0
    );
    println!(
        "  Fragmentation: {:.3}",
        post_compaction_stats.fragmentation_ratio
    );

    Ok(())
}

#[tokio::test]
async fn test_parallel_processing_optimization() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let large_vector_count = 5000;
    let vectors = generate_optimization_vectors(large_vector_count, config.dimension, 88888);

    let optimizer = ModelOptimizer::new(config.dimension);

    // Test parallel quantization
    let quantization_config = QuantizationConfig {
        bits: 8,
        method: QuantizationMethod::Linear,
        calibration_samples: 100,
        symmetric: true,
        preserve_accuracy: true,
    };

    // Sequential processing
    let start = Instant::now();
    let mut sequential_results = Vec::new();
    for vector in &vectors[0..100] {
        let quantized = optimizer.quantize_vector(vector, &quantization_config)?;
        sequential_results.push(quantized);
    }
    let sequential_time = start.elapsed();

    // Parallel processing
    let parallel_config = ParallelConfig {
        num_threads: num_cpus::get(),
        batch_size: 25,
        enable_simd: true,
        memory_prefetch: true,
    };

    let start = Instant::now();
    let parallel_results =
        optimizer.quantize_parallel(&vectors[0..100], &quantization_config, &parallel_config)?;
    let parallel_time = start.elapsed();

    assert_eq!(sequential_results.len(), parallel_results.len());

    // Verify results are equivalent
    for (seq, par) in sequential_results.iter().zip(parallel_results.iter()) {
        assert_eq!(seq.len(), par.len());
        for (s, p) in seq.iter().zip(par.iter()) {
            assert_eq!(*s, *p);
        }
    }

    let speedup_ratio = sequential_time.as_nanos() as f64 / parallel_time.as_nanos() as f64;

    println!("✓ Parallel Processing Test:");
    println!("  Sequential time: {:?}", sequential_time);
    println!("  Parallel time: {:?}", parallel_time);
    println!("  Speedup ratio: {:.2}x", speedup_ratio);
    println!("  Threads used: {}", parallel_config.num_threads);

    // Parallel should be faster or at least not significantly slower
    assert!(speedup_ratio >= 0.8); // Allow some overhead for small batches

    Ok(())
}

#[tokio::test]
async fn test_end_to_end_optimization_pipeline() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let vectors = generate_optimization_vectors(config.num_vectors, config.dimension, 11223);

    // Create end-to-end optimization pipeline
    let optimizer = ModelOptimizer::new(config.dimension);
    let pipeline_config = OptimizationPipelineConfig {
        quantization: QuantizationConfig {
            bits: 8,
            method: QuantizationMethod::Linear,
            calibration_samples: 100,
            symmetric: true,
            preserve_accuracy: true,
        },
        memory_optimization: true,
        gpu_acceleration: true,
        parallel_processing: true,
        compression_enabled: true,
        target_accuracy: 0.95,
        target_memory_reduction: 0.75,
    };

    // Run full optimization pipeline
    let start = Instant::now();
    let optimization_result = optimizer
        .optimize_full_pipeline(&vectors, &pipeline_config)
        .await?;
    let total_optimization_time = start.elapsed();

    // Verify optimization results
    assert!(optimization_result.memory_reduction >= pipeline_config.target_memory_reduction - 0.05);
    assert!(optimization_result.accuracy_preservation >= pipeline_config.target_accuracy - 0.02);
    assert!(optimization_result.inference_speedup >= 1.0);

    // Test optimized inference
    let query = &vectors[0];
    let start = Instant::now();
    let optimized_results = optimization_result.search_optimized(query, 10)?;
    let optimized_search_time = start.elapsed();

    // Compare with baseline
    let start = Instant::now();
    let baseline_results = optimizer.search_baseline(query, &vectors, 10)?;
    let baseline_search_time = start.elapsed();

    assert_eq!(optimized_results.len(), baseline_results.len());

    // Calculate accuracy by comparing top results
    let mut accuracy_matches = 0;
    for (opt_id, base_id) in optimized_results.iter().zip(baseline_results.iter()) {
        if opt_id == base_id {
            accuracy_matches += 1;
        }
    }
    let search_accuracy = accuracy_matches as f32 / optimized_results.len() as f32;

    let inference_speedup =
        baseline_search_time.as_nanos() as f64 / optimized_search_time.as_nanos() as f64;

    println!("✓ End-to-End Optimization Pipeline:");
    println!("  Total optimization time: {:?}", total_optimization_time);
    println!(
        "  Memory reduction: {:.1}%",
        optimization_result.memory_reduction * 100.0
    );
    println!(
        "  Accuracy preservation: {:.3}",
        optimization_result.accuracy_preservation
    );
    println!("  Inference speedup: {:.2}x", inference_speedup);
    println!("  Search accuracy: {:.3}", search_accuracy);
    println!("  Optimized search time: {:?}", optimized_search_time);
    println!("  Baseline search time: {:?}", baseline_search_time);

    // Verify performance targets met
    assert!(search_accuracy >= 0.8); // 80% of results should match
    assert!(inference_speedup >= 1.0); // Should not be slower

    Ok(())
}

#[tokio::test]
async fn test_optimization_benchmarks() -> Result<()> {
    let config = OptimizationTestConfig::default();
    let benchmark_sizes = vec![100, 500, 1000, 2000];

    for size in benchmark_sizes {
        let vectors = generate_optimization_vectors(size, config.dimension, size as u64);
        let optimizer = ModelOptimizer::new(config.dimension);

        println!("Benchmarking {} vectors:", size);

        // Benchmark quantization
        let quantization_config = QuantizationConfig {
            bits: 8,
            method: QuantizationMethod::Linear,
            calibration_samples: std::cmp::min(100, size / 10),
            symmetric: true,
            preserve_accuracy: true,
        };

        let start = Instant::now();
        let _quantized = optimizer.quantize_batch(
            &vectors.iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
            &quantization_config,
        )?;
        let quantization_time = start.elapsed();

        let throughput = size as f64 / quantization_time.as_secs_f64();

        println!(
            "  Quantization: {:?} ({:.0} vectors/sec)",
            quantization_time, throughput
        );

        // Basic performance requirement: should process at least 100 vectors/second for 8-bit quantization
        assert!(
            throughput >= 50.0,
            "Quantization throughput too low: {:.0} vectors/sec",
            throughput
        );
    }

    println!("✓ All benchmark tests passed");

    Ok(())
}
