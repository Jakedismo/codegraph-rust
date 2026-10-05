# Indexing performance implementation

This work implements the indexing review on `codex/indexing-performance`. Changes are
split into independently reviewable commits. Performance claims require measurements;
optional backend and quality tradeoffs remain opt-in.

## Work sequence

- Shared source snapshots, configured parsing concurrency, parser pooling, deterministic
  extraction, early tier gating, directory pruning, and accurate parsing metrics.
- Durable storage acknowledgements, typed scoped deletion, stable relationship identity,
  bounded batches and writer concurrency, bulk writes, and ingestion cache policy.
- Deterministic resolution before semantic inference, ambiguity handling, normalization
  caching, indexed fuzzy candidates, and unresolved-only embedding work.
- Parallel Unicode-safe chunking, tokenizer budgets, deduplicated persistent embedding
  caching, length-aware batching, and local runtime scheduling.
- One reconciliation path for full, incremental, and watch indexing, dependency and
  configuration invalidation, repeatable project statistics, and failure recovery.
- Cached/overlapped build context, shared analyzer inputs, pipelined language-server
  requests, direct definition identities, and scope-aware dataflow.
- Configurable index construction, optional splitters/SCIP/backend experiments, staged
  completion policies, and reproducible speed and quality benchmarks.

## Validation

Each behavioral change receives targeted regression coverage. Final validation includes
formatting, crate/workspace compilation and applicable tests. Provider- or service-dependent
checks are reported separately from offline checks. Benchmarks cover cold, warm, no-change,
single-file, cross-file rename/delete, documentation/manifest changes and tier upgrades.

## Completed commits

1. Shared immutable source snapshots, worker-controlled parsing and stable extraction
   order; early reference/Uses gating; parser reuse; generated-directory pruning;
   file-local FastML state and multiline patterns; real source line counts; delayed
   graph replacement and writer lifetime correction. Regression tests cover snapshots,
   syntax-recovery spans, tier policy, language filtering and worker-independent order.

`CODEGRAPH_SOURCE_MEMORY_MB` controls retained snapshot bytes (default 256 MiB).
Snapshots above the budget spill to temporary files, removed at the end of the run.
