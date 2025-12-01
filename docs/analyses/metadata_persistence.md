ABOUTME: Investigation—why file_metadata/project_metadata tables are empty
ABOUTME: Symptom and likely causes + next validation steps

Symptom
-------
- Index run logged: `📦 Metadata persisted: 318 files | 17010 edges | 13852 nodes` (2025-12-01T22:19:39Z).
- When inspecting SurrealDB, `file_metadata` and `project_metadata` tables are empty.

What the code does today (paths)
--------------------------------
- File metadata: `ProjectIndexer::persist_file_metadata` builds `FileMetadataRecord`s and calls `SurrealDbStorage::upsert_file_metadata_batch` (schema path: `schema/codegraph.surql`, table `file_metadata` SCHEMAFULL, unique idx on project_id + file_path).
- Project metadata: `persist_project_metadata` builds `ProjectMetadataRecord` then `SurrealDbStorage::upsert_project_metadata`.
- Both use the same live Surreal connection (`self.surreal`), not the async writer queue, so failures should surface as errors.

Likely root causes (ranked)
---------------------------
1) **Namespace/database mismatch at runtime**  
   - `CODEGRAPH_SURREALDB_{URL,NAMESPACE,DATABASE}` default to `ws://localhost:3004`, `codegraph`, `main`. If the index run pointed to one DB/namespace and inspection was against another, tables will appear empty.
2) **Schema not applied in the target DB**  
   - With `SCHEMAFULL`, missing table/fields can make UPSERT fail. If `schema/codegraph.surql` wasn’t applied to the namespace being written, the UPSERTs may error out. Errors would surface, but if indexing was run via a wrapper that hides stderr, they might go unnoticed.
3) **UPSERT silently rejected due to scope/auth**  
   - If Surreal auth isn’t set (username/password unset) and the server requires it, the connection may be read-only. Nodes/edges may succeed via a different scope while metadata tables are blocked; or writes could fail when switching database/namespace mid-session.
4) **Project_id mismatch in queries**  
   - Records use `project_id` = `CODEGRAPH_PROJECT_ID` or the canonical project_root path. If later queries use a different `project_id` (e.g., relative path vs absolute), counts will show 0 even though rows exist.
5) **Batch UPSERT error not logged**  
   - `upsert_file_metadata_batch` and `upsert_project_metadata` bubble errors, but the indexing log prints success before the Surreal calls are validated. A single failing UPSERT would still leave the “persisted” log misleading.

Quick validation steps
----------------------
- Confirm you’re inspecting the same namespace/database:  
  `surreal sql --conn $CODEGRAPH_SURREALDB_URL "USE NS $CODEGRAPH_SURREALDB_NAMESPACE DB $CODEGRAPH_SURREALDB_DATABASE; SELECT count() FROM file_metadata;"`.
- Check for rows with any project_id:  
  `SELECT project_id, count() FROM file_metadata GROUP BY project_id;`
- Run a manual UPSERT to see if the table rejects writes:  
  `UPSERT file_metadata:debug CONTENT { file_path: "ping", project_id: "ping", content_hash: "x", modified_at: time::now(), file_size: 1, last_indexed_at: time::now(), node_count: 0, edge_count: 0 };`
- Re-run indexing once with `RUST_LOG=info,codegraph_mcp=debug CODEGRAPH_DEBUG=1` and capture stderr—Surreal errors should appear if schema/auth mismatches exist.

Remediation options
-------------------
- Add a post-write check: after `upsert_file_metadata_batch`/`upsert_project_metadata`, query `count()` and log errors if zero—fail the index run on metadata write failure.
- Log the effective Surreal connection triple (URL/ns/db) alongside project_id when printing “Metadata persisted”.
- Expose a `codegraph debug db-check` command to verify schema presence and write permissions (attempt a canary UPSERT/DELETE in each table).
- If multiple namespaces are expected, allow `--surreal-namespace`/`--surreal-database` CLI flags to eliminate env drift.
- Schema parity check: indexer expects `file_metadata` and `project_metadata` tables to exist (SCHEMAFULL). Ensure schema applied matches the current definitions in `schema/codegraph.surql` (composite unique index on project_id+file_path, datetime fields present).
- Migrate metadata writes onto the same async Surreal writer used for nodes/edges to get uniform error handling, batching, and back-pressure. This also centralizes durability checks and makes “flush” semantics consistent across all tables.
