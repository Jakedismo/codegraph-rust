# CodeGraph configuration

`example.toml` in this directory is the reference configuration file. Copy it to one of:

- `./.codegraph.toml` in a project (used when present), or
- `~/.codegraph/config.toml` for all projects (`codegraph config init` creates this file with
  every default written out).

A `.env` file in the working directory (then `~/.codegraph.env`) and `CODEGRAPH_*` environment
variables override the config file. `../.env.example` lists the environment variables.
For a file named `codegraph.toml`, select it explicitly with
`--config codegraph.toml` or `CODEGRAPH_CONFIG_PATH=codegraph.toml`.

## What the config file controls

| Section | Effect |
| --- | --- |
| `[embedding]` | Embedding provider, model, dimension, batch size and provider URLs used by indexing and search. |
| `[rerank]` | Optional query-time semantic-search reranking (`none`, `jina`, `ollama`), independently of embeddings. Provider defaults are supplied; nested `[rerank.jina]` and `[rerank.ollama]` blocks customize them. |
| `[llm]` | Provider, model, context window and endpoint URLs for the agentic tools. Environment variables win over these keys; `enabled = false` makes the agent ignore the section; API keys come from the environment only. |
| `[indexing]` | Default indexing tier (`fast`, `balanced`, `full`). |
| `[daemon]` | File-watcher behaviour for `codegraph start --watch` and `codegraph daemon`. |

For Jina reranking, set `[rerank] provider = "jina"` and provide `JINA_API_KEY`
in the environment. Model and credential-variable names default to
`jina-reranker-v3` and `JINA_API_KEY`; optional `[rerank.jina] api_key_env` names a
different credential variable. These credentials are needed for retrieval, not indexing.
`JINA_API_BASE` overrides the reranking endpoint as well as the embedding endpoint.

`CODEGRAPH_RERANK_PROVIDER` overrides the TOML provider and legacy
`JINA_ENABLE_RERANKING` toggle. `CODEGRAPH_ENABLE_RERANKING=false` disables all
providers. Ollama model/endpoint overrides use `CODEGRAPH_OLLAMA_RERANK_MODEL`
and `CODEGRAPH_OLLAMA_URL`, ahead of `OLLAMA_RERANK_MODEL` and `OLLAMA_URL`.
Changes require restarting the process, without reindexing.

The loader also accepts `[performance]` and `[logging]`. Unknown sections and keys
are ignored silently.

Storage is not configured here. Each project uses an embedded store at
`<project>/.codegraph/db`; set `CODEGRAPH_SURREALDB_URL` to use a SurrealDB server instead.

## Checking what is in effect

```bash
codegraph config show          # merged configuration
codegraph config show --json   # resolved reranker provider/model and retained-result count
codegraph config agent-status  # provider, model, context tier the agent will use
codegraph config validate
```

See `../docs/INSTALLATION_GUIDE.md#configuration` and `../docs/AI_PROVIDERS.md` for
per-provider examples.
