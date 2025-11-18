# Codex Journal

## 2025-11-18
- Added `surreal_embedding_column_supports_2560_dimension` test and wired `SurrealEmbeddingColumn` to accept the 2,560-d slot in `crates/codegraph-mcp/src/indexer.rs` so qwen3 embeddings no longer panic.
- Verified with `cargo test -p codegraph-mcp --lib surreal_embedding_column_supports_2560_dimension`; still seeing external warnings from `server.rs`.
