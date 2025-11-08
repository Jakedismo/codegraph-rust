# SurrealDB Graph Query Capabilities Analysis
## CodeGraph Project - Architecture Deep Dive

**Last Updated**: November 2025  
**Status**: Comprehensive exploration completed  
**Scope**: SurrealDB integration, schema, relationship types, query patterns, and advanced capabilities

---

## Quick Summary

**What We Have**:
- ✅ SurrealDB HNSW vector search with metadata filtering
- ✅ Node/Edge CRUD operations
- ✅ Basic schema for 8 relationship types (Calls, Imports, Extends, Implements, Contains, Uses, Defines, References)
- ✅ In-memory graph traversal (BFS, DFS) in Rust

**What We're Missing**:
- ❌ SurrealDB graph traversal operators (→, ←, ↔)
- ❌ Transitive dependency analysis (via DB)
- ❌ Cyclic dependency detection (via DB)
- ❌ Server-side relationship filtering and aggregation
- ❌ Hybrid vector + structural scoring (in SurrealDB)

**Biggest Opportunity**: Replace in-memory traversal with optimized SurrealDB graph queries for better scalability.

---

## Detailed Findings

### Relationship Types Available

The system models 8 types of relationships between code entities:

| Type | Direction | Use Case |
|------|-----------|----------|
| `Calls` | A → B | Function/method invocation |
| `Imports` | A → B | Module/file import dependency |
| `Extends` | Child → Parent | Class inheritance |
| `Implements` | Class → Interface | Interface implementation |
| `Contains` | File → Definition | File contains declarations |
| `Uses` | A → B | Symbol/type usage |
| `Defines` | A → B | Definition relationship |
| `References` | A → B | Generic reference (default) |

**Stored Location**: `/crates/codegraph-core/src/types.rs` - `EdgeType` enum

---

### Current Query Capabilities

**Implemented (via SurrealDB)**:
```sql
-- Vector similarity search with HNSW
SELECT id, vector::distance::knn() AS score
FROM nodes
WHERE embedding <|$limit, $ef|> $query_embedding
ORDER BY score ASC

-- Metadata-filtered search
WHERE embedding <|limit, ef|> $embedding
  AND node_type = 'Rust'
  AND file_path CONTAINS 'src/'

-- Node lookup by ID(s)
SELECT * FROM nodes WHERE id IN $ids
SELECT * FROM nodes WHERE name = $name
```

**Implemented (via In-Memory Traversal)**:
```rust
get_neighbors(node_id)              // 1-hop outgoing
get_incoming_neighbors(node_id)     // 1-hop incoming
shortest_path(from, to)             // BFS pathfinding
bfs_iterator, dfs_iterator          // Bulk traversal
```

**NOT Implemented (Available in SurrealDB)**:
```sql
-- Multi-hop traversal with operators
(nodes:start_id)→[edges]→nodes      // Forward traversal
(nodes:start_id)←[edges]←nodes      // Backward traversal
(nodes:start_id)↔[edges]↔nodes      // Bidirectional

-- Transitive relationships
SELECT * FROM nodes WHERE id IN (
  SELECT <-edges[edge_type='imports']-* FROM nodes WHERE id = $module_id
)

-- Graph aggregation
SELECT id, count(->edges) AS out_degree FROM nodes GROUP BY id

-- Circular dependency detection
SELECT DISTINCT a.id FROM edges a, edges b
WHERE a.from = b.to AND b.from = a.to
```

---

### Database Schema Overview

**Tables**: 4 core tables

1. **nodes** (code entities)
   - 13 fields: id, name, node_type, language, content, file_path, line info, embedding, complexity, metadata, timestamps
   - 5 indexes on: id (unique), name, node_type, language, file_path

2. **edges** (relationships)
   - 7 fields: id, from, to, edge_type, weight, metadata, created_at
   - 3 indexes on: from, to, edge_type

3. **schema_versions** (migration tracking)
   - Tracks schema evolution

4. **metadata** (system configuration)
   - Key-value storage

**Files**: See `/crates/codegraph-graph/migrations/001_initial_schema.sql`

---

### Graph Operations Hierarchy

**Tier 1 - Simple (Implemented)**:
- Single-node fetch
- 1-hop neighborhood queries
- Basic metadata filtering

**Tier 2 - Intermediate (Partially Implemented)**:
- Multi-hop traversal (in-memory only)
- Path finding (BFS in-memory)
- Filtered edges by type

**Tier 3 - Advanced (NOT Implemented)**:
- Transitive closure queries
- Cyclic dependency detection
- Relationship aggregation
- Complexity propagation
- Hybrid scoring (vector + structural)

---

### High-Value Unexploited Queries

**1. Transitive Dependencies** (for impact analysis)
```sql
-- Find all modules imported transitively
SELECT DISTINCT * FROM nodes WHERE id IN (
  SELECT <-edges[edge_type='imports']-* 
  FROM nodes WHERE id = $module_id
)
```

**2. Circular Dependencies** (for quality gates)
```sql
-- Detect import cycles
SELECT DISTINCT a.id FROM edges a, edges b
WHERE a.from = b.to AND b.from = a.to 
  AND a.edge_type = 'imports'
```

**3. Call Chain Analysis** (for debugging)
```sql
-- Find paths between functions
SELECT ->edges[edge_type='calls']->to->edges[edge_type='calls']->to
FROM nodes WHERE id = $start_function
LIMIT 1000 BY depth
```

