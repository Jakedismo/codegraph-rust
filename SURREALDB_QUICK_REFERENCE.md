# SurrealDB Quick Reference - CodeGraph

## Schema Quick Lookup

### Relationship Types (8 types)
```
Calls       → A calls B
Imports     → A imports B  
Extends     → Child extends Parent
Implements  → Class implements Interface
Contains    → File contains Definition
Uses        → A uses B
Defines     → A defines B
References  → Generic reference
```

### Node Fields
```
id, name, node_type, language, content
file_path, start_line, end_line
embedding (vector), complexity (float)
metadata (object), created_at, updated_at
```

### Edge Fields
```
id, from (record), to (record), edge_type, weight, metadata, created_at
```

### Indexes Available
```
Nodes:  id (unique), name, node_type, language, file_path, created_at, updated_at
Edges:  from, to, edge_type, created_at
```

---

## Current Operations (Implemented)

### Vector Search
```rust
surrealdb_storage.vector_search_knn(embedding, limit, ef_search)
surrealdb_storage.vector_search_with_metadata(embedding, limit, ef, node_type, lang, path)
```

### CRUD Operations
```rust
storage.add_node(node)          // Create
storage.get_node(id)            // Read
storage.update_node(node)       // Update
storage.remove_node(id)         // Delete
storage.get_nodes_by_ids(ids)   // Batch read
```

### Edge Operations
```rust
graph.get_edges_from(node_id)   // Outgoing edges
graph.get_edges_to(node_id)     // Incoming edges
graph.get_neighbors(node_id)    // Neighbor IDs (1-hop)
graph.add_edge(edge)            // Create relationship
```

### Traversal (In-Memory)
```rust
graph.shortest_path(from, to)   // BFS pathfinding
BfsIterator::new(graph, start, config)
DfsIterator::new(graph, start, config)
```

---

## Advanced Queries (NOT Currently Implemented)

### Graph Operators
```sql
→   Forward traversal (outgoing)
←   Backward traversal (incoming)
↔   Bidirectional traversal
```

### 1. Transitive Dependencies
```sql
SELECT * FROM nodes 
WHERE id IN (
  SELECT <-edges[edge_type='imports']-* 
  FROM nodes WHERE id = $module_id
)
```
**Use**: Impact analysis, dependency graphs

### 2. Cyclic Dependencies
```sql
SELECT DISTINCT a.id FROM edges a, edges b
WHERE a.from = b.to AND b.from = a.to 
  AND a.edge_type = 'imports'
```
**Use**: Quality gates, architecture validation

### 3. Call Chains
```sql
SELECT ->edges[edge_type='calls']->to->edges[edge_type='calls']->to
FROM nodes WHERE id = $start_function
LIMIT 1000 BY depth
```
**Use**: Debugging, execution flow analysis

### 4. Hub Detection
```sql
SELECT id, count(->edges) AS out, count(<-edges) AS in
FROM nodes
GROUP BY id
ORDER BY (out + in) DESC
```
**Use**: Coupling analysis, refactoring prioritization

### 5. Hybrid Vector + Structure
```sql
SELECT id, 
  vector::distance::knn() AS semantic_score,
  count(->edges) AS structural
FROM nodes
WHERE embedding <|100, 50|> $query_embedding
GROUP BY id
ORDER BY (semantic_score * 0.6 + structural * 0.4)
```
**Use**: Context-aware search

### 6. Most-Called Functions
```sql
SELECT to, count() AS calls 
FROM edges WHERE edge_type = 'calls'
GROUP BY to
ORDER BY calls DESC
```
**Use**: API hotspot analysis

### 7. API Surface Analysis
```sql
SELECT DISTINCT * FROM nodes
WHERE node_type = 'Function' 
  AND metadata.exported = true
  AND id NOT IN (
    SELECT <-edges[edge_type='calls']-* FROM nodes
  )
```
**Use**: Unused export detection

### 8. Edge Metrics
```sql
SELECT from, to, edge_type, 
  SUM(weight) AS strength, COUNT(*) AS interactions
FROM edges
GROUP BY from, to, edge_type
HAVING SUM(weight) > 5.0
ORDER BY strength DESC
```
**Use**: Coupling strength analysis

