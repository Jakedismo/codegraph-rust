---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

# Reference Documentation

Complete reference documentation for CodeGraph APIs, configuration, and components.

## 📚 Quick Reference

### Core Components

| Component | Purpose | Key Types |
|-----------|---------|-----------|
| `codegraph-core` | Shared types and traits | `Node`, `Edge`, `GraphId` |
| `codegraph-parser` | Code parsing | `Parser`, `Language`, `Visitor` |
| `codegraph-graph` | Graph storage | `GraphStorage`, `Query`, `Transaction` |
| `codegraph-vector` | Vector operations | `EmbeddingStore`, `VectorSearch` |
| `codegraph-api` | REST API | `ApiServer`, `Request`, `Response` |

### Configuration Files

| File | Purpose | Location |
|------|---------|----------|
| `Cargo.toml` | Workspace configuration | Project root |
| `.clippy.toml` | Linting rules | Project root |
| `rustfmt.toml` | Code formatting | Project root |
| `docker-compose.yml` | Container setup | Project root |

## 🔧 API Reference

### REST API Endpoints

**Base URL**: `http://localhost:8080/api/v1`

#### Health Check
- **GET** `/health`
- **Response**: `200 OK` with status information

#### Graph Operations
- **POST** `/graph/parse` - Parse and add files to graph
- **GET** `/graph/nodes` - List graph nodes
- **GET** `/graph/nodes/{id}` - Get specific node
- **GET** `/graph/edges` - List graph edges
- **POST** `/graph/query` - Execute graph queries

#### Search Operations
- **GET** `/search` - Text-based search
- **POST** `/search/semantic` - Vector similarity search
- **GET** `/search/suggestions` - Search suggestions

#### Project Operations
- **POST** `/projects` - Create new project
- **GET** `/projects/{id}` - Get project details
- **PUT** `/projects/{id}` - Update project
- **DELETE** `/projects/{id}` - Delete project

### Rust API Reference

#### Core Types

```rust
// Node representation
pub struct Node {
    pub id: NodeId,
    pub node_type: String,
    pub name: String,
    pub location: SourceLocation,
    pub metadata: HashMap<String, Value>,
}

// Edge representation  
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: String,
    pub metadata: HashMap<String, Value>,
}

// Graph query
pub struct Query {
    pub node_types: Option<Vec<String>>,
    pub edge_types: Option<Vec<String>>,
    pub filters: Vec<Filter>,
    pub limit: Option<usize>,
}
```

#### Parser API

```rust
use codegraph_parser::{Parser, Language};

// Create parser
let parser = Parser::new();

// Parse file
let nodes = parser.parse_file("src/main.rs")?;

// Parse with specific language
let nodes = parser.parse_with_language("code.py", Language::Python)?;

// Parse string content
let nodes = parser.parse_string("fn main() {}", Language::Rust)?;
```

#### Graph Storage API

```rust
use codegraph_graph::GraphStorage;

// Create storage
let mut graph = GraphStorage::new("/path/to/db")?;

// Add nodes
graph.add_node(node)?;
graph.add_nodes(nodes)?;

// Add edges
graph.add_edge(edge)?;

// Query
let results = graph.query(&query)?;
let deps = graph.find_dependencies(node_id)?;
```

#### Vector Operations API

```rust
use codegraph_vector::{EmbeddingStore, VectorSearch};

// Create embedding store
let store = EmbeddingStore::new("/path/to/vectors")?;

// Add embeddings
store.add_embedding(node_id, vector)?;

// Search
let results = store.search(&query_vector, k)?;
let similar = store.find_similar(node_id, k)?;
```

## ⚙️ Configuration Reference

### Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `CODEGRAPH_HOST` | API server host | `0.0.0.0` | `localhost` |
| `CODEGRAPH_PORT` | API server port | `8080` | `3000` |
| `CODEGRAPH_DB_PATH` | Database directory | `./graph_db` | `/data/codegraph` |
| `CODEGRAPH_LOG_LEVEL` | Logging level | `info` | `debug` |
| `RUST_LOG` | Rust logging | unset | `codegraph=debug` |

### Database Configuration

#### RocksDB Options

```rust
use rocksdb::{Options, DB};

let mut opts = Options::default();

// Memory settings
opts.set_write_buffer_size(64 * 1024 * 1024);      // 64MB
opts.set_max_write_buffer_number(3);
opts.set_target_file_size_base(64 * 1024 * 1024);  // 64MB
opts.set_max_total_wal_size(256 * 1024 * 1024);    // 256MB

// Performance settings
opts.set_max_background_jobs(4);
opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

// Cache settings
let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024)?; // 256MB
opts.set_row_cache(&cache);
```

#### Vector Store Configuration

```rust
// FAISS configuration
use codegraph_vector::VectorConfig;

let config = VectorConfig {
    dimension: 384,              // Embedding dimension
    index_type: IndexType::IVF,  // Index algorithm
    nlist: 100,                  // Number of clusters
    metric: MetricType::L2,      // Distance metric
    use_gpu: false,              // GPU acceleration
};
```

