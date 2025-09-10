use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codegraph_cache::{CacheOptimizedHashMap, CacheEntriesSoA, PaddedAtomicUsize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use rayon::prelude::*;

const CACHE_SIZES: &[usize] = &[100, 1000, 10000];
const THREAD_COUNTS: &[usize] = &[1, 2, 4, 8];

fn generate_test_data(size: usize) -> Vec<(String, i64)> {
    (0..size)
        .map(|i| (format!("key_{}", i), i as i64))
        .collect()
}

fn bench_traditional_hashmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("traditional_hashmap");
    
    for size in CACHE_SIZES {
        let data = generate_test_data(*size);
        
        group.bench_with_input(
            BenchmarkId::new("insert", size),
            size,
            |b, _| {
                b.iter(|| {
                    let mut map = HashMap::new();
                    for (key, value) in &data {
                        map.insert(key.clone(), *value);
                    }
                    black_box(map);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("lookup", size),
            size,
            |b, _| {
                let mut map = HashMap::new();
                for (key, value) in &data {
                    map.insert(key.clone(), *value);
                }
                
                b.iter(|| {
                    for (key, _) in &data {
                        black_box(map.get(key));
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn bench_cache_optimized_hashmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimized_hashmap");
    
    for size in CACHE_SIZES {
        let data = generate_test_data(*size);
        
        group.bench_with_input(
            BenchmarkId::new("insert", size),
            size,
            |b, _| {
                b.iter(|| {
                    let map = CacheOptimizedHashMap::new(Some(4));
                    for (key, value) in &data {
                        map.insert(key.clone(), *value, 8);
                    }
                    black_box(map);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("lookup", size),
            size,
            |b, _| {
                let map = CacheOptimizedHashMap::new(Some(4));
                for (key, value) in &data {
                    map.insert(key.clone(), *value, 8);
                }
                
                b.iter(|| {
                    for (key, _) in &data {
                        black_box(map.get(key));
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn bench_structure_of_arrays(c: &mut Criterion) {
    let mut group = c.benchmark_group("structure_of_arrays");
    
    for size in CACHE_SIZES {
        let data = generate_test_data(*size);
        
        group.bench_with_input(
            BenchmarkId::new("insert", size),
            size,
            |b, _| {
                b.iter(|| {
                    let mut soa = CacheEntriesSoA::new(*size);
                    for (key, value) in &data {
                        soa.insert(key.clone(), *value, 8);
                    }
                    black_box(soa);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("lookup", size),
            size,
            |b, _| {
                let mut soa = CacheEntriesSoA::new(*size);
                for (key, value) in &data {
                    soa.insert(key.clone(), *value, 8);
                }
                
                b.iter(|| {
                    for (key, _) in &data {
                        black_box(soa.get(key));
                    }
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("sequential_scan", size),
            size,
            |b, _| {
                let mut soa = CacheEntriesSoA::new(*size);
                for (key, value) in &data {
                    soa.insert(key.clone(), *value, 8);
                }
                
                b.iter(|| {
                    // Simulate sequential processing of cache entries
                    let mut sum = 0i64;
                    for (key, _) in &data {
                        if let Some(value) = soa.get(key) {
                            sum += value;
                        }
                    }
                    black_box(sum);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_false_sharing_elimination(c: &mut Criterion) {
    let mut group = c.benchmark_group("false_sharing");
    
    for thread_count in THREAD_COUNTS {
        // Traditional unpadded counters
        group.bench_with_input(
            BenchmarkId::new("unpadded_counters", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Arc<Mutex<usize>>> = (0..threads)
                        .map(|_| Arc::new(Mutex::new(0)))
                        .collect();
                    
                    let handles: Vec<_> = counters
                        .iter()
                        .map(|counter| {
                            let counter = counter.clone();
                            thread::spawn(move || {
                                for _ in 0..10000 {
                                    let mut guard = counter.lock().unwrap();
                                    *guard += 1;
                                }
                            })
                        })
                        .collect();
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    
                    let sum: usize = counters.iter()
                        .map(|c| *c.lock().unwrap())
                        .sum();
                    black_box(sum);
                });
            },
        );
        
        // Padded counters to prevent false sharing
        group.bench_with_input(
            BenchmarkId::new("padded_counters", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let counters: Vec<Arc<PaddedAtomicUsize>> = (0..threads)
                        .map(|_| Arc::new(PaddedAtomicUsize::new(0)))
                        .collect();
                    
                    let handles: Vec<_> = counters
                        .iter()
                        .map(|counter| {
                            let counter = counter.clone();
                            thread::spawn(move || {
                                for _ in 0..10000 {
                                    counter.fetch_add(1, Ordering::Relaxed);
                                }
                            })
                        })
                        .collect();
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                    
                    let sum: usize = counters.iter()
                        .map(|c| c.load(Ordering::Relaxed))
                        .sum();
                    black_box(sum);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_parallel_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_access");
    
    let data = generate_test_data(10000);
    
    // Traditional HashMap with single lock
    group.bench_function("single_lock_hashmap", |b| {
        b.iter(|| {
            let map = Arc::new(Mutex::new(HashMap::new()));
            
            // Insert data
            for (key, value) in &data {
                map.lock().unwrap().insert(key.clone(), *value);
            }
            
            // Parallel lookups
            data.par_iter().for_each(|(key, _)| {
                black_box(map.lock().unwrap().get(key));
            });
        });
    });
    
    // Cache-optimized sharded HashMap
    group.bench_function("sharded_hashmap", |b| {
        b.iter(|| {
            let map = CacheOptimizedHashMap::new(Some(8));
            
            // Insert data
            for (key, value) in &data {
                map.insert(key.clone(), *value, 8);
            }
            
            // Parallel lookups
            data.par_iter().for_each(|(key, _)| {
                black_box(map.get(key));
            });
        });
    });
    
    group.finish();
}

fn bench_cache_line_utilization(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_line_utilization");
    
    // Benchmark row-major vs column-major access patterns
    let size = 1000;
    let matrix: Vec<Vec<i32>> = (0..size)
        .map(|i| (0..size).map(|j| (i * size + j) as i32).collect())
        .collect();
    
    group.bench_function("row_major_access", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for row in &matrix {
                for &value in row {
                    sum += value as i64;
                }
            }
            black_box(sum);
        });
    });
    
    group.bench_function("column_major_access", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for j in 0..size {
                for i in 0..size {
                    sum += matrix[i][j] as i64;
                }
            }
            black_box(sum);
        });
    });
    
    // Flat array with row-major access (more cache-friendly)
    let flat_matrix: Vec<i32> = (0..size * size).map(|i| i as i32).collect();
    
    group.bench_function("flat_array_sequential", |b| {
        b.iter(|| {
            let sum: i64 = flat_matrix.iter().map(|&x| x as i64).sum();
            black_box(sum);
        });
    });
    
    group.bench_function("flat_array_strided", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            // Access every 64th element to stress cache
            for i in (0..flat_matrix.len()).step_by(64) {
                sum += flat_matrix[i] as i64;
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_prefetch_hints(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_hints");
    
    let size = 10000;
    let data: Vec<Box<i32>> = (0..size).map(|i| Box::new(i as i32)).collect();
    
    group.bench_function("without_prefetch", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for item in &data {
                sum += **item as i64;
            }
            black_box(sum);
        });
    });
    
    group.bench_function("with_prefetch", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for (i, item) in data.iter().enumerate() {
                // Prefetch next few items
                if i + 4 < data.len() {
                    unsafe {
                        std::ptr::prefetch_read_data(
                            data[i + 4].as_ref() as *const i32 as *const u8,
                            1
                        );
                    }
                }
                sum += **item as i64;
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_traditional_hashmap,
    bench_cache_optimized_hashmap,
    bench_structure_of_arrays,
    bench_false_sharing_elimination,
    bench_parallel_access_patterns,
    bench_cache_line_utilization,
    bench_prefetch_hints
);

criterion_main!(benches);