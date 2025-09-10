# CodeGraph ASCII Architecture Diagrams

This document provides ASCII-based architecture diagrams that are compatible with any text editor or markdown viewer, ensuring accessibility across all platforms.

## System Architecture Overview

```ascii
╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                     CodeGraph System Architecture                                           ║
╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                      Client Layer                                                      │   ║
║  │                                                                                                       │   ║
║  │    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │   ║
║  │    │   Web UI    │    │  CLI Client │    │    VS Code  │    │   AI Agents │    │  Mobile App │       │   ║
║  │    │  (React)    │    │  (Native)   │    │ Extension   │    │    (MCP)    │    │ (Future)    │       │   ║
║  │    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘       │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                          │                                                                    ║
║                                          ▼                                                                    ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                    API Gateway Layer                                                  │   ║
║  │                                                                                                       │   ║
║  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │   ║
║  │  │ Load Balancer│  │     Auth     │  │ Rate Limiting│  │  API Router  │  │  WebSocket   │           │   ║
║  │  │ (codegraph-lb)│ │    (JWT)     │  │  (Governor)  │  │    (Axum)    │  │   Gateway    │           │   ║
║  │  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘           │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                          │                                                                    ║
║                                          ▼                                                                    ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                   Core Service Layer                                                  │   ║
║  │                                                                                                       │   ║
║  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────┐   │   ║
║  │  │ Code Parser │  │Vector Engine│  │Graph Engine │  │Cache Manager│  │Task Queue   │  │MCP Server│   │   ║
║  │  │(Tree-sitter)│  │   (FAISS)   │  │  (RocksDB)  │  │  (Memory)   │  │  (Async)    │  │(Protocol)│   │   ║
║  │  │             │  │             │  │             │  │             │  │             │  │          │   │   ║
║  │  │ • Rust      │  │ • Embeddings│  │ • AST Nodes │  │ • Hot Data  │  │ • Parsing   │  │• AI Tools│   │   ║
║  │  │ • Python    │  │ • Similarity│  │ • Relations │  │ • Query     │  │ • Indexing  │  │• Resources│  │   ║
║  │  │ • JS/TS     │  │ • Search    │  │ • Metadata  │  │ • Results   │  │ • Analysis  │  │• Prompts │   │   ║
║  │  │ • Go/Java   │  │ • Indexing  │  │ • History   │  │ • Sessions  │  │ • Updates   │  │          │   │   ║
║  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  └──────────┘   │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                          │                                                                    ║
║                                          ▼                                                                    ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                   Storage Layer                                                       │   ║
║  │                                                                                                       │   ║
║  │  ┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐        ┌──────────────┐   │   ║
║  │  │   RocksDB       │        │  FAISS Indices  │        │  Memory Cache   │        │ File System  │   │   ║
║  │  │                 │        │                 │        │                 │        │              │   │   ║
║  │  │ • Graph Nodes   │◄──────►│ • Code Vectors  │◄──────►│ • Parse Results │◄──────►│ • Git Repos  │   │   ║
║  │  │ • Relationships │        │ • Comment Embs  │        │ • Query Cache   │        │ • Projects   │   │   ║
║  │  │ • Metadata      │        │ • Similarity    │        │ • Session Data  │        │ • Artifacts  │   │   ║
║  │  │ • Versions      │        │ • Clusters      │        │ • Hot Objects   │        │ • Configs    │   │   ║
║  │  │                 │        │                 │        │                 │        │              │   ║
║  │  │ Persistent      │        │ Searchable      │        │ Fast Access     │        │ Source Data  │   │   ║
║  │  │ ACID Compliant  │        │ Vector Space    │        │ TTL Managed     │        │ Version Ctrl │   │   ║
║  │  └─────────────────┘        └─────────────────┘        └─────────────────┘        └──────────────┘   │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                  Infrastructure Layer                                                 │   ║
║  │                                                                                                       │   ║
║  │ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐│   ║
║  │ │  Monitoring  │ │   Tracing    │ │ Configuration│ │   Logging    │ │   Security   │ │ Health Check ││   ║
║  │ │ (Prometheus) │ │   (Jaeger)   │ │ (TOML/ENV)   │ │ (Structured) │ │ (Auth/TLS)   │ │  (Endpoint)  ││   ║
║  │ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘│   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  Performance Targets:  QPS: 1000+  │  Latency: <50ms p99  │  Memory: <500MB/1M LOC  │  Availability: 99.9% ║
╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Crate Dependency Tree

```ascii
CodeGraph Workspace Dependency Hierarchy
=========================================

