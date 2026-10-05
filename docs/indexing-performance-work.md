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
2. Bulk node/edge/symbol/chunk/file upserts, checked database statement errors,
   scoped transactional cleanup with native record IDs, stable occurrence-aware edge
   IDs, independent row/byte batching, bounded write-queue bytes and connection-level
   concurrency with deletion/completion barriers. Ingestion no longer duplicates the
   entire node catalog into storage caches. Offline tests exercise both main schemas,
   repeated writes, failure recovery and metadata barriers.
3. Ambiguity-preserving symbol catalog, file/scope-aware exact resolution, direct
   definition IDs, cached target normalization and lexical results, trigram candidate
   indexing and RapidFuzz batch comparison with score cutoffs. Semantic inference now
   follows deterministic resolution and embeds only remaining targets and candidate
   aliases. Invalid provider cardinality/dimensions fail the indexing run. Random
   placeholder source-edge IDs and per-symbol query-task fallback were removed.
4. Shared compressed artifact cache with canonical BLAKE3 fingerprints, atomic writes,
   corruption recovery and eviction. AST artifacts include file identity, source hash,
   extraction policy and a parser format version. A warm run reuses unchanged parses;
   source and tier changes invalidate them. Established SHA-256 node IDs remain intact.

5. Complete-project reconciliation shared by full, single-file, delete and watch paths.
   Unchanged ASTs retain callers; catalog differences remove stale identities without
   deleting a modified file wholesale. Canonical source/support/configuration fingerprints
   invalidate docs, manifests, tiers and model policies. Completion is acknowledged last;
   interrupted runs lose the ready marker and repair on retry. Stable project-scoped
   directory identities stop at the project root. Offline cold/warm/edit/delete/empty
   source and watch tests pass against schema v2.

`CODEGRAPH_SOURCE_MEMORY_MB` controls retained snapshot bytes (default 256 MiB).
Snapshots above the budget spill to temporary files, removed at the end of the run.

Storage controls are independent of inference batches: `CODEGRAPH_DB_BATCH_ROWS`
(512), `CODEGRAPH_DB_BATCH_BYTES` (4 MiB), `CODEGRAPH_WRITE_QUEUE_BYTES` (32 MiB)
and `CODEGRAPH_SURREAL_POOL_SIZE` (remote only). A single oversized record fails with
an actionable limit message rather than bypassing the budget. These bound queued
serialized payloads; they do not represent a process RSS limit.

`CODEGRAPH_SEMANTIC_RESOLUTION=off` skips optional semantic resolution independently
of the extraction tier (default `sync` with `ai-enhanced`).
`CODEGRAPH_SEMANTIC_CANDIDATES=0` retains all candidates meeting the existing lexical
overlap/length gates; a positive limit is an opt-in recall/latency tradeoff. Ambiguous
short names without a unique local scope remain unresolved. The implementation uses
[RapidFuzz](https://docs.rs/rapidfuzz/0.5.0/rapidfuzz/distance/levenshtein/index.html).

`CODEGRAPH_AST_CACHE_BYTES` bounds retained AST artifacts (default 2 GiB), with eviction
at a run boundary. Entries larger than 64 MiB are recomputed rather than cached.

6. Parallel deterministic Unicode-safe chunk plans preserve newlines/tabs/emoji and
   reuse tokenized artifacts. Prepared-text APIs bypass node wrapping and repeated
   provider chunking. Exact submitted-text caches include model/task/tokenizer/runtime
   identity; duplicate and concurrent misses share inference. Row/token/byte and provider
   concurrency limits, finite vector validation and real inference/cache counters are
   enforced. Unchanged chunk records avoid inference and database writes. ONNX work
   runs on blocking workers with explicit intra/inter-op limits; CoreML compiled-model
   cache and low-precision accumulation and Candle dtype experiments are opt-in.

Inference controls: `CODEGRAPH_PROVIDER_CONCURRENCY` (local 1, remote 4),
`CODEGRAPH_EMBEDDING_BATCH_TOKENS` (local 8192, remote 32768),
`CODEGRAPH_EMBEDDING_BATCH_BYTES` (1 MiB), `CODEGRAPH_EMBEDDING_CACHE_TTL_SECONDS`
(3600 for mutable model aliases). `CODEGRAPH_MODEL_REVISION` declares an immutable
revision and disables TTL expiration. `CODEGRAPH_TOKENIZER_PATH` selects the provider's
actual tokenizer; local engines expose their loaded tokenizer automatically.
`CODEGRAPH_ONNX_INTRA_THREADS`, `CODEGRAPH_COREML_CACHE_DIR`,
`CODEGRAPH_COREML_LOW_PRECISION=1`, and `CODEGRAPH_LOCAL_DTYPE=f16|bf16` are independent
runtime experiments. Reduced precision requires retrieval-quality measurements.
