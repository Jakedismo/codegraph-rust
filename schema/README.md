# CodeGraph SurrealDB Schema

This directory contains the SurrealDB schema definition for CodeGraph's code analysis graph storage.

## Files

- `codegraph.surql` - Complete schema definition for all tables, fields, and indexes

## Schema Overview

### Tables

#### 1. **nodes** - Code Entity Storage
Stores individual code entities (functions, classes, variables, etc.) with their metadata and embeddings.

**Key Fields:**
- `id` - Unique identifier
- `name` - Entity name
- `node_type` - Type of code entity (Function, Class, Method, Variable, etc.)
- `language` - Programming language
- `content` - Source code content
- `file_path` - Location in the codebase
- `start_line`, `end_line` - Source code position
- `embedding` - Vector embedding for semantic search
- `complexity` - Code complexity score
- `metadata` - Flexible JSON metadata

#### 2. **edges** - Relationship Storage
Stores relationships between code entities (calls, imports, inherits, etc.).

**Key Fields:**
- `id` - Unique identifier
- `from` - Source node reference
- `to` - Target node reference
- `edge_type` - Relationship type (calls, imports, inherits, contains)
- `weight` - Relationship strength
- `metadata` - Flexible JSON metadata

#### 3. **project_metadata** - Project Information
Tracks metadata about analyzed projects.

**Key Fields:**
- `project_id` - Unique project identifier
- `name` - Project name
- `root_path` - Project root directory
- `primary_language` - Main programming language
- `last_analyzed` - Last analysis timestamp
- `file_count`, `node_count`, `edge_count` - Statistics

#### 4. **schema_versions** - Migration Tracking
Tracks applied schema migrations for version control.

## Usage

### Option 1: Apply via SurrealDB CLI

```bash
# Connect to your SurrealDB instance
surreal sql --endpoint http://localhost:8000 --namespace your_namespace --database codegraph

# Apply the schema
< schema/codegraph.surql
```

### Option 2: Apply via SurrealDB SQL Command

```bash
# Using surrealist or web interface, paste the contents of codegraph.surql
# Or use the SQL command directly:
surreal sql --endpoint http://localhost:8000 \
  --namespace your_namespace \
  --database codegraph \
  --auth-level root \
  --username root \
  --password root \
  < schema/codegraph.surql
```

### Option 3: Apply via Rust Code

The schema is automatically applied when using `SchemaManager` in the codebase:

```rust
use codegraph_graph::{create_nodes_schema, create_edges_schema, SchemaManager};

// Create schema manager
let mut manager = SchemaManager::new(db);

// Define schemas
manager.define_table(create_nodes_schema());
manager.define_table(create_edges_schema());

// Apply schemas
manager.apply_schemas().await?;
```

## Configuration

Before applying the schema, you may want to customize:

1. **Namespace and Database**: Edit the USE statements at the top of `codegraph.surql`
   ```surrealql
   USE NS your_namespace;
   USE DB codegraph;
   ```

2. **Vector Dimensions**: If using vector search, add an appropriate index:
   ```surrealql
   -- For MTREE (exact nearest neighbor)
   DEFINE INDEX idx_nodes_embedding ON TABLE nodes FIELDS embedding MTREE DIMENSION 2048;

   -- For HNSW (approximate nearest neighbor, faster)
   DEFINE INDEX idx_nodes_embedding ON TABLE nodes FIELDS embedding HNSW DIMENSION 2048;
   ```

3. **Additional Indexes**: Based on your query patterns, you may want to add more indexes

## Schema Migrations

The schema uses a `schema_versions` table to track migrations. When making changes:

1. Create a new migration file: `schema/migrations/002_your_migration.surql`
2. Update the version number in the migration
3. Apply the migration in order