codegraph-core (Foundation - Tier 0)
│
├── codegraph-zerocopy (Serialization - Tier 1)
│   │
│   ├── codegraph-graph (Storage - Tier 2)
│   │   │
│   │   └── [RocksDB Integration]
│   │       ├── Column Families
│   │       ├── Bloom Filters  
│   │       ├── Compaction Strategies
│   │       └── Backup/Recovery
│   │
│   ├── codegraph-vector (Search - Tier 2)
│   │   │
│   │   └── [FAISS Integration]
│   │       ├── Index Types (Flat, IVF, HNSW, PQ)
│   │       ├── Distance Metrics
│   │       ├── Quantization
│   │       └── GPU Acceleration (Optional)
│   │
│   └── codegraph-cache (Memory - Tier 2)
│       │
│       └── [Cache Layers]
│           ├── L1: Hot AST Nodes
│           ├── L2: Query Results
│           ├── L3: Vector Embeddings
│           └── L4: Session Data
│
├── codegraph-parser (Processing - Tier 1)
│   │
│   ├── [Tree-sitter Languages]
│   │   ├── Rust (tree-sitter-rust v0.24)
│   │   ├── Python (tree-sitter-python v0.23)
│   │   ├── JavaScript (tree-sitter-javascript v0.25)
│   │   ├── TypeScript (tree-sitter-typescript v0.23)
│   │   ├── Go (tree-sitter-go v0.23)
│   │   ├── Java (tree-sitter-java v0.23)
│   │   └── C++ (tree-sitter-cpp v0.23)
│   │
│   └── [Processing Pipeline]
│       ├── Lexical Analysis
│       ├── Syntax Tree Generation
│       ├── Semantic Analysis
│       ├── Entity Extraction
│       └── Relationship Mapping
│
├── codegraph-queue (Async - Tier 1)
│   │
│   └── [Task Management]
│       ├── Priority Queues
│       ├── Worker Pools  
│       ├── Backpressure Control
│       ├── Dead Letter Queues
│       └── Retry Mechanisms
│
├── codegraph-git (VCS - Tier 1)
│   │
│   └── [Git Operations]
│       ├── Repository Scanning
│       ├── Diff Processing
│       ├── Branch Tracking
│       ├── Commit Analysis
│       └── File History
│
├── codegraph-concurrent (Parallel - Tier 1)
│   │
│   └── [Concurrency Primitives]
│       ├── Thread Pools (Rayon)
│       ├── Async Runtime (Tokio)
│       ├── Lock-Free Structures
│       ├── Work Stealing
│       └── NUMA Awareness
│
├── codegraph-mcp (Protocol - Tier 2)
│   │
│   ├── [Dependencies: core, graph, vector, parser]
│   │
│   └── [MCP Implementation]
│       ├── Transport Layer (STDIO/HTTP)
│       ├── Tool Registry
│       ├── Resource Management
│       ├── Prompt Templates
│       └── Client Sessions
│
├── core-rag-mcp-server (RAG - Tier 3)
│   │
│   ├── [Dependencies: mcp, api]
│   │
│   └── [RAG Implementation]
│       ├── Vector Retrieval
│       ├── Graph Retrieval  
│       ├── Hybrid Search
│       ├── Context Ranking
│       └── Response Generation
│
├── codegraph-api (Service - Tier 3)
│   │
│   ├── [Dependencies: ALL previous layers]
│   │
│   └── [API Implementation]
│       ├── REST Endpoints (Axum)
│       ├── GraphQL Schema (async-graphql)
│       ├── WebSocket Support
│       ├── Streaming Responses
│       ├── Authentication (JWT)
│       ├── Rate Limiting (Governor)
│       ├── Request Validation
│       ├── Error Handling
│       ├── Metrics Collection
│       └── Health Monitoring
│
└── codegraph-lb (Load Balancer - Tier 4)
    │
    ├── [Dependencies: core, api]
    │
    └── [Load Balancing]
        ├── Request Routing
        ├── Health Checks
        ├── Circuit Breakers
        ├── Traffic Shaping
        └── Auto-scaling Integration

