# CodeGraph System Architecture Overview

This document provides comprehensive architecture diagrams for the CodeGraph system, showing the relationships between its components, data flow, and technology stack.

## High-Level System Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        WebUI[Web UI]
        CLI[CLI Client]
        SDK[SDK/API Clients]
    end
    
    subgraph "API Gateway"
        API[CodeGraph API<br/>Axum + GraphQL]
        Auth[Authentication<br/>JWT]
        RateLimit[Rate Limiting<br/>Governor]
    end
    
    subgraph "Core Services"
        Parser[Code Parser<br/>Tree-sitter]
        Vector[Vector Engine<br/>FAISS]
        Graph[Graph Store<br/>RocksDB]
        Cache[Cache Layer<br/>Memory Cache]
    end
    
    subgraph "Processing Layer"
        Queue[Task Queue<br/>Async Processing]
        Git[Git Integration<br/>Repository Scanner]
        MCP[MCP Server<br/>Protocol Handler]
    end
    
    subgraph "Storage Layer"
        RocksDB[(RocksDB<br/>Graph Storage)]
        FAISS[(FAISS Index<br/>Vector Search)]
        MemCache[(Memory Cache<br/>Hot Data)]
    end
    
    subgraph "Infrastructure"
        Monitor[Monitoring<br/>Prometheus]
        Trace[Tracing<br/>OpenTelemetry]
        Config[Configuration<br/>TOML/ENV]
    end
    
    %% Client connections
    WebUI --> API
    CLI --> API
    SDK --> API
    
    %% API Gateway flow
    API --> Auth
    API --> RateLimit
    API --> Parser
    API --> Vector
    API --> Graph
    API --> Cache
    
    %% Service integrations
    Parser --> Queue
    Vector --> FAISS
    Graph --> RocksDB
    Cache --> MemCache
    
    %% Processing layer
    Queue --> Git
    Queue --> MCP
    Git --> Parser
    MCP --> API
    
    %% Infrastructure connections
    API --> Monitor
    API --> Trace
    Config --> API
    Config --> Parser
    Config --> Vector
    Config --> Graph
    
    %% Styling
    classDef client fill:#e1f5fe
    classDef api fill:#f3e5f5
    classDef core fill:#e8f5e8
    classDef storage fill:#fff3e0
    classDef infra fill:#fce4ec
    
    class WebUI,CLI,SDK client
    class API,Auth,RateLimit api
    class Parser,Vector,Graph,Cache,Queue,Git,MCP core
    class RocksDB,FAISS,MemCache storage
    class Monitor,Trace,Config infra
```

## Component Dependency Graph

```mermaid
graph LR
    subgraph "Foundation Layer"
        Core[codegraph-core<br/>Types & Traits]
        ZeroCopy[codegraph-zerocopy<br/>Serialization]
    end
    
    subgraph "Data Layer"
        Graph[codegraph-graph<br/>RocksDB Storage]
        Vector[codegraph-vector<br/>FAISS Integration]
        Cache[codegraph-cache<br/>Memory Management]
    end
    
    subgraph "Processing Layer"
        Parser[codegraph-parser<br/>Tree-sitter AST]
        Queue[codegraph-queue<br/>Task Processing]
        Git[codegraph-git<br/>Repository Handler]
        Concurrent[codegraph-concurrent<br/>Parallel Processing]
    end
    
    subgraph "Service Layer"
        API[codegraph-api<br/>REST + GraphQL]
        MCP[codegraph-mcp<br/>MCP Protocol]
        LB[codegraph-lb<br/>Load Balancer]
    end
    
    %% Foundation dependencies
    Graph --> Core
    Vector --> Core
    Cache --> Core
    Parser --> Core
    Queue --> Core
    Git --> Core
    Concurrent --> Core
    API --> Core
    MCP --> Core
    LB --> Core
    
    %% ZeroCopy integration
    Graph --> ZeroCopy
    Vector --> ZeroCopy
    Cache --> ZeroCopy
    
    %% Data layer dependencies
    API --> Graph
    API --> Vector
    API --> Cache
    MCP --> Graph
    MCP --> Vector
    
    %% Processing dependencies
    API --> Parser
    API --> Queue
    Queue --> Git
    Queue --> Parser
    Parser --> Concurrent
    
    %% Service layer dependencies
    LB --> API
    MCP --> Parser
    
    %% Cross-layer dependencies
    Vector --> Graph
    Cache --> Graph
    Queue --> Concurrent
    
    %% Styling
    classDef foundation fill:#e3f2fd
    classDef data fill:#e8f5e8
    classDef processing fill:#fff3e0
    classDef service fill:#f3e5f5
    
    class Core,ZeroCopy foundation
    class Graph,Vector,Cache data
    class Parser,Queue,Git,Concurrent processing
    class API,MCP,LB service
