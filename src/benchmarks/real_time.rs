use super::*;
use crate::embedding::{CodeEmbeddingModel, EmbeddingError};

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::{Semaphore, RwLock};
use tokio::time::{timeout, interval};

pub struct RealTimeBenchmark {
    config: RealTimeBenchmarkConfig,
    metrics_collector: MetricsCollector,
}

impl RealTimeBenchmark {
    pub fn new(config: RealTimeBenchmarkConfig) -> Self {
        Self {
            config,
            metrics_collector: MetricsCollector::new(),
        }
    }

    pub async fn run(&mut self, model: &mut CodeEmbeddingModel) -> Result<RealTimeBenchmarkResult, EmbeddingError> {
        println!("  ⚡ Real-time benchmark: {} concurrent requests, target {}ms latency", 
                 self.config.concurrent_requests, self.config.target_latency_ms);

        let start_time = Instant::now();
        let duration = Duration::from_secs(self.config.duration_seconds);
        
        // Generate test workload
        let test_workload = self.generate_workload().await?;
        
        // Run concurrent load simulation
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_requests));
        let model = Arc::new(RwLock::new(model));
        let mut handles = Vec::new();
        
        let end_time = start_time + duration;
        
        // Spawn workers
        for worker_id in 0..self.config.concurrent_requests {
            let semaphore = semaphore.clone();
            let model = model.clone();
            let workload = test_workload.clone();
            let target_latency = self.config.target_latency_ms;
            let metrics = self.metrics_collector.clone();

            let handle = tokio::spawn(async move {
                let mut local_stats = WorkerStats::new(worker_id);
                
                while Instant::now() < end_time {
                    let _permit = semaphore.acquire().await.unwrap();
                    
                    for work_item in &workload.items {
                        if Instant::now() >= end_time {
                            break;
                        }

                        let request_start = Instant::now();
                        
                        let result = timeout(
                            Duration::from_millis((target_latency * 2.0) as u64),
                            async {
                                let model_guard = model.read().await;
                                model_guard.embed_code(&work_item.code, work_item.language).await
                            }
                        ).await;

                        let latency = request_start.elapsed();
                        
                        match result {
                            Ok(Ok(_embeddings)) => {
                                local_stats.successful_requests += 1;
                                local_stats.total_latency += latency;
                                local_stats.max_latency = local_stats.max_latency.max(latency);
                                
                                if latency.as_millis() as f64 <= target_latency {
                                    local_stats.within_sla_requests += 1;
                                }
                                
                                metrics.record_success(latency).await;
                            }
                            Ok(Err(_)) => {
                                local_stats.failed_requests += 1;
                                metrics.record_error("embedding_error").await;
                            }
                            Err(_) => {
                                local_stats.timeout_requests += 1;
                                metrics.record_error("timeout").await;
                            }
                        }
                        
                        // Small delay to simulate realistic request patterns
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
                
                local_stats
            });
            
            handles.push(handle);
        }

        // Collect results from all workers
        let mut all_stats = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(stats) => all_stats.push(stats),
                Err(_) => {
                    // Worker failed, create empty stats
                    all_stats.push(WorkerStats::new(999));
                }
            }
        }

        self.compute_real_time_result(all_stats, start_time.elapsed()).await
    }

    async fn generate_workload(&self) -> Result<TestWorkload, EmbeddingError> {
        let mut items = Vec::new();
        
        for size_category in &self.config.code_sizes {
            let code_samples = self.generate_code_samples(size_category, 10).await?;
            
            for sample in code_samples {
                items.push(WorkItem {
                    code: sample.content,
                    language: sample.language,
                    expected_latency_ms: self.estimate_latency(size_category),
                    priority: Priority::Normal,
                });
            }
        }

        Ok(TestWorkload { items })
    }

    async fn generate_code_samples(&self, size_category: &CodeSize, count: usize) -> Result<Vec<CodeSample>, EmbeddingError> {
        let mut samples = Vec::new();
        
        for i in 0..count {
            let sample = match size_category {
                CodeSize::Small => CodeSample {
                    content: format!("fn small_function_{}() {{ println!(\"Hello {}\"); }}", i, i),
                    language: CodeLanguage::Rust,
                    lines: 3,
                    complexity: 1,
                },
                CodeSize::Medium => {
                    let mut code = String::new();
                    code.push_str(&format!("struct MediumStruct{} {{\n", i));
                    for j in 0..20 {
                        code.push_str(&format!("    field_{}: i32,\n", j));
                    }
                    code.push_str("}\n\n");
                    code.push_str(&format!("impl MediumStruct{} {{\n", i));
                    for j in 0..10 {
                        code.push_str(&format!("    fn method_{}(&self) -> i32 {{ self.field_{} }}\n", j, j));
                    }
                    code.push_str("}\n");
                    
                    CodeSample {
                        content: code,
                        language: CodeLanguage::Rust,
                        lines: 32,
                        complexity: 5,
                    }
                },
                CodeSize::Large => {
                    let mut code = String::new();
                    for i in 0..100 {
                        code.push_str(&format!("fn function_{}(x: i32, y: i32) -> i32 {{\n", i));
                        code.push_str("    if x > y {\n");
                        code.push_str("        x * 2 + y\n");
                        code.push_str("    } else {\n");
                        code.push_str("        y * 2 + x\n");
                        code.push_str("    }\n");
                        code.push_str("}\n\n");
                    }
                    
                    CodeSample {
                        content: code,
                        language: CodeLanguage::Rust,
                        lines: 700,
                        complexity: 20,
                    }
                },
                CodeSize::XLarge => {
                    // Generate a very large code sample
                    let mut code = String::new();
                    code.push_str("// Large application simulation\n");
                    
                    for module in 0..50 {
                        code.push_str(&format!("mod module_{} {{\n", module));
                        for struct_i in 0..20 {
                            code.push_str(&format!("    pub struct Struct{}_{} {{\n", module, struct_i));
                            for field in 0..10 {
                                code.push_str(&format!("        pub field_{}: i32,\n", field));
                            }
                            code.push_str("    }\n");
                        }
                        code.push_str("}\n\n");
                    }
                    
                    CodeSample {
                        content: code,
                        language: CodeLanguage::Rust,
                        lines: 3000,
                        complexity: 50,
                    }
                }
            };
            
            samples.push(sample);
        }
        
        Ok(samples)
    }

    fn estimate_latency(&self, size_category: &CodeSize) -> f64 {
        match size_category {
            CodeSize::Small => 20.0,
            CodeSize::Medium => 50.0,
            CodeSize::Large => 150.0,
            CodeSize::XLarge => 500.0,
        }
    }

    async fn compute_real_time_result(&self, all_stats: Vec<WorkerStats>, total_duration: Duration) -> Result<RealTimeBenchmarkResult, EmbeddingError> {
        let total_successful: usize = all_stats.iter().map(|s| s.successful_requests).sum();
        let total_failed: usize = all_stats.iter().map(|s| s.failed_requests).sum();
        let total_timeout: usize = all_stats.iter().map(|s| s.timeout_requests).sum();
        let total_within_sla: usize = all_stats.iter().map(|s| s.within_sla_requests).sum();

        let total_requests = total_successful + total_failed + total_timeout;
        let dropped_requests = total_failed + total_timeout;

        // Compute average latency
        let total_latency_ms: f64 = all_stats.iter()
            .map(|s| s.total_latency.as_millis() as f64)
            .sum();
        let achieved_latency_ms = if total_successful > 0 {
            total_latency_ms / total_successful as f64
        } else {
            f64::INFINITY
        };

        // Compute latency variance
        let latency_samples: Vec<f64> = all_stats.iter()
            .filter(|s| s.successful_requests > 0)
            .map(|s| s.total_latency.as_millis() as f64 / s.successful_requests as f64)
            .collect();
        
        let latency_variance = if latency_samples.len() > 1 {
            let mean = latency_samples.iter().sum::<f64>() / latency_samples.len() as f64;
            let variance = latency_samples.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / (latency_samples.len() - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // SLA compliance
        let sla_compliance_rate = if total_successful > 0 {
            total_within_sla as f64 / total_successful as f64
        } else {
            0.0
        };

        // Resource utilization (simplified)
        let cpu_usage = (total_requests as f64 / total_duration.as_secs_f64() / 100.0).min(100.0);
        let memory_usage = (total_requests * 1024) as f64; // Rough estimate
        
        let resource_utilization = ResourceUtilization {
            cpu_usage_percent: cpu_usage,
            memory_usage_mb: memory_usage / 1024.0 / 1024.0,
            gpu_usage_percent: None,
            io_operations: total_requests as u64,
        };

        // Estimate concurrent capacity based on performance
        let concurrent_capacity = if achieved_latency_ms <= self.config.target_latency_ms * 1.2 {
            self.config.concurrent_requests
        } else {
            (self.config.concurrent_requests as f64 * self.config.target_latency_ms / achieved_latency_ms) as usize
        };

        Ok(RealTimeBenchmarkResult {
            name: self.config.name.clone(),
            achieved_latency_ms,
            latency_variance,
            dropped_requests,
            successful_requests: total_successful,
            concurrent_capacity,
            resource_utilization,
        })
    }
}

pub struct ThroughputBenchmark {
    target_throughput: f64, // requests per second
    ramp_up_duration: Duration,
    sustained_duration: Duration,
    code_samples: Vec<CodeSample>,
}

impl ThroughputBenchmark {
    pub fn new(target_throughput: f64, sustained_duration: Duration) -> Self {
        Self {
            target_throughput,
            ramp_up_duration: Duration::from_secs(30), // 30 second ramp-up
            sustained_duration,
            code_samples: Vec::new(),
        }
    }

    pub fn add_code_samples(&mut self, samples: Vec<CodeSample>) {
        self.code_samples.extend(samples);
    }

    pub async fn run(&self, model: &CodeEmbeddingModel) -> Result<ThroughputBenchmarkResult, EmbeddingError> {
        println!("  🔥 Throughput test: target {}rps for {}s", 
                 self.target_throughput, self.sustained_duration.as_secs());

        let mut throughput_measurements = Vec::new();
        let mut latency_measurements = Vec::new();
        let mut error_count = 0;

        // Ramp-up phase
        println!("    Ramping up over {}s...", self.ramp_up_duration.as_secs());
        let ramp_steps = 10;
        let ramp_step_duration = self.ramp_up_duration / ramp_steps;
        
        for step in 1..=ramp_steps {
            let step_throughput = (step as f64 / ramp_steps as f64) * self.target_throughput;
            let step_result = self.run_throughput_step(model, step_throughput, ramp_step_duration).await?;
            throughput_measurements.push(step_result.achieved_throughput);
            latency_measurements.extend(step_result.latencies);
            error_count += step_result.errors;
        }

        // Sustained phase  
        println!("    Sustained load at {}rps for {}s...", 
                 self.target_throughput, self.sustained_duration.as_secs());
        let sustained_result = self.run_throughput_step(model, self.target_throughput, self.sustained_duration).await?;
        
        let sustained_throughput = sustained_result.achieved_throughput;
        latency_measurements.extend(sustained_result.latencies);
        error_count += sustained_result.errors;

        // Compute statistics
        let avg_throughput = throughput_measurements.iter().sum::<f64>() / throughput_measurements.len() as f64;
        let peak_throughput = throughput_measurements.iter().fold(0.0f64, |acc, &x| acc.max(x));
        
        let avg_latency = if !latency_measurements.is_empty() {
            latency_measurements.iter().sum::<f64>() / latency_measurements.len() as f64
        } else {
            0.0
        };

        let mut sorted_latencies = latency_measurements.clone();
        sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95_latency = if !sorted_latencies.is_empty() {
            let idx = (sorted_latencies.len() as f64 * 0.95) as usize;
            sorted_latencies[idx.min(sorted_latencies.len() - 1)]
        } else {
            0.0
        };

        let throughput_efficiency = sustained_throughput / self.target_throughput;
        let total_requests = throughput_measurements.len() * 100 + (sustained_result.requests as usize); // Estimate
        let error_rate = error_count as f64 / total_requests as f64;

        Ok(ThroughputBenchmarkResult {
            target_throughput: self.target_throughput,
            achieved_throughput: sustained_throughput,
            peak_throughput,
            avg_throughput,
            throughput_efficiency,
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            error_rate,
            sustained_duration: self.sustained_duration,
        })
    }

    async fn run_throughput_step(&self, model: &CodeEmbeddingModel, target_rps: f64, duration: Duration) -> Result<ThroughputStepResult, EmbeddingError> {
        let start_time = Instant::now();
        let end_time = start_time + duration;
        let request_interval = Duration::from_millis((1000.0 / target_rps) as u64);
        
        let mut latencies = Vec::new();
        let mut successful_requests = 0;
        let mut errors = 0;
        let mut interval_timer = interval(request_interval);
        
        while Instant::now() < end_time {
            interval_timer.tick().await;
            
            if let Some(sample) = self.code_samples.first() {
                let request_start = Instant::now();
                
                match model.embed_code(&sample.content, sample.language).await {
                    Ok(_) => {
                        let latency = request_start.elapsed().as_millis() as f64;
                        latencies.push(latency);
                        successful_requests += 1;
                    }
                    Err(_) => {
                        errors += 1;
                    }
                }
            }
        }

        let actual_duration = start_time.elapsed().as_secs_f64();
        let achieved_throughput = successful_requests as f64 / actual_duration;

        Ok(ThroughputStepResult {
            achieved_throughput,
            latencies,
            errors,
            requests: successful_requests + errors,
        })
    }
}

pub struct LatencyBenchmark {
    percentiles: Vec<f64>, // e.g., [0.5, 0.95, 0.99]
    sample_size: usize,
    code_samples: Vec<CodeSample>,
}

impl LatencyBenchmark {
    pub fn new(sample_size: usize) -> Self {
        Self {
            percentiles: vec![0.5, 0.90, 0.95, 0.99, 0.999],
            sample_size,
            code_samples: Vec::new(),
        }
    }

    pub fn add_code_samples(&mut self, samples: Vec<CodeSample>) {
        self.code_samples.extend(samples);
    }

    pub async fn run(&self, model: &CodeEmbeddingModel) -> Result<LatencyBenchmarkResult, EmbeddingError> {
        println!("  ⏱️  Latency benchmark: {} samples across {} percentiles", 
                 self.sample_size, self.percentiles.len());

        let mut all_latencies = Vec::with_capacity(self.sample_size);
        let mut errors = 0;

        for i in 0..self.sample_size {
            if i % (self.sample_size / 10).max(1) == 0 {
                println!("    Progress: {}/{}  ({:.1}%)", 
                        i, self.sample_size, 
                        (i as f64 / self.sample_size as f64) * 100.0);
            }

            let sample = &self.code_samples[i % self.code_samples.len()];
            let start_time = Instant::now();
            
            match model.embed_code(&sample.content, sample.language).await {
                Ok(_) => {
                    let latency = start_time.elapsed().as_millis() as f64;
                    all_latencies.push(latency);
                }
                Err(_) => {
                    errors += 1;
                }
            }
        }

        if all_latencies.is_empty() {
            return Err(EmbeddingError::InferenceError("No successful latency measurements".to_string()));
        }

        // Sort latencies for percentile calculation
        all_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Calculate percentiles
        let mut percentile_results = Vec::new();
        for &p in &self.percentiles {
            let idx = ((all_latencies.len() - 1) as f64 * p) as usize;
            let latency = all_latencies[idx];
            percentile_results.push(LatencyPercentile {
                percentile: p,
                latency_ms: latency,
            });
        }

        let avg_latency = all_latencies.iter().sum::<f64>() / all_latencies.len() as f64;
        let min_latency = all_latencies[0];
        let max_latency = all_latencies[all_latencies.len() - 1];
        let error_rate = errors as f64 / self.sample_size as f64;

        Ok(LatencyBenchmarkResult {
            sample_size: self.sample_size,
            successful_samples: all_latencies.len(),
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            percentiles: percentile_results,
            error_rate,
        })
    }
}

// Supporting types
#[derive(Debug, Clone)]
struct TestWorkload {
    items: Vec<WorkItem>,
}

#[derive(Debug, Clone)]
struct WorkItem {
    code: String,
    language: CodeLanguage,
    expected_latency_ms: f64,
    priority: Priority,
}

#[derive(Debug, Clone)]
enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone)]
pub struct CodeSample {
    pub content: String,
    pub language: CodeLanguage,
    pub lines: usize,
    pub complexity: u32,
}