Complexity Legend:
==================
Tier 0: Foundation (0 deps)     🟢 Stable
Tier 1: Basic Services (1 dep)  🟡 Moderate  
Tier 2: Data Services (2-3 deps) 🟠 Complex
Tier 3: Application (4+ deps)   🔴 Critical
Tier 4: Infrastructure (All)    ⚫ Maximum
```

## Data Flow Architecture

```ascii
╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                    CodeGraph Data Flow Diagram                                             ║
╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                                              ║
║  Input Sources                    Processing Pipeline                     Storage & Indexing                ║
║  ──────────────                   ──────────────────                     ──────────────────                 ║
║                                                                                                              ║
║  ┌─────────────┐                                                                                             ║
║  │Git Repository│ ─────┐                                                                                      ║
║  └─────────────┘      │                                                                                      ║
║                       │         ┌─────────────────────────────────────────────────────────────────────┐     ║
║  ┌─────────────┐      │         │                    File Processing Queue                            │     ║
║  │ File System │ ─────┼────────►│                                                                     │     ║
║  └─────────────┘      │         │  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐       │     ║
║                       │         │  │File 1  │  │File 2  │  │File 3  │  │ ... │  │File N  │       │     ║
║  ┌─────────────┐      │         │  │(.rs)   │  │(.py)   │  │(.ts)   │  │     │  │(.java) │       │     ║
║  │   Archive   │ ─────┘         │  └────────┘  └────────┘  └────────┘  └────────┘  └────────┘       │     ║
║  │(ZIP/TAR.GZ) │                │                                                                     │     ║
║  └─────────────┘                │  Rate Limit: 50k files/min  │  Batch Size: 100 files             │     ║
║                                 └─────────────────────────────┼─────────────────────────────────────┘     ║
║                                                               │                                             ║
║                                                               ▼                                             ║
║  Language Detection              ┌─────────────────────────────────────────────────────────────────────┐     ║
║  ──────────────────              │                  Tree-sitter Parser Pool                          │     ║
║                                 │                                                                     │     ║
║  ┌─────────────┐                │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │     ║
║  │   .rs    ──►│                │  │ Rust     │ │ Python   │ │   JS     │ │    TS    │ │   Go     │ │     ║
║  │   .py    ──►│                │  │ Parser   │ │ Parser   │ │ Parser   │ │ Parser   │ │ Parser   │ │     ║
║  │   .js    ──►│ Language       │  │  (AST)   │ │  (AST)   │ │  (AST)   │ │  (AST)   │ │  (AST)   │ │     ║
║  │   .ts    ──►│ Router         │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │     ║
║  │   .go    ──►│                │      │           │           │           │           │         │     ║
║  │  .java   ──►│                │      ▼           ▼           ▼           ▼           ▼         │     ║
║  │   .cpp   ──►│                │                                                                     │     ║
║  └─────────────┘                │  Abstract Syntax Trees (ASTs) → Parallel Processing              │     ║
║                                 └─────────────────────────────┼─────────────────────────────────────┘     ║
║                                                               │                                             ║
║                                                               ▼                                             ║
║  Entity Extraction              ┌─────────────────────────────────────────────────────────────────────┐     ║
║  ─────────────────              │                     Entity Extraction Engine                       │     ║
║                                 │                                                                     │     ║
║  From AST Nodes:               │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │     ║
║  • Functions                    │  │  Functions  │  │   Classes   │  │  Variables  │  │   Imports   │ │     ║
║  • Classes                      │  │             │  │             │  │             │  │             │ │     ║
║  • Variables                    │  │ • Name      │  │ • Name      │  │ • Name      │  │ • Module    │ │     ║
║  • Imports                      │  │ • Params    │  │ • Methods   │  │ • Type      │  │ • Items     │ │     ║
║  • Comments                     │  │ • Return    │  │ • Fields    │  │ • Scope     │  │ • Aliases   │ │     ║
║  • Modules                      │  │ • Body      │  │ • Inherits  │  │ • Value     │  │ • Source    │ │     ║
║                                 │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘ │     ║
║                                 └─────────────────────────────┼─────────────────────────────────────┘     ║
║                                                               │                                             ║
║                                                               ▼                                             ║
║  Parallel Storage Streams       ┌─────────────────────────────────────────────────────────────────────┐     ║
║  ────────────────────────       │                   Parallel Output Processing                        │     ║
║                                 │                                                                     │     ║
║  Stream 1: Graph Nodes         │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │     ║
║  Stream 2: Vector Embeddings    │  │   Graph     │  │   Vector    │  │    Cache    │  │   Search    │ │     ║
║  Stream 3: Cache Updates        │  │   Store     │  │  Embedding  │  │   Update    │  │   Index     │ │     ║
║  Stream 4: Search Index         │  │             │  │             │  │             │  │             │ │     ║
║                                 │  │ RocksDB ◄───┼──┼─► FAISS ◄───┼──┼─► Memory ◄──┼──┼─► FTS       │ │     ║
║                                 │  │ • Nodes     │  │ • Vectors   │  │ • Hot Data  │  │ • Keywords  │ │     ║
║                                 │  │ • Edges     │  │ • Clusters  │  │ • Sessions  │  │ • Metadata  │ │     ║
║                                 │  │ • Props     │  │ • Indices   │  │ • Results   │  │ • Rankings  │ │     ║
║                                 │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘ │     ║
║                                 └─────────────────────────────┼─────────────────────────────────────┘     ║
║                                                               │                                             ║
║                                                               ▼                                             ║
║  Query Processing               ┌─────────────────────────────────────────────────────────────────────┐     ║
║  ────────────────               │                      Query Interface                                │     ║
║                                 │                                                                     │     ║
║  Query Types:                   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │     ║
║  • Text Search                  │  │    Text     │  │   Vector    │  │    Graph    │  │   Hybrid    │ │     ║
║  • Vector Similarity            │  │   Search    │  │ Similarity  │  │  Traversal  │  │   Search    │ │     ║
║  • Graph Traversal              │  │             │  │             │  │             │  │             │ │     ║
║  • Hybrid Queries               │  │ Keywords ───┼──┼─► Embeddings─┼──┼─► Relations─┼──┼─► Combined   │ │     ║
║                                 │  │ Fuzzy       │  │ Cosine Sim  │  │ BFS/DFS     │  │ Ranking     │ │     ║
║                                 │  │ Regex       │  │ Euclidean   │  │ PageRank    │  │ Fusion      │ │     ║
║                                 │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘ │     ║
║                                 └─────────────────────────────┼─────────────────────────────────────┘     ║
║                                                               │                                             ║
║                                                               ▼                                             ║
║                                 ┌─────────────────────────────────────────────────────────────────────┐     ║
║                                 │                     Response Pipeline                                │     ║
║                                 │                                                                     │     ║
║                                 │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │     ║
║                                 │  │   Result    │  │    Cache    │  │   Format    │  │   Deliver   │ │     ║
║                                 │  │Aggregation  │  │   Store     │  │  Response   │  │   to User   │ │     ║
║                                 │  │             │  │             │  │             │  │             │ │     ║
║                                 │  │ • Scoring   │  │ • TTL       │  │ • JSON      │  │ • REST      │ │     ║
║                                 │  │ • Ranking   │  │ • LRU       │  │ • GraphQL   │  │ • GraphQL   │ │     ║
║                                 │  │ • Filtering │  │ • Compress  │  │ • Stream    │  │ • WebSocket │ │     ║
║                                 │  │ • Pagination│  │ • Invalidate│  │ • Compress  │  │ • MCP       │ │     ║
║                                 │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘ │     ║
║                                 └─────────────────────────────────────────────────────────────────────┘     ║
║                                                                                                              ║
║  Performance Metrics:                                                                                       ║
║  • Parse Rate: 50k LOC/min       • Query Latency: <50ms p99      • Cache Hit: >95%                         ║
║  • Throughput: 1000+ QPS         • Memory Usage: <500MB/1M LOC   • Availability: 99.9%                     ║
╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Memory Layout & Performance Optimization

