## Development Context

- Feature: Lock-free concurrent data structures for CodeGraph
- Technical Stack: Rust 1.75+, atomics (Acquire/Release), crossbeam (queue, utils, skiplist), arc-swap (RCU), tokio (tests), loom (memory ordering tests)
- Constraints: Zero global locks on hot paths; bounded memory; no blocking ops in async; integrate cleanly with existing workspace; avoid breaking existing crates
- Success Criteria: 10,000+ QPS throughput for queueing pipelines; lock-free graph adjacency operations; memory ordering validated with targeted loom tests; unit tests and examples included

## Library Insights (Phase 1)

- crossbeam-queue::ArrayQueue
  - Bounded MPMC, lock-free; `push`, `pop` return errors on full/empty
  - Very fast for many-producer/consumer scenarios; backoff recommended under contention

- crossbeam-skiplist::SkipMap
  - Lock-free concurrent map with ordered keys; fast reads/writes without global locks
  - Suited for per-node adjacency indices and node storage

- arc-swap::ArcSwap
  - Atomic `Arc<T>` with lock-free reads; `rcu` for copy-on-write updates
  - Ensures correct Acquire/Release semantics for pointer publication

- loom
  - Deterministic exploration of interleavings for atomic algorithms
  - Use feature-gated tests for SPSC memory ordering sanity checks

## Best Practices (Phase 2)

- Prefer wait-free SPSC for hottest paths (producer-consumer pipelines). Use Acquire/Release for head/tail publishing.
- For MPMC, use proven lock-free bounded queues (ArrayQueue) and prefer backoff/yield on contention to reduce cache-line ping-pong.
- For lock-free graph ops, keep reads trivially lock-free (ArcSwap + clone-on-write); writers retry via RCU CAS.
- Validate invariants with unit tests; use loom selectively to keep test times in check.

