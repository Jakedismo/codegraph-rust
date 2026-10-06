# Repository Guidelines

## Project Structure & Module Organization

CodeGraph is a Rust 2024 workspace (Rust 1.95 or newer) that indexes code into a SurrealDB knowledge graph and exposes MCP and CLI agent tools. Source lives in `crates/codegraph-*/src/`: `core` holds shared types/configuration and artifact caching, `parser` captures sources and extracts syntax, `mcp` reconciles indexing and runs analyzers, `graph` handles storage, and `vector` provides embeddings/search. The CLI is `crates/codegraph-mcp-server/src/bin/codegraph.rs`; the agent backend lives in `codegraph-mcp-rig`.

Unit tests sit alongside modules; integration tests use each crate's `tests/`. Schemas live in `schema/`, configuration examples in `config/`, documentation in `docs/`, and images in `docs/assets/`.

## Build, Test, and Development Commands

Bare Cargo commands target only `codegraph-core`; specify `--workspace` or `-p <crate>`.

- `cargo check --workspace`: check all crates without producing binaries.
- `cargo build -p codegraph-mcp-server --bin codegraph --features full`: build the CLI with agent, embedding, and HTTP features; add `--release` for optimized builds.
- `./target/debug/codegraph start stdio`: run the built MCP server after configuring providers. New embedded stores apply the bundled v2 schema automatically; remote stores require `schema/apply-schema.sh`. Respect `CARGO_TARGET_DIR` when locating binaries.
- `cargo test -p codegraph-mcp`: test the indexing crate; use `cargo test --workspace` for all crates.
- `cargo test -p codegraph-mcp-server --lib` and `cargo test -p codegraph-mcp-server --test project_init_integration --test agent_cli_integration`: verify project setup, hook preservation and CLI contracts without live providers.
- `cargo fmt --all -- --check`: verify formatting; omit `-- --check` to format.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: run CI-style linting. `make lint` checks only core and suppresses warnings.

## Coding Style & Naming Conventions

Follow `rustfmt.toml`: four spaces, no tabs, 100-column width, and Unix newlines. Use `snake_case` for modules/functions, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Follow existing `ABOUTME` file comments. Keep analyzer output deterministic, scoped by `project_id`, and annotated with provenance.

## Testing Guidelines

Use `#[test]` and `#[tokio::test]` with descriptive snake_case names. Add regression tests for changed behavior. After schema edits, run `cargo test -p codegraph-graph --test schema_indexes_test` and keep both main schemas consistent.

Service tests can return successfully without executing when configuration is absent; `graph_tools_smoke` requires `CODEGRAPH_SURREALDB_URL`. Report these skips. CI collects coverage with `cargo llvm-cov`; no numeric minimum is configured.

## Indexing Invariants & Performance Checks

Full, single-file, delete and watch paths share complete-project reconciliation. Capture immutable source snapshots once, reuse versioned AST/analyzer artifacts and resolve against the full catalog, including unchanged callers. `--force` reconciles without trusting the previous catalog; it does not wipe the project before preparation.

Keep SHA node identities compatible, distinguish real same-line collisions and retain stable occurrence-aware edge IDs. Resolve definitions/exact/contextual and lexical targets before semantic inference; leave ambiguity unresolved and retain resolution provenance. Chunking must preserve Unicode/structural whitespace and enforce the provider's actual tokenizer budget. Cache keys must include source, extraction policy, model/task/revision/tokenizer/runtime identity and relevant build/doc inputs.

Keep fitting AST embedding units intact; split oversized units at real syntax
boundaries and use UTF-8-safe fallback cuts for oversized leaves. Recheck complete
inputs, including task prefixes and special tokens; count overlap with the actual
tokenizer. `CODEGRAPH_EMBEDDING_SKIP_CHUNKING=1` must reject oversized nodes, not
silently split/truncate them. Ollama resolves model/serving context and matching
publisher tokenizers in `codegraph-vector::input_policy`; every request disables
truncation. Unknown tokenizers require an explicit path/repository. Changes to this
policy must invalidate chunk, prepared-vector and reconciliation caches. Provider
initialization/chunk-planning APIs return `Result`; preserve error propagation.
Use isolated fixtures and mock HTTP services to validate these contracts.

Semantic scoring caches raw-vector norms and parallelizes independent targets in the shared worker-limited CPU pool through `spawn_blocking`. Preserve candidate order, scalar floating-point summation/division, the 0.75 threshold and 1e-6 tie rule; vector normalization or parallel dot-product reductions can change decisions. `semantic_scoring` tests and the offline `semantic_scoring_benchmark` compare node IDs and score bits against the frozen scalar reference. Resolution timings in `phase_ms` separate matching, inference, scoring and writes.

