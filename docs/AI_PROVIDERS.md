# CodeGraph AI providers: embeddings + LLMs

This document explains how to configure CodeGraph’s embedding providers (for indexing/search) and LLM providers (for the built-in agentic tools).

It is based on the runtime configuration loader in `crates/codegraph-core/src/config_manager.rs` and provider implementations in:

- `crates/codegraph-vector/src/*` (embeddings)
- `crates/codegraph-ai/src/*` (LLM providers)

## Configuration sources and precedence

When CodeGraph starts, it loads configuration in this order:

1. `.env` in the current directory (if present)
2. `~/.codegraph.env` (if present)
3. A TOML config file:
   - `./.codegraph.toml` (project-local), else
   - `~/.codegraph/config.toml` (user-global)
4. Environment variable overrides (e.g. `CODEGRAPH_EMBEDDING_PROVIDER`, `OPENAI_API_KEY`)

If you use `.env`, it’s loaded automatically at startup (you do not need `direnv`).

## Minimal setup checklist

1. SurrealDB is running and schema is applied (see `docs/INSTALLATION_GUIDE.md`).
2. You have a `./.codegraph.toml` or `~/.codegraph/config.toml` with at least:
   - `[embedding] provider = ...`
   - `[llm] enabled = true` and a working LLM provider configuration (required for agentic tools)
3. Secrets are present via `.env` or your shell environment (recommended).

## `.env` examples

### Local (Ollama embeddings + Ollama LLM)

```bash
# SurrealDB connection (used by indexing and agentic tools)
CODEGRAPH_SURREALDB_URL=ws://localhost:3004
CODEGRAPH_SURREALDB_NAMESPACE=ouroboros
CODEGRAPH_SURREALDB_DATABASE=codegraph
CODEGRAPH_SURREALDB_USERNAME=root
CODEGRAPH_SURREALDB_PASSWORD=root

# Embeddings
CODEGRAPH_EMBEDDING_PROVIDER=ollama
CODEGRAPH_OLLAMA_URL=http://localhost:11434
CODEGRAPH_EMBEDDING_MODEL=hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M

# Built-in agent LLM
CODEGRAPH_LLM_PROVIDER=ollama
CODEGRAPH_MODEL=qwen2.5-coder:14b
```

### Local (LM Studio embeddings + LM Studio LLM)

```bash
CODEGRAPH_EMBEDDING_PROVIDER=lmstudio
CODEGRAPH_EMBEDDING_MODEL=jinaai/jina-embeddings-v3

CODEGRAPH_LLM_PROVIDER=lmstudio
CODEGRAPH_MODEL=local-model
```

Note: `lmstudio_url` is configured via TOML (`embedding.lmstudio_url` / `llm.lmstudio_url`); there is no `CODEGRAPH_LMSTUDIO_URL` env override today.

### Remote (Jina embeddings + OpenAI LLM)

```bash
CODEGRAPH_EMBEDDING_PROVIDER=jina
JINA_API_KEY=...
CODEGRAPH_EMBEDDING_MODEL=jina-embeddings-v4
JINA_API_BASE=https://api.jina.ai/v1

CODEGRAPH_LLM_PROVIDER=openai
OPENAI_API_KEY=...
CODEGRAPH_MODEL=gpt-5.1-codex
```

Notes:

- `.env` secrets should never be committed. Prefer `~/.codegraph.env` for global secrets.
- You can also put many non-secret defaults into the TOML config file and keep only keys in `.env`.

## TOML config file examples

Create either `./.codegraph.toml` (project-local) or `~/.codegraph/config.toml` (global).

### Example: Jina embeddings + xAI Grok LLM

```toml
[embedding]
provider = "jina"
model = "jina-embeddings-v4"
jina_api_base = "https://api.jina.ai/v1"
jina_task = "code.query"
jina_late_chunking = true
dimension = 2048
batch_size = 64

[llm]
enabled = true
provider = "xai"
model = "grok-4-1-fast-reasoning"
xai_base_url = "https://api.x.ai/v1"
context_window = 2000000
timeout_secs = 120
```

## Embedding providers (indexing + search)

The embedding provider is configured under `[embedding]` (or via env).

Valid values:

- `auto`
- `onnx`
- `ollama`
- `lmstudio` (sometimes spelled “mlstudio” colloquially, but the config value is `lmstudio`)
- `openai`
- `jina`

### `embedding.provider = "auto"`

“Auto” tries to find a reasonable local option:

- If an Ollama server is reachable, it prefers Ollama.
- Otherwise it tries to find an ONNX model in the local HuggingFace cache.

For reproducibility, prefer setting an explicit provider instead of `auto`.

### `embedding.provider = "ollama"` (local)

Requirements:

- Ollama running (default `http://localhost:11434`)
- An embedding model pulled into Ollama

Config inputs:

- `embedding.ollama_url` (or `CODEGRAPH_OLLAMA_URL`)
- `embedding.model` (or `CODEGRAPH_EMBEDDING_MODEL`)

### `embedding.provider = "lmstudio"` (local)

Requirements:

