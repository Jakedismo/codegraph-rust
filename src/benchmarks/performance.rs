use super::*;
use crate::embedding::{CodeEmbeddingModel, EmbeddingError};
use crate::languages::CodeInput;

use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time::timeout;

pub struct PerformanceBenchmark {
    config: PerformanceBenchmarkConfig,
    latency_measurements: Vec<Duration>,
    throughput_measurements: Vec<f64>,
    memory_measurements: Vec<usize>,
    cache_stats: Vec<(u64, u64)>, // (hits, total)
}

impl PerformanceBenchmark {
    pub fn new(config: PerformanceBenchmarkConfig) -> Self {
        Self {
            config,
            latency_measurements: Vec::new(),
            throughput_measurements: Vec::new(),
            memory_measurements: Vec::new(),
            cache_stats: Vec::new(),
        }
    }

    pub async fn run(&mut self, model: &mut CodeEmbeddingModel) -> Result<PerformanceBenchmarkResult, EmbeddingError> {
        println!("  🔥 Warming up with {} iterations...", self.config.warmup_iterations);
        
        // Warmup phase
        for _ in 0..self.config.warmup_iterations {
            for test_case in &self.config.test_cases {
                let _ = model.embed_code(&test_case.code_content, test_case.language).await;
            }
        }

        println!("  📊 Running {} benchmark iterations...", self.config.iterations);
        
        let mut error_count = 0;
        let mut language_performances: HashMap<CodeLanguage, Vec<f64>> = HashMap::new();

        // Main benchmark phase
        for iteration in 0..self.config.iterations {
            if iteration % (self.config.iterations / 10).max(1) == 0 {
                println!("    Progress: {}/{}  ({:.1}%)", 
                    iteration, self.config.iterations, 
                    (iteration as f64 / self.config.iterations as f64) * 100.0);
            }

            for test_case in &self.config.test_cases {
                let start_memory = self.get_memory_usage();
                let start_time = Instant::now();
                
                match model.embed_code(&test_case.code_content, test_case.language).await {
                    Ok(_embeddings) => {
                        let latency = start_time.elapsed();
                        self.latency_measurements.push(latency);
                        
                        // Record per-language performance
                        language_performances.entry(test_case.language)
                            .or_insert_with(Vec::new)
                            .push(latency.as_millis() as f64);
                    }
                    Err(_) => {
                        error_count += 1;
                    }
                }
                
                let end_memory = self.get_memory_usage();
                self.memory_measurements.push(end_memory.saturating_sub(start_memory));
                
                // Collect cache stats
                let (hits, total, _) = model.cache_stats().await;
                self.cache_stats.push((hits, total));
            }

            // Throughput measurement for this iteration
            let iteration_start = Instant::now();
            let mut ops_in_iteration = 0;

            for test_case in &self.config.test_cases {
                if model.embed_code(&test_case.code_content, test_case.language).await.is_ok() {
                    ops_in_iteration += 1;
                }
            }

            let iteration_time = iteration_start.elapsed();
            if iteration_time.as_secs_f64() > 0.0 {
                let throughput = ops_in_iteration as f64 / iteration_time.as_secs_f64();
                self.throughput_measurements.push(throughput);
            }
        }

        self.compute_result(error_count, language_performances)
    }

