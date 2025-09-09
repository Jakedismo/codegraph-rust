pub mod performance;
pub mod real_time;
pub mod memory;
pub mod quality;

pub use performance::{PerformanceBenchmark, BenchmarkConfig, BenchmarkResult};
pub use real_time::{RealTimeBenchmark, ThroughputBenchmark, LatencyBenchmark};
pub use memory::{MemoryBenchmark, MemoryProfile, AllocationTracker};
pub use quality::{QualityBenchmark, SimilarityMetrics, EmbeddingQualityScore};

use crate::embedding::{CodeEmbeddingModel, EmbeddingError};
use crate::languages::{CodeLanguage, CodeInput};
use crate::optimizer::{EmbeddingOptimizer, OptimizationConfig};

use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveBenchmarkSuite {
    pub performance_benchmarks: Vec<PerformanceBenchmarkConfig>,
    pub real_time_benchmarks: Vec<RealTimeBenchmarkConfig>,
    pub memory_benchmarks: Vec<MemoryBenchmarkConfig>,
    pub quality_benchmarks: Vec<QualityBenchmarkConfig>,
    pub system_config: SystemConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarkConfig {
    pub name: String,
    pub test_cases: Vec<TestCase>,
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub languages: Vec<CodeLanguage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeBenchmarkConfig {
    pub name: String,
    pub target_latency_ms: f64,
    pub concurrent_requests: usize,
    pub duration_seconds: u64,
    pub code_sizes: Vec<CodeSize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBenchmarkConfig {
    pub name: String,
    pub cache_sizes: Vec<usize>,
    pub batch_sizes: Vec<usize>,
    pub memory_limit_mb: usize,
    pub gc_behavior: GcBehavior,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityBenchmarkConfig {
    pub name: String,
    pub similarity_threshold: f32,
    pub code_variants: Vec<CodeVariant>,
    pub comparison_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub code_content: String,
    pub language: CodeLanguage,
    pub expected_features: Option<ExpectedFeatures>,
    pub size_category: CodeSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeSize {
    Small,   // < 100 lines
    Medium,  // 100-1000 lines
    Large,   // 1000-10000 lines
    XLarge,  // > 10000 lines
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeVariant {
    SameLogicDifferentNames,
    RefactoredCode,
    CommentChanges,
    WhitespaceChanges,
    LanguageTranslation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GcBehavior {
    Aggressive,
    Conservative,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedFeatures {
    pub function_count: usize,
    pub complexity_score: f32,
    pub import_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub device: String,
    pub model_config: String,
    pub optimization_level: OptimizationLevel,
    pub cache_enabled: bool,
    pub parallel_processing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationLevel {
    None,
    Basic,
    Aggressive,
    Custom(OptimizationConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuiteResult {
    pub timestamp: std::time::SystemTime,
    pub system_info: SystemInfo,
    pub performance_results: Vec<PerformanceBenchmarkResult>,
    pub real_time_results: Vec<RealTimeBenchmarkResult>,
    pub memory_results: Vec<MemoryBenchmarkResult>,
    pub quality_results: Vec<QualityBenchmarkResult>,
    pub summary: BenchmarkSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub ram_gb: usize,
    pub gpu_info: Option<String>,
    pub os: String,
    pub rust_version: String,
    pub candle_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarkResult {
    pub name: String,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub memory_peak_mb: f64,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
    pub language_breakdown: HashMap<CodeLanguage, LanguagePerformance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeBenchmarkResult {
    pub name: String,
    pub achieved_latency_ms: f64,
    pub latency_variance: f64,
    pub dropped_requests: usize,
    pub successful_requests: usize,
    pub concurrent_capacity: usize,
    pub resource_utilization: ResourceUtilization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBenchmarkResult {
    pub name: String,
    pub peak_memory_mb: f64,
    pub avg_memory_mb: f64,
    pub memory_efficiency: f64,
    pub gc_pressure: GcPressure,
    pub cache_efficiency: CacheEfficiency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityBenchmarkResult {
    pub name: String,
    pub avg_similarity_score: f32,
    pub quality_consistency: f32,
    pub false_positive_rate: f32,
    pub false_negative_rate: f32,
    pub semantic_preservation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePerformance {
    pub avg_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub embedding_quality: f32,
    pub parse_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub gpu_usage_percent: Option<f64>,
    pub io_operations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPressure {
    pub collections_per_sec: f64,
    pub avg_collection_time_ms: f64,
    pub memory_freed_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEfficiency {
    pub hit_rate: f64,
    pub eviction_rate: f64,
    pub memory_utilization: f64,
    pub avg_lookup_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub overall_score: f64,
    pub performance_grade: Grade,
    pub real_time_grade: Grade,
    pub memory_grade: Grade,
    pub quality_grade: Grade,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Grade {
    Excellent, // A
    Good,      // B
    Average,   // C
    Poor,      // D
    Failing,   // F
}

pub struct BenchmarkRunner {
    embedding_model: CodeEmbeddingModel,
    optimizer: Option<EmbeddingOptimizer>,
    system_monitor: SystemMonitor,
}

impl BenchmarkRunner {
    pub fn new(embedding_model: CodeEmbeddingModel) -> Self {
        Self {
            embedding_model,
            optimizer: None,
            system_monitor: SystemMonitor::new(),
        }
    }

    pub fn with_optimizer(mut self, optimizer: EmbeddingOptimizer) -> Self {
        self.optimizer = Some(optimizer);
        self
    }

    pub async fn run_comprehensive_benchmark(&mut self, suite: &ComprehensiveBenchmarkSuite) -> Result<BenchmarkSuiteResult, EmbeddingError> {
        let start_time = Instant::now();
        let system_info = self.collect_system_info().await;

        println!("🚀 Starting comprehensive benchmark suite...");
        println!("System: {} cores, {}GB RAM", system_info.cpu_cores, system_info.ram_gb);

        // Run performance benchmarks
        let mut performance_results = Vec::new();
        for config in &suite.performance_benchmarks {
            println!("📊 Running performance benchmark: {}", config.name);
            let result = self.run_performance_benchmark(config).await?;
            performance_results.push(result);
        }

        // Run real-time benchmarks
        let mut real_time_results = Vec::new();
        for config in &suite.real_time_benchmarks {
            println!("⚡ Running real-time benchmark: {}", config.name);
            let result = self.run_real_time_benchmark(config).await?;
            real_time_results.push(result);
        }

        // Run memory benchmarks
        let mut memory_results = Vec::new();
        for config in &suite.memory_benchmarks {
            println!("🧠 Running memory benchmark: {}", config.name);
            let result = self.run_memory_benchmark(config).await?;
            memory_results.push(result);
        }

        // Run quality benchmarks
        let mut quality_results = Vec::new();
        for config in &suite.quality_benchmarks {
            println!("✨ Running quality benchmark: {}", config.name);
            let result = self.run_quality_benchmark(config).await?;
            quality_results.push(result);
        }

        let summary = self.compute_summary(&performance_results, &real_time_results, &memory_results, &quality_results);
        
        let total_duration = start_time.elapsed();
        println!("✅ Benchmark suite completed in {:.2}s", total_duration.as_secs_f64());
        println!("📈 Overall Score: {:.1}/100", summary.overall_score);

        Ok(BenchmarkSuiteResult {
            timestamp: std::time::SystemTime::now(),
            system_info,
            performance_results,
            real_time_results,
            memory_results,
            quality_results,
            summary,
        })
    }

    async fn run_performance_benchmark(&mut self, config: &PerformanceBenchmarkConfig) -> Result<PerformanceBenchmarkResult, EmbeddingError> {
        let benchmark = PerformanceBenchmark::new(config.clone());
        benchmark.run(&mut self.embedding_model).await
    }

    async fn run_real_time_benchmark(&mut self, config: &RealTimeBenchmarkConfig) -> Result<RealTimeBenchmarkResult, EmbeddingError> {
        let benchmark = RealTimeBenchmark::new(config.clone());
        benchmark.run(&mut self.embedding_model).await
    }

    async fn run_memory_benchmark(&mut self, config: &MemoryBenchmarkConfig) -> Result<MemoryBenchmarkResult, EmbeddingError> {
        let benchmark = MemoryBenchmark::new(config.clone());
        benchmark.run(&mut self.embedding_model).await
    }

    async fn run_quality_benchmark(&mut self, config: &QualityBenchmarkConfig) -> Result<QualityBenchmarkResult, EmbeddingError> {
        let benchmark = QualityBenchmark::new(config.clone());
        benchmark.run(&mut self.embedding_model).await
    }

    async fn collect_system_info(&self) -> SystemInfo {
        SystemInfo {
            cpu_cores: num_cpus::get(),
            ram_gb: 16, // Would get actual RAM in real implementation
            gpu_info: None, // Would detect GPU in real implementation
            os: std::env::consts::OS.to_string(),
            rust_version: env!("CARGO_PKG_VERSION").to_string(),
            candle_version: "0.4.0".to_string(), // Would get actual version
        }
    }

    fn compute_summary(&self, 
        performance_results: &[PerformanceBenchmarkResult],
        real_time_results: &[RealTimeBenchmarkResult],
        memory_results: &[MemoryBenchmarkResult],
        quality_results: &[QualityBenchmarkResult]) -> BenchmarkSummary {
        
        // Compute weighted scores
        let perf_score = self.compute_performance_score(performance_results);
        let rt_score = self.compute_real_time_score(real_time_results);
        let mem_score = self.compute_memory_score(memory_results);
        let qual_score = self.compute_quality_score(quality_results);

        let overall_score = (perf_score * 0.3) + (rt_score * 0.3) + (mem_score * 0.2) + (qual_score * 0.2);

        let recommendations = self.generate_recommendations(performance_results, real_time_results, memory_results, quality_results);

        BenchmarkSummary {
            overall_score,
            performance_grade: Self::score_to_grade(perf_score),
            real_time_grade: Self::score_to_grade(rt_score),
            memory_grade: Self::score_to_grade(mem_score),
            quality_grade: Self::score_to_grade(qual_score),
            recommendations,
        }
    }

    fn compute_performance_score(&self, results: &[PerformanceBenchmarkResult]) -> f64 {
        if results.is_empty() { return 0.0; }
        
        let avg_throughput: f64 = results.iter().map(|r| r.throughput_ops_per_sec).sum::<f64>() / results.len() as f64;
        let avg_latency: f64 = results.iter().map(|r| r.avg_latency_ms).sum::<f64>() / results.len() as f64;
        
        // Higher throughput is better, lower latency is better
        let throughput_score = (avg_throughput / 1000.0).min(100.0); // Normalize to ~100 scale
        let latency_score = (200.0 - avg_latency).max(0.0); // 200ms baseline
        
        (throughput_score + latency_score) / 2.0
    }

    fn compute_real_time_score(&self, results: &[RealTimeBenchmarkResult]) -> f64 {
        if results.is_empty() { return 0.0; }
        
        let avg_latency: f64 = results.iter().map(|r| r.achieved_latency_ms).sum::<f64>() / results.len() as f64;
        let success_rate: f64 = results.iter().map(|r| {
            let total = r.successful_requests + r.dropped_requests;
            if total > 0 { r.successful_requests as f64 / total as f64 } else { 0.0 }
        }).sum::<f64>() / results.len() as f64;
        
        let latency_score = (100.0 - avg_latency).max(0.0);
        let success_score = success_rate * 100.0;
        
        (latency_score + success_score) / 2.0
    }

    fn compute_memory_score(&self, results: &[MemoryBenchmarkResult]) -> f64 {
        if results.is_empty() { return 0.0; }
        
        let avg_efficiency: f64 = results.iter().map(|r| r.memory_efficiency).sum::<f64>() / results.len() as f64;
        let avg_cache_hit_rate: f64 = results.iter().map(|r| r.cache_efficiency.hit_rate).sum::<f64>() / results.len() as f64;
        
        (avg_efficiency * 100.0 * 0.6) + (avg_cache_hit_rate * 100.0 * 0.4)
    }

    fn compute_quality_score(&self, results: &[QualityBenchmarkResult]) -> f64 {
        if results.is_empty() { return 0.0; }
        
        let avg_similarity: f64 = results.iter().map(|r| r.avg_similarity_score as f64).sum::<f64>() / results.len() as f64;
        let avg_consistency: f64 = results.iter().map(|r| r.quality_consistency as f64).sum::<f64>() / results.len() as f64;
        
        (avg_similarity * 100.0 * 0.7) + (avg_consistency * 100.0 * 0.3)
    }

    fn score_to_grade(score: f64) -> Grade {
        match score as i32 {
            90..=100 => Grade::Excellent,
            80..=89 => Grade::Good,
            70..=79 => Grade::Average,
            60..=69 => Grade::Poor,
            _ => Grade::Failing,
        }
    }

    fn generate_recommendations(&self, 
        _performance_results: &[PerformanceBenchmarkResult],
        _real_time_results: &[RealTimeBenchmarkResult],
        _memory_results: &[MemoryBenchmarkResult],
        _quality_results: &[QualityBenchmarkResult]) -> Vec<String> {
        
        vec![
            "Consider enabling quantization for better memory efficiency".to_string(),
            "Increase cache size for better performance".to_string(),
            "Use batch processing for higher throughput".to_string(),
        ]
    }
}

pub struct SystemMonitor {
    start_time: Instant,
    peak_memory: usize,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            peak_memory: 0,
        }
    }

    pub fn record_memory_usage(&mut self, usage: usize) {
        self.peak_memory = self.peak_memory.max(usage);
    }

    pub fn get_runtime_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn get_peak_memory_mb(&self) -> f64 {
        self.peak_memory as f64 / 1024.0 / 1024.0
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}