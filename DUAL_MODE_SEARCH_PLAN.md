# Dual-Mode Search Architecture Implementation Plan

## Overview

This plan implements a dual-mode search architecture for CodeGraph MCP that supports both:
- **Local Mode**: FAISS in-memory vector search (existing)
- **Cloud Mode**: SurrealDB HNSW indexes + Jina embeddings + Jina reranking (new)

Mode is automatically detected from `CODEGRAPH_EMBEDDING_PROVIDER` environment variable.

## Critical Context

### Problem Statement
- Current system returns empty results when FAISS is disabled (`server.rs:660-664`)
- Jina integration was incomplete - embedding provider worked but reranking never triggered
- System assumed FAISS would always be available
- No support for cloud-based vector search with SurrealDB

### Architecture Discovery
- SurrealDB supports HNSW (Hierarchical Navigable Small World) indexes for efficient vector search
- Query syntax: `WHERE embedding <|K,EF|> $query_vector` (K=results, EF=search breadth)
- `vector::distance::knn()` reuses pre-computed distance from HNSW search (efficient!)
- BM25 full-text search available for keyword matching
- Hybrid search fusion with `search::linear()` and `search::rrf()`

### Key Technical Requirements
1. Mode detection from `.env` file (CODEGRAPH_EMBEDDING_PROVIDER)
2. Preserve existing FAISS functionality (backward compatible)
3. Add SurrealDB HNSW vector search for cloud mode
4. Wire Jina reranking into search pipeline
5. Support metadata filtering (node_type, language, file_path)
6. Graceful degradation and fallback handling
7. All MCP tools must work in both modes

## Implementation Phases

### Phase 1: Configuration & Mode Detection (Foundation)
**Estimated Time**: 2-3 hours

#### Task 1.1: Environment Configuration Detection
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Create `SearchMode` enum:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    /// Local FAISS-based vector search with local embeddings
    Local,
    /// Cloud SurrealDB HNSW + Jina embeddings + reranking
    Cloud,
}
```

- Add detection function:
```rust
fn detect_search_mode() -> SearchMode {
    let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER")
        .unwrap_or_default()
        .to_lowercase();
    match provider.as_str() {
        "jina" => SearchMode::Cloud,
        "ollama" | "local" => SearchMode::Local,
        _ => SearchMode::Local, // backward compatible default
    }
}
```

- Add logging for mode detection at startup

**Tests**:
- Unit test for mode detection with different env values
- Verify default fallback to Local mode

#### Task 1.2: .env.example Documentation
**File**: `.env.example`

**Implementation**:
- Document local mode configuration:
```bash
# Local Mode (FAISS + local/ollama embeddings)
CODEGRAPH_EMBEDDING_PROVIDER=local  # or ollama
# FAISS feature required in build
```

- Document cloud mode configuration:
```bash
# Cloud Mode (SurrealDB HNSW + Jina)
CODEGRAPH_EMBEDDING_PROVIDER=jina
JINA_API_KEY=your-jina-api-key

# SurrealDB Connection (required for cloud mode)
SURREALDB_URL=ws://localhost:3004
SURREALDB_NAMESPACE=codegraph
SURREALDB_DATABASE=main
SURREALDB_USERNAME=root
SURREALDB_PASSWORD=root
```

- Add notes about HNSW index dimension matching embedding provider

---

### Phase 2: SurrealDB Vector Search Implementation
**Estimated Time**: 4-5 hours

#### Task 2.1: Schema Updates
**File**: `schema/codegraph.surql`

**Implementation**:
- HNSW index already added (line 72):
```sql
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_hnsw
ON TABLE nodes FIELDS embedding
HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
```

- Optional BM25 full-text index for hybrid search:
```sql
DEFINE ANALYZER code_analyzer
TOKENIZERS class,punct
FILTERS lowercase,ascii;