```

## Technology Stack Visualization

```mermaid
graph TB
    subgraph "Runtime Environment"
        Tokio[Tokio Async Runtime<br/>v1.39+]
        Rust[Rust 2021 Edition<br/>MSRV 1.70+]
    end
    
    subgraph "Web Framework Stack"
        Axum[Axum Web Framework<br/>v0.7]
        Tower[Tower Middleware<br/>HTTP Services]
        Hyper[Hyper HTTP<br/>v1.0]
        GraphQL[async-graphql<br/>v7.0]
    end
    
    subgraph "Database & Storage"
        RocksDB[RocksDB<br/>v0.22 - Persistent Storage]
        FAISS[FAISS<br/>v0.12 - Vector Search]
        MemMap[Memory Mapping<br/>memmap2]
    end
    
    subgraph "Parsing & Language Support"
        TreeSitter[Tree-sitter<br/>v0.25]
        Rust_Parser[tree-sitter-rust<br/>v0.24]
        Python_Parser[tree-sitter-python<br/>v0.23]
        JS_Parser[tree-sitter-javascript<br/>v0.25]
        TS_Parser[tree-sitter-typescript<br/>v0.23]
        Go_Parser[tree-sitter-go<br/>v0.23]
        Java_Parser[tree-sitter-java<br/>v0.23]
    end
    
    subgraph "Serialization & Performance"
        Serde[Serde<br/>JSON Serialization]
        RKYV[rkyv<br/>Zero-copy Archives]
        Compression[Compression<br/>zstd, lz4, flate2]
        Parallel[Parallel Processing<br/>rayon, crossbeam]
    end
    
    subgraph "Observability"
        Tracing[Tracing<br/>Structured Logging]
        Prometheus[Prometheus<br/>Metrics Collection]
        OpenTel[OpenTelemetry<br/>Distributed Tracing]
    end
    
    subgraph "Security & Auth"
        JWT[JWT Tokens<br/>jsonwebtoken]
        Argon2[Argon2<br/>Password Hashing]
        TLS[TLS Support<br/>native-tls]
    end
    
    %% Technology relationships
    Rust --> Tokio
    Tokio --> Axum
    Axum --> Tower
    Tower --> Hyper
    Axum --> GraphQL
    
    TreeSitter --> Rust_Parser
    TreeSitter --> Python_Parser
    TreeSitter --> JS_Parser
    TreeSitter --> TS_Parser
    TreeSitter --> Go_Parser
    TreeSitter --> Java_Parser
    
    %% Styling
    classDef runtime fill:#e3f2fd
    classDef web fill:#e8f5e8
    classDef storage fill:#fff3e0
    classDef parsing fill:#f3e5f5
    classDef perf fill:#fce4ec
    classDef observ fill:#e0f2f1
    classDef security fill:#fff8e1
    
    class Tokio,Rust runtime
    class Axum,Tower,Hyper,GraphQL web
    class RocksDB,FAISS,MemMap storage
    class TreeSitter,Rust_Parser,Python_Parser,JS_Parser,TS_Parser,Go_Parser,Java_Parser parsing
    class Serde,RKYV,Compression,Parallel perf
    class Tracing,Prometheus,OpenTel observ
    class JWT,Argon2,TLS security
```

## Data Flow Architecture

```mermaid
sequenceDiagram
    participant Client
    participant API as API Gateway
    participant Auth as Authentication
    participant Parser as Code Parser
    participant Graph as Graph Store
    participant Vector as Vector Engine
    participant Cache as Cache Layer
    participant Queue as Task Queue
    
    Client->>API: Request with JWT
    API->>Auth: Validate token
    Auth-->>API: Token valid
    
    API->>Cache: Check cached result
    alt Cache hit
        Cache-->>API: Return cached data
        API-->>Client: Response
    else Cache miss
        API->>Queue: Queue analysis task
        Queue->>Parser: Parse code files
        Parser->>Graph: Store AST nodes
        Parser->>Vector: Generate embeddings
        Vector->>Graph: Link vector indices
        Graph-->>API: Analysis complete
        API->>Cache: Store result
        API-->>Client: Response
    end
    
    Note over Parser,Vector: Parallel processing for large codebases
    Note over Cache: TTL-based invalidation
    Note over Queue: Async task processing with backpressure
