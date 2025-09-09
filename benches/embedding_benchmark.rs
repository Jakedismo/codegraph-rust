use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// Mock embedding system for benchmarking
struct MockEmbeddingSystem;

impl MockEmbeddingSystem {
    fn new() -> Self {
        Self
    }

    fn embed_code(&self, code: &str) -> Vec<f32> {
        // Mock embedding generation - in real implementation this would
        // call the actual embedding system
        let mut embedding = Vec::with_capacity(768);
        let hash = code.len() as f32;
        for i in 0..768 {
            embedding.push((hash * (i as f32 + 1.0)).sin());
        }
        embedding
    }

    fn batch_embed(&self, codes: Vec<&str>) -> Vec<Vec<f32>> {
        codes.into_iter().map(|code| self.embed_code(code)).collect()
    }
}

fn generate_code_samples(size: usize) -> Vec<String> {
    (0..size)
        .map(|i| {
            format!(
                r#"
                fn function_{}() -> i32 {{
                    let mut sum = 0;
                    for i in 0..{} {{
                        sum += i * 2 + {};
                        if sum > 1000 {{
                            break;
                        }}
                    }}
                    sum
                }}
                "#,
                i,
                i * 10 + 50,
                i
            )
        })
        .collect()
}

fn benchmark_single_embedding(c: &mut Criterion) {
    let embedding_system = MockEmbeddingSystem::new();
    let code_samples = generate_code_samples(1);
    let sample_code = &code_samples[0];

    c.bench_function("embed_single", |b| {
        b.iter(|| {
            embedding_system.embed_code(black_box(sample_code))
        })
    });
}

fn benchmark_batch_embedding(c: &mut Criterion) {
    let embedding_system = MockEmbeddingSystem::new();
    let mut group = c.benchmark_group("batch_embedding");

    for batch_size in [1, 10, 50, 100, 500].iter() {
        let code_samples = generate_code_samples(*batch_size);
        let code_refs: Vec<&str> = code_samples.iter().map(|s| s.as_str()).collect();

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    embedding_system.batch_embed(black_box(code_refs.clone()))
                })
            },
        );
    }
    group.finish();
}

fn benchmark_embedding_by_code_size(c: &mut Criterion) {
    let embedding_system = MockEmbeddingSystem::new();
    let mut group = c.benchmark_group("embedding_by_code_size");

    // Test with different code sizes
    let code_sizes = [100, 500, 1000, 5000, 10000]; // characters

    for &size in code_sizes.iter() {
        let code = "x".repeat(size);
        
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("code_size", size),
            &code,
            |b, code| {
                b.iter(|| {
                    embedding_system.embed_code(black_box(code))
                })
            },
        );
    }
    group.finish();
}

fn benchmark_concurrent_embedding(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_embedding");
    group.measurement_time(Duration::from_secs(10));

    for thread_count in [1, 2, 4, 8].iter() {
        let code_samples = generate_code_samples(100);
        
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    use std::sync::Arc;
                    use std::thread;
                    
                    let embedding_system = Arc::new(MockEmbeddingSystem::new());
                    let samples_per_thread = code_samples.len() / threads;
                    
                    let handles: Vec<_> = (0..threads)
                        .map(|i| {
                            let system = Arc::clone(&embedding_system);
                            let start = i * samples_per_thread;
                            let end = if i == threads - 1 {
                                code_samples.len()
                            } else {
                                start + samples_per_thread
                            };
                            let samples = code_samples[start..end].to_vec();
                            
                            thread::spawn(move || {
                                for sample in samples {
                                    black_box(system.embed_code(&sample));
                                }
                            })
                        })
                        .collect();
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }
    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    
    // Benchmark memory allocation patterns
    group.bench_function("embedding_allocation", |b| {
        b.iter(|| {
            let embedding_system = MockEmbeddingSystem::new();
            let code = "fn test() { println!(\"Hello, world!\"); }";
            black_box(embedding_system.embed_code(code))
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_single_embedding,
    benchmark_batch_embedding,
    benchmark_embedding_by_code_size,
    benchmark_concurrent_embedding,
    benchmark_memory_usage
);
criterion_main!(benches);