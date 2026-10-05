# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

CodeGraph indexes a codebase into a SurrealDB knowledge graph (AST nodes, edges, chunk embeddings) and serves it to AI clients over MCP as four agentic tools (`agentic_context`, `agentic_impact`, `agentic_architecture`, `agentic_quality`). It is a Rust workspace of 13 crates under `crates/`; the single shipped binary is `codegraph`, built from `crates/codegraph-mcp-server/src/bin/codegraph.rs`.

## Commands

**Workspace gotcha:** `default-members = ["crates/codegraph-core"]`, so a bare `cargo build` / `cargo test` / `cargo clippy` only touches `codegraph-core`. Always pass `--workspace` or `-p <crate>`.

```bash
# Fast hygiene pass
cargo check --workspace

# Build the binary (default features = ["daemon"] only: no agentic tools, no embeddings)
cargo build --release -p codegraph-mcp-server --bin codegraph --features full
# What the install scripts do (install-codegraph*.sh):
cargo install --path crates/codegraph-mcp-server --bin codegraph --all-features --force

# Tests
cargo test --workspace                      # or: make test
cargo test -p codegraph-mcp                 # one crate
cargo test -p codegraph-mcp <test_name>     # one test by name filter
cargo test -p codegraph-graph --test schema_indexes_test   # one integration-test file

# Format / lint
cargo fmt --all                             # rustfmt.toml: max_width = 100
cargo clippy --workspace --all-targets --all-features -- -D warnings   # what CI runs
```

- `make lint` is effectively a no-op (clippy on `codegraph-core` only, with `-A clippy::all -A warnings`). Use the CI form above.
- On macOS, `make build-llvm` / `make test-llvm` link with `lld` for faster builds (requires `ld64.lld` on PATH, e.g. `brew install llvm`).
- Release profile is `lto = "fat"`, `codegen-units = 1`: release builds are slow. Use dev builds (or `--profile fast-dev`) while iterating.

### Stale references to ignore

- Makefile targets referring to `codegraph-api`, `high_perf_test/`, or `scripts/` point at things that no longer exist.
- `docs/TESTING.md` uses feature names (`onnx`, `ollama`, `cloud-jina`, ...) and a `test_mcp_tools.py` that don't match the current tree.
- `[workspace.metadata.cargo-features]` in the root `Cargo.toml` is documentation only; cargo does not consume it. The real feature flags are on `codegraph-mcp-server` (`ai-enhanced`, `server-http`, `daemon`, `embeddings-*`, `rig-*`, `legacy-agentic-tools`, `full`).
- `docs/README.md` mentions a `codegraph-cache` crate that has been removed.

### Tests that need external services

There are no `#[ignore]` tests; DB-dependent tests are gated by env vars and silently skip (pass) otherwise:

- `crates/codegraph-mcp/tests/graph_tools_smoke.rs` runs only when `CODEGRAPH_SURREALDB_URL` is set (plus optional `CODEGRAPH_SURREALDB_NAMESPACE|DATABASE|USERNAME|PASSWORD`, `CODEGRAPH_EMBEDDING_MODEL`, `CODEGRAPH_EMBEDDING_DIMENSION`), against an already-indexed database.
- `crates/codegraph-graph/tests/semantic_search_nodes_via_chunks_test.rs` runs only with `CODEGRAPH_RUN_SEMANTIC_SEARCH_NODES_VIA_CHUNKS_TEST=1` (uses in-memory SurrealDB).
- `test_http_mcp.py` exercises a running HTTP server (`codegraph start http --port 3000`, needs `server-http`).

### Running locally

```bash
surreal start --bind 0.0.0.0:3004 --user root --pass root file://$HOME/.codegraph/surreal.db
cd schema && ./apply-schema.sh          # defaults: ns "ouroboros", db "codegraph"; schema is NOT auto-applied at runtime
codegraph db-check                      # connectivity/schema canary
codegraph index /path/to/project -r -l rust,typescript --index-tier fast|balanced|full
codegraph start stdio --watch           # MCP server over stdio, with re-index-on-change daemon
```

Config resolution (`crates/codegraph-core/src/config_manager.rs`): `./.codegraph.toml`, then `~/.codegraph/config.toml`, overridden by `.env` (cwd, then `~/.codegraph.env`) and `CODEGRAPH_*` env vars. See `.env.example` and `config/example.toml`.

## Architecture

### Crate layering

Dependencies point downward; do not add upward imports (they create cycles):

- `codegraph-core`: types, config, `CodeNode`. Everything depends on it.
- `codegraph-parser` (tree-sitter) and `codegraph-graph` (SurrealDB storage) depend only on core. `codegraph-vector` (embedding providers, reranking) sits on graph. `codegraph-ai` (LLM providers) sits on graph + vector.
- `codegraph-mcp-core`: shared MCP types (`ContextTier`, `AgentArchitecture`), on core only. `codegraph-mcp-tools` (inner graph tools) adds graph + vector.
- On top of mcp-tools / ai / graph: `codegraph-mcp-rig` (the agent backend, built on the Rig framework) and `codegraph-mcp` (indexer + analyzers). Neither depends on the other.
- `codegraph-mcp-daemon` (file watching) depends on `codegraph-mcp`.
- `codegraph-mcp-server` (CLI, stdio/HTTP transports, MCP tool entrypoints) is the only crate that depends on everything.
- `codegraph-concurrent` and `codegraph-zerocopy` are standalone utility crates.

