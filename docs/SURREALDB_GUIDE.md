# SurrealDB Integration Guide

This guide explains how to use SurrealDB as the database backend for CodeGraph.

## Why SurrealDB?

SurrealDB offers several advantages for CodeGraph:

- **Flexible Schema**: Schema-less JSON storage makes it easy to evolve your data model
- **Native Relationships**: Built-in support for graph relationships and traversals
- **ACID Transactions**: Full transactional support out of the box
- **Multiple Deployment Options**: Embedded (file-based), in-memory, or remote server
- **Time-Travel Queries**: Query historical data states
- **Real-time Subscriptions**: Live query support for reactive applications

## Installation

### 1. Enable the SurrealDB Feature

Build CodeGraph with the SurrealDB feature enabled:

```bash
cargo build --features surrealdb
```

Or add it to your `Cargo.toml`:

```toml
[dependencies]
codegraph-graph = { version = "*", features = ["surrealdb"] }
```

### 2. Install SurrealDB (Optional for Remote Mode)

For remote deployment, install the SurrealDB server:

```bash
# macOS or Linux
curl -sSf https://install.surrealdb.com | sh

# Or via Homebrew
brew install surrealdb/tap/surreal

# Windows (via cargo)
cargo install surrealdb --locked
```

## Configuration

### Basic Configuration (WebSocket - Default)

Create a configuration file (e.g., `config/surrealdb.toml`):

```toml
[database]
backend = "surrealdb"

[database.surrealdb]
connection = "ws://localhost:8000"  # Standard SurrealDB connection
namespace = "codegraph"
database = "graph"
auto_migrate = true
```

**Note:** This requires SurrealDB server running on port 8000 (see "Running SurrealDB Server" below).

### File-Based Configuration (Embedded)

For embedded/local-only usage without a server:

```toml
[database]
backend = "surrealdb"

[database.surrealdb]
connection = "file://data/surrealdb/graph.db"
namespace = "codegraph"
database = "graph"
auto_migrate = true
```

### In-Memory Configuration (Testing)

```toml
[database]
backend = "surrealdb"

[database.surrealdb]
connection = "mem://"
namespace = "test"
database = "graph"
```

### Remote Server Configuration

```toml
[database]
backend = "surrealdb"

[database.surrealdb]
connection = "http://localhost:8000"
namespace = "production"
database = "codegraph"
username = "admin"
# Set password via environment variable: CODEGRAPH__DATABASE__SURREALDB__PASSWORD
auto_migrate = false
strict_mode = true
```

## Running SurrealDB Server

### Local Development Server

```bash
surreal start --bind 0.0.0.0:8000 --user root --pass root file://data/surrealdb
```

### Production Server

```bash
surreal start \
  --bind 0.0.0.0:8000 \
  --user admin \
  --pass "$SURREALDB_PASSWORD" \
  --log info \
  --strict \
  file://var/lib/surrealdb/data
```

### Docker Deployment

```bash
docker run -d \
  --name surrealdb \
  -p 8000:8000 \
  -v $(pwd)/data:/data \
  surrealdb/surrealdb:latest \
  start --bind 0.0.0.0:8000 --user root --pass root file://data/surrealdb
```

## Schema Management

### Flexible Schema Design

SurrealDB storage is designed for easy schema evolution:

1. **Schema-less Fields**: New fields can be added without migrations
2. **Type Safety**: Optional strict mode for production environments
3. **Migration System**: Versioned migrations for controlled schema changes

### Adding New Fields

No migration needed (in flexible mode):

```rust
// Just add the field to your CodeNode and it will be stored
node.metadata.attributes.insert("new_field", "value");
storage.update_node(node).await?;
```

With migration (recommended for production):

```rust
use codegraph_graph::surrealdb_schema::*;

// Create a new field definition
let field = FieldDefinition {
    name: "new_field".to_string(),
    field_type: FieldType::String,
    optional: true,
    default: None,
};

// Add to schema
schema_manager.add_field("nodes", field).await?;
```

### Running Migrations

Migrations are automatically applied when `auto_migrate = true`. To manually control migrations:

```rust
use codegraph_graph::surrealdb_migrations::*;
use surrealdb::{engine::any::Any, Surreal};

let db = Surreal::new::<Any>("file://data/graph.db").await?;
let runner = MigrationRunner::new(Arc::new(db));

// Check current version
let version = runner.get_current_version().await?;
println!("Current schema version: {}", version);

// View migration status
let statuses = runner.status().await?;
for status in statuses {
    println!("{} v{}: {} ({})",
        if status.applied { "✓" } else { "○" },
        status.version,
        status.name,
        status.description
    );
}

// Apply pending migrations
runner.migrate().await?;

// Rollback to specific version (if needed)
runner.rollback(1).await?;

// Verify integrity
runner.verify().await?;
```

### Creating Custom Migrations

1. **Create migration file** in `crates/codegraph-graph/migrations/`:

```sql
-- migrations/004_add_tags.sql
-- Migration 004: Add tags support

-- UP Migration
DEFINE FIELD IF NOT EXISTS tags ON TABLE nodes TYPE option<array<string>>;
DEFINE INDEX IF NOT EXISTS idx_nodes_tags ON TABLE nodes COLUMNS tags;

-- DOWN Migration (for rollback)
-- REMOVE FIELD tags ON TABLE nodes;
-- REMOVE INDEX idx_nodes_tags ON TABLE nodes;
```