    fn compute_result(&self, error_count: usize, language_performances: HashMap<CodeLanguage, Vec<f64>>) -> Result<PerformanceBenchmarkResult, EmbeddingError> {
        if self.latency_measurements.is_empty() {
            return Err(EmbeddingError::InferenceError("No successful measurements".to_string()));
        }

        // Latency statistics
        let mut sorted_latencies: Vec<f64> = self.latency_measurements
            .iter()
            .map(|d| d.as_millis() as f64)
            .collect();
        sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_latency_ms = sorted_latencies.iter().sum::<f64>() / sorted_latencies.len() as f64;
        let p95_latency_ms = self.percentile(&sorted_latencies, 0.95);
        let p99_latency_ms = self.percentile(&sorted_latencies, 0.99);

        // Throughput statistics
        let avg_throughput = if !self.throughput_measurements.is_empty() {
            self.throughput_measurements.iter().sum::<f64>() / self.throughput_measurements.len() as f64
        } else {
            0.0
        };

        // Memory statistics
        let peak_memory_mb = if !self.memory_measurements.is_empty() {
            *self.memory_measurements.iter().max().unwrap() as f64 / 1024.0 / 1024.0
        } else {
            0.0
        };

        // Cache statistics
        let (total_hits, total_requests) = self.cache_stats.iter().fold((0, 0), |(h, r), &(hits, reqs)| {
            (h + hits, r + reqs)
        });
        let cache_hit_rate = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        // Error rate
        let total_operations = self.config.iterations * self.config.test_cases.len();
        let error_rate = error_count as f64 / total_operations as f64;

        // Language breakdown
        let mut language_breakdown = HashMap::new();
        for (language, latencies) in language_performances {
            if !latencies.is_empty() {
                let avg_lang_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
                let lang_throughput = if avg_lang_latency > 0.0 {
                    1000.0 / avg_lang_latency
                } else {
                    0.0
                };
                
                language_breakdown.insert(language, LanguagePerformance {
                    avg_latency_ms: avg_lang_latency,
                    throughput_ops_per_sec: lang_throughput,
                    embedding_quality: 0.85, // Would measure actual quality
                    parse_success_rate: 1.0 - (error_rate / self.config.languages.len() as f64),
                });
            }
        }

        Ok(PerformanceBenchmarkResult {
            name: self.config.name.clone(),
            avg_latency_ms,
            p95_latency_ms,
            p99_latency_ms,
            throughput_ops_per_sec: avg_throughput,
            memory_peak_mb: peak_memory_mb,
            cache_hit_rate,
            error_rate,
            language_breakdown,
        })
    }

    fn percentile(&self, sorted_values: &[f64], p: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        
        let index = ((sorted_values.len() - 1) as f64 * p) as usize;
        sorted_values[index]
    }

    fn get_memory_usage(&self) -> usize {
        // In a real implementation, this would use OS-specific APIs
        // For now, return a placeholder
        std::mem::size_of::<Self>()
    }
}

pub struct StressBenchmark {
    concurrent_requests: usize,
    duration: Duration,
    ramp_up_time: Duration,
    test_cases: Vec<TestCase>,
}

impl StressBenchmark {
    pub fn new(concurrent_requests: usize, duration_seconds: u64, test_cases: Vec<TestCase>) -> Self {
        Self {
            concurrent_requests,
            duration: Duration::from_secs(duration_seconds),
            ramp_up_time: Duration::from_secs(duration_seconds / 10), // 10% of total time for ramp-up
            test_cases,
        }
    }

    pub async fn run(&self, model: &CodeEmbeddingModel) -> Result<StressBenchmarkResult, EmbeddingError> {
        println!("  🚀 Starting stress test with {} concurrent requests for {}s", 
                 self.concurrent_requests, self.duration.as_secs());

        let start_time = Instant::now();
        let mut handles = Vec::new();
        let mut results = Vec::new();

        // Create semaphore to limit concurrent requests
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(self.concurrent_requests));
        