```ascii
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 CodeGraph Memory Architecture                                               │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                             │
│  Process Memory Space (Virtual Address Space)                                                              │
│  ═══════════════════════════════════════════════                                                           │
│                                                                                                             │
│  0x00000000 ┌─────────────────────────────────────────────────────────────────────────────────┐ 0xFFFFFFFF │
│             │                                                                                 │             │
│             │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────│             │
│             │  │    Stack    │  │    Heap     │  │   Memory    │  │ Zero-Copy   │  │  Kernel │             │
│             │  │  (8-16 MB)  │  │  (Dynamic)  │  │   Mapped    │  │  Archives   │  │  Space  │             │
│             │  │             │  │             │  │   Files     │  │  (rkyv)     │  │         │             │
│             │  │ • Tokio     │  │ • AST Nodes │  │             │  │             │  │ • System│             │
│             │  │   Tasks     │  │ • Hash Maps │  │ • RocksDB   │  │ • Immutable │  │   Calls │             │
│             │  │ • Function  │  │ • Vectors   │  │   Files     │  │   Data      │  │ • I/O   │             │
│             │  │   Frames    │  │ • Buffers   │  │ • FAISS     │  │ • Direct    │  │   Ops   │             │
│             │  │ • Local     │  │ • Cache     │  │   Indices   │  │   Access    │  │         │             │
│             │  │   Variables │  │   Objects   │  │ • Log Files │  │ • No Copy   │  │         │             │
│             │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  └─────────│             │
│             │      ▲               ▲               ▲               ▲                         │             │
│             │      │               │               │               │                         │             │
│             │  ┌───▼───┐       ┌───▼───┐       ┌───▼───┐       ┌───▼───┐                     │             │
│             │  │  Fast │       │Dynamic│       │ mmap()│       │ rkyv  │                     │             │
│             │  │ Access│       │ Alloc │       │Virtual│       │Layout │                     │             │
│             │  │~1ns   │       │~100ns │       │Memory │       │Control│                     │             │
│             │  └───────┘       └───────┘       └───────┘       └───────┘                     │             │
│             └─────────────────────────────────────────────────────────────────────────────────┘             │
│                                                                                                             │
│  Memory Optimization Strategies                                                                            │
│  ═══════════════════════════════                                                                            │
│                                                                                                             │
│  1. Arena Allocation (bumpalo)           │  2. Object Pooling                                              │
│     ┌─────────────────────────────────┐  │     ┌─────────────────────────────────┐                        │
│     │    Parse Session Arena          │  │     │        Object Pools             │                        │
│     │  ┌─────────┬─────────┬─────────┐│  │     │  ┌─────────┬─────────┬─────────┐│                        │
│     │  │ Node 1  │ Node 2  │ Node 3  ││  │     │  │ Parser  │ Buffer  │ Query   ││                        │
│     │  │ (1KB)   │ (2KB)   │ (3KB)   ││  │     │  │ Pool    │ Pool    │ Pool    ││                        │
│     │  └─────────┴─────────┴─────────┘│  │     │  │ (16)    │ (32)    │ (8)     ││                        │
│     │  Bulk Free → ~1μs                │  │     │  └─────────┴─────────┴─────────┘│                        │
│     └─────────────────────────────────┘  │     │  Reuse → ~10ns access           │                        │
│                                          │     └─────────────────────────────────┘                        │
│                                          │                                                                 │
│  3. Lock-Free Data Structures            │  4. Memory Mapping Strategy                                     │
│     ┌─────────────────────────────────┐  │     ┌─────────────────────────────────┐                        │
│     │     DashMap<K,V>                │  │     │    Large File Handling          │                        │
│     │  ┌─────────┬─────────┬─────────┐│  │     │  ┌─────────┬─────────┬─────────┐│                        │
│     │  │Shard 1  │Shard 2  │Shard N  ││  │     │  │  File   │ Memory  │Virtual  ││                        │
│     │  │(Lock)   │(Lock)   │(Lock)   ││  │     │  │ System  │ Mapped  │ Pages   ││                        │
│     │  └─────────┴─────────┴─────────┘│  │     │  │ (Disk)  │ (RAM)   │ (OS)    ││                        │
│     │  Concurrent Reads/Writes        │  │     │  └─────────┴─────────┴─────────┘│                        │
│     └─────────────────────────────────┘  │     │  Lazy Loading → On-demand       │                        │
│                                          │     └─────────────────────────────────┘                        │
│                                          │                                                                 │
│  5. Zero-Copy Serialization (rkyv)       │  6. Compression & Encoding                                      │
│     ┌─────────────────────────────────┐  │     ┌─────────────────────────────────┐                        │
│     │    Archive Layout               │  │     │     Compression Pipeline        │                        │
│     │  ┌─────────┬─────────┬─────────┐│  │     │  ┌─────────┬─────────┬─────────┐│                        │
│     │  │ Header  │  Data   │ Index   ││  │     │  │   LZ4   │  Zstd   │ Custom  ││                        │
│     │  │ (Meta)  │(Aligned)│(Offsets)││  │     │  │ (Fast)  │(Ratio)  │(Domain) ││                        │
│     │  └─────────┴─────────┴─────────┘│  │     │  │ ~1GB/s  │ ~500MB/s│ ~2GB/s  ││                        │
│     │  Direct Access → 0ns            │  │     │  └─────────┴─────────┴─────────┘│                        │
│     └─────────────────────────────────┘  │     │  Adaptive → Content-aware       │                        │
│                                          │     └─────────────────────────────────┘                        │
│                                                                                                             │
│  Memory Usage Breakdown (1M LOC Project)                                                                   │
│  ═══════════════════════════════════════                                                                   │
│                                                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐ │
│  │ Component                    │ Memory Usage │ Optimization                │ Notes                     │ │
│  ├─────────────────────────────────────────────────────────────────────────────────────────────────────┤ │
│  │ AST Nodes (Graph)           │   ~200 MB    │ Node compression, pooling   │ Largest component         │ │
│  │ Vector Embeddings (FAISS)   │   ~150 MB    │ Quantization, clustering    │ Dimension-dependent       │ │
│  │ RocksDB Block Cache         │   ~100 MB    │ Adaptive cache sizing       │ Configurable              │ │
│  │ Parser Working Set          │    ~50 MB    │ Arena allocation            │ Per-session temporary     │ │
│  │ Query Result Cache          │    ~30 MB    │ LRU eviction, TTL           │ Hot query optimization    │ │
│  │ Runtime (Tokio + Heap)      │    ~20 MB    │ Task scheduling, GC         │ Baseline overhead         │ │
│  │ Total Working Set           │   ~550 MB    │ Various strategies          │ Target: <500MB            │ │
│  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                                             │
│  Garbage Collection Strategy                                                                               │
│  ═══════════════════════════                                                                               │
│                                                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐ │
│  │ 1. Reference Counting (Rc/Arc)  │ 4. Memory Leak Detection                                            │ │
│  │    • Automatic cleanup          │    • memscope-rs in development                                    │ │
│  │    • Cycle detection           │    • Stack trace collection                                        │ │
│  │    • Weak references           │    • Periodic memory audits                                        │ │
│  │                                │                                                                     │ │
│  │ 2. RAII Pattern                │ 5. Proactive Cleanup                                                │ │
│  │    • Deterministic destructors │    • Session lifecycle management                                   │ │
│  │    • Resource guards          │    • Cache eviction policies                                       │ │
│  │    • Scope-based cleanup       │    • Background compaction                                         │ │
│  │                                │                                                                     │ │
│  │ 3. Arena Disposal              │ 6. Memory Pressure Response                                         │ │
│  │    • Bulk free operations      │    • Dynamic cache resizing                                        │ │
│  │    • Session boundaries        │    • Graceful degradation                                          │ │
│  │    • Parse context cleanup     │    • Emergency cleanup routines                                    │ │
│  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Deployment Configuration Matrix

```ascii
╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                              CodeGraph Deployment Configurations                                           ║
╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                   Development Environment                                              │   ║
║  │                                                                                                       │   ║
║  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐           │   ║
║  │  │   Local IDE     │    │  codegraph-api  │    │   File System   │    │   Local Browser │           │   ║
║  │  │                 │    │                 │    │                 │    │                 │           │   ║
║  │  │ • VS Code       │───►│ • Single Process│───►│ • Git Repos     │◄───│ • Web UI        │           │   ║
║  │  │ • Rust Analyzer │    │ • Debug Build   │    │ • RocksDB       │    │ • API Testing   │           │   ║
║  │  │ • Extensions    │    │ • Hot Reload    │    │ • FAISS Index   │    │ • Documentation │           │   ║
║  │  │ • Terminal      │    │ • Memory Leak   │    │ • Logs          │    │ • Monitoring    │           │   ║
║  │  │                 │    │   Detection     │    │ • Config        │    │                 │           │   ║
║  │  └─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘           │   ║
║  │                                                                                                       │   ║
║  │  Resources: 4-8 cores, 8-16 GB RAM, 100 GB SSD                                                      │   ║
║  │  Performance: QPS: 10-50, Latency: <100ms, Projects: 1-5, Users: 1-3                               │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                    Staging Environment                                                │   ║
║  │                                                                                                       │   ║
║  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐           │   ║
║  │  │ Load Balancer   │    │    API Cluster  │    │  Storage Cluster│    │   Monitoring    │           │   ║
║  │  │                 │    │                 │    │                 │    │                 │           │   ║
║  │  │ • Nginx/HAProxy │───►│ • 2 Instances   │───►│ • Shared RocksDB│◄───│ • Prometheus    │           │   ║
║  │  │ • Health Checks │    │ • Release Build │    │ • FAISS Cluster │    │ • Grafana       │           │   ║
║  │  │ • SSL Termination│   │ • Auto-restart  │    │ • Redis Cache   │    │ • Log Aggreg.  │           │   ║
║  │  │ • Rate Limiting │    │ • Rolling Deploy│    │ • Backup/Restore│    │ • Alert Rules   │           │   ║
║  │  │                 │    │                 │    │                 │    │                 │           │   ║
║  │  └─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘           │   ║
║  │                                                                                                       │   ║
║  │  Resources: 8-16 cores, 32-64 GB RAM, 500 GB NVMe                                                   │   ║
║  │  Performance: QPS: 100-500, Latency: <75ms, Projects: 10-50, Users: 5-25                           │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                  Production Environment                                               │   ║
║  │                                                                                                       │   ║
║  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐           │   ║
║  │  │ Global CDN      │    │ API Gateway     │    │ Processing Tier │    │ Storage Tier    │           │   ║
║  │  │                 │    │                 │    │                 │    │                 │           │   ║
║  │  │ • CloudFlare    │───►│ • 3+ Instances  │───►│ • Worker Nodes  │───►│ • Sharded DB    │           │   ║
║  │  │ • DDoS Protect  │    │ • Auto-scaling  │    │ • Task Queue    │    │ • Read Replicas │           │   ║
║  │  │ • Geo Routing   │    │ • Circuit Break │    │ • MCP Servers   │    │ • Vector Shards │           │   ║
║  │  │ • Edge Caching  │    │ • JWT Auth      │    │ • Background    │    │ • Backup/DR     │           │   ║
║  │  │                 │    │ • Rate Limiting │    │   Jobs          │    │                 │           │   ║
║  │  └─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘           │   ║
║  │           │                       │                       │                       │                  │   ║
║  │           ▼                       ▼                       ▼                       ▼                  │   ║
║  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐           │   ║
║  │  │  Observability  │    │   Security      │    │ Compliance      │    │ Disaster Recov. │           │   ║
║  │  │                 │    │                 │    │                 │    │                 │           │   ║
║  │  │ • Prometheus    │    │ • WAF           │    │ • Audit Logs    │    │ • Multi-region  │           │   ║
║  │  │ • Grafana       │    │ • Secrets Mgmt  │    │ • Data Privacy  │    │ • Snapshots     │           │   ║
║  │  │ • Jaeger        │    │ • Network Pol   │    │ • Retention     │    │ • Failover      │           │   ║
║  │  │ • ELK Stack     │    │ • Vulnerability │    │ • Encryption    │    │ • RTO: <1hr     │           │   ║
║  │  │ • PagerDuty     │    │   Scanning      │    │                 │    │ • RPO: <15min   │           │   ║
║  │  └─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘           │   ║
║  │                                                                                                       │   ║
║  │  Resources: 32+ cores/node, 128+ GB RAM/node, 2+ TB NVMe/node                                       │   ║
║  │  Performance: QPS: 1000-10000, Latency: <50ms, Projects: 100-1000, Users: 100-10000                │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                  Container Orchestration                                              │   ║
║  │                                                                                                       │   ║
║  │  Docker Compose (Dev/Staging)              │  Kubernetes (Production)                                │   ║
║  │  ──────────────────────────────            │  ─────────────────────────                              │   ║
║  │                                            │                                                         │   ║
║  │  services:                                │  apiVersion: apps/v1                                   │   ║
║  │    codegraph-api:                         │  kind: Deployment                                      │   ║
║  │      image: codegraph/api:latest          │  metadata:                                             │   ║
║  │      ports: ["8000:8000"]                │    name: codegraph-api                                 │   ║
║  │      environment:                         │  spec:                                                 │   ║
║  │        - RUST_LOG=info                    │    replicas: 3                                         │   ║
║  │        - CODEGRAPH_DB_PATH=/data          │    selector:                                           │   ║
║  │      volumes:                             │      matchLabels:                                      │   ║
║  │        - ./data:/data                     │        app: codegraph-api                              │   ║
║  │      healthcheck:                         │    template:                                           │   ║
║  │        test: curl -f http://localhost:8000/health │  metadata:                                     │   ║
║  │        interval: 30s                      │          labels:                                       │   ║
║  │        timeout: 10s                       │            app: codegraph-api                          │   ║
║  │        retries: 3                         │        spec:                                           │   ║
║  │                                            │          containers:                                   │   ║
║  │    prometheus:                            │          - name: codegraph-api                         │   ║
║  │      image: prom/prometheus:latest        │            image: codegraph/api:latest                 │   ║
║  │      ports: ["9090:9090"]                │            ports:                                      │   ║
║  │                                            │            - containerPort: 8000                       │   ║
║  │    grafana:                               │            resources:                                  │   ║
║  │      image: grafana/grafana:latest        │              requests:                                 │   ║
║  │      ports: ["3000:3000"]                │                memory: "512Mi"                         │   ║
║  │      environment:                         │                cpu: "250m"                            │   ║
║  │        - GF_SECURITY_ADMIN_PASSWORD=admin │              limits:                                   │   ║
║  │                                            │                memory: "2Gi"                           │   ║
║  │                                            │                cpu: "1000m"                           │   ║
║  │                                            │            livenessProbe:                             │   ║
║  │                                            │              httpGet:                                 │   ║
║  │                                            │                path: /health                          │   ║
║  │                                            │                port: 8000                             │   ║
║  │                                            │              initialDelaySeconds: 30                  │   ║
║  │                                            │              periodSeconds: 10                        │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  Scaling Triggers & Auto-scaling Policies                                                                   ║
║  ═════════════════════════════════════════                                                                   ║
║                                                                                                              ║
║  • CPU Utilization > 70% for 5 minutes      → Scale up by 1 instance                                       ║
║  • Memory Usage > 80% for 5 minutes         → Scale up by 1 instance                                       ║
║  • Queue Depth > 1000 pending tasks         → Scale up processing workers                                   ║
║  • Response Latency > 100ms p95 for 2 min   → Scale up and investigate bottlenecks                        ║
║  • Error Rate > 1% for 3 minutes            → Alert and potential emergency scaling                         ║
║                                                                                                              ║
║  Maximum Instances: 10 (API), 20 (Workers)  │  Minimum Instances: 2 (API), 1 (Workers)                    ║
║  Scale Up Cooldown: 300s                    │  Scale Down Cooldown: 600s                                   ║
║  Target CPU: 50%                            │  Target Memory: 60%                                          ║
╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Performance Monitoring Dashboard Layout

