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

Bound parser workers, retained source bytes, queued writer payloads and provider concurrency independently. Database errors and flush acknowledgements must propagate. Reconcile stale project-scoped records and validate final inputs before persisting readiness. Graph completion, pending/off inference and vector-index readiness are separate states; do not mark a failed or deferred stage complete.

`CODEGRAPH_EMBEDDING_POLICY` and `CODEGRAPH_SEMANTIC_RESOLUTION` accept `sync|deferred|off` independently of tier. Resume persisted inference with `codegraph index <root> --complete-deferred`. Precision, candidate limits, text-splitter and SCIP choices remain opt-in. See [implementation controls](docs/indexing-performance-work.md) and [speed/quality probes](docs/indexing-benchmarks.md); debug fixture timings are not production speedup claims.

Useful offline regressions: `cargo test -p codegraph-mcp --lib`, `cargo test -p codegraph-mcp --test reconciliation_test --test daemon_watch`, `cargo test -p codegraph-mcp --features embeddings --test deferred_policy_test`, `cargo test -p codegraph-graph --features surrealdb --test ingestion_write_test`, and `python3 -m unittest discover -s scripts/benchmarks -p 'test_*.py'`. Check the full CLI features after provider/runtime changes. Strict workspace Clippy currently reports existing core/zerocopy diagnostics; disclose that limitation instead of calling a non-strict run lint-clean.

## Commit & Pull Request Guidelines

Follow the history's Conventional Commit pattern, e.g. `fix(lsp): prevent premature process kill`. Use semantic PR titles and `.github/PULL_REQUEST_TEMPLATE.md`. Explain motivation, link issues, report validation and performance impact, and update affected documentation. Include screenshots when relevant.

## Security & Configuration

Use `.env.example` and `config/example.toml` as references. Keep credentials in ignored local configuration; never commit API keys or database passwords.
