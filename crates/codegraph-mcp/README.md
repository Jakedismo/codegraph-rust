# CodeGraph MCP – Agentic Tool Server

CodeGraph MCP is the SurrealDB-backed Model Context Protocol server that powers the
`agentic_*` tool suite used by AutoAgents and other reasoning-first assistants. The
crate wraps our AutoAgents orchestrator, Surreal graph functions, and semantic search
pipeline into a single binary so LLM agents can issue rich analysis requests over
MCP (STDIO or HTTP) without having to understand the underlying codebase structure.

## Highlights

- **AutoAgents orchestration** – every tool request spins up a ReAct-style plan
  (tier-aware prompts, max-step guards, intermediate reasoning logs) so LLMs
  explore the graph incrementally instead of returning shallow answers.
- **Seven advanced MCP tools** – multi-hop graph queries, semantic synthesis, and
  architecture diagnostics wrapped behind stable MCP endpoints.
- **SurrealDB for graph + embeddings** – all structure resides in SurrealDB
  (nodes, edges, embeddings, context caches). No RocksDB/FAISS dependencies remain.
- **Flexible transports** – STDIO for local agents (Claude, GPTs, AutoAgents) and
  an optional HTTP transport for remote evaluation (`test_http_mcp.py`).
- **Structured outputs** – every response follows our JSON schemas so downstream
  agents can capture file paths, node IDs, and prompt traces deterministically.

## Agentic Tool Suite

| Tool name                   | Purpose                                                                                           |
|-----------------------------|---------------------------------------------------------------------------------------------------|
| `agentic_code_search`       | Multi-step semantic/code search with AutoAgents planning.                                          |
| `agentic_dependency_analysis` | Explores transitive `Imports`/`Calls` edges for a symbol, ranks hotspots, flags potential risks. |
| `agentic_call_chain_analysis` | Traces execution from an entrypoint (e.g., `execute_agentic_workflow`) to graph tools.            |
| `agentic_architecture_analysis` | Calculates coupling metrics, hub nodes, and layering issues for a subsystem.                    |
| `agentic_api_surface_analysis` | Enumerates public functions/structs of a component and links them back to files/lines.           |
| `agentic_context_builder`   | Gathers everything needed for prompt construction (files, dependencies, semantic neighbors).       |
| `agentic_semantic_question` | Free-form “explain X” queries that blend embeddings + structured graph walks.                      |

Each handler lives in `official_server.rs` and funnels into
`execute_agentic_workflow`, which:

1. Detects the LLM tier (`Small`, `Medium`, `Large`, `Massive`) using context-window metadata.
2. Picks the corresponding prompt template from `prompts/`.
3. Executes SurrealDB graph functions (`codegraph_graph::GraphFunctions`) and semantic
   search helpers.
4. Streams reasoning steps + final answers back through MCP.

## Architecture at a Glance

- **AutoAgents + CodeGraph AI** – enabled via the `ai-enhanced` feature flag. The orchestrator uses
  AutoAgents’ planner/critic/executor roles but our prompts + structured tool calls.
- **Surreal Graph Storage** – compile with `codegraph-graph/surrealdb` (default in this repo) and
  provide Surreal credentials via `CODEGRAPH_SURREALDB_*` env vars or `config/surrealdb_example.toml`.
- **Embedding Providers** – choose via `CODEGRAPH_LLM_PROVIDER` and the `embeddings-*` Cargo features
  (Ollama, OpenAI, Jina, etc.). All agentic tools require embeddings because they mix symbolic +
  vector context.
- **Transports** – STDIO transport listens on stdin/stdout (ideal for MCP-compliant hosts) and the
  optional HTTP server exposes `/mcp` for streaming JSON-RPC over HTTP.

## Quick Start

### 1. Prepare configuration

