# Contributing to CodeGraph

Thanks for helping improve CodeGraph. This repo is a Rust workspace with multiple crates under `crates/`, plus SurrealDB schemas under `schema/`.

## Quick start

1. Install Rust (via `rustup`) and any platform prerequisites from `docs/INSTALLATION_GUIDE.md`.
2. Build and run a fast hygiene pass:

```bash
cargo check --workspace
```

3. Run tests (start with the crate you touched):

```bash
cargo test -p codegraph-mcp
```

4. Format before you send a PR:

```bash
cargo fmt
```

## What to change where

### Indexing pipeline

- Orchestration and analyzer phases: `crates/codegraph-mcp/src/indexer.rs`
- Analyzer implementations: `crates/codegraph-mcp/src/analyzers/`
- Parser and AST extraction: `crates/codegraph-parser/src/`

### Embeddings and reranking

- Provider implementations: `crates/codegraph-vector/src/`
- Runtime config loader (TOML + `.env` + env overrides): `crates/codegraph-core/src/config_manager.rs`
- Provider-specific config structs: `crates/codegraph-core/src/config_manager.rs` and `crates/codegraph-core/src/rerank_config.rs`

### Built-in agent (agentic MCP tools)

- MCP server and tool entrypoints: `crates/codegraph-mcp-server/src/official_server.rs`
- Prompt tier selection: `crates/codegraph-mcp-server/src/prompt_selector.rs`
- Tier prompts per analysis type: `crates/codegraph-mcp-server/src/*_prompts.rs`
- LLM providers: `crates/codegraph-ai/src/`

## Adding a new language

CodeGraph language support typically requires three layers:

1. **Core language enum**: add or confirm the language exists in `crates/codegraph-core/src/types.rs`
2. **Tree-sitter registration**: add the grammar dependency and register extensions in `crates/codegraph-parser/src/language.rs`
3. **Extraction**: implement node/edge extraction for the new language in the parser pipeline (follow the existing extractor patterns in `crates/codegraph-parser/src/`)

After you add a language:

- Update `docs/SUPPORTED_LANGUAGES.md`
- Add or update tests that validate language registration and basic parsing

## Adding an analyzer

Analyzers run during `codegraph index` to enrich the initial AST graph.

Guidelines:

- Prefer small, composable analyzers that add clearly attributable nodes/edges.
- Ensure analyzer output is deterministic and scoped by `project_id`.
- Emit provenance metadata (analyzer name, confidence) so later tools can explain where facts came from.

If your analyzer depends on external tools (e.g. language servers), add the tool requirement in:

- `crates/codegraph-mcp/src/analyzers/mod.rs`

## Schema changes (SurrealDB)

Schema files live in `schema/`:

- `schema/codegraph.surql` (default schema)
- `schema/codegraph_graph_experimental.surql` (experimental graph-oriented schema)

If you change a schema:

- Update any schema-level tests that validate required functions/indexes.
- Keep SurrealQL compatible with current SurrealDB parsing rules.

## Documentation updates

If your change affects configuration, providers, or language support, update:

- `docs/AI_PROVIDERS.md`
- `docs/AGENT_PROMPT_TIERS.md`
- `docs/SUPPORTED_LANGUAGES.md`

## PR checklist

- `cargo test` passes for touched crates
- `cargo fmt` clean
- No new secrets committed
- Docs updated for user-facing behavior changes