Provider and backend code is heavily `#[cfg(feature = ...)]`-gated, so a change can compile under default features and break under `--all-features` (or vice versa). Check with the features that actually enable the code you touched.

### Flow 1: indexing (`codegraph index`)

`bin/codegraph.rs` `handle_index` → `ProjectIndexer::index_project` in `crates/codegraph-mcp/src/indexer.rs`:

1. Collect files (respects `.gitignore`, filters secret patterns); decide incremental vs clean-slate from stored per-file metadata (`--force` wipes the project).
2. Per file: `TreeSitterParser::parse_file_with_edges` → language extractor in `codegraph-parser/src/languages/` → `fast_ml::enhance_extraction` (pattern-based edges + local symbol heuristics).
3. Deterministic project-scoped node IDs; chunking + embeddings (feature-gated) persisted to SurrealDB.
4. Analyzer stages in `crates/codegraph-mcp/src/analyzers/`: `build_context` (Cargo packages/features), `lsp` (language-server resolution), `enrichment` (rustdoc/API surface), `module_linker`, `dataflow` (Rust-local def-use), `docs_contracts` (doc/spec nodes linked to backticked symbols), `architecture` (cycles, `codegraph.boundaries.toml` deny rules).
5. Resolve edge targets (string symbol → node ID) against a project-wide symbol index, persist edges and metadata.

The index tier controls which analyzers run and which edge types are filtered: `fast` (default; AST + core edges only), `balanced` (adds build context, LSP symbols, enrichment, module linking, docs), `full` (everything). LSP-enabled tiers **fail fast** if the language server for an indexed language is missing (`rust-analyzer`, `typescript-language-server`, `pyright-langserver`, `gopls`, `jdtls`, `clangd`); external tool requirements are declared in `analyzers/mod.rs`. Details: `docs/architecture/indexing-analyzers.md`, `docs/specifications/indexing_tiers.spec.md`.

### Flow 2: agentic MCP tools

`crates/codegraph-mcp-server/src/official_server.rs` exposes the four consolidated tools via a single rmcp `#[tool_router]` (the SDK does not support multiple router blocks, so the deprecated legacy tools are feature-gated by `legacy-agentic-tools` within the same router). Each tool calls `execute_agentic_workflow`, which:

1. Picks a prompt/step tier from the configured LLM context window (`CODEGRAPH_CONTEXT_WINDOW` → `ContextTier` in `codegraph-mcp-core/src/context_aware_limits.rs`); the same value bounds per-tool result size and accumulated context.
2. Runs the Rig backend (`codegraph-mcp-rig`), whose builder (`agent/builder.rs`) picks the agent from `CODEGRAPH_AGENT_ARCHITECTURE`: `react` (default, alias `rig`), `lats` (tree search over reasoning steps; does not call graph tools), or `reflexion`. A failed run is retried through Reflexion.
3. The agent loops over the inner graph tools in `codegraph-mcp-tools` (`GraphToolSchemas` / `GraphToolExecutor`: transitive deps, reverse deps, cycles, call chains, coupling, hub nodes, semantic search, complexity hotspots).
4. Those call `fn::*` SurrealQL functions defined in `schema/codegraph.surql`, wrapped by `crates/codegraph-graph/src/graph_functions.rs`.

The agent's system prompt is built in `codegraph-mcp-rig/src/prompts/tier_prompts.rs`, and tool semantics live in the tool descriptions in `codegraph-mcp-rig/src/tools/graph_tools.rs`.

### Schema

Two SurrealDB schemas must stay in sync with the storage layer and with each other: `schema/codegraph.surql` (default) and `schema/codegraph_graph_experimental.surql` (selected with `CODEGRAPH_USE_GRAPH_SCHEMA=true` + `CODEGRAPH_GRAPH_DB_DATABASE`). `crates/codegraph-graph/tests/schema_indexes_test.rs` statically validates both files (project-scoped indexes, required `fn::*` functions, SurrealQL parsing constraints); run it after any schema edit. A new graph function generally needs: the `fn::` definition in both schemas, a wrapper in `graph_functions.rs`, and a schema + executor entry in `codegraph-mcp-tools`.

## Conventions

- About half the source files start with two `// ABOUTME:` comment lines summarising the file's purpose (`# ABOUTME:` in shell scripts). Follow this on new files.
- Analyzer output must be deterministic, scoped by `project_id`, and carry provenance metadata (analyzer name, confidence).
- Adding a language touches three layers: the enum in `codegraph-core/src/types.rs`, grammar registration in `codegraph-parser/src/language.rs`, and an extractor in `codegraph-parser/src/languages/`; then update `docs/SUPPORTED_LANGUAGES.md`.
- User-facing changes to configuration, providers, or tiers should update `docs/AI_PROVIDERS.md` / `docs/AGENT_PROMPT_TIERS.md`. `CONTRIBUTING.md` has the full "what to change where" map.
- `rules-for-claude-code/codegraph_rule.md` is guidance for *consumers* of the CodeGraph MCP server, not for developing this repo.
