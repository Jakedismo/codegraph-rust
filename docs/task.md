# CodeGraph: High-Performance Code Intelligence System
## Architecture Design & Implementation Plan

### Document Version: 1.0
### Date: September 2025
### Status: Draft

---

## Executive Summary

CodeGraph is a high-performance, embedded graph database system designed for AI-assisted code intelligence. Built with Rust for maximum performance, it provides real-time code analysis, semantic search, and multi-agent coordination through GraphQL and MCP protocols. The system achieves 10-20x performance improvements over traditional implementations while maintaining a single-binary deployment model.

### Key Capabilities
- **Embedded graph database** with sub-50ms query latency
- **AI-powered semantic search** with local and cloud embedding support
- **Real-time incremental indexing** with <1 second update propagation
- **Multi-protocol API** supporting GraphQL, REST, and MCP
- **Single binary deployment** under 50MB with embedded resources

---

## 1. System Architecture

### 1.1 Core Design Principles

- **Zero-copy operations** wherever possible
- **Lock-free concurrency** for hot paths
- **Memory-mapped I/O** for large data structures
- **Async-first** architecture with Tokio runtime
- **Type safety** enforced at compile time
- **Modular boundaries** with clear trait definitions

### 1.2 Technology Stack

```toml
# Core Runtime
runtime = "tokio"
web_framework = "axum"
graphql = "async-graphql"

# Storage
embedded_db = "rocksdb"
vector_store = "faiss-rs"
serialization = "bincode + rkyv"

# Code Analysis
parser = "tree-sitter"
ast_processing = "syn + quote"

# AI/ML
embeddings_local = "candle"
embeddings_remote = "reqwest + openai-api"
vector_ops = "ndarray"

# Observability
logging = "tracing + tracing-subscriber"
metrics = "prometheus"
profiling = "pprof"
```

### 1.3 System Components

```rust
// High-level module architecture
crate codegraph {
    pub mod core {
        pub mod graph;      // Graph store abstraction
        pub mod storage;    // RocksDB persistence layer
        pub mod index;      // FAISS vector indexing
        pub mod snapshot;   // Versioning and snapshots
    }
    
    pub mod parser {
        pub mod ast;        // Tree-sitter integration
        pub mod analyzer;   // Language-specific analysis
        pub mod extractor;  // Entity extraction
        pub mod delta;      // Incremental parsing
    }
    
    pub mod ai {
        pub mod embeddings; // Embedding generation
        pub mod rag;        // Retrieval-augmented generation
        pub mod nlp;        // Natural language processing
        pub mod cache;      // Model and result caching
    }
    
    pub mod api {
        pub mod graphql;    // GraphQL server
        pub mod mcp;        // MCP protocol server
        pub mod rest;       // REST endpoints
        pub mod auth;       // Authentication middleware
    }
    
    pub mod sync {
        pub mod watcher;    // File system monitoring
        pub mod git;        // Git integration
        pub mod queue;      // Update queue management
        pub mod scheduler;  // Background task scheduling
    }
    
    pub mod telemetry {
        pub mod metrics;    // Performance metrics
        pub mod tracing;    // Distributed tracing
        pub mod health;     // Health checks
    }
}
```

---

## 2. Data Model

