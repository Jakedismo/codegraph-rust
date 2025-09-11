use codegraph_api::streaming_handlers::*;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use futures::stream::{self, StreamExt};
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_create_backpressure_stream(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("create_backpressure_stream_1000_items", |b| {
        b.to_async(&rt).iter_batched(
            || {
                // Setup: Create mock search results
                (0..1000)
                    .map(|i| codegraph_vector::SearchResult {
                        node_id: uuid::Uuid::new_v4(),
                        score: 1.0 - (i as f32 / 1000.0),
                    })
                    .collect::<Vec<_>>()
            },
            |results| async move {
                // We can't easily create a proper RwLockReadGuard for benchmarking
                // So we'll benchmark the stream creation logic without the graph dependency
                let stream = stream::iter(results.into_iter().enumerate())
                    .chunks(50)
                    .enumerate()
                    .then(|(batch_idx, batch)| async move {
                        if batch_idx > 0 {
                            tokio::time::sleep(Duration::from_millis(1)).await;
                        }

                        let batch_results: Vec<_> = batch
                            .into_iter()
                            .map(|(idx, search_result)| StreamingSearchResult {
                                node_id: search_result.node_id.to_string(),
                                score: search_result.score,
                                name: format!("Node_{}", idx),
                                node_type: "BenchmarkNode".to_string(),
                                language: "Rust".to_string(),
                                file_path: format!("/bench/node_{}.rs", idx),
                                batch_id: batch_idx,
                                total_processed: idx + 1,
                            })
                            .collect();

                        stream::iter(batch_results)
                    })
                    .flatten();

                // Collect all items to measure complete stream processing
                let items: Vec<_> = stream.collect().await;
                black_box(items)
            },
            BatchSize::SmallInput,
        );
    });
}

fn benchmark_large_dataset_stream(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("large_dataset_stream_10000_items", |b| {
        b.to_async(&rt).iter(|| async {
            let stream = create_large_dataset_stream(100, 1);

            // Take first 1000 items for benchmark
            let items: Vec<_> = stream.take(1000).collect().await;
            black_box(items)
        });
    });
}

fn benchmark_csv_stream_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("csv_stream_500_items", |b| {
        b.to_async(&rt).iter_batched(
            || {
                (0..500)
                    .map(|i| codegraph_vector::SearchResult {
                        node_id: uuid::Uuid::new_v4(),
                        score: 1.0 - (i as f32 / 500.0),
                    })
                    .collect::<Vec<_>>()
            },
            |results| async move {
                let stream = create_csv_stream(results, 25);
                let items: Vec<_> = stream.collect().await;
                black_box(items)
            },
            BatchSize::SmallInput,
        );
    });
}

fn benchmark_optimized_stream(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("optimized_stream_1000_items", |b| {
        b.to_async(&rt).iter_batched(
            || {
                (0..1000)
                    .map(|i| StreamingSearchResult {
                        node_id: uuid::Uuid::new_v4().to_string(),
                        score: 1.0 - (i as f32 / 1000.0),
                        name: format!("OptimizedNode_{}", i),
                        node_type: "Optimized".to_string(),
                        language: "Rust".to_string(),
                        file_path: format!("/opt/node_{}.rs", i),
                        batch_id: i / 50,
                        total_processed: i + 1,
                    })
                    .collect::<Vec<_>>()
            },
            |items| async move {
                let stream = create_optimized_stream(
                    items,
                    50,                       // batch_size
                    Duration::from_millis(1), // throttle_duration
                    10,                       // max_concurrent
                );

                let results: Vec<_> = stream.collect().await;
                black_box(results)
            },
            BatchSize::SmallInput,
        );
    });
}

fn benchmark_stream_different_batch_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_size_comparison");

    for batch_size in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            format!("batch_size_{}", batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter_batched(
                    || {
                        (0..1000)
                            .map(|i| codegraph_vector::SearchResult {
                                node_id: uuid::Uuid::new_v4(),
                                score: 1.0 - (i as f32 / 1000.0),
                            })
                            .collect::<Vec<_>>()
                    },
                    |results| async move {
                        let stream = create_csv_stream(results, batch_size);
                        let items: Vec<_> = stream.collect().await;
                        black_box(items)
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn benchmark_throttling_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("throttling_comparison");

    for throttle_ms in [0, 1, 5, 10].iter() {
        group.bench_with_input(
            format!("throttle_{}ms", throttle_ms),
            throttle_ms,
            |b, &throttle_ms| {
                b.to_async(&rt).iter(|| async move {
                    let stream = create_large_dataset_stream(50, throttle_ms);
                    // Take fewer items when throttling to keep benchmark reasonable
                    let take_count = if throttle_ms == 0 { 1000 } else { 100 };
                    let items: Vec<_> = stream.take(take_count).collect().await;
                    black_box(items)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_create_backpressure_stream,
    benchmark_large_dataset_stream,
    benchmark_csv_stream_creation,
    benchmark_optimized_stream,
    benchmark_stream_different_batch_sizes,
    benchmark_throttling_impact
);

criterion_main!(benches);