- LM Studio server running with an embedding model loaded
- The server exposes an OpenAI-compatible embeddings endpoint at:
  - `http://localhost:1234/v1/embeddings` (default)

Config inputs:

- `embedding.lmstudio_url` (TOML only; no env override)
- `embedding.model` (or `CODEGRAPH_LMSTUDIO_MODEL` / `CODEGRAPH_EMBEDDING_MODEL`)

### `embedding.provider = "jina"` (remote)

Requirements:

- `JINA_API_KEY` (or `embedding.jina_api_key`)

Config inputs:

- `embedding.model` (e.g. `jina-embeddings-v4`)
- `embedding.jina_api_base` (default `https://api.jina.ai/v1`)
- `embedding.jina_task` (commonly `code.query`)
- `embedding.jina_late_chunking` (default is provider-dependent; set explicitly if you care)

Optional env tuning (provider-specific):

- `JINA_MAX_TOKENS`, `JINA_MAX_TEXTS`, `JINA_REQUEST_DELAY_MS`
- `JINA_LATE_CHUNKING`, `JINA_TRUNCATE`

### `embedding.provider = "openai"` (remote)

Requirements:

- `OPENAI_API_KEY` (or `embedding.openai_api_key`)

Config inputs:

- `embedding.model` (e.g. `text-embedding-3-small`)

### `embedding.provider = "onnx"` (local)

This uses a local ONNX embedding engine (no external service), typically pointing at a model directory.

Common env inputs:

- `CODEGRAPH_LOCAL_MODEL` (model directory / repo path)

### Chunking controls (provider-agnostic)

Chunking and batching can be tuned via environment variables:

- `CODEGRAPH_CHUNK_MAX_TOKENS`
- `CODEGRAPH_CHUNK_OVERLAP_TOKENS`
- `CODEGRAPH_CHUNK_SMART_SPLIT` (`true`/`false`)
- `CODEGRAPH_EMBEDDING_SKIP_CHUNKING` (`true`/`false`)
- `CODEGRAPH_EMBEDDING_BATCH_SIZE` (config loader) and `CODEGRAPH_EMBEDDINGS_BATCH_SIZE` (embedding engine)

If you change embedding dimensions/models, ensure your SurrealDB schema supports the chosen dimension fields.

## LLM providers (built-in agentic tools)

The agentic MCP tools run a built-in agent server-side. That agent needs an LLM provider.

Important: the LLM provider factory requires `llm.enabled = true`. If it is `false`, agentic tools fail with “LLM is not enabled in configuration”.

You can enable it by:

- setting `llm.enabled = true` in your TOML config, or
- setting `CODEGRAPH_MODEL=...` (this env var implicitly enables the LLM configuration)

Valid values for `llm.provider` (availability depends on build features):

- `ollama`
- `lmstudio`
- `anthropic`
- `openai`
- `xai`
- `openai-compatible`

### `llm.provider = "ollama"`

Uses Ollama’s OpenAI-compatible endpoint at `<ollama_url>/v1`.

Config inputs:

- `llm.ollama_url` (default `http://localhost:11434`)
- `llm.model` (e.g. `qwen2.5-coder:14b`)

### `llm.provider = "lmstudio"`

Uses LM Studio’s OpenAI-compatible endpoint at `<lmstudio_url>/v1`.

Config inputs:

- `llm.lmstudio_url` (default `http://localhost:1234`)
- `llm.model` (your local model id/name)

### `llm.provider = "openai"`

Config inputs:

- `OPENAI_API_KEY` (or `llm.openai_api_key`)
- `llm.model`

Optional:

- `OPENAI_ORG_ID`
- `llm.reasoning_effort` (for reasoning-style models)

### `llm.provider = "xai"`

Config inputs:

- `XAI_API_KEY` (or `llm.xai_api_key`)
- `llm.model`
- `llm.xai_base_url` (default `https://api.x.ai/v1`)

### `llm.provider = "anthropic"`

Config inputs:

- `ANTHROPIC_API_KEY` (or `llm.anthropic_api_key`)
- `llm.model`

### `llm.provider = "openai-compatible"`

Use this for OpenAI-shaped APIs (self-hosted gateways, proxies, etc.).

Config inputs:

- `llm.openai_compatible_url` (must be set)
- `llm.model` (required)
- `llm.openai_api_key` (optional; depends on your endpoint)

### Context window and tier selection

The 4-tier prompt system uses `llm.context_window` (or `CODEGRAPH_CONTEXT_WINDOW`) to decide how verbose prompts can be and how much to retrieve. See `docs/AGENT_PROMPT_TIERS.md`.

## Graph schema selection (optional)

If you want CodeGraph to use the experimental graph schema database for agentic tools and indexing, set:

- `CODEGRAPH_USE_GRAPH_SCHEMA=true`
- `CODEGRAPH_GRAPH_DB_DATABASE=codegraph_experimental` (or your chosen db name)

The schema still must be applied manually to that database (see `docs/INSTALLATION_GUIDE.md` and `schema/codegraph_graph_experimental.surql`).
