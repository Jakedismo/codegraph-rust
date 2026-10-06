# Agent memory: design

Status: proposal, not implemented. This records the design agreed in October 2026 so it can
be iterated on. Decisions are marked as such; open questions are listed at the end.

## Summary

CodeGraph becomes a memory store for the agents that use it. The agent holding the
conversation (Claude Code, Codex, or any MCP client) decides what is worth remembering and
writes it as plain sentences. CodeGraph stores those sentences, anchors them to the code graph
it already has, and serves them back through one retrieval tool that combines semantic search,
graph traversal and reciprocal rank fusion. CodeGraph never writes memories on its own and
never injects them into answers unless asked.

## Division of responsibility (decided)

| Concern | Owner | Why |
| --- | --- | --- |
| Deciding what is worth storing | The using agent | It has the conversation history and knows what it did and why. CodeGraph's internal agent answers one question at a time with no history and cannot judge novelty. |
| Phrasing the memory | The using agent | Memories are insights ("the batch-size precedence lives in `ConfigManager`"), not records of inputs. Raw queries, answers and transcripts are never stored. |
| Anchoring a memory to code | CodeGraph | The writer knows nothing about the schema. CodeGraph resolves identifiers in the sentence to `nodes` records and creates the links. |
| Storage, embedding, deduplication, retrieval, staleness | CodeGraph | These need the graph, the vector index and the project scope that already exist in the store. |
| Deciding when to write | Guidance to the using agent (`rules-for-claude-code/codegraph_rule.md`, hooks installed by `codegraph init`) | The discipline lives in the writer, not in the store. |

Consequences:

- CodeGraph runs no background LLM for memory. Its only inference is embedding the sentence
  with the project's configured embedding model.
- No session correlation is attempted. A memory is what the agent chose to say, at the time it
  said it.
- The agentic tools (`agentic_context` and friends) do not return memories. The using agent's
  context already contains what it has been doing; it asks for memories when it wants them.

## Non-goals (decided)

- Storing queries, answers, diffs or transcripts. The database must grow with insight, not with
  traffic.
- Automatic observation by CodeGraph's internal agent, or a post-answer "what did we learn"
  step. Both were considered and rejected: without conversation history they produce
  restatements of graph results, and they spend inference on every question.
- A background consolidation job that merges memories with an LLM. Write-time deduplication
  and explicit supersede keep the store bounded without it. If merging is ever needed, it is an
  explicit tool call, not a scheduled process.
- A fixed taxonomy of memory kinds. The writer expresses tier through scope and wording.

## Scopes (decided)

Two scopes, two stores:

- `project`: knowledge about this codebase (practices observed, decisions, what changed where
  and why). Lives in the project's embedded store, `<project>/.codegraph/db`, next to the code
  graph, so memories can link to `nodes` records and are deleted with the project store.
- `user`: knowledge about the person (preferences, working style). Cross-project, so it lives in
  a separate embedded store under `~/.codegraph/` and is never written into a repository's
  store. User memories have no node links.

Both stores sit behind the same single-process lock as everything else; the MCP server is the
only writer and reader, which is the intended deployment.

## Data model

Tables in the project store (user store has `memories` only, without the link tables).

```
memories
  id              record
  statement       string          -- the sentence as written, unmodified
  scope           'project' | 'user'
  tags            array<string>   -- free-form, optional
  embedding_<dim> option<array<float, dim>>   -- same dimension columns and HNSW indexes as nodes
  created_at      datetime
  updated_at      datetime
  status          'active' | 'superseded'
  stale           bool            -- true when any linked node was replaced or removed since created_at
  project_id      option<string>  -- NONE for user scope
  written_by      option<string>  -- client-reported agent name, informational only

memory_links                      -- memory -> code anchor, built by CodeGraph at write time
  memory          record<memories>
  node            record<nodes>   -- no cascade: the link is what detects staleness
  symbol          string          -- the identifier as it appeared in the sentence
  method          'backtick' | 'name_index' | 'semantic'
  confidence      float
  node_updated_at datetime        -- nodes.updated_at when the link was made

memory_supersedes                 -- new memory -> the one it replaces
  from            record<memories>
  to              record<memories>
```

Indexes: HNSW on each `embedding_<dim>` column of `memories`; FULLTEXT on `statement` with the
`code_text` analyzer; `(project_id, status)`; `(memory)` and `(node)` on `memory_links`.
Memories reuse the `knn_*` dimension dispatch pattern from `codegraph_v2.surql`.

Why links are not `REFERENCE ON DELETE CASCADE`: the indexer deletes and re-creates a changed
file's nodes under the same ids. A cascade would drop the anchor exactly when it should mark
the memory stale (see `bcc8154` for the same lesson with edges).

## Write path: `memory_write`