Example migration:
```surrealql
-- Migration: Add full-text search to nodes
-- Version: 2

-- Define analyzer for code search
DEFINE ANALYZER code_analyzer TOKENIZERS class FILTERS ascii;

-- Add full-text index
DEFINE INDEX idx_nodes_name_fulltext ON TABLE nodes COLUMNS name
  FULLTEXT ANALYZER code_analyzer BM25 HIGHLIGHTS;

-- Update schema version
INSERT INTO schema_versions (version, description, applied_at)
VALUES (2, 'Added full-text search capabilities', time::now());
```

## Vector Search Setup

For semantic code search with embeddings:

### Using Jina Embeddings (dimension: 2048)
```surrealql
-- MTREE index for exact search
DEFINE INDEX idx_nodes_embedding_mtree ON TABLE nodes
  FIELDS embedding MTREE DIMENSION 2048 DIST COSINE;

-- Or HNSW for faster approximate search
DEFINE INDEX idx_nodes_embedding_hnsw ON TABLE nodes
  FIELDS embedding HNSW DIMENSION 2048 DIST COSINE EFC 100 M 16;
```

### Using OpenAI Embeddings (dimension: 1536)
```surrealql
DEFINE INDEX idx_nodes_embedding_mtree ON TABLE nodes
  FIELDS embedding MTREE DIMENSION 1536 DIST COSINE;
```

## Example Queries

### Find all functions in a file
```surrealql
SELECT * FROM nodes
WHERE file_path = '/path/to/file.rs'
  AND node_type = 'Function';
```

### Find all function calls from a specific function
```surrealql
SELECT ->to.* FROM edges
WHERE edge_type = 'calls'
  AND from = nodes:function_id;
```

### Find all dependencies (imports)
```surrealql
SELECT ->to.name, ->to.file_path FROM edges
WHERE edge_type = 'imports'
  AND from = nodes:module_id;
```

### Get call graph depth
```surrealql
-- Find all functions called by a function, recursively
SELECT * FROM (
  SELECT ->to.* FROM edges
  WHERE from = nodes:function_id
    AND edge_type = 'calls'
).*;
```

### Semantic search (with vector index)
```surrealql
-- Find similar code entities by embedding
SELECT * FROM nodes
WHERE embedding <|10|> [/* your query embedding vector */];
```

## Backup and Restore

### Backup Schema
```bash
# Export schema definitions
surreal export --endpoint http://localhost:8000 \
  --namespace your_namespace \
  --database codegraph \
  --output backup.surql
```

### Restore Schema
```bash
# Import schema
surreal import --endpoint http://localhost:8000 \
  --namespace your_namespace \
  --database codegraph \
  --input backup.surql
```

## Performance Tuning

### Index Optimization
- Add composite indexes for frequently used query combinations
- Use COUNT indexes for tables with frequent count operations
- Consider FULLTEXT indexes for text search operations

### Query Optimization
- Use record links (`record(table)`) instead of strings for references
- Leverage indexes in WHERE clauses
- Use pagination (LIMIT/START) for large result sets

### Storage Optimization
- Consider using `CAPACITY` parameter for MTREE indexes
- Adjust `EFC` and `M` parameters for HNSW indexes based on your speed/accuracy tradeoff

## Troubleshooting

### Schema Already Exists
The schema uses `IF NOT EXISTS` clauses, so it's safe to rerun. To force update:
```surrealql
-- Remove and recreate
REMOVE TABLE nodes;
-- Then reapply schema
```

### Vector Index Issues
Ensure all embeddings have the same dimension:
```surrealql
-- Check embedding dimensions
SELECT file_path, array::len(embedding) as dim
FROM nodes
WHERE embedding IS NOT NONE;
```

### Performance Issues
```surrealql
-- Check table statistics
INFO FOR TABLE nodes;
INFO FOR TABLE edges;

-- Analyze query performance
EXPLAIN SELECT * FROM nodes WHERE node_type = 'Function';
```

## Contributing

When modifying the schema:
1. Test changes on a development database first
2. Create a migration file with version number
3. Update this README with new features
4. Document any breaking changes