Bound parser workers, retained source bytes, queued writer payloads and provider concurrency independently. Database errors and flush acknowledgements must propagate. Reconcile stale project-scoped records and validate final inputs before persisting readiness. Graph completion, pending/off inference and vector-index readiness are separate states; do not mark a failed or deferred stage complete.

For indexing batches, explicit `--batch-size` overrides `CODEGRAPH_EMBEDDINGS_BATCH_SIZE`, legacy `CODEGRAPH_EMBEDDING_BATCH_SIZE`, TOML `[embedding] batch_size`, then the default 64. Resolve environment defaults once, preserve explicit positive values (including 100), and pass the same row limit to the indexer, provider and submitted-text cache. Do not silently cap Ollama/LM Studio at 256. Token/byte/provider limits and smaller DB write batches remain independent; report their limits accurately. Verify actual HTTP request sizes with the loopback mock in `index_cli_integration`, without querying live providers or the working index.

Rust LSP preflight must verify `rust-analyzer --version` in the target project, including rustup shims; presence on PATH is insufficient. Start language servers in that project directory, preserve a bounded stderr tail in failure reports and replace stopped pooled sessions. Retry only `ContentModified` (`-32801`) for symbol/definition reads, with fresh IDs, unchanged document versions and at most five retries within one 30-second deadline. Propagate cancellation, other errors, changed versions and exhausted retries. Test startup/crash/retry handling with isolated mock servers and `index_cli_integration` without installing toolchains or indexing the working repository.

`CODEGRAPH_EMBEDDING_POLICY` and `CODEGRAPH_SEMANTIC_RESOLUTION` accept `sync|deferred|off` independently of tier. Resume persisted inference with `codegraph index <root> --complete-deferred`. Precision, candidate limits, text-splitter and SCIP choices remain opt-in. See [implementation controls](docs/indexing-performance-work.md) and [speed/quality probes](docs/indexing-benchmarks.md); debug fixture timings are not production speedup claims.

Useful offline regressions: `cargo test -p codegraph-mcp --lib`, `cargo test -p codegraph-mcp --test reconciliation_test --test daemon_watch`, `cargo test -p codegraph-mcp --features embeddings --test deferred_policy_test`, `cargo test -p codegraph-graph --features surrealdb --test ingestion_write_test`, and `python3 -m unittest discover -s scripts/benchmarks -p 'test_*.py'`. Check the full CLI features after provider/runtime changes. Strict workspace Clippy currently reports existing core/zerocopy diagnostics; disclose that limitation instead of calling a non-strict run lint-clean.

## Commit & Pull Request Guidelines

Follow the history's Conventional Commit pattern, e.g. `fix(lsp): prevent premature process kill`. Use semantic PR titles and `.github/PULL_REQUEST_TEMPLATE.md`. Explain motivation, link issues, report validation and performance impact, and update affected documentation. Include screenshots when relevant.

## Security & Configuration

Use `.env.example` and `config/example.toml` as references. Keep credentials in ignored local configuration; never commit API keys or database passwords.

`codegraph init [project]` selects project-local Claude/Codex hooks before merging both agent instruction files and indexing. `--hooks claude|codex|both|none` supports scripts; `--no-index` performs provider-independent setup. Keep user-level settings untouched, preserve unrelated hooks/instructions, and retain idempotent managed blocks. See [init and CLI usage](docs/AGENTIC_CLI.md).

<!-- codegraph:begin -->
# codegraph

When `codegraph` is available, start code exploration with its agent tools. Ask a
specific question about the task, relevant symbols or paths instead of starting
with broad grep/rg searches:

- `codegraph agent context "Find the implementation and callers for <task>" --focus search`
  locates code; use `--focus builder` to gather implementation context or `question`
  to explain behavior.
- `codegraph agent impact "What depends on <symbol> and what would <change> affect?"`
  checks dependencies before editing; `--focus call_chain` follows call flows.
- `codegraph agent architecture "Describe <area> and its interfaces"`
  maps structure; `--focus api_surface` inspects public interfaces.
- `codegraph agent quality "Assess coupling, complexity and risks in <area>"`
  supports refactoring decisions and targeted follow-up checks.

Run from the indexed project root, or append `--project /path/to/project`; retain
the indexed `--project-id` if one was configured. Prefer the default JSON output:
inspect source locations, findings and partial-result warnings, then read the
specific files/lines before editing. Reuse useful findings and narrow follow-up
questions rather than repeating broad queries. After changes, verify against
current source and run relevant tests; the index may lag uncommitted work.

If CodeGraph is unavailable, the project is not indexed, a command fails or returns
insufficient evidence, fall back to targeted source reads and rg/grep. Known file
locations and exact-string verification also warrant direct reads/searches.
Do not install CodeGraph, download models or reindex solely to satisfy these
instructions. Agent queries use the configured model and may incur provider costs.
Use the four public agent commands; internal graph tools belong to CodeGraph's
built-in agents. Reload usage details with `codegraph agent instructions`.
<!-- codegraph:end -->