```ascii
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 CodeGraph Performance Dashboard                                             │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                             │
│  ┌─────────────────────────────────┐ ┌─────────────────────────────────┐ ┌─────────────────────────────────┐ │
│  │         Request Metrics         │ │        System Metrics           │ │       Business Metrics         │ │
│  │                                 │ │                                 │ │                                 │ │
│  │  QPS: ████████████ 1,247 req/s │ │  CPU: ██████░░░░ 60%            │ │  Projects: 156 active          │ │
│  │  Latency p50: 23ms             │ │  Memory: ████████░ 80%          │ │  Users: 1,337 online           │ │
│  │  Latency p95: 89ms             │ │  Disk I/O: ███░░░░░ 30%         │ │  Parse Rate: 47k LOC/min       │ │
│  │  Latency p99: 156ms            │ │  Network: ██░░░░░░░ 20%         │ │  Cache Hit: 96.4%              │ │
│  │  Error Rate: 0.12%             │ │  Load Avg: 2.34                 │ │  Throughput: 2.3 GB/hour       │ │
│  │                                 │ │                                 │ │                                 │ │
│  │  ┌─ Last 24 Hours ────────────┐ │ │  ┌─ Resource Usage ───────────┐ │ │  ┌─ Top Languages ───────────┐ │ │
│  │  │     ▄▄                     │ │ │  │  CPU Cores:                │ │ │  │  Rust       ████████ 45%  │ │ │
│  │  │    ████  ▄▄                │ │ │  │  ┌─┬─┬─┬─┬─┬─┬─┬─┐          │ │ │  │  Python     ██████░░ 32%  │ │ │
│  │  │   ██████████ ▄             │ │ │  │  │▉│▉│▊│▊│▅│▅│▃│▃│          │ │ │  │  TypeScript ████░░░░ 18%  │ │ │
│  │  │  ████████████▄▄▄           │ │ │  │  └─┴─┴─┴─┴─┴─┴─┴─┘          │ │ │  │  Go         ██░░░░░░  8%  │ │ │
│  │  │ ██████████████████         │ │ │  │  1  2  3  4  5  6  7  8     │ │ │  │  Java       █░░░░░░░  5%  │ │ │
│  │  └────────────────────────────┘ │ │  │                             │ │ │  │  Other      ░░░░░░░░  2%  │ │ │
│  └─────────────────────────────────┘ │ │  │  Memory Usage:              │ │ │  └─────────────────────────────┘ │ │
│                                      │ │  │  Heap: ████████░ 402/512MB │ │ │                                 │ │
│  ┌─────────────────────────────────┐ │ │  │  Cache: ██████░░ 1.2/2.0GB │ │ │  ┌─────────────────────────────┐ │ │
│  │        Database Metrics         │ │ │  │  Buffers: ███░░░ 156/512MB │ │ │  │         Alert Status        │ │ │
│  │                                 │ │ │  └─────────────────────────────┘ │ │  │                             │ │ │
│  │  RocksDB:                       │ │ └─────────────────────────────────┘ │ │  │  🟢 API Health: OK           │ │ │
│  │  • Read Ops: 2,341/s            │ │                                      │ │  │  🟢 Database: OK             │ │ │
│  │  • Write Ops: 456/s             │ │ ┌─────────────────────────────────┐ │ │  │  🟡 Memory: 80% (Warning)   │ │ │
│  │  • Cache Hit: 94.2%             │ │ │         Component Health        │ │ │  │  🟢 Disk Space: OK          │ │ │
│  │  • Compaction: 2 pending        │ │ │                                 │ │ │  │  🟢 Network: OK             │ │ │
│  │                                 │ │ │  API Gateway:    🟢 Healthy     │ │ │  │                             │ │ │
│  │  FAISS:                         │ │ │  Parser Engine:  🟢 Healthy     │ │ │  │  Active Alerts: 1           │ │ │
│  │  • Index Size: 1.2M vectors     │ │ │  Graph Store:    🟢 Healthy     │ │ │  │  • Memory usage high        │ │ │
│  │  • Query Time: 8.9ms avg        │ │ │  Vector Engine:  🟢 Healthy     │ │ │  │                             │ │ │
│  │  • Recall Rate: 96.7%           │ │ │  Cache Layer:    🟡 Degraded    │ │ │  │  Recent Issues: 0           │ │ │
│  │  • Index Builds: 3 queued       │ │ │  Task Queue:     🟢 Healthy     │ │ │  │  Recovery Time: N/A         │ │ │
│  │                                 │ │ │  MCP Server:     🟢 Healthy     │ │ │  │                             │ │ │
│  │  Cache:                         │ │ │  Load Balancer:  🟢 Healthy     │ │ │  │  SLA Compliance: 99.97%     │ │ │
│  │  • L1 Hit Rate: 89.3%           │ │ │                                 │ │ │  │  Uptime: 99.98%             │ │ │
│  │  • L2 Hit Rate: 76.8%           │ │ │  Response Times (p95):          │ │ │  │                             │ │ │
│  │  • L3 Hit Rate: 45.2%           │ │ │  • Parse Request: 45ms          │ │ │  └─────────────────────────────┘ │ │
│  │  • Evictions: 234/hour          │ │ │  • Vector Search: 12ms          │ │ │                                 │ │
│  └─────────────────────────────────┘ │ │  • Graph Query: 28ms            │ │ │ ┌─────────────────────────────┐ │ │
│                                      │ │  • API Response: 89ms           │ │ │ │      Recent Activity        │ │ │
│  ┌─────────────────────────────────┐ │ │                                 │ │ │ │                             │ │ │
│  │        Queue Metrics            │ │ └─────────────────────────────────┘ │ │ │  15:42 Project added: rust-  │ │ │
│  │                                 │ │                                      │ │ │        analyzer              │ │ │
│  │  Pending Tasks: 127             │ │ ┌─────────────────────────────────┐ │ │ │  15:41 Index rebuild: FAISS  │ │ │
│  │  Processing: 8 workers          │ │ │         Error Analysis          │ │ │ │  15:40 Cache eviction: L2    │ │ │
│  │  Completed: 12,456 today        │ │ │                                 │ │ │ │  15:38 User login: alice      │ │ │
│  │  Failed: 23 (retry queue)       │ │ │  Error Rate Trend:              │ │ │ │  15:37 Parse complete: 50k   │ │ │
│  │                                 │ │ │  ░░░▁▁▂▃▂▁░░░░░░░░░░░░░         │ │ │ │        LOC                   │ │ │
│  │  Parse Queue: 45 files          │ │ │                                 │ │ │ │  15:35 Query: "async fn"     │ │ │
│  │  Index Queue: 12 updates        │ │ │  Top Errors (Last Hour):        │ │ │ │  15:34 API call: /search     │ │ │
│  │  Vector Queue: 3 builds         │ │ │  • Timeout (408): 5 times       │ │ │ │  15:33 Cache miss: vector    │ │ │
│  │  Export Queue: 1 pending        │ │ │  • Rate Limited (429): 3 times  │ │ │ │  15:32 Background: compact   │ │ │
│  │                                 │ │ │  • Parse Error (422): 2 times   │ │ │ │  15:31 Health check: OK      │ │ │
│  │  Average Wait: 2.3 seconds      │ │ │  • Server Error (500): 1 time   │ │ │ │                             │ │ │
│  │  Worker Utilization: 67%        │ │ │                                 │ │ │ │  Auto-refresh: 30s          │ │ │
│  │                                 │ │ │  Error Rate: 0.12% (Target<1%) │ │ │ │  Last Update: 15:42:15      │ │ │
│  └─────────────────────────────────┘ │ └─────────────────────────────────┘ │ │ └─────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

*Generated by CodeGraph Documentation Specialist - ASCII Architecture Diagrams*