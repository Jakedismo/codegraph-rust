# CodeGraph configuration

`example.toml` in this directory is the reference configuration file. Copy it to one of:

- `./.codegraph.toml` in a project (used when present), or
- `~/.codegraph/config.toml` for all projects (`codegraph config init` creates this file with
  every default written out).

A `.env` file in the working directory (then `~/.codegraph.env`) and `CODEGRAPH_*` environment
variables override the config file. `../.env.example` lists the environment variables.

## What the config file controls

| Section | Effect |
| --- | --- |
| `[embedding]` | Embedding provider, model, dimension, batch size and provider URLs used by indexing and search. |
| `[llm]` | Provider, model, context window and endpoint URLs for the agentic tools. Environment variables win over these keys; `enabled = false` makes the agent ignore the section; API keys come from the environment only. |
| `[indexing]` | Default indexing tier (`fast`, `balanced`, `full`). |
| `[daemon]` | File-watcher behaviour for `codegraph start --watch` and `codegraph daemon`. |

The loader also accepts `[rerank]`, `[performance]` and `[logging]`, but nothing outside
`codegraph config show` / `agent-status` reads them at present. Unknown sections and keys are
ignored silently.

Storage is not configured here. Each project uses an embedded store at
`<project>/.codegraph/db`; set `CODEGRAPH_SURREALDB_URL` to use a SurrealDB server instead.

## Checking what is in effect

```bash
codegraph config show          # merged configuration
codegraph config agent-status  # provider, model, context tier the agent will use
codegraph config validate
```

See `../docs/INSTALLATION_GUIDE.md#configuration` and `../docs/AI_PROVIDERS.md` for
per-provider examples.
