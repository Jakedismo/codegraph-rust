# MCP Tools for CodeGraph

This document describes the tool surface exposed by CodeGraph for AI agents. Each tool includes guidance for when to use it and example calls.

## graph.neighbors
- Purpose: Fetch outgoing neighbors of a node (IDs and optional node data).
- Use when: You need immediate context around a known symbol or search hit.
- Arguments:
  - `node`: UUID string of the node
  - `limit`: max neighbors (default 20)
- Returns: `{ neighbors: [{ id, name, path }] }`
  - Depth: Implicitly 1; for multi-hop, see `graph.traverse`.

## graph.traverse
- Purpose: Bounded breadth-first traversal.
- Use when: Expanding context beyond direct neighbors.
- Arguments:
  - `start`: UUID
  - `depth`: 1–3 recommended
  - `limit`: max nodes to visit
- Returns: Ordered list with `(id, depth)` so shallow nodes rank higher.

## vector.search
- Purpose: Semantic search over FAISS index.
- Use when: Locating relevant code across the repo quickly.
- Arguments:
  - `query`: string
  - `paths`: optional path prefixes to shard search
  - `langs`: optional languages to shard search
  - `limit`: number of results (default 10)
- Returns: List of `{ id, name, path, depth, summary }` (depth=0; if you then expand with `graph.traverse`, depth>0 entries appear).

## code.read
- Purpose: Read a file or a specific line range.
- Use when: Inspecting the full source of a candidate node.
- Arguments:
  - `path`: file path
  - `start`, `end`: optional 1-based line range
- Returns: Annotated text with line numbers.

## code.patch
- Purpose: Apply a simple find/replace patch.
- Use when: Small, safe edits or to prepare a larger change.
- Arguments:
  - `path`: file path
  - `find`: literal string
  - `replace`: replacement
  - `dry_run`: if true, print counts, do not write
- Returns: Status; number of replacements (dry-run).

## test.run
- Purpose: Execute tests to validate changes.
- Use when: After patching or before proposing edits.
- Arguments:
  - `package`: optional crate/package name
  - `args`: trailing extra test args
- Returns: Exit status; stream logs.

## Selection Guidance
- Start with `vector.search` to find candidates → expand with `graph.traverse` (depth=1–2).
- Use `graph.neighbors` for high-precision context of a known node.
- Always `code.read` before editing; prefer `code.patch --dry-run` first.
- After edits, `test.run` to verify; iterate if needed.

