# codegraph-graph — I/O Batching Enhancements

This crate now includes an adaptive I/O batching layer to reduce syscall overhead and improve end-to-end latency for graph storage backed by RocksDB.

## Highlights

- Read coalescing for `get_node` via a background aggregator that batches requests within a short coalescing window.
- Adaptive write batching with dynamic thresholds tuned by EWMA of flush latencies and a time-based flush fallback.
- Sequential access optimizations for prefix scans with `iterator_cf_opt` and 2MB readahead.
- Conservative defaults that preserve safety (WAL enabled; `sync=false` for throughput). 

## Key Components

- `io_batcher::ReadCoalescer`: Groups concurrent node reads and serves them in batches.
- `io_batcher::WriteBatchOptimizer`: Tunes write-batch operation thresholds to meet latency targets.
- `storage::HighPerformanceRocksDbStorage`: Integrates batching and sequential-scan optimizations.

## Configuration

`BatchingConfig` (internal default) controls:
- `max_read_batch` (default 256)
- `read_coalesce_delay` (default 300µs)
- `initial_write_ops_threshold` (default 1000)
- `min_write_ops_threshold` / `max_write_ops_threshold`
- `write_flush_interval` (default 5ms)
- `target_flush_latency_ms` (default 2.0ms)

These defaults are tuned for low latency under mixed workloads and can be adjusted if a configuration surface is later exposed.

