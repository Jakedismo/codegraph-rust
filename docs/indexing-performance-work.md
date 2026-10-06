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

## Implemented slices

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

Semantic cosine scoring caches each candidate norm once and each unresolved query
norm once, then evaluates independent targets in the shared bounded Rayon CPU pool.
`RAYON_NUM_THREADS`, `CODEGRAPH_WORKERS` or CLI worker configuration determine its
existing worker limit, capped at available parallelism. The job runs through
`spawn_blocking`, so CPU scoring does not occupy an async runtime worker. Raw vectors
are borrowed, without copying/rescaling them or changing per-target candidate order.
The original scalar sum/division order, 0.75 threshold and 1e-6 tie rule are retained.
Offline regression and benchmark comparisons check exact node IDs and score bits.

Relationship timing retains the existing `resolve_and_enqueue_edges` total and adds
subphases in `--stats-json`'s `phase_ms`: `resolution_exact_lexical`,
`resolution_semantic_candidates`, `resolution_symbol_embeddings`,
`resolution_semantic_scoring`, `resolution_symbol_writes_flush` and
`resolution_edge_preparation_and_writes`. Symbol embedding includes enqueue/backpressure;
its final writer flush is measured separately. Scoring includes norm preparation and
blocking-job scheduling; the edge phase includes record construction and deduplication.
Subphases overlap their parent total and must not be added to that total again. Verbose
logs report scoring workers, comparison count, cached norm count and elapsed time.

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

7. Cargo metadata is cached by manifests/locks/configuration and checked external path
   manifests, overlapped with AST parsing; resolver package identities retain distinct
   versions. Shared source snapshots feed enrichment and LSP. Warm language servers
   share TS/JS sessions, handle server requests, retain versioned documents, bound all
   requests and deduplicate/pipeline definition positions. Failures propagate and
   direct node identities survive symbol aliases. Documentation token/line artifacts
   rebind against the current catalog; document contents are indexed. Scope-aware AST
   dataflow replaces per-variable regex scans, handles shadowing and caches function
   artifacts. Architecture patches cache package/boundary results; invalid boundary
   files fail visibly. Parser timeouts cooperatively cancel AST parsing and retain
   worker permits until blocking work ends. Legacy node-only parsing shares recovery
   without rewriting source spans. Same-line colliding identities are distinguished
   while existing non-colliding SHA node IDs remain unchanged.

`CODEGRAPH_LSP_REQUESTS` bounds outstanding requests per server (default 32).
`CODEGRAPH_PARSER_TIMEOUT_SECS` sets the small-file timeout (default 10 seconds),
scaled by three/six for medium/large files. AST extraction after parsing remains
bounded by worker permits even if its timeout expires.

8. Provider-aware token counting (OpenAI BPE or loaded local tokenizer), final input
   budget enforcement even when semantic splitting is disabled, file-grouped source
   reuse and an opt-in `CODEGRAPH_CHUNK_SPLITTER=text-splitter` comparison path.
   `CODEGRAPH_SCIP_INDEX=/path/index.scip` substitutes an existing compiler index for
   LSP analysis. Two streaming passes preserve local/global identities and reject
   ambiguous definitions, invalid positions and stale source. Balanced enriches existing
   links; Full also adds compiler references. SCIP documents with embedded text need no
   sidecar; otherwise generate `index.sources.json` with
   `scripts/benchmarks/scip-source-manifest.py` alongside fresh compiler output.
   `CODEGRAPH_SCIP_TRUST_SOURCE=1` explicitly accepts unverified source identity.
   Token-budget and compiler-link/stale-source regressions run without external services.

9. Independent `CODEGRAPH_EMBEDDING_POLICY` and `CODEGRAPH_SEMANTIC_RESOLUTION`
   accept `sync|deferred|off`. Defaults preserve enabled-feature behavior. Deferred
   runs persist a resumable job, mark the graph ready and inference pending, and avoid
   provider startup when neither stage is synchronous. Resume with
   `codegraph index <root> --complete-deferred`; current sources are reconciled again
   and only pending stages become synchronous. Failed runs retain their job.
   `--stats-json <path>` writes structured metrics and explicit stage status.

