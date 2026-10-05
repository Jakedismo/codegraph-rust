# CodeGraph agentic CLI

Use CodeGraph's built-in reasoning agents for code discovery, change impact,
architecture, and quality questions. They gather evidence with internal graph tools
and return synthesized answers with source locations. Call these public commands:

| Need | Command | Optional `--focus` |
| --- | --- | --- |
| Find code or gather context | `codegraph agent context "question or task"` | `search`, `builder` (default), `question` |
| Assess a proposed change | `codegraph agent impact "what would change?"` | `dependencies` (default), `call_chain` |
| Explain structure/interfaces | `codegraph agent architecture "area to understand"` | `structure` (default), `api_surface` |
| Identify quality risks | `codegraph agent quality "area to assess"` | `complexity`, `coupling`, `hotspots` (all use the quality workflow) |

Examples:
```sh
codegraph agent context "Where is request authentication implemented?" --focus search
codegraph agent impact "What breaks if token validation changes?" --focus call_chain
codegraph agent architecture "Describe the public authentication interfaces" --focus api_surface
codegraph agent quality "Find coupling and hotspots in authentication" --focus coupling
```

Run from the indexed project root, or pass `--project /path/to/project`. For a custom
indexed identifier, pass `--project-id ID` or retain `CODEGRAPH_PROJECT_ID`. Existing
CodeGraph provider/database configuration applies; no running MCP server is needed.
The binary must be built with `ai-enhanced` (or `full`), and the project must already
be indexed in SurrealDB. Calls use the configured model and can incur provider costs.

Stdout is one JSON response with `answer`, `findings`, workflow metadata, and
`structured_output` when available, matching MCP. Read source locations and risk
notes from this evidence. `--format text` prints just the answer; use JSON when
checking warnings and partial results. Logs go to stderr. Runtime failures emit
`{"error": {...}}` and exit 1; argument errors exit 2. The default command deadline
is 300 seconds; override with `--timeout-secs N`. Use `--query-file FILE` instead of
a positional query for long questions, or `--query-file -` to read stdin.

Start with a specific question including relevant symbols, paths, or the intended
change. Follow returned file:line locations with targeted reads before editing.
Verify the answer against current code: an index can lag behind uncommitted work,
and a successful response can contain a timeout/partial-result warning. If the
index or service is unavailable, results lack evidence, or a location is already
known, use ordinary repository tools. Do not install, configure, reindex, or call
models merely because this guidance was injected. Internal graph analysis tools
are for CodeGraph's built-in agents; use only the four agentic commands above.

Reload this guidance with `codegraph agent instructions` after context loss.