DEFINE INDEX IF NOT EXISTS idx_nodes_content_bm25
ON TABLE nodes FIELDS content
FULLTEXT ANALYZER code_analyzer BM25;
```

- Add example queries (lines 204-242 already added)

**Notes**:
- DIMENSION must match embedding provider (2048 for Jina v4)
- EFC=200, M=16 are good defaults for accuracy/speed balance

#### Task 2.2: SurrealDB Storage Methods
**File**: `crates/codegraph-core/src/storage/surrealdb_storage.rs`

**Implementation**:
- Add `vector_search_knn()` method:
```rust
pub async fn vector_search_knn(
    &self,
    query_embedding: Vec<f32>,
    limit: usize,
    ef_search: usize,
) -> Result<Vec<(String, f32)>> {
    let sql = r#"
        SELECT id, vector::distance::knn() AS score
        FROM nodes
        WHERE embedding <|$limit,$ef_search|> $query_embedding
        ORDER BY score ASC
        LIMIT $limit
    "#;

    let mut result = self.db
        .query(sql)
        .bind(("query_embedding", query_embedding))
        .bind(("limit", limit))
        .bind(("ef_search", ef_search))
        .await?;

    // Parse and return results
}
```

- Add `vector_search_with_metadata()` for filtered search:
```rust
pub async fn vector_search_with_metadata(
    &self,
    query_embedding: Vec<f32>,
    limit: usize,
    ef_search: usize,
    node_type: Option<String>,
    language: Option<String>,
    file_path_pattern: Option<String>,
) -> Result<Vec<(String, f32)>> {
    // Build dynamic WHERE clause with filters
    // Use embedding <|K,EF|> operator + metadata filters
}
```

**Tests**:
- Test vector search returns correct number of results
- Test filtering by node_type, language, file_path
- Test empty results handling

---

### Phase 3: Mode-Aware Search Router
**Estimated Time**: 5-6 hours

#### Task 3.1: Refactor bin_search_with_scores_shared()
**File**: `crates/codegraph-mcp/src/server.rs`

**Current Structure** (lines 450-664):
- Currently has `#[cfg(feature = "faiss")]` block with full implementation
- Falls back to empty results when FAISS disabled (lines 660-664)

**New Structure**:
```rust
pub async fn bin_search_with_scores_shared(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    let mode = detect_search_mode();

    match mode {
        SearchMode::Local => {
            #[cfg(feature = "faiss")]
            {
                faiss_search_impl(query, paths, langs, limit, graph).await
            }
            #[cfg(not(feature = "faiss"))]
            {
                Err(anyhow::anyhow!(
                    "Local mode requires FAISS feature. Either:\n\
                     1. Rebuild with --features faiss, or\n\
                     2. Switch to cloud mode: CODEGRAPH_EMBEDDING_PROVIDER=jina"
                ))
            }
        }
        SearchMode::Cloud => {
            cloud_search_impl(query, paths, langs, limit, graph).await
        }
    }
}
```

- Extract existing FAISS code into `faiss_search_impl()` function
- Keep all existing caching and performance optimizations

#### Task 3.2: Implement cloud_search_impl()
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
```rust
async fn cloud_search_impl(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    let start_total = Instant::now();

    // 1. Generate query embedding using Jina
    let embedding_gen = get_embedding_generator().await;
    let query_embedding = embedding_gen.generate_text_embedding(&query).await?;

    // 2. Overretrieve for reranking (3x limit)
    let overretrieve_limit = limit * 3;

    // 3. SurrealDB HNSW search with metadata filters
    let storage = graph.get_storage();
    let initial_results = storage.vector_search_with_metadata(
        query_embedding,
        overretrieve_limit,
        100, // ef_search parameter
        None, // node_type filter (from langs)
        None, // language filter
        paths.as_ref().map(|p| p.join("|")), // file path pattern
    ).await?;

    // 4. Load nodes from SurrealDB
    let node_ids: Vec<String> = initial_results.iter()
        .map(|(id, _score)| id.clone())
        .collect();
    let nodes = storage.get_nodes_by_ids(&node_ids).await?;

    // 5. Jina reranking
    let jina_provider = get_jina_provider().await?;
    let documents: Vec<String> = nodes.iter()
        .map(|n| format!("{}\n{}", n.name, n.content.as_deref().unwrap_or("")))
        .collect();

    let reranked = jina_provider.rerank(&query, documents).await?;

    // 6. Format results (top N after reranking)
    let results = reranked.iter()
        .take(limit)
        .map(|r| {
            let node = &nodes[r.index];
            json!({
                "id": node.id,
                "name": node.name,
                "path": node.location.file_path,
                "node_type": format!("{:?}", node.node_type),
                "language": format!("{:?}", node.language),
                "score": r.relevance_score,
                "summary": node.content.as_deref().unwrap_or("").chars().take(160).collect::<String>()
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "results": results,
        "performance": {
            "total_ms": start_total.elapsed().as_millis(),
            "mode": "cloud",
            "reranked": true
        }
    }))
}
```

**Tests**:
- Test cloud search returns reranked results
- Test filtering works correctly
- Test error handling when SurrealDB unavailable

---

### Phase 4: Enhanced Jina Integration
**Estimated Time**: 2-3 hours