---

## File Locations

```
Core SurrealDB:
  /crates/codegraph-graph/src/surrealdb_storage.rs      [626 lines]
  /crates/codegraph-graph/src/surrealdb_schema.rs       [542 lines]
  /crates/codegraph-graph/src/surrealdb_migrations.rs   [406 lines]
  /crates/codegraph-graph/migrations/001_initial_schema.sql

Integration:
  /crates/codegraph-mcp/src/server.rs (lines 819-1050)  [cloud_search_impl]
  /crates/codegraph-ai/src/rag/engine.rs                [retrieve_hybrid_context]

Local Graph API:
  /crates/codegraph-graph/src/graph.rs                  [CodeGraph struct]
  /crates/codegraph-graph/src/traversal.rs              [BfsIterator, DfsIterator]

Types:
  /crates/codegraph-core/src/types.rs                   [EdgeType enum]
```

---

## Configuration

```rust
SurrealDbConfig {
    connection: String,      // "ws://localhost:3004"
    namespace: String,       // "codegraph"
    database: String,        // "main"
    username: Option,        // For auth
    password: Option,        // For auth
    strict_mode: bool,       // Schema validation
    auto_migrate: bool,      // Run migrations
    cache_enabled: bool,     // Node caching
}
```

### Environment Variables
```
SURREALDB_URL              ws://localhost:3004
SURREALDB_NAMESPACE        codegraph
SURREALDB_DATABASE         main
SURREALDB_USERNAME         (optional)
SURREALDB_PASSWORD         (optional)
CODEGRAPH_EMBEDDING_PROVIDER  jina (for cloud)
JINA_API_KEY              (for reranking)
```

---

## Performance Notes

**Vector Search**: ~100-200ms
- Includes HNSW index lookup
- With metadata filtering
- Returns top K results

**Node Loading**: ~50ms per 10 nodes
- Batch retrieval by ID
- Includes in-memory cache

**Caching**: 
- LRU cache: 1000 query results
- Node cache: Dashboard with ~100K entries
- TTL: Configurable (typically 60 seconds)

**Indexes**:
- `idx_edges_from`, `idx_edges_to` - Critical for neighbor queries
- `idx_nodes_language`, `idx_nodes_type` - For filtering
- HNSW index on `embedding` field - For vector search

---

## Common Patterns

### Search with Filters
```rust
// Find Rust functions containing "parse"
storage.vector_search_with_metadata(
    embedding,
    limit: 20,
    ef_search: 100,
    node_type: Some("Function"),
    language: Some("Rust"),
    file_path_pattern: Some("parser/"),
)
```

### Graph Expansion
```rust
// Get all callers of a function
let incoming = graph.get_edges_to(function_id).await?;
let callers: Vec<_> = incoming
    .iter()
    .filter(|e| e.edge_type == EdgeType::Calls)
    .map(|e| e.from)
    .collect();
```

### Hybrid Retrieval
```rust
// RAG pattern: vector + neighbors
let semantic_results = vector_search().await?;
for result in semantic_results.iter().take(3) {
    let neighbors = graph.get_neighbors(result.id).await?;
    expand_results_with(neighbors);
}
```

---

## Testing Checklist

- [ ] Can connect to SurrealDB instance
- [ ] Can perform vector search with HNSW
- [ ] Can filter by metadata (language, path, type)
- [ ] Can retrieve nodes by ID
- [ ] Can navigate edges (from/to)
- [ ] Can find 1-hop neighbors
- [ ] Schema migrations apply correctly
- [ ] Cache is populated and working

---

## Next Steps

**To implement advanced queries**:
1. Identify which query (from section 2 above) you need
2. Write/test in SurrealDB directly
3. Add method to `SurrealDbStorage` struct
4. Test with real codebase data
5. Benchmark vs. in-memory implementation
6. Integrate into MCP server if beneficial

**Recommended Order**:
1. Transitive dependencies (enables dependency visualization)
2. Cyclic detection (enables quality gates)
3. Hub detection (enables coupling analysis)
4. Hybrid scoring (enables smarter search)

