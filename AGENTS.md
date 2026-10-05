# Repository Guidelines

## Project Structure & Module Organization

CodeGraph is a Rust 2021 workspace that indexes code into a SurrealDB knowledge graph and exposes MCP tools. Source lives in `crates/codegraph-*/src/`: `core` holds shared types/configuration, `parser` extracts syntax, `mcp` orchestrates indexing, `graph` handles storage, and `vector` provides embeddings/search. The CLI is `crates/codegraph-mcp-server/src/bin/codegraph.rs`; agent backends live in `codegraph-mcp-rig` and `codegraph-mcp-autoagents`.

Unit tests sit alongside modules; integration tests use each crate's `tests/`. Schemas live in `schema/`, configuration examples in `config/`, documentation in `docs/`, and images in `docs/assets/`.

## Build, Test, and Development Commands

Bare Cargo commands target only `codegraph-core`; specify `--workspace` or `-p <crate>`.

- `cargo check --workspace`: check all crates without producing binaries.
- `cargo build -p codegraph-mcp-server --bin codegraph --features full`: build the CLI with agent, embedding, and HTTP features; add `--release` for optimized builds.
- `./target/debug/codegraph start stdio`: run the built MCP server after configuring providers and SurrealDB and applying `schema/apply-schema.sh`.
- `cargo test -p codegraph-mcp`: test the indexing crate; use `cargo test --workspace` for all crates.
- `cargo fmt --all -- --check`: verify formatting; omit `-- --check` to format.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: run CI-style linting. `make lint` checks only core and suppresses warnings.

## Coding Style & Naming Conventions

Follow `rustfmt.toml`: four spaces, no tabs, 100-column width, and Unix newlines. Use `snake_case` for modules/functions, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Follow existing `ABOUTME` file comments. Keep analyzer output deterministic, scoped by `project_id`, and annotated with provenance.

## Testing Guidelines

Use `#[test]` and `#[tokio::test]` with descriptive snake_case names. Add regression tests for changed behavior. After schema edits, run `cargo test -p codegraph-graph --test schema_indexes_test` and keep both main schemas consistent.

Service tests can return successfully without executing when configuration is absent; `graph_tools_smoke` requires `CODEGRAPH_SURREALDB_URL`. Report these skips. CI collects coverage with `cargo llvm-cov`; no numeric minimum is configured.

## Commit & Pull Request Guidelines

Follow the history's Conventional Commit pattern, e.g. `fix(lsp): prevent premature process kill`. Use semantic PR titles and `.github/PULL_REQUEST_TEMPLATE.md`. Explain motivation, link issues, report validation and performance impact, and update affected documentation. Include screenshots when relevant.

## Security & Configuration

Use `.env.example` and `config/example.toml` as references. Keep credentials in ignored local configuration; never commit API keys or database passwords.