#### Task 4.1: Jina Provider Singleton
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Add global cache for Jina provider:
```rust
#[cfg(feature = "jina")]
static JINA_PROVIDER: Lazy<tokio::sync::OnceCell<Arc<codegraph_vector::jina_provider::JinaEmbeddingProvider>>> =
    Lazy::new(|| tokio::sync::OnceCell::new());

#[cfg(feature = "jina")]
async fn get_jina_provider() -> anyhow::Result<Arc<codegraph_vector::jina_provider::JinaEmbeddingProvider>> {
    JINA_PROVIDER
        .get_or_try_init(|| async {
            tracing::info!("Initializing Jina provider for reranking");
            let config = codegraph_vector::jina_provider::JinaConfig {
                enable_reranking: true,
                reranking_model: "jina-reranker-v3".to_string(),
                reranking_top_n: 100,
                ..Default::default()
            };
            let provider = codegraph_vector::jina_provider::JinaEmbeddingProvider::new(config)?;
            Ok(Arc::new(provider))
        })
        .await
        .map(|arc| arc.clone())
}
```

**Tests**:
- Test singleton initialization
- Test reranking functionality

#### Task 4.2: MCP Tools Verification
**Files**: All MCP tool handlers in `server.rs`

**Verification**:
- Test `search_code` tool in both modes
- Test `find_similar` tool in both modes
- Test `semantic_search` tool in both modes
- Verify all return properly formatted results
- Verify performance metrics are logged correctly

---

### Phase 5: Error Handling & Fallback
**Estimated Time**: 3-4 hours

#### Task 5.1: Graceful Degradation
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Update `cloud_search_impl()` to handle failures:
```rust
async fn cloud_search_impl(...) -> anyhow::Result<Value> {
    // Try cloud search
    match attempt_cloud_search(...).await {
        Ok(results) => Ok(results),
        Err(e) => {
            tracing::error!("Cloud search failed: {}. Attempting FAISS fallback", e);

            #[cfg(feature = "faiss")]
            {
                tracing::info!("Falling back to local FAISS search");
                faiss_search_impl(query, paths, langs, limit, graph).await
            }
            #[cfg(not(feature = "faiss"))]
            {
                Err(anyhow::anyhow!(
                    "Cloud search failed and no FAISS fallback available: {}", e
                ))
            }
        }
    }
}
```

- Add retry logic for SurrealDB connection failures
- Add timeout handling for Jina API calls

#### Task 5.2: Configuration Validation
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Add startup validation function:
```rust
async fn validate_configuration() -> anyhow::Result<()> {
    let mode = detect_search_mode();

    match mode {
        SearchMode::Cloud => {
            // Check Jina API key
            if std::env::var("JINA_API_KEY").is_err() {
                return Err(anyhow::anyhow!(
                    "Cloud mode requires JINA_API_KEY environment variable"
                ));
            }

            // Check SurrealDB connection
            if std::env::var("SURREALDB_URL").is_err() {
                return Err(anyhow::anyhow!(
                    "Cloud mode requires SURREALDB_URL environment variable"
                ));
            }

            tracing::info!("✅ Cloud mode configuration validated");
        }
        SearchMode::Local => {
            #[cfg(not(feature = "faiss"))]
            {
                tracing::warn!(
                    "Local mode selected but FAISS feature not enabled. \
                     Build with --features faiss or use cloud mode."
                );
            }

            tracing::info!("✅ Local mode configuration validated");
        }
    }

    Ok(())
}
```

- Call from `serve_stdio()` and `serve_http()` on startup

**Tests**:
- Test validation catches missing API keys
- Test validation catches missing SurrealDB config
- Test validation passes with correct config

---

### Phase 6: Testing & Benchmarking
**Estimated Time**: 4-5 hours

#### Task 6.1: Unit Tests
**File**: `crates/codegraph-mcp/src/server.rs` (test module)