        // Spawn concurrent tasks
        for i in 0..self.concurrent_requests {
            let model_clone = model.clone(); // Assuming Clone is implemented
            let test_cases = self.test_cases.clone();
            let duration = self.duration;
            let semaphore_clone = semaphore.clone();
            let ramp_delay = Duration::from_millis((i as u64 * self.ramp_up_time.as_millis() as u64) / self.concurrent_requests as u64);

            let handle = tokio::spawn(async move {
                tokio::time::sleep(ramp_delay).await;
                
                let _permit = semaphore_clone.acquire().await.unwrap();
                let mut local_results = StressWorkerResult {
                    successful_requests: 0,
                    failed_requests: 0,
                    total_latency_ms: 0.0,
                    max_latency_ms: 0.0,
                };

                let worker_start = Instant::now();
                while worker_start.elapsed() < duration {
                    for test_case in &test_cases {
                        let request_start = Instant::now();
                        
                        match timeout(Duration::from_secs(30), 
                                    model_clone.embed_code(&test_case.code_content, test_case.language)).await {
                            Ok(Ok(_)) => {
                                let latency = request_start.elapsed().as_millis() as f64;
                                local_results.successful_requests += 1;
                                local_results.total_latency_ms += latency;
                                local_results.max_latency_ms = local_results.max_latency_ms.max(latency);
                            }
                            _ => {
                                local_results.failed_requests += 1;
                            }
                        }

                        if worker_start.elapsed() >= duration {
                            break;
                        }
                    }
                }

                local_results
            });

            handles.push(handle);
        }

        // Collect results from all workers
        for handle in handles {
            match handle.await {
                Ok(worker_result) => results.push(worker_result),
                Err(_) => {
                    // Worker panicked, count as all failed
                    results.push(StressWorkerResult {
                        successful_requests: 0,
                        failed_requests: self.test_cases.len(),
                        total_latency_ms: 0.0,
                        max_latency_ms: 0.0,
                    });
                }
            }
        }

        self.aggregate_stress_results(results, start_time.elapsed())
    }

    fn aggregate_stress_results(&self, results: Vec<StressWorkerResult>, total_duration: Duration) -> Result<StressBenchmarkResult, EmbeddingError> {
        let total_successful: usize = results.iter().map(|r| r.successful_requests).sum();
        let total_failed: usize = results.iter().map(|r| r.failed_requests).sum();
        let total_latency: f64 = results.iter().map(|r| r.total_latency_ms).sum();
        let max_latency: f64 = results.iter().map(|r| r.max_latency_ms).fold(0.0, |acc, &x| acc.max(x));

        let avg_latency = if total_successful > 0 {
            total_latency / total_successful as f64
        } else {
            0.0
        };

        let throughput = total_successful as f64 / total_duration.as_secs_f64();
        let error_rate = if total_successful + total_failed > 0 {
            total_failed as f64 / (total_successful + total_failed) as f64
        } else {
            1.0
        };

        Ok(StressBenchmarkResult {
            concurrent_requests: self.concurrent_requests,
            duration_seconds: total_duration.as_secs(),
            total_requests: total_successful + total_failed,
            successful_requests: total_successful,
            failed_requests: total_failed,
            avg_latency_ms: avg_latency,
            max_latency_ms: max_latency,
            throughput_rps: throughput,
            error_rate,
            resource_exhaustion: error_rate > 0.1, // Consider >10% error rate as resource exhaustion
        })
    }
}

#[derive(Debug, Clone)]
pub struct StressWorkerResult {
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub total_latency_ms: f64,
    pub max_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressBenchmarkResult {
    pub concurrent_requests: usize,
    pub duration_seconds: u64,
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub avg_latency_ms: f64,
    pub max_latency_ms: f64,
    pub throughput_rps: f64,
    pub error_rate: f64,
    pub resource_exhaustion: bool,
}

pub struct LoadTestBenchmark {
    ramp_up_pattern: RampUpPattern,
    peak_load_duration: Duration,
    test_scenarios: Vec<LoadTestScenario>,
}

#[derive(Debug, Clone)]
pub enum RampUpPattern {
    Linear(Duration),
    Exponential(Duration, f64), // duration, multiplier
    Step(Duration, usize),      // duration, steps
}

#[derive(Debug, Clone)]
pub struct LoadTestScenario {
    pub name: String,
    pub concurrent_users: usize,
    pub requests_per_user: usize,
    pub think_time_ms: u64,
    pub test_cases: Vec<TestCase>,
}

impl LoadTestBenchmark {
    pub fn new(ramp_up_pattern: RampUpPattern, peak_load_duration: Duration) -> Self {
        Self {
            ramp_up_pattern,
            peak_load_duration,
            test_scenarios: Vec::new(),
        }
    }