```

## Performance Optimization Layers

```mermaid
graph TD
    subgraph "Request Level Optimizations"
        Compress[Response Compression<br/>Brotli, Gzip, Deflate]
        Stream[Streaming Responses<br/>Large Dataset Handling]
        Batch[Request Batching<br/>Multiple Operations]
    end
    
    subgraph "Application Level Optimizations"
        MemPool[Memory Pooling<br/>Object Reuse]
        ZeroCopy[Zero-Copy Serialization<br/>rkyv Archives]
        Parallel[Parallel Processing<br/>rayon, crossbeam]
        Cache[Multi-Level Caching<br/>Hot Data Management]
    end
    
    subgraph "Database Optimizations"
        RocksOpt[RocksDB Tuning<br/>Column Families, Bloom Filters]
        FAISSIdx[FAISS Index Optimization<br/>IVF, PQ Compression]
        MemMap[Memory Mapping<br/>Large File Handling]
        Bulk[Bulk Operations<br/>Batch Writes]
    end
    
    subgraph "System Level Optimizations"
        Async[Async I/O<br/>Tokio Runtime]
        LockFree[Lock-Free Data Structures<br/>DashMap, Arc-Swap]
        NUMA[NUMA Awareness<br/>Thread Affinity]
        Profiling[Continuous Profiling<br/>Memory Leak Detection]
    end
    
    %% Performance flow
    Compress --> Stream
    Stream --> Batch
    Batch --> MemPool
    
    MemPool --> ZeroCopy
    ZeroCopy --> Parallel
    Parallel --> Cache
    
    Cache --> RocksOpt
    RocksOpt --> FAISSIdx
    FAISSIdx --> MemMap
    MemMap --> Bulk
    
    Bulk --> Async
    Async --> LockFree
    LockFree --> NUMA
    NUMA --> Profiling
    
    %% Styling
    classDef request fill:#e3f2fd
    classDef app fill:#e8f5e8
    classDef db fill:#fff3e0
    classDef system fill:#f3e5f5
    
    class Compress,Stream,Batch request
    class MemPool,ZeroCopy,Parallel,Cache app
    class RocksOpt,FAISSIdx,MemMap,Bulk db
    class Async,LockFree,NUMA,Profiling system
```

## Deployment Architecture

```mermaid
graph TB
    subgraph "Load Balancer Tier"
        LB[Load Balancer<br/>codegraph-lb]
        Health[Health Checks<br/>HTTP/TCP]
    end
    
    subgraph "Application Tier"
        API1[API Instance 1<br/>codegraph-api]
        API2[API Instance 2<br/>codegraph-api]
        API3[API Instance N<br/>codegraph-api]
    end
    
    subgraph "Processing Tier"
        Worker1[Worker Node 1<br/>Parser + Vector]
        Worker2[Worker Node 2<br/>Parser + Vector]
        Queue[Message Queue<br/>Redis/RabbitMQ]
    end
    
    subgraph "Storage Tier"
        RocksCluster[RocksDB Cluster<br/>Sharded Storage]
        FAISSCluster[FAISS Cluster<br/>Distributed Index]
        CacheCluster[Cache Cluster<br/>Redis/Memcached]
    end
    
    subgraph "Monitoring Tier"
        Prometheus[Prometheus<br/>Metrics Collection]
        Grafana[Grafana<br/>Visualization]
        Jaeger[Jaeger<br/>Distributed Tracing]
        Logs[Centralized Logging<br/>ELK Stack]
    end
    
    %% Load balancing
    LB --> API1
    LB --> API2
    LB --> API3
    Health --> API1
    Health --> API2
    Health --> API3
    
    %% API to processing
    API1 --> Queue
    API2 --> Queue
    API3 --> Queue
    Queue --> Worker1
    Queue --> Worker2
    
    %% Storage connections
    API1 --> RocksCluster
    API2 --> RocksCluster
    API3 --> RocksCluster
    Worker1 --> FAISSCluster
    Worker2 --> FAISSCluster
    API1 --> CacheCluster
    API2 --> CacheCluster
    API3 --> CacheCluster
    
    %% Monitoring connections
    API1 --> Prometheus
    API2 --> Prometheus
    API3 --> Prometheus
    Worker1 --> Prometheus
    Worker2 --> Prometheus
    Prometheus --> Grafana
    API1 --> Jaeger
    API2 --> Jaeger
    API3 --> Jaeger
    API1 --> Logs
    API2 --> Logs
    API3 --> Logs
    
    %% Styling
    classDef lb fill:#e3f2fd
    classDef app fill:#e8f5e8
    classDef processing fill:#fff3e0
    classDef storage fill:#f3e5f5
    classDef monitoring fill:#fce4ec
    
    class LB,Health lb
    class API1,API2,API3 app
    class Worker1,Worker2,Queue processing
    class RocksCluster,FAISSCluster,CacheCluster storage
    class Prometheus,Grafana,Jaeger,Logs monitoring