**4. Hub Detection** (code with high coupling)
```sql
-- Find modules with most dependencies
SELECT id, 
  count(->edges) AS out_degree, 
  count(<-edges) AS in_degree
FROM nodes
WHERE node_type = 'Module'
GROUP BY id
ORDER BY (out_degree + in_degree) DESC
```

**5. Hybrid Vector + Structure Search**
```sql
-- Combine semantic similarity with structural proximity
SELECT id, 
  vector::distance::knn() AS semantic_score,
  count(->edges) AS structural_relevance
FROM nodes
WHERE embedding <|100, 50|> $query_embedding
GROUP BY id
ORDER BY (semantic_score * 0.6 + structural_relevance * 0.4) ASC
```

---

### Implementation Details

**SurrealDB Storage Module**: `/crates/codegraph-graph/src/surrealdb_storage.rs`
- `SurrealDbStorage` struct: Handles all DB operations
- Configuration via `SurrealDbConfig` (connection string, namespace, database, auth)
- Vector search via `vector_search_knn()` and `vector_search_with_metadata()`
- In-memory cache for frequently accessed nodes

**Connection Flow**:
1. MCP server detects search mode (Cloud vs Local)
2. Cloud mode creates SurrealDB connection on-demand
3. Query embedding generated via Jina provider
4. HNSW search returns candidate IDs
5. Jina reranking applied if enabled
6. Full nodes loaded from SurrealDB

**Performance Characteristics**:
- Vector search: ~100-200ms (includes HNSW index lookup)
- Node loading: ~50ms per 10 nodes
- Metadata filtering: Included in HNSW search query
- Caching: LRU cache for repeated queries (1000 entries)

---

### Architecture Visualization

```
┌─────────────────────────────────────┐
│       MCP Server / API Layer        │
│  (codegraph-mcp/src/server.rs)      │
└──────────────┬──────────────────────┘
               │
        ┌──────┴──────┐
        │             │
    ┌───▼────┐    ┌──▼────────┐
    │ Local  │    │   Cloud    │
    │ Mode   │    │   Mode     │
    │ FAISS  │    │SurrealDB   │
    └────────┘    └──┬─────────┘
                     │
          ┌──────────┴──────────┐
          │                     │
    ┌─────▼──────┐      ┌──────▼────────┐
    │ HNSW Index │      │ Node Retrieval │
    │   Search   │      │    by IDs      │
    └────────────┘      └────────────────┘
          │
    ┌─────▼──────────────────┐
    │  Jina Reranking (opt)  │
    └────────────────────────┘
```

---

### Files & Locations

**Key SurrealDB Files**:
- `/crates/codegraph-graph/src/surrealdb_storage.rs` - Main implementation (626 lines)
- `/crates/codegraph-graph/src/surrealdb_schema.rs` - Schema definitions (542 lines)
- `/crates/codegraph-graph/src/surrealdb_migrations.rs` - Migration runner (406 lines)
- `/crates/codegraph-graph/migrations/001_initial_schema.sql` - Initial schema

**Related Integration**:
- `/crates/codegraph-mcp/src/server.rs` - Cloud search implementation (lines 819-1050)
- `/crates/codegraph-ai/src/rag/engine.rs` - RAG with graph neighbor expansion
- `/crates/codegraph-graph/src/graph.rs` - Local graph API (in-memory)

---

### Recommendations

**Immediate (1-2 weeks)**:
1. ✅ Document current capabilities (this document)
2. Implement transitive dependency queries in SurrealDB
3. Add cyclic dependency detection query
4. Test performance vs. in-memory implementation

**Short-term (1-2 months)**:
1. Implement relationship filtering in cloud search
2. Add graph aggregation (hub detection, coupling analysis)
3. Optimize HNSW search with relationship context
4. Create hybrid vector+structure scoring

**Long-term (3+ months)**:
1. Migrate all traversal operations from Rust to SurrealDB
2. Implement materialized views for common transitive queries
3. Add graph analytics (centrality, betweenness, PageRank)
4. Support larger codebases (10M+ nodes) with server-side computation

**Technical Debt**:
- Async closure lifetime issues in migrations (currently disabled)
- Limited error handling for SurrealDB-specific failures
- No retry logic for transient connection failures
- Cache eviction policy could be more sophisticated

---

## Testing & Validation

To test advanced queries against your schema:

```sql
-- Basic setup
use ns codegraph db main;

-- Test 1: Vector search with metadata
SELECT id FROM nodes 
WHERE embedding <|10, 50|> [0.1, 0.2, 0.3, ...]
  AND language = 'Rust'

-- Test 2: Edge navigation (requires edges populated)
SELECT * FROM (nodes:start_id)->edges->to

-- Test 3: Aggregation
SELECT edge_type, count() FROM edges GROUP BY edge_type

-- Test 4: Hybrid scoring
SELECT id, count(->edges) AS connectivity FROM nodes GROUP BY id
```

---

## References

- **SurrealDB Documentation**: https://surrealdb.com/docs
- **Current Implementation**: `/crates/codegraph-graph/` (all three .rs files)
- **Migration System**: `MigrationRunner` in `surrealdb_migrations.rs`
- **Vector Indexing**: SurrealDB HNSW support (v2.0+)

---

**End of Analysis Document**
