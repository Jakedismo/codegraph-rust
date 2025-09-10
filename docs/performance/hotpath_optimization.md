## Hot Path Optimization Report

Role: hotpath_optimization
Cluster: cpu_optimization
Specialization: Critical path optimization

### Phase 0: Development Context

- Feature: Vector search hot path optimization (query-time + index build clustering)
- Technical Stack: `rust`, `tokio`, `rayon`, `dashmap`, `parking_lot`, optional `faiss`, `ndarray`, `criterion`
- Constraints:
  - Keep interfaces stable (public API unchanged)
  - No heavy new dependencies; avoid nightly
  - Optimize CPU-critical 20% code paths first
  - Maintain thread-safety and async correctness
- Success Criteria:
  - Reduced CPU time along hot paths (distance calc, ranking, mapping)
  - No regressions in results correctness
  - Minimal memory overhead; no extra allocations on hot path

### Phase 1: Library Insights (Context7)

- Tokio: avoid blocking in async; move CPU-heavy tasks to `spawn_blocking` (already done for FAISS search). Prefer work-stealing scheduler defaults. Use `Arc` and lock-free patterns where possible.
- Rayon: beneficial for large parallel workloads; small collections should stay sequential to avoid scheduling overhead.
- DashMap/parking_lot: use short-lived guard scopes; read-mostly patterns with `RwLock` are preferred.
- FAISS: heavy vector ops are native; optimize wrapper overhead (allocation, mapping, copies) around calls.

### Phase 2: Current Best Practices (summary)

- Prefer simple index loops over iterator adapters on hot math paths.
- Use `mul_add` where appropriate for tighter FP loops.
- Avoid unnecessary `sqrt` in comparisons (use squared distances) to reduce FLOPs.
- Pre-allocate buffers with `with_capacity` for known sizes.
- Use parallelism only above a size threshold to avoid overhead.

### Phase 5: TDD Setup (targeted)

- Criterion benches already present in `codegraph-vector/benches/knn_benchmark.rs`.
- Planned micro-bench: distance kernel perf (optional) to isolate changes.

### Phase 6–7: Core Implementation & Optimization (Completed)

Applied optimizations in `crates/codegraph-vector/src/knn.rs`:

- Distance kernel:
  - Replaced iterator-based Euclidean distance with unrolled index loop using `mul_add` and `#[inline(always)]`.
  - Added `squared_distance` and switched k-means assignment to squared distances (removes `sqrt()` in the hot inner loop).
- Result mapping: pre-allocated `node_results` with `Vec::with_capacity(k)`.
- Context ranking: sequential fast-path for small `k` (<= 64) to avoid Rayon overhead; parallel path retained for larger batches.
- Vector prep: pre-allocated vector and mapping capacities based on expected sizes.
- Marked `calculate_context_score` as `#[inline]`.

All changes are allocation- and branch-prediction-friendly and keep external behavior identical.

### Phase 8–9: Integration & Security

- No API changes; thread-safety preserved. No new external deps added.

### Phase 13: Benchmarking Plan (to run locally/CI)

Run with FAISS enabled where available:

- Build: `cargo build --release -p codegraph-vector --features faiss`
- Bench (subset): `cargo bench -p codegraph-vector --bench knn_benchmark --features faiss -- --warm-up-time 2 --measurement-time 5`
- Optional CPU profiles:
  - Linux: `cargo flamegraph -p codegraph-vector --bench knn_benchmark --features faiss`
  - macOS: `cargo instruments -t time -- cargo bench -p codegraph-vector --bench knn_benchmark --features faiss`

Focus metrics:
- Query-time CPU: FAISS call wrapper, result mapping, contextual ranking.
- Index build CPU: clustering assignment loop (distance function).

### Notes

- If FAISS is unavailable, validate distance and ranking micro-benchmarks in isolation (unit benches) to confirm kernel speedups.
- No behavioral changes expected; only performance improvements.