    pub fn add_scenario(&mut self, scenario: LoadTestScenario) {
        self.test_scenarios.push(scenario);
    }

    pub async fn run(&self, model: &CodeEmbeddingModel) -> Result<LoadTestResult, EmbeddingError> {
        let mut scenario_results = Vec::new();

        for scenario in &self.test_scenarios {
            println!("  📈 Running load test scenario: {}", scenario.name);
            let result = self.run_scenario(scenario, model).await?;
            scenario_results.push(result);
        }

        Ok(LoadTestResult {
            scenarios: scenario_results,
            total_duration: self.calculate_total_duration(),
            peak_concurrent_users: self.test_scenarios.iter().map(|s| s.concurrent_users).sum(),
        })
    }

    async fn run_scenario(&self, scenario: &LoadTestScenario, model: &CodeEmbeddingModel) -> Result<ScenarioResult, EmbeddingError> {
        // Implementation would handle ramp-up patterns and load generation
        // For now, simplified implementation
        let stress_test = StressBenchmark::new(
            scenario.concurrent_users,
            self.peak_load_duration.as_secs(),
            scenario.test_cases.clone(),
        );

        let stress_result = stress_test.run(model).await?;

        Ok(ScenarioResult {
            name: scenario.name.clone(),
            concurrent_users: scenario.concurrent_users,
            stress_result,
        })
    }

    fn calculate_total_duration(&self) -> Duration {
        let ramp_duration = match &self.ramp_up_pattern {
            RampUpPattern::Linear(d) => *d,
            RampUpPattern::Exponential(d, _) => *d,
            RampUpPattern::Step(d, _) => *d,
        };
        ramp_duration + self.peak_load_duration
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResult {
    pub scenarios: Vec<ScenarioResult>,
    pub total_duration: Duration,
    pub peak_concurrent_users: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub concurrent_users: usize,
    pub stress_result: StressBenchmarkResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::{CodeLanguage, CodeProcessor};
    use crate::embedding::backend::MockBackend;
    use candle_core::Device;

    #[tokio::test]
    async fn test_performance_benchmark() {
        let backend = Box::new(MockBackend::new());
        let processor = CodeProcessor::new();
        let device = Device::Cpu;
        
        let mut model = CodeEmbeddingModel::new(backend, processor, device);
        
        let config = PerformanceBenchmarkConfig {
            name: "Test Benchmark".to_string(),
            test_cases: vec![
                TestCase {
                    name: "Simple Rust Function".to_string(),
                    code_content: "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                    language: CodeLanguage::Rust,
                    expected_features: None,
                    size_category: CodeSize::Small,
                }
            ],
            iterations: 10,
            warmup_iterations: 2,
            languages: vec![CodeLanguage::Rust],
        };

        let mut benchmark = PerformanceBenchmark::new(config);
        let result = benchmark.run(&mut model).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.avg_latency_ms > 0.0);
        assert!(!result.language_breakdown.is_empty());
    }

    #[test]
    fn test_percentile_calculation() {
        let benchmark = PerformanceBenchmark::new(PerformanceBenchmarkConfig {
            name: "Test".to_string(),
            test_cases: vec![],
            iterations: 0,
            warmup_iterations: 0,
            languages: vec![],
        });

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        assert_eq!(benchmark.percentile(&values, 0.5), 5.0);  // 50th percentile
        assert_eq!(benchmark.percentile(&values, 0.95), 10.0); // 95th percentile
        assert_eq!(benchmark.percentile(&values, 0.0), 1.0);   // 0th percentile
    }
}