2. **Register migration** in code:

```rust
use codegraph_graph::surrealdb_migrations::*;

let mut runner = MigrationRunner::new(db);

runner.add_migration(Migration {
    version: 4,
    name: "add_tags".to_string(),
    description: "Add tagging support to nodes".to_string(),
    up_sql: vec![
        include_str!("../migrations/004_add_tags.sql").to_string(),
    ],
    down_sql: Some(vec![
        "REMOVE FIELD tags ON TABLE nodes;".to_string(),
        "REMOVE INDEX idx_nodes_tags ON TABLE nodes;".to_string(),
    ]),
});

runner.migrate().await?;
```

## Usage Examples

### Basic CRUD Operations

```rust
use codegraph_graph::surrealdb_storage::*;
use codegraph_core::*;

// Initialize storage (default: ws://localhost:8000)
let config = SurrealDbConfig::default();
let mut storage = SurrealDbStorage::new(config).await?;

// Or specify custom connection
let config = SurrealDbConfig {
    connection: "ws://localhost:8000".to_string(),
    namespace: "codegraph".to_string(),
    database: "graph".to_string(),
    ..Default::default()
};

let mut storage = SurrealDbStorage::new(config).await?;

// Create a node
let node = CodeNode {
    id: NodeId::new(),
    name: "my_function".into(),
    node_type: Some(NodeType::Function),
    language: Some(Language::Rust),
    // ... other fields
};

storage.add_node(node).await?;

// Query nodes
let results = storage.find_nodes_by_name("my_function").await?;

// Update node
let mut node = storage.get_node(node_id).await?.unwrap();
node.complexity = Some(5.0);
storage.update_node(node).await?;

// Delete node
storage.remove_node(node_id).await?;
```

### Advanced Queries

SurrealDB allows complex queries directly:

```rust
// Find all Rust functions with high complexity
let query = r#"
    SELECT * FROM nodes
    WHERE language = "Rust"
      AND node_type = "Function"
      AND complexity > 10.0
    ORDER BY complexity DESC
    LIMIT 10
"#;

let results = storage.db.query(query).await?;
```

### Schema Export/Import

```rust
use codegraph_graph::surrealdb_schema::*;

let manager = SchemaManager::new(db);

// Export current schema
let schema_json = manager.export_schema().await?;
std::fs::write("schema_backup.json", schema_json)?;

// Import schema
let schema_json = std::fs::read_to_string("schema_backup.json")?;
manager.import_schema(&schema_json)?;
manager.apply_schemas().await?;
```

## Performance Optimization

### Caching

Enable in-memory caching for frequently accessed nodes:

```rust
let config = SurrealDbConfig {
    cache_enabled: true,  // Default: true
    ..Default::default()
};
```

### Indexes

Add custom indexes for common queries:

```rust
use codegraph_graph::surrealdb_schema::*;

let index = IndexDefinition {
    name: "idx_custom".to_string(),
    columns: vec!["field1".to_string(), "field2".to_string()],
    unique: false,
};

schema_manager.add_index("nodes", index).await?;
```

### Batch Operations

Use transactions for bulk operations:

```rust
// SurrealDB natively supports transactions
let nodes = vec![node1, node2, node3];

for node in nodes {
    storage.add_node(node).await?;
}
// All operations are atomic within the transaction
```

## Migrating from RocksDB

### 1. Export Data from RocksDB

```rust
// Export all nodes from RocksDB
let rocksdb_storage = HighPerformanceRocksDbStorage::new("data/graph.db")?;
let nodes = rocksdb_storage.get_all_nodes()?;  // Implement this method
```

### 2. Import to SurrealDB

```rust
// Import to SurrealDB
let mut surrealdb_storage = SurrealDbStorage::new(config).await?;

for node in nodes {
    surrealdb_storage.add_node(node).await?;
}
```

### 3. Update Configuration

Change `backend` in your config from `rocksdb` to `surrealdb`.

## Troubleshooting

### Connection Issues

```bash
# Verify SurrealDB is running
curl http://localhost:8000/health

# Check logs
tail -f /var/log/surrealdb.log
```

### Schema Issues

```rust
// Check current schema version
let version = runner.get_current_version().await?;

// Verify migrations
let valid = runner.verify().await?;

// View migration status
let statuses = runner.status().await?;
```

### Performance Issues

1. Add indexes for frequently queried fields
2. Enable caching in config
3. Use batch operations for bulk inserts
4. Consider using remote SurrealDB server for better concurrency

## Best Practices

1. **Use Migrations**: Even in flexible mode, use migrations for major schema changes
2. **Version Control**: Store migration files in version control
3. **Test Rollbacks**: Ensure rollback SQL is tested and working
4. **Backup Regularly**: Use SurrealDB's export feature for backups
5. **Monitor Performance**: Add indexes based on query patterns
6. **Use Namespaces**: Separate dev/staging/prod with different namespaces
7. **Secure Credentials**: Use environment variables for passwords
8. **Enable Strict Mode in Production**: Add type safety for production environments

## Further Resources

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [SurrealDB Rust SDK](https://surrealdb.com/docs/sdk/rust)
- [SurrealQL Query Language](https://surrealdb.com/docs/surrealql)
- [SurrealDB Discord](https://discord.com/invite/surrealdb)