```

## Crate Interaction Matrix

```ascii
╔═══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                    CodeGraph Crate Dependency Matrix                                         ║
╠═══════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║ Crate              │ core │ zero │ graph│ vect │ cache│ parse│ queue│ git  │ conc │ api  │ mcp  │ lb   │ Description ║
╠════════════════════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼══════┼═══════════════╣
║ codegraph-core     │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │ Foundation    ║
║ codegraph-zerocopy │  ●   │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │ Serialization ║
║ codegraph-graph    │  ●   │  ●   │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │ RocksDB Store ║
║ codegraph-vector   │  ●   │  ●   │  ○   │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │ FAISS Search  ║
║ codegraph-cache    │  ●   │  ●   │  ○   │  -   │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │ Memory Cache  ║
║ codegraph-parser   │  ●   │  -   │  -   │  -   │  -   │  ●   │  -   │  -   │  ○   │  -   │  -   │  -   │ Tree-sitter   ║
║ codegraph-queue    │  ●   │  -   │  -   │  -   │  -   │  -   │  ●   │  -   │  ○   │  -   │  -   │  -   │ Task Queue    ║
║ codegraph-git      │  ●   │  -   │  -   │  -   │  -   │  ○   │  -   │  ●   │  -   │  -   │  -   │  -   │ Git Handler   ║
║ codegraph-concurrent│ ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  ●   │  -   │  -   │  -   │ Parallelism   ║
║ codegraph-api      │  ●   │  -   │  ●   │  ●   │  ●   │  ●   │  ●   │  -   │  -   │  ●   │  -   │  -   │ REST+GraphQL  ║
║ codegraph-mcp      │  ●   │  -   │  ●   │  ●   │  -   │  ●   │  -   │  -   │  -   │  -   │  ●   │  -   │ MCP Protocol  ║
║ codegraph-lb       │  ●   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  -   │  ●   │  -   │  ●   │ Load Balancer ║
╚════════════════════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴══════┴═══════════════╝

Legend:
● Direct dependency (required)
○ Optional dependency (feature-gated or weak)
- No dependency

Critical Paths:
1. API → Core → Graph → RocksDB (Data persistence)
2. API → Vector → FAISS (Similarity search)
3. Parser → Core → Concurrent (Code analysis)
4. MCP → API → All services (Protocol integration)
```

## Memory Layout and Optimization

```ascii
╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                     CodeGraph Memory Architecture                                           ║
╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                    Process Memory Space                                              │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                              ║
║  ┌───────────────┐  ┌─────────────────┐  ┌──────────────────┐  ┌─────────────────────────────────────┐     ║
║  │  Stack        │  │  Heap           │  │  Memory Mapped   │  │  Zero-Copy Archives                 │     ║
║  │               │  │                 │  │  Files           │  │                                     │     ║
║  │  - Tokio      │  │  - AST Nodes    │  │                  │  │  - rkyv Serialized                 │     ║
║  │    Tasks      │  │  - Hash Maps    │  │  - Large RocksDB │  │    Data Structures                 │     ║
║  │  - Function   │  │  - Vector Data  │  │    Files         │  │  - Immutable Archives              │     ║
║  │    Calls      │  │  - Cache Entries│  │  - FAISS Indices │  │  - Direct Memory Access            │     ║
║  │  - Local Vars │  │  - Buffers      │  │  - Log Files     │  │  - No Serialization Cost           │     ║
║  └───────────────┘  └─────────────────┘  └──────────────────┘  └─────────────────────────────────────┘     ║
║        ↕                    ↕                      ↕                             ↕                         ║
║    8-16 MB             Dynamic Growth           File-backed                  Controlled Layout             ║
║                        (GC via Rust)           Virtual Memory                  (rkyv format)              ║
║                                                                                                              ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                              Optimization Strategies                                                 │   ║
║  ├─────────────────────────────────────────────────────────────────────────────────────────────────────┤   ║
║  │  1. Arena Allocation    │ Use bumpalo for temporary allocations during parsing                      │   ║
║  │  2. Object Pooling      │ Reuse expensive objects (parsers, buffers)                               │   ║
║  │  3. Lock-Free Structs   │ DashMap, ArcSwap for concurrent access without locks                     │   ║
║  │  4. Memory Mapping      │ Direct file access for large datasets                                    │   ║
║  │  5. Zero-Copy Serde     │ rkyv for serialization without intermediate allocations                  │   ║
║  │  6. Compression         │ LZ4/Zstd for reducing memory footprint                                   │   ║
║  │  7. Lazy Loading        │ Load AST nodes on-demand from storage                                    │   ║
║  │  8. Memory Profiling    │ memscope-rs for leak detection in development                            │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

*Generated by CodeGraph Documentation Specialist - Architecture Visualization*