```bash
cp config/surrealdb_example.toml ~/.codegraph/config.toml
export CODEGRAPH_SURREALDB_URL=ws://localhost:3004
export CODEGRAPH_SURREALDB_NAMESPACE=ouroboros
export CODEGRAPH_SURREALDB_DATABASE=codegraph
export CODEGRAPH_SURREALDB_USERNAME=root
export CODEGRAPH_SURREALDB_PASSWORD=root
export CODEGRAPH_LLM_PROVIDER=ollama          # or openai / anthropic / xai
export MCP_CODE_AGENT_MAX_OUTPUT_TOKENS=4096  # optional override
```

### 2. Run the STDIO MCP server

```bash
cargo run -p codegraph-mcp \
  --features "ai-enhanced,embeddings-ollama,autoagents-experimental" \
  -- start stdio
```

Hook this process up to your MCP host (Claude Desktop, custom AutoAgents runner, etc.).

### 3. (Optional) Run the HTTP transport

```bash
cargo run -p codegraph-mcp \
  --features "ai-enhanced,embeddings-ollama,server-http,autoagents-experimental" \
  -- start http --host 127.0.0.1 --port 3003
```

Point the Python harness at it:

```bash
CODEGRAPH_HTTP_HOST=127.0.0.1 CODEGRAPH_HTTP_PORT=3003 \
python test_http_mcp.py
```

or use the streaming tester (`test_agentic_tools_http.py`) which logs each request under
`test_output_http/`.

## Invoking Tools Manually

All MCP requests follow the standard JSON-RPC envelope. Example HTTP payload for
`agentic_dependency_analysis`:

```jsonc
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "agentic_dependency_analysis",
  "params": {
    "query": "Analyze the dependency chain for the AgenticOrchestrator. What does it depend on?",
    "workspaceId": "default"
  }
}
```

The response contains:

- `reasoning`: step-by-step chain of thought
- `tool_call`: the Surreal function invocation (e.g., `get_transitive_dependencies`)
- `analysis`: markdown/JSON summary
- `components` + `file_locations`: structured references with file paths + line numbers

## Configuration + Environment Variables

| Variable                               | Purpose                                                                    |
|----------------------------------------|----------------------------------------------------------------------------|
| `CODEGRAPH_SURREALDB_URL` (+ namespace/db/user/password) | Points the server at your SurrealDB instance.                             |
| `CODEGRAPH_LLM_PROVIDER`               | `ollama`, `openai`, `anthropic`, `xai`, etc.                               |
| `CODEGRAPH_EMBEDDING_PROVIDER`         | Chooses embedding backend (see Cargo feature flags above).                 |
| `MCP_CODE_AGENT_MAX_OUTPUT_TOKENS`     | Hard override for AutoAgents’ final response length.                       |
| `CODEGRAPH_HTTP_HOST` / `CODEGRAPH_HTTP_PORT` | Used by the HTTP server & test harnesses.                        |

For advanced tuning (batch sizes, prompt overrides, tier thresholds) see
`crates/codegraph-mcp/src/context_aware_limits.rs` and the prompt files under `src/prompts/`.

## Testing

- `python test_agentic_tools.py` – exercises all tools over STDIO.
- `python test_agentic_tools_http.py` – same via HTTP transport (outputs logs to `test_output_http/`).
- `python test_http_mcp.py` – minimal MCP smoke test for custom HTTP clients.
- `cargo test -p codegraph-mcp --features "ai-enhanced"` – Rust-level tests for orchestrator pieces.

Every successful run drops JSON logs into `test_output/` (STDIO) or
`test_output_http/` (HTTP) so you can diff reasoning traces between commits.

## Observability Tips

- Set `--debug` when starting the server to tee AutoAgents traces into
  `~/.codegraph/logs`.
- Each tool emits structured output; store them if you need regressions.
- The Python harness prints timing data (e.g., 26.6s for `agentic_code_search`) so
  you can monitor throughput after embedding/provider changes.

## Need More?

- See `docs/faiss_rocksdb_deprecation_plan.md` for the Surreal-only roadmap.
- `TESTING.md` documents the recommended feature flags for Ollama / OpenAI / Anthropic setups.
- The `codegraph-api` crate exposes the same Surreal graph via GraphQL/REST if you need to build custom dashboards.