### API Server Configuration

```rust
// Server configuration
use codegraph_api::ServerConfig;

let config = ServerConfig {
    host: "0.0.0.0".to_string(),
    port: 8080,
    max_connections: 1000,
    request_timeout: Duration::from_secs(30),
    cors_enabled: true,
    auth_enabled: false,
};
```

## 📋 Command Line Reference

### Build Commands

```bash
# Standard build
cargo build

# Release build
cargo build --release

# Build specific crate
cargo build -p codegraph-parser

# Build with features
cargo build --features "faiss,gpu"

# Check without building
cargo check
```

### Development Commands

```bash
# Format code
cargo fmt

# Lint code  
cargo clippy

# Run tests
cargo test

# Run specific test
cargo test test_parse_rust

# Run benchmarks
cargo bench

# Generate documentation
cargo doc --open
```

### Make Commands

```bash
# Development check
make dev

# Quick check (no tests)
make quick

# Watch mode
make watch

# Clean build
make clean

# Docker build
make docker-build
```

### API Server Commands

```bash
# Start server
cargo run --bin codegraph-api

# Start with custom port
cargo run --bin codegraph-api -- --port 3000

# Start with debug logging
RUST_LOG=debug cargo run --bin codegraph-api

# Background mode
nohup cargo run --bin codegraph-api > server.log 2>&1 &
```

## 🎯 Language Support Reference

### Supported Languages

| Language | Parser | File Extensions | Tree-sitter Grammar |
|----------|--------|-----------------|-------------------|
| Rust | ✅ | `.rs` | `tree-sitter-rust` |
| Python | ✅ | `.py` | `tree-sitter-python` |
| JavaScript | ✅ | `.js`, `.mjs` | `tree-sitter-javascript` |
| TypeScript | ✅ | `.ts`, `.tsx` | `tree-sitter-typescript` |
| Go | ✅ | `.go` | `tree-sitter-go` |
| C/C++ | 🚧 | `.c`, `.cpp`, `.h` | `tree-sitter-c` |
| Java | 🚧 | `.java` | `tree-sitter-java` |

### Node Types by Language

#### Rust
- `function` - Function definitions
- `struct` - Struct definitions  
- `enum` - Enum definitions
- `impl` - Implementation blocks
- `mod` - Module definitions
- `use` - Import statements
- `macro` - Macro definitions

#### Python
- `function_definition` - Functions
- `class_definition` - Classes
- `import_statement` - Imports
- `from_import_statement` - From imports
- `assignment` - Variable assignments
- `decorator` - Decorators

#### JavaScript/TypeScript
- `function_declaration` - Functions
- `class_declaration` - Classes
- `import_statement` - ES6 imports
- `export_statement` - ES6 exports
- `variable_declaration` - Variables
- `interface_declaration` - TS interfaces

## 🔍 Query Reference

### Graph Query Language

```rust
// Node filters
let query = Query::new()
    .with_node_type("function")
    .with_name_pattern("test_*")
    .with_metadata("visibility", "public");

// Edge filters
let query = Query::new()
    .with_edge_type("calls")
    .with_source_type("function")
    .with_target_type("function");

// Complex queries
let query = Query::new()
    .with_node_types(vec!["function", "method"])
    .with_path_length(1, 3)
    .limit(100);
```

### Vector Search Queries

```rust
// Semantic similarity
let results = store.search_by_text("error handling patterns", 10)?;

// Similar code
let results = store.find_similar_to_node(node_id, 5)?;

// Combined filters
let results = store.search_filtered(
    &query_vector,
    &NodeFilter::new().with_type("function"),
    10
)?;
```

## 🛠️ Error Reference

### Common Error Types

| Error Type | Description | Common Causes |
|------------|-------------|---------------|
| `ParseError` | Code parsing failed | Invalid syntax, unsupported language |
| `StorageError` | Database operation failed | Permissions, disk space, corruption |
| `VectorError` | Vector operation failed | Dimension mismatch, index corruption |
| `ApiError` | API request failed | Invalid input, server error |
| `ConfigError` | Configuration invalid | Missing files, invalid values |

### Error Handling Patterns

```rust
use codegraph_core::{Result, Error};

// Basic error handling
match parser.parse_file("test.rs") {
    Ok(nodes) => println!("Parsed {} nodes", nodes.len()),
    Err(Error::Parse(e)) => eprintln!("Parse error: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}

// Error propagation
fn analyze_project(path: &str) -> Result<Summary> {
    let nodes = parser.parse_directory(path)?;
    let graph = GraphStorage::new("./db")?;
    graph.add_nodes(nodes)?;
    Ok(Summary::new(graph))
}
```

## 📖 Further Reading

- **[Architecture Documentation](../architecture/)** - System design details
- **[API Documentation](../api/)** - Complete API specifications  
- **[Examples](../examples/)** - Practical usage examples
- **[Troubleshooting](../troubleshooting/)** - Problem resolution guide

---

**Navigation**: [Documentation Hub](../index.md) | [Getting Started](../guides/getting-started.md) | [Examples](../examples/)