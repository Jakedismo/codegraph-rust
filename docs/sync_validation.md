## Sync Validation: Incremental Update Testing

This standalone harness validates concurrency, propagation performance, consistency, and edge cases for incremental updates without requiring the full workspace to compile.

It lives under `high_perf_test/` and can be run directly.

### Testing Context Summary

- Project Type: Rust workspace (graph + API + vector + parser)
- Technology Stack: Rust (tokio, dashmap, parking_lot), Axum, async-graphql; separate in-memory test harness
- Key Requirements:
  - Concurrency testing for parallel update scenarios
  - Propagation performance < 1s under load
  - Consistency checks for distributed/transactional updates
  - Edge case handling for complex overlapping updates
- Testing Constraints:
  - Core workspace currently has compilation issues; the harness is isolated and self-contained

### How to Run

- Using Makefile:
  - `make sync-validate`

- Direct cargo:
  - `cargo run --manifest-path high_perf_test/Cargo.toml`

### What It Does

- Concurrency Stress: Spawns multiple concurrent selective updates over overlapping regions, reporting ops/sec.
- Propagation Benchmark: Broadcasts update events to many subscribers and asserts worst-case latency < 1s.
- Consistency Checks: Simulated transactional manager (Serializable isolation) executes conflicting writes concurrently; ensures progress and detects conflicts.
- Edge Cases: Validates empty updates and racing overlapping updates produce consistent outcomes.

### Parameters (defaults in `main.rs`)

- Concurrency Stress: 16 workers x 100 rounds.
- Propagation: 200 subscribers, 8 publishers, 1 event/publisher.
- Consistency: 12 concurrent workers, tx size 32.

### Notes

- The harness uses an in-memory store (`parking_lot::RwLock`) and `tokio::sync::broadcast` for event propagation.
- It is designed to validate behavior and targets in isolation while parts of the main workspace are stabilized.