**Tests to Add**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_search_mode_jina() {
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "jina");
        assert_eq!(detect_search_mode(), SearchMode::Cloud);
    }

    #[test]
    fn test_detect_search_mode_local() {
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "local");
        assert_eq!(detect_search_mode(), SearchMode::Local);
    }

    #[test]
    fn test_detect_search_mode_default() {
        std::env::remove_var("CODEGRAPH_EMBEDDING_PROVIDER");
        assert_eq!(detect_search_mode(), SearchMode::Local);
    }

    // Add tests for vector search, reranking, etc.
}
```

#### Task 6.2: Integration Tests
**File**: `crates/codegraph-mcp/tests/dual_mode_search.rs` (new)

**Tests**:
- Test full local mode workflow (index → search)
- Test full cloud mode workflow (index → search → rerank)
- Test mode switching with .env changes
- Test fallback from cloud to local on error

#### Task 6.3: Performance Benchmarks
**File**: `crates/codegraph-mcp/benches/search_performance.rs` (new)

**Benchmarks**:
- Compare local vs cloud search latency
- Measure reranking overhead
- Measure HNSW vs FAISS search speed
- Measure cache hit performance

---

### Phase 7: Documentation & Migration
**Estimated Time**: 3-4 hours

#### Task 7.1: Documentation Updates
**Files**:
- `README.md`
- `docs/CLOUD_DEPLOYMENT.md` (new)
- `docs/ARCHITECTURE.md` (update)

**Content**:
- Explain dual-mode architecture
- Document configuration options
- Provide deployment examples
- Add troubleshooting guide
- Document SurrealDB HNSW setup
- Document Jina API setup and pricing

#### Task 7.2: Migration Script
**File**: `scripts/migrate_faiss_to_surrealdb.sh` (new)

**Implementation**:
- Export embeddings from FAISS indexes
- Transform to SurrealDB format
- Bulk import to SurrealDB
- Verify HNSW index creation
- Provide rollback instructions

---

### Phase 8: Optional Enhancements
**Estimated Time**: 4-6 hours (optional)

#### Task 8.1: Hybrid Search (Vector + BM25)
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Add `hybrid_search_impl()` function:
```rust
async fn hybrid_search_impl(
    query: String,
    limit: usize,
    graph: &CodeGraph,
) -> anyhow::Result<Value> {
    // 1. Vector search
    let vector_results = storage.vector_search_knn(...).await?;

    // 2. BM25 keyword search
    let text_results = storage.bm25_search(&query, limit * 2).await?;

    // 3. Fusion with search::rrf or search::linear
    let sql = r#"
        RETURN search::rrf([$vector_results, $text_results], $limit, 60);
    "#;

    // 4. Rerank fused results with Jina
}
```

#### Task 8.2: Cloud Mode Caching
**File**: `crates/codegraph-mcp/src/server.rs`

**Implementation**:
- Add Redis/Valkey cache for cloud mode results
- Cache query embeddings
- Cache reranking results
- Implement TTL and cache invalidation

---

## Testing Strategy

### Unit Tests
- [ ] Mode detection logic
- [ ] SurrealDB vector search methods
- [ ] Configuration validation
- [ ] Error handling and fallbacks

### Integration Tests
- [ ] End-to-end local mode
- [ ] End-to-end cloud mode
- [ ] Mode switching
- [ ] MCP tools in both modes

### Performance Tests
- [ ] Search latency comparison
- [ ] Reranking overhead
- [ ] Cache effectiveness
- [ ] Concurrent query handling

### Manual Testing
- [ ] Test with real codebases (small, medium, large)
- [ ] Test with different embedding providers
- [ ] Test error scenarios (API failures, network issues)
- [ ] Test with different query types

---

## Deployment Checklist

### Local Mode
- [ ] Build with `--features faiss,embeddings`
- [ ] Configure local/ollama embedding provider
- [ ] Index codebase
- [ ] Verify search works

### Cloud Mode
- [ ] Start SurrealDB instance
- [ ] Create database and schema
- [ ] Configure Jina API key in .env
- [ ] Build without FAISS (optional)
- [ ] Index codebase (embeddings go to SurrealDB)
- [ ] Verify HNSW index created
- [ ] Verify search + reranking works

---

## Success Criteria

1. ✅ Mode automatically detected from environment
2. ✅ Local mode preserves all existing FAISS functionality
3. ✅ Cloud mode uses SurrealDB HNSW + Jina reranking
4. ✅ All MCP tools work in both modes
5. ✅ Graceful fallback from cloud to local on errors
6. ✅ Clear error messages for configuration issues
7. ✅ Performance acceptable in both modes (<500ms p95)
8. ✅ 99% test coverage for new code
9. ✅ Documentation complete and accurate
10. ✅ Migration path documented for existing users

---

## Estimated Total Time
- **Phase 1**: 2-3 hours
- **Phase 2**: 4-5 hours
- **Phase 3**: 5-6 hours
- **Phase 4**: 2-3 hours
- **Phase 5**: 3-4 hours
- **Phase 6**: 4-5 hours
- **Phase 7**: 3-4 hours
- **Phase 8**: 4-6 hours (optional)

**Total**: 22-33 hours for core implementation (Phases 1-7)

---

## Current Progress

- ✅ Jina embedding provider integration complete
- ✅ Schema updated with HNSW index
- ✅ Research on SurrealDB capabilities complete
- ✅ Plan created and documented
- 🔄 Phase 1 in progress: Mode detection implementation using TDD
- ⏳ Remaining phases pending

---

## Next Steps

1. Complete TDD cycle for mode detection (Phase 1.1)
2. Update .env.example (Phase 1.2)
3. Implement SurrealDB vector search methods (Phase 2.2)
4. Refactor search router (Phase 3.1)
5. Implement cloud search with reranking (Phase 3.2)
6. Continue through remaining phases
