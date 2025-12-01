ABOUTME: Analysis of watch-mode incremental updates and embedding refresh
ABOUTME: Current gaps and risks

Symptom / Need
--------------
Watch mode (`codegraph start stdio --watch` / daemon) should keep SurrealDB in sync when files change: re-parse, re-embed, and overwrite nodes/edges/symbol embeddings/file metadata. Reports of stale context suggest gaps in the current pipeline.

Observed architecture (today)
-----------------------------
- Watch daemon (codegraph-mcp-daemon) fires change events and drives reindex of changed paths.
- Incremental path in `ProjectIndexer`:
  - Detects changed files and re-parses them.
  - Persists nodes, edges, embeddings, and file metadata via Surreal writer (nodes/edges) and direct writes (metadata).
  - Deletes stale nodes/edges for the file only when `--force` or when the file is removed (partial cleanup).
- Symbol embeddings and node embeddings are regenerated in full runs; incremental path may skip symbol embedding refresh.

Gaps / Risks
------------
1) **Stale nodes/edges**: IDs are UUID-based; reindexing the same file can append new nodes/edges without deleting the old ones, leading to duplicates and inflated edge counts.
2) **Embeddings not refreshed**: Incremental path may not regenerate node embeddings or symbol embeddings for changed files, leaving stale vectors.
3) **Metadata divergence**: file_metadata/project_metadata are written directly, not through the async writer; failures aren’t surfaced consistently, and counts can mismatch.
4) **Delete handling**: File deletions rely on explicit delete path; if events are missed or deletion fails, orphaned nodes/edges remain.
5) **Event noise**: Rapid change bursts may enqueue multiple reindexes; lack of coalescing can race and leave mixed states.
6) **Diagnostics**: No per-change summary (how many nodes/edges/embeddings deleted/inserted); hard to confirm correctness.

Key improvements (linked to spec)
---------------------------------
- Make incremental updates deterministic: delete old nodes/edges for a file before inserting new, or compute stable IDs from file_path+position.
- Regenerate and overwrite node embeddings and symbol embeddings for changed files (honor current dimension/provider).
- Move file/project metadata writes onto the async writer and add post-write verification.
- Add unresolved/duplicate checks under CODEGRAPH_DEBUG to spot stale data.
- Debounce/coalesce events; process latest snapshot per file.

Validation plan
---------------
- Integration tests: modify file → assert single set of nodes/edges/embeddings; delete file → assert rows removed.
- Surreal checks after change: counts per project, absence of duplicate file_path rows, embeddings updated.
