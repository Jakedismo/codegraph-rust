# CodeGraph AI providers: embeddings + LLMs

This document explains how to configure CodeGraph’s embedding providers (for indexing/search) and LLM providers (for the built-in agentic tools).

It is based on the runtime configuration loader in `crates/codegraph-core/src/config_manager.rs` and provider implementations in:

- `crates/codegraph-vector/src/*` (embeddings)
- `crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs` (the agent's LLM provider, model and endpoints)

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

1. The project has been indexed (the embedded store needs no setup; a SurrealDB server needs the schema applied, see `docs/INSTALLATION_GUIDE.md`).
2. You have a `./.codegraph.toml` or `~/.codegraph/config.toml` with at least:
   - `[embedding] provider = ...`
   - an LLM for the agentic tools: `[llm] provider` and `model` in the config file, or `CODEGRAPH_LLM_PROVIDER` and `CODEGRAPH_LLM_MODEL` in the environment, plus the provider's API key in the environment
3. Secrets are present via `.env` or your shell environment (recommended).

## `.env` examples

### Local (Ollama embeddings + Ollama LLM)

```bash
# SurrealDB server (optional; omit to use the embedded per-project store)
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
CODEGRAPH_LLM_MODEL=qwen2.5-coder:14b
```

### Local (LM Studio embeddings + LM Studio LLM)

```bash
CODEGRAPH_EMBEDDING_PROVIDER=lmstudio
CODEGRAPH_EMBEDDING_MODEL=jinaai/jina-embeddings-v3

CODEGRAPH_LLM_PROVIDER=lmstudio
CODEGRAPH_LLM_MODEL=local-model
```

Note: the embedding endpoint is `embedding.lmstudio_url` in TOML. The agent's LM Studio endpoint is `LMSTUDIO_URL` / `CODEGRAPH_LMSTUDIO_URL`, or `llm.lmstudio_url` in TOML.

### Remote (Jina embeddings + OpenAI LLM)

```bash
CODEGRAPH_EMBEDDING_PROVIDER=jina
JINA_API_KEY=...
CODEGRAPH_EMBEDDING_MODEL=jina-embeddings-v4
JINA_API_BASE=https://api.jina.ai/v1

CODEGRAPH_LLM_PROVIDER=openai
OPENAI_API_KEY=...
CODEGRAPH_LLM_MODEL=gpt-5.1-codex
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
context_window = 2000000
```

`XAI_API_KEY` and `JINA_API_KEY` go in the environment.

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

The agentic tools run a built-in agent (the Rig backend in `crates/codegraph-mcp-rig`). The built-in agent resolves each LLM setting in three steps: the environment variable (a project `.env` counts), then the key in the config file's `[llm]` section, then a default.

| Setting | Environment variable | `[llm]` key | Default |
| --- | --- | --- | --- |
| Provider | `CODEGRAPH_LLM_PROVIDER` | `provider` | inferred from whichever API key is set |
| Model | `CODEGRAPH_LLM_MODEL`, then `CODEGRAPH_AGENT_MODEL`, then `CODEGRAPH_MODEL` | `model` | the provider's built-in default |
| Context window | `CODEGRAPH_CONTEXT_WINDOW` | `context_window` | 128000 |
| Ollama endpoint | `OLLAMA_API_BASE_URL`, `OLLAMA_API_URL`, `OLLAMA_HOST` | `ollama_url` | `http://localhost:11434` |
| LM Studio endpoint | `LMSTUDIO_URL`, `CODEGRAPH_LMSTUDIO_URL` | `lmstudio_url` (`/v1` is appended) | `http://localhost:1234/v1` |
| OpenAI-compatible endpoint | `CODEGRAPH_OPENAI_COMPATIBLE_URL` | `openai_compatible_url` | `http://localhost:1234/v1` |
| API keys | `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `XAI_API_KEY`, `OPENAI_COMPATIBLE_API_KEY` | not read | none |

Set the model explicitly. Without one the agent requests its provider's built-in default (`gpt-4o` for OpenAI, `claude-sonnet-4-20250514` for Anthropic, `llama3.2` for Ollama, `grok-3-latest` for xAI).

`[llm] enabled = false` makes the agent ignore the whole section. `codegraph config init` writes a config file with every default spelled out and `enabled = false`, so a generated file does not select a provider until you set `enabled = true` or remove the line. Other `[llm]` keys (`temperature`, `max_tokens`, `timeout_secs`, `reasoning_effort`, `xai_base_url`, the `*_api_key` fields) are accepted by the loader but do not reach the agent.

Check the result with `codegraph config agent-status`.

Valid providers (availability depends on build features): `ollama`, `lmstudio`, `anthropic`, `openai`, `xai`, `openai-compatible`.

### Ollama

```toml
[llm]
provider = "ollama"
model = "qwen2.5-coder:14b"
context_window = 32768
# ollama_url = "http://localhost:11434"
```

### LM Studio

```toml
[llm]
provider = "lmstudio"
model = "your-local-model-id"
# lmstudio_url = "http://localhost:1234"
```

### Anthropic, OpenAI, xAI

```toml
[llm]
provider = "anthropic"            # or "openai" / "xai"
model = "claude-sonnet-4"
context_window = 200000
```

with the key in the environment: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` or `XAI_API_KEY`.

### OpenAI-compatible

For OpenAI-shaped APIs (self-hosted gateways, proxies):

```toml
[llm]
provider = "openai-compatible"
model = "served-model-name"
openai_compatible_url = "http://localhost:8000/v1"
```

The key, if the endpoint needs one, comes from `OPENAI_COMPATIBLE_API_KEY` or `OPENAI_API_KEY`.

### Context window and tier selection

The context window decides the prompt tier, the tool-round budget and the size limit for tool results. Set it to your model's real limit. See `docs/AGENT_PROMPT_TIERS.md`.

## Graph schema selection (optional)

To use the experimental graph schema, set `CODEGRAPH_USE_GRAPH_SCHEMA=true` before the project's embedded store is created (or delete `<project>/.codegraph/db` and re-index).

With a SurrealDB server, also set `CODEGRAPH_GRAPH_DB_DATABASE=codegraph_experimental` (or your chosen database name) and apply `schema/codegraph_graph_experimental.surql` to that database yourself (see `docs/INSTALLATION_GUIDE.md`).
