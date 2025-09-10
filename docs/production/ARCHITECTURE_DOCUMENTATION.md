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

# CodeGraph Architecture Documentation

## Table of Contents

1. [System Overview](#system-overview)
2. [High-Level Architecture](#high-level-architecture)
3. [Component Architecture](#component-architecture)
4. [Data Flow Architecture](#data-flow-architecture)
5. [Storage Architecture](#storage-architecture)
6. [API Architecture](#api-architecture)
7. [Security Architecture](#security-architecture)
8. [Deployment Architecture](#deployment-architecture)
9. [Performance Architecture](#performance-architecture)
10. [Scalability Considerations](#scalability-considerations)

## System Overview

CodeGraph is a sophisticated code analysis and embedding system designed for high-performance graph-based code understanding. The system transforms source code into intelligent, searchable knowledge graphs that enable advanced code analysis, similarity search, and relationship discovery.

### Core Capabilities

- **Multi-language Code Parsing**: Support for Rust, Python, JavaScript, TypeScript, Go, Java, and C++
- **Graph-based Analysis**: Rich code relationships and dependency tracking
- **Vector Embeddings**: Semantic code search using FAISS vector similarity
- **Version Management**: Git-like versioning with transaction support
- **Real-time Processing**: Streaming APIs for large-scale operations
- **High Performance**: Optimized for concurrent operations with Rust's safety guarantees

### Design Principles

1. **Performance First**: Zero-cost abstractions and memory-efficient operations
2. **Concurrency Safe**: Thread-safe operations using Rust's ownership model
3. **Horizontally Scalable**: Stateless API design with distributed storage support
4. **Fault Tolerant**: Comprehensive error handling and recovery mechanisms
5. **Developer Friendly**: Clear APIs and extensive monitoring capabilities

## High-Level Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        CLI[CLI Tool]
        SDK[SDKs]
        WEB[Web Dashboard]
        API_CLIENT[API Clients]
    end

    subgraph "API Gateway Layer"
        LB[Load Balancer]
        GATEWAY[API Gateway]
        AUTH[Authentication]
        RATE[Rate Limiting]
    end

    subgraph "Application Layer"
        API[CodeGraph API Server]
        GRAPHQL[GraphQL Endpoint]
        REST[REST Endpoints]
        STREAM[Streaming Endpoints]
        WS[WebSocket Support]
    end

    subgraph "Business Logic Layer"
        PARSER[Code Parser]
        GRAPH[Graph Engine]
        VECTOR[Vector Engine]
        VERSION[Version Engine]
        SEARCH[Search Engine]
    end

    subgraph "Data Layer"
        ROCKSDB[RocksDB Storage]
        VECTOR_INDEX[Vector Index]
        CACHE[Cache Layer]
        BACKUP[Backup Storage]
    end

    subgraph "Infrastructure Layer"
        MONITORING[Monitoring]
        LOGGING[Logging]
        METRICS[Metrics]
        ALERTS[Alerting]
    end

    CLI --> LB
    SDK --> LB
    WEB --> LB
    API_CLIENT --> LB

    LB --> GATEWAY
    GATEWAY --> AUTH
    AUTH --> RATE
    RATE --> API

    API --> GRAPHQL
    API --> REST
    API --> STREAM
    API --> WS

    GRAPHQL --> PARSER
    REST --> PARSER
    STREAM --> PARSER
    
    PARSER --> GRAPH
    GRAPH --> VECTOR
    VECTOR --> VERSION
    VERSION --> SEARCH

    GRAPH --> ROCKSDB
    VECTOR --> VECTOR_INDEX
    SEARCH --> CACHE
    VERSION --> BACKUP

    API --> MONITORING
    MONITORING --> LOGGING
    LOGGING --> METRICS
    METRICS --> ALERTS
```

### Architecture Layers

#### 1. Client Layer
- **CLI Tool**: Command-line interface for direct operations
- **SDKs**: Language-specific client libraries (Rust, Python, JavaScript)
- **Web Dashboard**: Browser-based management interface
- **API Clients**: Third-party integrations and custom applications

#### 2. API Gateway Layer
- **Load Balancer**: Distributes incoming requests across instances
- **API Gateway**: Central entry point with routing and protocol handling
- **Authentication**: JWT and API key validation
- **Rate Limiting**: Request throttling and abuse prevention

#### 3. Application Layer
- **CodeGraph API Server**: Core Axum-based HTTP server
- **GraphQL Endpoint**: Flexible query interface with subscriptions
- **REST Endpoints**: RESTful API for standard operations
- **Streaming Endpoints**: High-throughput data streaming
- **WebSocket Support**: Real-time bidirectional communication

#### 4. Business Logic Layer
- **Code Parser**: Tree-sitter based multi-language parsing
- **Graph Engine**: Relationship management and graph operations
- **Vector Engine**: Embedding generation and similarity search
- **Version Engine**: Git-like versioning and transaction management
- **Search Engine**: Full-text and semantic search capabilities

#### 5. Data Layer
- **RocksDB Storage**: Primary persistent storage for graph data
- **Vector Index**: FAISS-based vector similarity index
- **Cache Layer**: In-memory caching for performance optimization
- **Backup Storage**: Automated backup and recovery systems

#### 6. Infrastructure Layer
- **Monitoring**: Health checks and system monitoring
- **Logging**: Structured logging with tracing
- **Metrics**: Prometheus-compatible metrics collection
- **Alerting**: Automated alert generation and notification

## Component Architecture

### Workspace Structure

```
crates/
├── codegraph-core/        # Core types and shared functionality
├── codegraph-graph/       # Graph data structures and RocksDB storage
├── codegraph-parser/      # Tree-sitter based code parsing
├── codegraph-vector/      # Vector embeddings and FAISS search
├── codegraph-cache/       # Caching and performance optimization
├── codegraph-api/         # REST API server using Axum
├── codegraph-mcp/         # Model Context Protocol support
├── codegraph-queue/       # Asynchronous task processing
├── codegraph-git/         # Git integration and version control
├── codegraph-concurrent/  # Concurrency primitives
├── codegraph-zerocopy/    # Zero-copy serialization
└── codegraph-lb/          # Load balancing components
```

### Component Dependencies

```mermaid
graph TD
    CORE[codegraph-core]
    GRAPH[codegraph-graph]
    PARSER[codegraph-parser]
    VECTOR[codegraph-vector]
    CACHE[codegraph-cache]
    API[codegraph-api]
    MCP[codegraph-mcp]
    QUEUE[codegraph-queue]
    GIT[codegraph-git]
    CONCURRENT[codegraph-concurrent]
    ZEROCOPY[codegraph-zerocopy]
    LB[codegraph-lb]

    API --> CORE
    API --> GRAPH
    API --> PARSER
    API --> VECTOR
    API --> CACHE
    API --> MCP
    API --> QUEUE

    GRAPH --> CORE
    GRAPH --> CONCURRENT
    GRAPH --> ZEROCOPY

    PARSER --> CORE
    VECTOR --> CORE
    CACHE --> CORE
    
    MCP --> CORE
    QUEUE --> CORE
    GIT --> CORE
    
    LB --> CORE
    LB --> API
```

### Core Component Details

#### codegraph-core
**Purpose**: Shared types, traits, and foundational functionality

**Key Components**:
- `NodeId`, `EdgeId`: Type-safe identifiers
- `Error`: Unified error handling
- `Result<T>`: Standard result type
- `Config`: Configuration management
- `Metrics`: Performance tracking

**Traits**:
```rust
pub trait NodeStorage {
    fn get_node(&self, id: NodeId) -> Result<Option<Node>>;
    fn insert_node(&mut self, node: Node) -> Result<NodeId>;
    fn update_node(&mut self, id: NodeId, node: Node) -> Result<()>;
    fn delete_node(&mut self, id: NodeId) -> Result<()>;
}

pub trait VectorStore {
    fn search(&self, vector: &[f32], k: usize) -> Result<Vec<SimilarityResult>>;
    fn insert(&mut self, id: NodeId, vector: Vec<f32>) -> Result<()>;
    fn delete(&mut self, id: NodeId) -> Result<()>;
}
```

#### codegraph-graph
**Purpose**: Graph data structures and RocksDB storage

**Key Components**:
- `GraphStorage`: Main graph storage implementation
- `Node`: Code element representation
- `Edge`: Relationship representation
- `GraphQuery`: Query interface

**Storage Architecture**:
```rust
pub struct GraphStorage {
    db: Arc<RocksDB>,
    node_cache: Arc<DashMap<NodeId, Node>>,
    edge_cache: Arc<DashMap<EdgeId, Edge>>,
    config: StorageConfig,
}

// Column families for data organization
const NODE_CF: &str = "nodes";
const EDGE_CF: &str = "edges";
const INDEX_CF: &str = "indexes";
const METADATA_CF: &str = "metadata";
```

#### codegraph-vector
**Purpose**: Vector embeddings and FAISS search

**Key Components**:
- `VectorIndex`: FAISS index wrapper
- `EmbeddingGenerator`: Text-to-vector conversion
- `SimilaritySearch`: Search interface
- `IndexBuilder`: Index construction and optimization

**Vector Architecture**:
```rust
pub struct VectorIndex {
    index: faiss::Index,
    dimension: usize,
    metric: MetricType,
    config: IndexConfig,
}

pub enum IndexType {
    Flat,           // Exact search
    IVF(u32),      // Inverted file index
    HNSW {         // Hierarchical NSW
        m: u32,
        ef_construction: u32,
    },
}
```

## Data Flow Architecture

### Request Processing Flow

```mermaid
sequenceDiagram
    participant Client
    participant Gateway
    participant API
    participant Parser
    participant Graph
    participant Vector
    participant Storage

    Client->>Gateway: HTTP Request
    Gateway->>Gateway: Authentication
    Gateway->>Gateway: Rate Limiting
    Gateway->>API: Validated Request
    
    API->>API: Request Validation
    API->>Parser: Parse Code (if needed)
    Parser->>Parser: Tree-sitter Parse
    Parser->>API: AST Nodes
    
    API->>Graph: Store/Query Nodes
    Graph->>Storage: RocksDB Operations
    Storage-->>Graph: Data
    Graph-->>API: Graph Results
    
    API->>Vector: Generate/Search Embeddings
    Vector->>Vector: FAISS Operations
    Vector-->>API: Vector Results
    
    API->>API: Aggregate Results
    API-->>Gateway: Response
    Gateway-->>Client: HTTP Response
```

### Code Parsing Flow

```mermaid
graph TD
    INPUT[Source Code Input]
    DETECT[Language Detection]
    TOKENIZE[Tokenization]
    PARSE[Tree-sitter Parsing]
    AST[Abstract Syntax Tree]
    EXTRACT[Node Extraction]
    RELATIONSHIP[Relationship Analysis]
    EMBED[Embedding Generation]
    STORE[Storage]

    INPUT --> DETECT
    DETECT --> TOKENIZE
    TOKENIZE --> PARSE
    PARSE --> AST
    AST --> EXTRACT
    EXTRACT --> RELATIONSHIP
    RELATIONSHIP --> EMBED
    EMBED --> STORE

    EXTRACT --> FUNCTIONS[Functions]
    EXTRACT --> CLASSES[Classes]
    EXTRACT --> VARIABLES[Variables]
    EXTRACT --> IMPORTS[Imports]

    RELATIONSHIP --> CALLS[Function Calls]
    RELATIONSHIP --> INHERITANCE[Inheritance]
    RELATIONSHIP --> DEPENDENCIES[Dependencies]
    RELATIONSHIP --> REFERENCES[References]
```

### Vector Search Flow

```mermaid
graph TD
    QUERY[Search Query]
    EMBED_QUERY[Query Embedding]
    INDEX_SEARCH[FAISS Index Search]
    CANDIDATE_FILTER[Candidate Filtering]
    GRAPH_LOOKUP[Graph Data Lookup]
    RESULT_RANKING[Result Ranking]
    RESPONSE[Search Response]

    QUERY --> EMBED_QUERY
    EMBED_QUERY --> INDEX_SEARCH
    INDEX_SEARCH --> CANDIDATE_FILTER
    CANDIDATE_FILTER --> GRAPH_LOOKUP
    GRAPH_LOOKUP --> RESULT_RANKING
    RESULT_RANKING --> RESPONSE

    INDEX_SEARCH --> SIMILARITY[Similarity Scores]
    CANDIDATE_FILTER --> THRESHOLD[Threshold Filtering]
    CANDIDATE_FILTER --> METADATA[Metadata Filtering]
    RESULT_RANKING --> HYBRID[Hybrid Scoring]
```

## Storage Architecture

### RocksDB Organization

```mermaid
graph TD
    subgraph "RocksDB Instance"
        subgraph "Column Families"
            NODE_CF[nodes]
            EDGE_CF[edges]
            INDEX_CF[indexes]
            META_CF[metadata]
            VERSION_CF[versions]
        end

        subgraph "Storage Layout"
            L0[Level 0 - Recent Writes]
            L1[Level 1 - First Compaction]
            L2[Level 2 - Medium Term]
            L3[Level 3 - Long Term]
            L4[Level 4 - Cold Storage]
        end

        subgraph "Components"
            MEMTABLE[MemTable]
            IMMUTABLE[Immutable MemTable]
            SST[SST Files]
            WAL[Write Ahead Log]
        end
    end

    MEMTABLE --> IMMUTABLE
    IMMUTABLE --> L0
    L0 --> L1
    L1 --> L2
    L2 --> L3
    L3 --> L4

    NODE_CF --> SST
    EDGE_CF --> SST
    INDEX_CF --> SST
    META_CF --> SST
    VERSION_CF --> SST
```

### Data Partitioning Strategy

**Horizontal Partitioning**:
```
nodes/
├── {shard_id}/
│   ├── functions/
│   ├── classes/
│   ├── variables/
│   └── modules/
```

**Key Encoding Scheme**:
```rust
// Node keys: {shard_id}:{node_type}:{node_id}
// Edge keys: {shard_id}:edge:{source_id}:{target_id}
// Index keys: {shard_id}:idx:{index_type}:{key}

pub fn encode_node_key(shard_id: u32, node_type: NodeType, node_id: NodeId) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(&shard_id.to_be_bytes());
    key.push(b':');
    key.extend_from_slice(node_type.as_bytes());
    key.push(b':');
    key.extend_from_slice(node_id.as_bytes());
    key
}
```

### Cache Architecture

```mermaid
graph TD
    subgraph "Multi-Level Cache"
        L1[L1 - Hot Data Cache]
        L2[L2 - Node Cache]
        L3[L3 - Query Result Cache]
        BLOCK[Block Cache]
        OS[OS Page Cache]
    end

    subgraph "Cache Policies"
        LRU[LRU Eviction]
        TTL[TTL Expiration]
        SIZE[Size Limits]
    end

    subgraph "Cache Warming"
        PRELOAD[Preload Popular]
        PREDICT[Predictive Loading]
        BACKGROUND[Background Refresh]
    end

    L1 --> L2
    L2 --> L3
    L3 --> BLOCK
    BLOCK --> OS

    L1 --> LRU
    L2 --> TTL
    L3 --> SIZE

    L1 --> PRELOAD
    L2 --> PREDICT
    L3 --> BACKGROUND
```

### Backup and Recovery Architecture

```mermaid
graph TD
    subgraph "Backup Types"
        FULL[Full Backup]
        INCREMENTAL[Incremental Backup]
        CONTINUOUS[Continuous Backup]
    end

    subgraph "Backup Storage"
        LOCAL[Local Storage]
        S3[S3 Compatible]
        DISTRIBUTED[Distributed Storage]
    end

    subgraph "Recovery Points"
        SNAPSHOT[Snapshots]
        WAL_REPLAY[WAL Replay]
        POINT_IN_TIME[Point-in-Time]
    end

    FULL --> LOCAL
    INCREMENTAL --> S3
    CONTINUOUS --> DISTRIBUTED

    SNAPSHOT --> FULL
    WAL_REPLAY --> INCREMENTAL
    POINT_IN_TIME --> CONTINUOUS
```

## API Architecture

### REST API Design

```mermaid
graph TD
    subgraph "REST Endpoints"
        HEALTH[/health]
        NODES[/nodes]
        SEARCH[/search]
        PARSE[/parse]
        VECTOR[/vector]
        STREAM[/stream]
        VERSION[/versions]
    end

    subgraph "HTTP Methods"
        GET[GET - Retrieve]
        POST[POST - Create]
        PUT[PUT - Update]
        DELETE[DELETE - Remove]
        PATCH[PATCH - Partial Update]
    end

    subgraph "Content Types"
        JSON[application/json]
        NDJSON[application/x-ndjson]
        SSE[text/event-stream]
        BINARY[application/octet-stream]
    end

    NODES --> GET
    NODES --> POST
    NODES --> PUT
    NODES --> DELETE

    SEARCH --> GET
    PARSE --> POST
    VECTOR --> POST
    STREAM --> GET

    GET --> JSON
    POST --> JSON
    STREAM --> NDJSON
    STREAM --> SSE
```

### GraphQL Schema Architecture

```graphql
# Core Types
type Node {
  id: ID!
  nodeType: NodeType!
  name: String!
  filePath: String!
  lineNumber: Int!
  metadata: JSON
  relationships: [Relationship!]!
  embeddings: [Float!]
}

type Relationship {
  id: ID!
  type: RelationshipType!
  source: Node!
  target: Node!
  metadata: JSON
}

# Query Interface
type Query {
  # Node operations
  node(id: ID!): Node
  nodes(filter: NodeFilter, pagination: Pagination): NodeConnection!
  
  # Search operations
  search(query: String!, options: SearchOptions): SearchResult!
  similarNodes(nodeId: ID!, threshold: Float): [SimilarityMatch!]!
  
  # Graph traversal
  dependencies(nodeId: ID!, depth: Int): [Node!]!
  dependents(nodeId: ID!, depth: Int): [Node!]!
}

# Mutation Interface
type Mutation {
  # Node management
  createNode(input: CreateNodeInput!): Node!
  updateNode(id: ID!, input: UpdateNodeInput!): Node!
  deleteNode(id: ID!): Boolean!
  
  # Parsing operations
  parseFile(input: ParseFileInput!): ParseResult!
  parseProject(input: ParseProjectInput!): ParseResult!
  
  # Index management
  rebuildIndex(type: IndexType!): IndexRebuildResult!
}

# Real-time updates
type Subscription {
  nodeCreated: Node!
  nodeUpdated: NodeUpdateEvent!
  nodeDeleted: NodeDeleteEvent!
  parseProgress(taskId: ID!): ParseProgressEvent!
}
```

### WebSocket Architecture

```mermaid
sequenceDiagram
    participant Client
    participant WSHandler
    participant EventBus
    participant GraphEngine
    participant VectorEngine

    Client->>WSHandler: WebSocket Connect
    WSHandler->>WSHandler: Authentication
    WSHandler->>EventBus: Subscribe to Events
    
    Client->>WSHandler: GraphQL Subscription
    WSHandler->>GraphEngine: Register Query
    GraphEngine->>EventBus: Emit Node Created
    EventBus->>WSHandler: Forward Event
    WSHandler->>Client: Real-time Update
    
    Client->>WSHandler: Vector Search Stream
    WSHandler->>VectorEngine: Stream Search
    VectorEngine->>WSHandler: Result Batch
    WSHandler->>Client: Streaming Results
```

## Security Architecture

### Authentication and Authorization

```mermaid
graph TD
    subgraph "Authentication Methods"
        API_KEY[API Key]
        JWT[JWT Tokens]
        OAUTH[OAuth 2.0]
        MUTUAL_TLS[Mutual TLS]
    end

    subgraph "Authorization Layers"
        RBAC[Role-Based Access Control]
        ABAC[Attribute-Based Access Control]
        RESOURCE[Resource Permissions]
        OPERATION[Operation Permissions]
    end

    subgraph "Security Middleware"
        AUTH_MW[Authentication Middleware]
        RATE_MW[Rate Limiting Middleware]
        AUDIT_MW[Audit Logging Middleware]
        CORS_MW[CORS Middleware]
    end

    API_KEY --> AUTH_MW
    JWT --> AUTH_MW
    OAUTH --> AUTH_MW
    MUTUAL_TLS --> AUTH_MW

    AUTH_MW --> RBAC
    RBAC --> ABAC
    ABAC --> RESOURCE
    RESOURCE --> OPERATION

    AUTH_MW --> RATE_MW
    RATE_MW --> AUDIT_MW
    AUDIT_MW --> CORS_MW
```

### Data Protection

```mermaid
graph TD
    subgraph "Encryption at Rest"
        DB_ENCRYPT[Database Encryption]
        FILE_ENCRYPT[File System Encryption]
        BACKUP_ENCRYPT[Backup Encryption]
    end

    subgraph "Encryption in Transit"
        TLS[TLS 1.3]
        MUTUAL_TLS[Mutual TLS]
        VPN[VPN Tunnels]
    end

    subgraph "Key Management"
        HSM[Hardware Security Module]
        VAULT[Key Vault]
        ROTATION[Key Rotation]
    end

    subgraph "Data Classification"
        PUBLIC[Public Data]
        INTERNAL[Internal Data]
        CONFIDENTIAL[Confidential Data]
        RESTRICTED[Restricted Data]
    end

    DB_ENCRYPT --> HSM
    FILE_ENCRYPT --> VAULT
    BACKUP_ENCRYPT --> ROTATION

    TLS --> VAULT
    MUTUAL_TLS --> HSM
    VPN --> ROTATION
```

### Network Security

```mermaid
graph TD
    subgraph "Network Layers"
        WAF[Web Application Firewall]
        LOAD_BALANCER[Load Balancer]
        API_GATEWAY[API Gateway]
        APPLICATION[Application Server]
    end

    subgraph "Security Controls"
        DDOS[DDoS Protection]
        IP_FILTER[IP Filtering]
        GEO_BLOCK[Geo Blocking]
        RATE_LIMIT[Rate Limiting]
    end

    subgraph "Monitoring"
        IDS[Intrusion Detection]
        SIEM[SIEM Integration]
        ANOMALY[Anomaly Detection]
        THREAT[Threat Intelligence]
    end

    WAF --> DDOS
    WAF --> IP_FILTER
    LOAD_BALANCER --> GEO_BLOCK
    API_GATEWAY --> RATE_LIMIT

    WAF --> IDS
    LOAD_BALANCER --> SIEM
    API_GATEWAY --> ANOMALY
    APPLICATION --> THREAT
```

## Deployment Architecture

### Container Architecture

```mermaid
graph TD
    subgraph "Container Layer"
        APP_CONTAINER[Application Container]
        SIDECAR[Sidecar Containers]
        INIT[Init Containers]
    end

    subgraph "Application Container"
        API_SERVER[API Server]
        CONFIG[Configuration]
        HEALTH[Health Checks]
    end

    subgraph "Sidecar Containers"
        PROXY[Service Proxy]
        MONITOR[Monitoring Agent]
        LOG_AGENT[Log Forwarder]
        SECURITY[Security Scanner]
    end

    subgraph "Init Containers"
        DB_MIGRATE[DB Migration]
        CONFIG_INIT[Config Initialization]
        CERT_FETCH[Certificate Fetcher]
    end

    APP_CONTAINER --> API_SERVER
    APP_CONTAINER --> CONFIG
    APP_CONTAINER --> HEALTH

    SIDECAR --> PROXY
    SIDECAR --> MONITOR
    SIDECAR --> LOG_AGENT
    SIDECAR --> SECURITY

    INIT --> DB_MIGRATE
    INIT --> CONFIG_INIT
    INIT --> CERT_FETCH
```

### Kubernetes Deployment

```yaml
# Deployment configuration
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codegraph-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: codegraph-api
  template:
    metadata:
      labels:
        app: codegraph-api
    spec:
      containers:
      - name: api-server
        image: codegraph/api:latest
        ports:
        - containerPort: 8080
        - containerPort: 9090
        env:
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: data-volume
          mountPath: /opt/codegraph/data
        - name: config-volume
          mountPath: /opt/codegraph/config
      volumes:
      - name: data-volume
        persistentVolumeClaim:
          claimName: codegraph-data
      - name: config-volume
        configMap:
          name: codegraph-config
```

### High Availability Setup

```mermaid
graph TD
    subgraph "Load Balancer Tier"
        LB1[Load Balancer 1]
        LB2[Load Balancer 2]
        VIP[Virtual IP]
    end

    subgraph "Application Tier"
        APP1[API Server 1]
        APP2[API Server 2]
        APP3[API Server 3]
    end

    subgraph "Data Tier"
        DB_PRIMARY[Primary RocksDB]
        DB_REPLICA1[Replica 1]
        DB_REPLICA2[Replica 2]
    end

    subgraph "Storage Tier"
        STORAGE1[Storage Node 1]
        STORAGE2[Storage Node 2]
        STORAGE3[Storage Node 3]
    end

    VIP --> LB1
    VIP --> LB2
    
    LB1 --> APP1
    LB1 --> APP2
    LB2 --> APP2
    LB2 --> APP3

    APP1 --> DB_PRIMARY
    APP2 --> DB_PRIMARY
    APP3 --> DB_PRIMARY

    DB_PRIMARY --> DB_REPLICA1
    DB_PRIMARY --> DB_REPLICA2

    DB_PRIMARY --> STORAGE1
    DB_REPLICA1 --> STORAGE2
    DB_REPLICA2 --> STORAGE3
```

## Performance Architecture

### Performance Optimization Strategies

```mermaid
graph TD
    subgraph "Application Level"
        ASYNC[Async Processing]
        BATCH[Batch Operations]
        PIPELINE[Request Pipelining]
        CACHE[Smart Caching]
    end

    subgraph "Database Level"
        COMPACTION[Compaction Tuning]
        BLOOM[Bloom Filters]
        COMPRESSION[Compression]
        SHARDING[Data Sharding]
    end

    subgraph "Vector Level"
        INDEX_OPT[Index Optimization]
        QUANTIZATION[Vector Quantization]
        PRUNING[Index Pruning]
        PARALLEL[Parallel Search]
    end

    subgraph "Network Level"
        HTTP2[HTTP/2 Push]
        COMPRESSION_NET[Response Compression]
        CDN[CDN Caching]
        KEEPALIVE[Connection Pooling]
    end

    ASYNC --> COMPACTION
    BATCH --> BLOOM
    PIPELINE --> COMPRESSION
    CACHE --> SHARDING

    INDEX_OPT --> HTTP2
    QUANTIZATION --> COMPRESSION_NET
    PRUNING --> CDN
    PARALLEL --> KEEPALIVE
```

### Performance Monitoring

```mermaid
graph TD
    subgraph "Application Metrics"
        REQUEST_RATE[Request Rate]
        RESPONSE_TIME[Response Time]
        ERROR_RATE[Error Rate]
        THROUGHPUT[Throughput]
    end

    subgraph "System Metrics"
        CPU_USAGE[CPU Usage]
        MEMORY_USAGE[Memory Usage]
        DISK_IO[Disk I/O]
        NETWORK_IO[Network I/O]
    end

    subgraph "Database Metrics"
        COMPACTION_STATS[Compaction Stats]
        CACHE_HIT_RATE[Cache Hit Rate]
        WRITE_AMPLIFICATION[Write Amplification]
        READ_AMPLIFICATION[Read Amplification]
    end

    subgraph "Vector Metrics"
        SEARCH_LATENCY[Search Latency]
        INDEX_SIZE[Index Size]
        RECALL_ACCURACY[Recall Accuracy]
        BUILD_TIME[Build Time]
    end

    REQUEST_RATE --> CPU_USAGE
    RESPONSE_TIME --> MEMORY_USAGE
    ERROR_RATE --> DISK_IO
    THROUGHPUT --> NETWORK_IO

    CPU_USAGE --> COMPACTION_STATS
    MEMORY_USAGE --> CACHE_HIT_RATE
    DISK_IO --> WRITE_AMPLIFICATION
    NETWORK_IO --> READ_AMPLIFICATION

    COMPACTION_STATS --> SEARCH_LATENCY
    CACHE_HIT_RATE --> INDEX_SIZE
    WRITE_AMPLIFICATION --> RECALL_ACCURACY
    READ_AMPLIFICATION --> BUILD_TIME
```

## Scalability Considerations

### Horizontal Scaling Strategy

```mermaid
graph TD
    subgraph "Scaling Dimensions"
        COMPUTE[Compute Scaling]
        STORAGE[Storage Scaling]
        NETWORK[Network Scaling]
        MEMORY[Memory Scaling]
    end

    subgraph "Scaling Patterns"
        STATELESS[Stateless Services]
        SHARDING[Data Sharding]
        REPLICATION[Read Replicas]
        PARTITIONING[Functional Partitioning]
    end

    subgraph "Auto-scaling Triggers"
        CPU_THRESHOLD[CPU > 70%]
        MEMORY_THRESHOLD[Memory > 80%]
        QUEUE_DEPTH[Queue Depth > 100]
        RESPONSE_TIME[Response Time > 2s]
    end

    COMPUTE --> STATELESS
    STORAGE --> SHARDING
    NETWORK --> REPLICATION
    MEMORY --> PARTITIONING

    STATELESS --> CPU_THRESHOLD
    SHARDING --> MEMORY_THRESHOLD
    REPLICATION --> QUEUE_DEPTH
    PARTITIONING --> RESPONSE_TIME
```

### Data Partitioning Strategy

```mermaid
graph TD
    subgraph "Partitioning Methods"
        HASH[Hash Partitioning]
        RANGE[Range Partitioning]
        DIRECTORY[Directory Partitioning]
        HYBRID[Hybrid Partitioning]
    end

    subgraph "Partition Keys"
        PROJECT_ID[Project ID]
        FILE_PATH[File Path]
        NODE_TYPE[Node Type]
        TIMESTAMP[Timestamp]
    end

    subgraph "Rebalancing"
        CONSISTENT_HASH[Consistent Hashing]
        VIRTUAL_NODES[Virtual Nodes]
        MIGRATION[Live Migration]
        HOTSPOT[Hotspot Detection]
    end

    HASH --> PROJECT_ID
    RANGE --> FILE_PATH
    DIRECTORY --> NODE_TYPE
    HYBRID --> TIMESTAMP

    PROJECT_ID --> CONSISTENT_HASH
    FILE_PATH --> VIRTUAL_NODES
    NODE_TYPE --> MIGRATION
    TIMESTAMP --> HOTSPOT
```

### Capacity Planning

**Growth Projections**:
- **Data Growth**: 50% annually
- **Query Growth**: 100% annually
- **User Growth**: 200% annually

**Resource Requirements**:
```
Current Baseline (1M nodes):
- Storage: 100GB RocksDB + 50GB Vector Index
- Memory: 16GB (8GB cache + 8GB application)
- CPU: 8 cores (4 for API + 4 for background tasks)
- Network: 1Gbps

Projected 12 months (10M nodes):
- Storage: 1TB RocksDB + 500GB Vector Index
- Memory: 64GB (32GB cache + 32GB application)
- CPU: 32 cores (16 for API + 16 for background tasks)
- Network: 10Gbps
```

**Scaling Checkpoints**:
- **1M nodes**: Single instance sufficient
- **10M nodes**: Require read replicas and caching
- **100M nodes**: Require sharding and distributed architecture
- **1B nodes**: Require specialized distributed vector databases

This architecture documentation provides a comprehensive foundation for understanding, deploying, and maintaining the CodeGraph system in production environments. For operational procedures, refer to the [Operations Runbook](OPERATIONS_RUNBOOK.md) and [Troubleshooting Guide](TROUBLESHOOTING_GUIDE.md).