#[derive(Debug, Clone)]
struct WorkerStats {
    worker_id: usize,
    successful_requests: usize,
    failed_requests: usize,
    timeout_requests: usize,
    within_sla_requests: usize,
    total_latency: Duration,
    max_latency: Duration,
}

impl WorkerStats {
    fn new(worker_id: usize) -> Self {
        Self {
            worker_id,
            successful_requests: 0,
            failed_requests: 0,
            timeout_requests: 0,
            within_sla_requests: 0,
            total_latency: Duration::ZERO,
            max_latency: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
struct ThroughputStepResult {
    achieved_throughput: f64,
    latencies: Vec<f64>,
    errors: usize,
    requests: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputBenchmarkResult {
    pub target_throughput: f64,
    pub achieved_throughput: f64,
    pub peak_throughput: f64,
    pub avg_throughput: f64,
    pub throughput_efficiency: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub error_rate: f64,
    pub sustained_duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBenchmarkResult {
    pub sample_size: usize,
    pub successful_samples: usize,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub percentiles: Vec<LatencyPercentile>,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyPercentile {
    pub percentile: f64,
    pub latency_ms: f64,
}

// Metrics collector for real-time monitoring
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    // Would contain actual metrics collection logic
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn record_success(&self, _latency: Duration) {
        // Record successful request metrics
    }

    pub async fn record_error(&self, _error_type: &str) {
        // Record error metrics
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}