Input from the agent:

```
{ "statement": "...", "scope": "project" | "user", "tags": [..]?, "replace": "<memory id>"? }
```

Steps, all inside CodeGraph:

1. Embed the statement with the configured embedding model (query task, same as search).
2. Deduplicate. Search the nearest active memories in the same scope (HNSW, top 3). If the best
   cosine similarity is above a threshold (start at 0.92) and `replace` was not given, do not
   insert; return `{ "duplicate_of": <id>, "statement": <existing> }` so the agent can keep the
   old one or call again with `replace`.
3. Anchor (project scope only). Extract candidate identifiers from the statement:
   backticked spans first; then tokens that look like identifiers (snake_case, CamelCase,
   paths with `/` or `::`). Resolve each against the project: exact name match through
   `fn::find_nodes_by_name`, then a semantic search over nodes for the full sentence for up to
   three more anchors. Record each link with its method and confidence. Unresolved identifiers
   stay as text only.
4. Insert the memory; if `replace` was given, mark the old memory `superseded` and add a
   `memory_supersedes` edge. The old statement is kept for history, never returned by default.
5. Return `{ "id", "links": [{ "symbol", "file_path", "start_line", "method" }] }` so the agent
   sees what the memory was anchored to and can correct a bad anchor by rewriting.

The write never calls an LLM.

## Read path: `memory_read`

Input from the agent:

```
{ "query": "...", "scope": "project" | "user" | "both" = "both", "limit": 10,
  "since": "<ISO datetime>"?, "symbol": "<name>"?, "include_stale": false }
```

Three ranked lists, fused with `search::rrf` (the same mechanism `fn::semantic_search_nodes_via_chunks` uses in v2):

1. Vector neighbours of the query over `memories.embedding_<dim>`.
2. BM25 over `memories.statement`.
3. Graph list: run the project's semantic code search for the query, take the returned node ids
   and their one-hop neighbours over dependency edges, and collect the memories linked to any of
   them. This is what lets "what do we know about config loading" find a memory that never used
   those words but is anchored next to `load_config`.

`symbol` short-circuits list 3 to the memories linked to that node and its neighbours.
`since` filters on `created_at`. Stale memories are excluded unless requested.

Each result carries: `statement`, `created_at`, `stale`, `tags`, and `links` with current
file:line locations resolved through the node, so the reader can verify before relying on it.
Superseded memories are not returned; their successor is.

## Staleness

After every re-index, a cheap pass compares `memory_links.node_updated_at` with the current
`nodes.updated_at` and sets `memories.stale = true` where a linked node changed or disappeared.
Stale memories are still readable with `include_stale`; the writer can confirm (which refreshes
the link timestamps), replace, or ignore them. Nothing is deleted automatically.

User-scope memories have no links and never go stale by this mechanism; they are only ever
replaced by the writer.

## MCP surface

Two tools in `official_server.rs`, in the same `#[tool_router]` as the agentic tools:

- `memory_write` — see write path.
- `memory_read` — see read path.

Both available on the `codegraph agent` CLI as `codegraph memory write` / `codegraph memory read`
for scripts and hooks. Neither requires the LLM feature; they need embeddings only.

## Guidance for writing agents (goes in `codegraph_rule.md`)

Write a memory when: a task is finished (what changed and why, one or two sentences); a finding
surprised you or contradicted documentation; a decision was made that a future session would
otherwise re-litigate; a user preference became clear (user scope). Do not write restatements of
code that the graph already answers, and do not write progress notes. Read memories at the start
of a task and before changing an area you have not touched this session.

## Implementation order

1. Schema: `memories`, `memory_links`, `memory_supersedes` in `codegraph_v2.surql`, plus
   `fn::memory_search` (the three-list RRF) and `fn::memories_for_nodes`. Unit test on `mem://`
   in the style of `schema_v2_test.rs`.
2. Storage: write path (embed, dedupe, anchor, insert) and the staleness pass hooked into the
   end of `index_project` in `codegraph-graph` / `codegraph-mcp`.
3. User store: `SurrealDbConfig::user_store()` under `~/.codegraph/`, opened lazily, same
   bootstrap and lock handling as the project store.
4. MCP tools and CLI subcommands.
5. Guidance text and the hook that reminds the agent to write at task end.

## Open questions

- Dedup threshold and whether near-duplicates should merge tags into the kept memory.
- Whether anchors resolved semantically (not by name) should count toward staleness, or only
  exact-name anchors. Start with all anchors and watch for false stale flags.
- Whether `memory_read` should accept a node id directly, for callers that already have one
  from an agentic answer.
- Retention for superseded memories (keep forever, or drop after N successors).
- Multi-agent attribution: `written_by` is informational now; whether it should become a
  filter depends on how teams use shared project stores.
