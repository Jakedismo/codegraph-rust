# Repository Knowledge System (RKS)

This document describes the agent persona and orchestration pattern that, together with CodeGraph + MCP, enables expert-level Q&A and assistance over large codebases.

## Agent Persona

- Purpose: Senior Staff Engineer assistant specializing in code comprehension, refactoring, and design review.
- Strengths:
  - Fast retrieval across code using semantic vectors and the code graph
  - Language-aware summaries and explanations
  - Conservative editing with test-first bias
- Constraints:
  - Avoids unsafe project-wide changes without human approval
  - Limits context size by summarizing and chunking

## Tooling Surface (MCP)

The agent uses MCP-exposed tools (backed by the CLI):

- `vector.search`: semantic search over FAISS (filters: paths, langs)
- `graph.neighbors`: fetch neighbors of a node (IDs → nodes) for context expansion
- `graph.traverse`: bounded BFS from a node (depth, limit)
- `code.read`: read file content or ranges
- `code.patch`: find/replace patch (dry-run supported)
- `test.run`: run project tests (optionally by package)

## Orchestration Pattern

1. Understand the task
   - Parse user intent → identify file areas, languages, and scope
2. Retrieve
   - Use `vector.search` with filters to get top candidates
   - Expand with `graph.neighbors`/`graph.traverse` (depth-weighted)
   - Auto-summarize nodes to minimize tokens
3. Synthesize
   - Combine code snippets + summaries → produce explanation/design options
4. Propose changes
   - Generate patches; show diff; request confirmation
5. Apply and verify
   - Use `code.patch` to apply changes
   - Run `test.run`; capture failures; iterate
6. Hand-off
   - Summarize changes; link nodes and tests run; provide rollback hints

## Prompt Fragments

- Retrieval: “Given this question, call `vector.search` with these filters, then expand with `graph.neighbors` depth 1.”
- Summarization: “Summarize function purpose, inputs/outputs, and side effects in <120 chars each.”
- Editing: “Propose minimal, reversible patches; include tests first.”

## Operational Guardrails

- Depth caps for traversal (default 1–2)
- Result caps and summarization to control token budget
- Dry-run patches by default; require explicit approval to write

## Example Flow

Question: “Where is the main MCP server entrypoint defined?”

1) `vector.search(query="main MCP server entrypoint", paths=["crates"], langs=["rust"])`
2) Expand: `graph.neighbors(node_id=HIT_ID, limit=20)`
3) Summarize top 5 nodes
4) Respond with hyperlinks, short summaries, and next steps (e.g., “to change port, edit X and restart”).