10. `CODEGRAPH_VECTOR_INDEX_MODE=all|selected|deferred|off` controls fresh-store HNSW
    construction. The default retains all dimensions; selected builds the active
    dimension before ingestion; deferred builds it after durable writes. Off reports
    vector readiness false. Shared existing indexes are never dropped. M/EFC tuning
    uses `CODEGRAPH_HNSW_M`/`CODEGRAPH_HNSW_EFC` for newly created indexes only.
    Completion verifies `ready` and no pending vector compaction through
    [SurrealDB index information](https://surrealdb.com/docs/reference/query-language/statements/info).
    Final source/support validation rejects edits during indexing. External Cargo
    manifests, tokenizer contents, chunk policy and analyzer settings invalidate
    project readiness; mutable model aliases periodically refresh it. Warm LSP
    sessions restart when build inputs change. In-memory embedding cache hits avoid
    redundant persistent cache reads.

11. Structured stats separate provider/database startup from indexing, report phase
    wall times (overlapped phases are not additive), real source-capture reads/bytes,
    spilled bytes, inference texts/tokens, cache hits and acknowledged writer
    jobs/rows/serialized payload bytes. Writer metrics cover queued ingestion jobs
    before the final completion marker, not every database RPC. No-change runs reset
    work counters instead of repeating previous inference totals. CLI wall timing now
    includes startup. Explicit Send future boundaries keep feature-enabled daemon
    integration compiling without materializing all inference batches.

12. Final regressions correct backend selection and actual model dimensions, honor
    disabled analyzers across tier upgrades, propagate language-server closure without
    waiting for request timeouts, compare definition columns in UTF-16, and preserve
    ambiguous same-line symbols. Missing external manifests invalidate cached build
    inputs instead of preventing a rebuild. Sync/deferred scheduling shares output
    identity, while pending status still forces synchronous completion; completed
    outputs are reused by subsequent deferred commands. Mutable model epochs also
    invalidate chunk records. Aggregate artifact eviction protects durable pending jobs.

`CODEGRAPH_INDEX_CACHE_BYTES` bounds all derived artifact namespaces together (default
4 GiB), checked at run boundaries. Pending inference jobs are excluded from eviction.
`CODEGRAPH_ANALYZERS=0|false|off` disables analyzers independently of extraction tier;
`CODEGRAPH_ANALYZERS_REQUIRE_TOOLS=0|false|off` relaxes tool availability checks.
Explicit embedding backends cannot silently fall back to different model weights.

13. [Reproducible benchmark instructions](indexing-benchmarks.md) cover ten indexing
    scenarios with actual embedded graph writes and forced-index equivalence checks.
    Same-model runtime probes export real vectors and identity metadata; labeled
    retrieval comparisons reject mismatched inputs and enforce recall/overlap gates.
    These experiments require explicitly selected model weights.

14. File metadata joins nodes/chunks/edges in the reconciliation digest catalog; edits
    update only changed records and preserve unchanged files' indexing timestamps.
    Candidate normalization, character lengths and trigram cardinalities are prepared
    once. Semantic gates reuse inverted-index overlap counts instead of allocating
    trigram sets for every comparison. Resolved edges retain their method and actual
    lexical/cosine score (a similarity measure, not a calibrated probability).
    CLI help now describes SurrealDB storage and independent inference policies.

## Final verification record

Workspace compilation and the full-feature CLI build passed. The macOS debug linker
emitted an unwind-section size warning; it did not prevent either binary from building.
Offline parser, indexing, vector,
artifact-cache, reconciliation/watch, tokenizer, text-cache and SCIP regression suites
passed. Embedded storage tests exercised both schemas, ingestion acknowledgements,
failure recovery and selected HNSW readiness. Deferred-policy tests verified persisted
jobs without provider initialization; a completed-catalog fixture separately verifies
scheduling reuse, without claiming actual provider completion. Python quality-probe
regressions passed.

A debug benchmark with 100 Rust files and three repeats completed all ten scenarios.
No-change runs reused all 100 ASTs and acknowledged zero writer rows or inference
texts. Single-file edits reused 99 ASTs and acknowledged three rows. Its timings
validate the harness, not a speedup against the original code.
See the benchmark instructions for measured samples and their limits.

Strict workspace Clippy (`--all-targets --all-features -- -D warnings`) remains blocked
by existing diagnostics in untouched core configuration/watch/memory/updater code and
zerocopy buffers/shared memory. Targeted Clippy for parser/indexing/vector/graph completed
with existing warnings; no diagnostics touched lines changed by this work. It is not
a strict-lint pass. No live language-server benchmark,
remote provider run, real-model quality comparison or persistent-disk benchmark was
performed. Optional precision, compiler-index and splitter choices remain opt-in.