### 2.1 Graph Schema

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    pub id: Uuid,
    pub node_type: NodeType,
    pub label: String,
    pub properties: HashMap<String, Value>,
    pub embedding_id: Option<Uuid>,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NodeType {
    Repository,
    Directory,
    File,
    Class,
    Function,
    Method,
    Variable,
    Import,
    Dependency,
    Module,
    Api,
    EmbeddingBlock,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Edge {
    pub id: Uuid,
    pub from_node: Uuid,
    pub to_node: Uuid,
    pub edge_type: EdgeType,
    pub properties: HashMap<String, Value>,
    pub weight: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EdgeType {
    Contains,
    PartOf,
    Declares,
    Calls,
    Uses,
    Imports,
    DependsOn,
    IndexedAs,
    TaggedWith,
    Version,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Embedding {
    pub id: Uuid,
    pub node_id: Uuid,
    pub vector: Vec<f32>,
    pub model: String,
    pub metadata: EmbeddingMetadata,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbeddingMetadata {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub content_hash: String,
    pub token_count: usize,
}
```

### 2.2 Storage Layout

```rust
// RocksDB column families
pub struct StorageLayout {
    pub nodes: ColumnFamily,        // Key: node_id, Value: Node
    pub edges: ColumnFamily,        // Key: edge_id, Value: Edge
    pub embeddings: ColumnFamily,   // Key: embedding_id, Value: Embedding
    pub indices: ColumnFamily,      // Key: index_type:value, Value: Vec<NodeId>
    pub snapshots: ColumnFamily,    // Key: snapshot_id, Value: Snapshot
    pub metadata: ColumnFamily,     // Key: string, Value: arbitrary
}
```

---

## 3. Implementation Roadmap

### Phase 0: Project Setup & Foundation (Week 1)

#### 3.1 Workspace Initialization

```bash
# Project structure
codegraph/
├── Cargo.toml                 # Workspace configuration
├── crates/
│   ├── codegraph-core/       # Core graph engine
│   ├── codegraph-parser/     # Code parsing
│   ├── codegraph-ai/         # AI/ML capabilities
│   ├── codegraph-api/        # API servers
│   ├── codegraph-sync/       # Synchronization
│   ├── codegraph-cli/        # CLI interface
│   └── codegraph-telemetry/  # Observability
├── benches/                   # Benchmarks
├── tests/                     # Integration tests
├── assets/                    # Embedded resources
├── config/                    # Configuration files
└── scripts/                   # Build and deployment scripts
```

#### 3.2 Workspace Configuration

```toml
# Cargo.toml (root)
[workspace]
resolver = "2"
members = [
    "crates/codegraph-core",
    "crates/codegraph-parser",
    "crates/codegraph-ai",
    "crates/codegraph-api",
    "crates/codegraph-sync",
    "crates/codegraph-cli",
    "crates/codegraph-telemetry",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["CodeGraph Team"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/org/codegraph"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.38", features = ["full"] }
async-trait = "0.1"

# Web framework
axum = { version = "0.7", features = ["ws", "macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "compression", "trace"] }

# GraphQL
async-graphql = { version = "7.0", features = ["apollo-tracing", "log"] }
async-graphql-axum = "7.0"

# Storage
rocksdb = { version = "0.22", features = ["multi-threaded-cf"] }
faiss = "0.12"
bincode = "1.3"
rkyv = { version = "0.7", features = ["validation"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Code parsing
tree-sitter = "0.22"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-rust = "0.21"

# AI/ML
candle = { version = "0.5", optional = true }
ort = { version = "2.0", optional = true }
ndarray = "0.15"

# Utilities
uuid = { version = "1.8", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "1.0"
dashmap = "6.0"
crossbeam = "0.8"
rayon = "1.10"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
prometheus = "0.13"
opentelemetry = "0.22"

# Testing
criterion = "0.5"
proptest = "1.4"
tempfile = "3.10"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"

[profile.bench]
inherits = "release"
```

#### 3.3 Core Traits Definition

```rust
// crates/codegraph-core/src/traits.rs

use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait GraphStore: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    
    async fn add_node(&self, node: Node) -> Result<Uuid, Self::Error>;
    async fn get_node(&self, id: Uuid) -> Result<Option<Node>, Self::Error>;
    async fn update_node(&self, id: Uuid, node: Node) -> Result<(), Self::Error>;
    async fn delete_node(&self, id: Uuid) -> Result<(), Self::Error>;
    
    async fn add_edge(&self, edge: Edge) -> Result<Uuid, Self::Error>;
    async fn get_edges(&self, from: Uuid) -> Result<Vec<Edge>, Self::Error>;
    
    async fn get_subgraph(
        &self, 
        root: Uuid, 
        depth: u32,
        filters: Option<GraphFilters>
    ) -> Result<Subgraph, Self::Error>;
    
    async fn create_snapshot(&self, description: String) -> Result<Uuid, Self::Error>;
    async fn restore_snapshot(&self, id: Uuid) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait VectorIndex: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    
    async fn add(&self, id: Uuid, vector: Vec<f32>) -> Result<(), Self::Error>;
    async fn search(&self, query: Vec<f32>, k: usize) -> Result<Vec<(Uuid, f32)>, Self::Error>;
    async fn update(&self, id: Uuid, vector: Vec<f32>) -> Result<(), Self::Error>;
    async fn delete(&self, id: Uuid) -> Result<(), Self::Error>;
    async fn rebuild(&self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait CodeAnalyzer: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    
    async fn parse_file(&self, path: &Path) -> Result<ParsedFile, Self::Error>;
    async fn extract_entities(&self, ast: &ParsedFile) -> Result<Vec<Entity>, Self::Error>;
    async fn compute_dependencies(&self, entities: &[Entity]) -> Result<Vec<Dependency>, Self::Error>;
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;
    
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, Self::Error>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Self::Error>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}
```

#### 3.4 CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  RUST_BACKTRACE: 1
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --all-features

  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-features -- -D warnings

  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo bench --no-run
```

### Phase 1: Core Infrastructure (Weeks 2-4)

#### Track A: Graph Engine Team

**Objectives:**
- Implement RocksDB-backed graph store
- Build graph traversal algorithms
- Create snapshot/versioning system

**Deliverables:**

```rust
// crates/codegraph-core/src/graph.rs
pub struct RocksGraphStore {
    db: Arc<rocksdb::DB>,
    nodes: ColumnFamily,
    edges: ColumnFamily,
    indices: Arc<DashMap<String, Vec<Uuid>>>,
}

impl RocksGraphStore {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        // Implementation
    }
    
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Transaction) -> Result<R>,
    {
        // ACID transaction support
    }
}
```

#### Track B: Parser Team

**Objectives:**
- Integrate tree-sitter for multiple languages
- Build AST to graph conversion
- Implement incremental parsing

**Deliverables:**

```rust
// crates/codegraph-parser/src/analyzer.rs
pub struct MultiLanguageAnalyzer {
    parsers: HashMap<Language, Parser>,
    extractors: HashMap<Language, Box<dyn EntityExtractor>>,
}

impl MultiLanguageAnalyzer {
    pub async fn analyze_repository(&self, path: &Path) -> Result<Repository> {
        // Parallel file analysis with rayon
    }
}
```

#### Track C: Vector Index Team

**Objectives:**
- FAISS index management
- Persistent vector storage
- Optimized KNN search

**Deliverables:**

```rust
// crates/codegraph-core/src/index.rs
pub struct FaissIndex {
    index: Arc<RwLock<faiss::Index>>,
    id_map: Arc<DashMap<Uuid, u64>>,
    persistence: IndexPersistence,
}

impl FaissIndex {
    pub async fn build_from_embeddings(&mut self, embeddings: &[(Uuid, Vec<f32>)]) -> Result<()> {
        // Batch index building
    }
}
```

### Phase 2: AI Integration (Weeks 4-6)

#### Track D: AI/ML Team

**Objectives:**
- Local embedding generation with Candle
- OpenAI API integration
- RAG implementation

**Deliverables:**

```rust
// crates/codegraph-ai/src/embeddings.rs
pub enum EmbeddingBackend {
    Local(LocalEmbeddings),
    OpenAI(OpenAIEmbeddings),
    Hybrid(HybridEmbeddings),
}

pub struct EmbeddingPipeline {
    backend: EmbeddingBackend,
    cache: Arc<DashMap<u64, Vec<f32>>>,
    batch_size: usize,
}

impl EmbeddingPipeline {
    pub async fn process_codebase(&self, entities: &[Entity]) -> Result<Vec<Embedding>> {
        // Parallel batch processing
    }
}
```

### Phase 3: API Layer (Weeks 5-7)

#### Track E: GraphQL API Team

**Objectives:**
- Complete GraphQL schema
- Real-time subscriptions
- Authentication middleware

**Deliverables:**

```rust
// crates/codegraph-api/src/graphql.rs
#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn search_code(
        &self,
        ctx: &Context<'_>,
        query: String,
        language: Option<Language>,
        limit: Option<i32>,
    ) -> Result<SearchResults> {
        // NLP-powered code search
    }
    
    async fn get_subgraph(
        &self,
        ctx: &Context<'_>,
        root_id: ID,
        depth: Option<i32>,
        filters: Option<GraphFilters>,
    ) -> Result<Subgraph> {
        // Efficient subgraph retrieval
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn graph_updates(&self, repository_id: ID) -> impl Stream<Item = GraphUpdate> {
        // Real-time graph updates
    }
}
```

#### Track F: MCP Protocol Team

**Objectives:**
- MCP server implementation
- WebSocket transport
- Agent SDK

**Deliverables:**

```rust
// crates/codegraph-api/src/mcp.rs
pub struct McpServer {
    graph: Arc<dyn GraphStore>,
    subscriptions: Arc<RwLock<HashMap<ClientId, Subscription>>>,
    message_bus: broadcast::Sender<McpMessage>,
}

impl McpServer {
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        // WebSocket server with protocol handling
    }
}
```

### Phase 4: Incremental Updates (Weeks 7-8)

#### Track G: Sync Team

**Objectives:**
- File system monitoring
- Git integration
- Delta computation

**Deliverables:**

```rust
// crates/codegraph-sync/src/watcher.rs
pub struct IncrementalIndexer {
    graph: Arc<dyn GraphStore>,
    analyzer: Arc<MultiLanguageAnalyzer>,
    embeddings: Arc<EmbeddingPipeline>,
    update_queue: Arc<SegQueue<UpdateEvent>>,
}

impl IncrementalIndexer {
    pub async fn watch(&self, path: impl AsRef<Path>) -> Result<()> {
        // Debounced file watching with efficient delta updates
    }
}
```

### Phase 5: Optimization & Polish (Weeks 8-10)

**All Teams:**

1. **Performance Optimization**
   - Profile with flamegraph and pprof
   - Optimize hot paths identified in profiling
   - Implement connection pooling and caching

2. **Memory Optimization**
   - Implement arena allocators for graph operations
   - Use memory-mapped files for large indices
   - Optimize struct layouts for cache efficiency

3. **Testing**
   - Property-based testing with proptest
   - Fuzzing critical components
   - Load testing with realistic workloads

4. **Documentation**
   - API documentation with examples
   - Architecture decision records (ADRs)
   - Performance tuning guide

### Phase 6: Deployment & Packaging (Weeks 10-11)

**Deliverables:**

1. **Single Binary Build**
```rust
// build.rs
fn main() {
    // Embed assets at compile time
    println!("cargo:rerun-if-changed=assets/");
    
    // Generate GraphQL schema
    generate_schema();
    
    // Optimize binary size
    configure_optimization();
}
```

2. **Configuration Management**
```rust
// crates/codegraph-cli/src/config.rs
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub ai: AiConfig,
    pub api: ApiConfig,
    pub telemetry: TelemetryConfig,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Layered configuration: defaults -> file -> env -> args
    }
}
```

3. **Deployment Artifacts**
```dockerfile
# Dockerfile.scratch
FROM scratch
COPY target/release/codegraph /
EXPOSE 8080 8081 9090
ENTRYPOINT ["/codegraph"]
```

---

## 4. Performance Specifications

### 4.1 Target Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Indexing Throughput | 10,000 LOC in <30s | Criterion benchmark |
| Query Latency (small) | <50ms | p99 latency |
| Query Latency (large) | <200ms | p99 latency |
| Incremental Update | <1s | End-to-end measurement |
| Memory Usage | <500MB for 100k LOC | RSS monitoring |
| Startup Time | <100ms | Cold start timing |
| Binary Size | <50MB | Stripped release build |

### 4.2 Optimization Strategies

```rust
// Zero-copy operations
pub struct ZeroCopyNode<'a> {
    data: &'a [u8],
    // Use rkyv for zero-copy deserialization
}

// Lock-free operations
pub struct LockFreeGraph {
    nodes: Arc<DashMap<Uuid, Node>>,
    edges: Arc<DashMap<Uuid, Edge>>,
}

// SIMD operations for vector search
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn simd_dot_product(a: &[f32], b: &[f32]) -> f32 {
    // AVX2 implementation
}
```

---

## 5. Security Specifications

### 5.1 Authentication & Authorization

```rust
// JWT-based authentication
pub struct AuthMiddleware {
    jwt_secret: Arc<String>,
    permissions: Arc<DashMap<UserId, Permissions>>,
}

// Rate limiting
pub struct RateLimiter {
    limiters: Arc<DashMap<IpAddr, Governor>>,
}
```

### 5.2 Data Protection

- **Encryption at rest**: Optional RocksDB encryption
- **TLS support**: Rustls for all network communication
- **Audit logging**: Structured logs with tracing
- **Input validation**: Type-safe parsing with strong types

---

## 6. Testing Strategy

### 6.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_graph_traversal() {
        let store = TestGraphStore::new();
        // Test implementation
    }
    
    #[proptest]
    fn test_node_serialization(node: Node) {
        let serialized = bincode::serialize(&node).unwrap();
        let deserialized: Node = bincode::deserialize(&serialized).unwrap();
        prop_assert_eq!(node, deserialized);
    }
}
```

### 6.2 Integration Tests

```rust
// tests/integration/indexing.rs
#[tokio::test]
async fn test_full_repository_indexing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let graph = create_test_graph(temp_dir.path()).await.unwrap();
    
    // Index sample repository
    let repo_path = Path::new("tests/fixtures/sample_repo");
    let result = graph.index_repository(repo_path).await.unwrap();
    
    assert!(result.nodes_created > 100);
    assert!(result.edges_created > 200);
    assert!(result.embeddings_generated > 50);
}
```

### 6.3 Benchmarks

```rust
// benches/graph_operations.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_subgraph_query(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let graph = setup_test_graph();
    
    c.bench_function("subgraph_depth_3", |b| {
        b.to_async(&rt).iter(|| async {
            graph.get_subgraph(black_box(root_id), black_box(3)).await
        });
    });
}

criterion_group!(benches, benchmark_subgraph_query);
criterion_main!(benches);
```

---

## 7. Monitoring & Observability

### 7.1 Metrics Collection

```rust
// crates/codegraph-telemetry/src/metrics.rs
pub struct Metrics {
    pub indexing_duration: Histogram,
    pub query_latency: Histogram,
    pub active_connections: Gauge,
    pub graph_size: Gauge,
    pub embedding_cache_hits: Counter,
}

impl Metrics {
    pub fn record_query(&self, duration: Duration, query_type: &str) {
        self.query_latency
            .with_label_values(&[query_type])
            .observe(duration.as_secs_f64());
    }
}
```

### 7.2 Health Checks

```rust
// Health check endpoint
pub async fn health_check(State(app): State<AppState>) -> impl IntoResponse {
    let checks = vec![
        ("database", app.graph.health_check().await),
        ("vector_index", app.index.health_check().await),
        ("embeddings", app.embeddings.health_check().await),
    ];
    
    Json(HealthStatus {
        status: if checks.iter().all(|(_, ok)| *ok) { "healthy" } else { "degraded" },
        checks,
        version: env!("CARGO_PKG_VERSION"),
        uptime: app.start_time.elapsed(),
    })
}
```

---

## 8. Future Considerations

### 8.1 Scalability Path

1. **Distributed Graph Sharding**
   - Shard by repository or organization
   - Consistent hashing for node distribution
   - Cross-shard query federation

2. **GPU Acceleration**
   - CUDA kernels for vector operations
   - GPU-accelerated FAISS indices
   - Batch embedding generation on GPU

3. **Multi-tenancy**
   - Isolated graph namespaces
   - Per-tenant resource quotas
   - Tenant-aware caching

### 8.2 Advanced Features

1. **Code Intelligence**
   - Type inference and analysis
   - Data flow analysis
   - Security vulnerability detection

2. **Collaboration Features**
   - Real-time collaborative code exploration
   - Shared query sessions
   - Annotation and commenting system

3. **Extended Language Support**
   - Support for 50+ programming languages
   - Language-specific semantic analysis
   - Cross-language dependency tracking

---

## 9. Appendices

### Appendix A: GraphQL Schema

```graphql
type Query {
  searchCode(query: String!, language: Language, limit: Int): SearchResults!
  getNode(id: ID!): Node
  getSubgraph(rootId: ID!, depth: Int, filters: GraphFilters): Subgraph!
  searchByEmbedding(vector: [Float!]!, k: Int!): [SimilarityResult!]!
}

type Mutation {
  indexRepository(path: String!): IndexResult!
  updateNode(id: ID!, properties: JSON!): Node!
  triggerReindex(nodeId: ID!): Boolean!
}

type Subscription {
  graphUpdates(repositoryId: ID!): GraphUpdate!
  indexingProgress(sessionId: ID!): IndexingProgress!
}
```

### Appendix B: MCP Protocol Messages

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpMessage {
    Handshake { version: String, capabilities: Vec<String> },
    Subscribe { topics: Vec<String> },
    Publish { topic: String, payload: Value },
    Request { id: Uuid, method: String, params: Value },
    Response { id: Uuid, result: Option<Value>, error: Option<String> },
}
```

### Appendix C: Configuration Schema

```toml
# config/default.toml
[database]
path = "/var/lib/codegraph/db"
cache_size_mb = 512
compression = "snappy"

[ai]
embedding_model = "sentence-transformers/all-MiniLM-L6-v2"
embedding_cache_size = 10000
batch_size = 32

[api]
graphql_port = 8080
mcp_port = 8081
max_connections = 1000
request_timeout_ms = 30000

[telemetry]
metrics_port = 9090
log_level = "info"
trace_sampling_rate = 0.01
```