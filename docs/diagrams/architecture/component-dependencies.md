# CodeGraph Component Dependencies

## Detailed Crate Dependency Analysis

### Foundation Layer Dependencies

```mermaid
graph TB
    subgraph "Core Foundation"
        core["`**codegraph-core**
        Types & Traits
        Error Handling
        Configuration
        Memory Management`"]
        
        zerocopy["`**codegraph-zerocopy**
        rkyv Archives
        Zero-copy Serialization
        Memory Layout Control
        Performance Optimization`"]
    end
    
    subgraph "Platform Dependencies"
        unix["`**Unix Platform**
        libc 0.2
        Memory Management
        System Calls`"]
        
        windows["`**Windows Platform**
        windows-sys 0.59
        Win32 APIs
        Memory Management`"]
    end
    
    core --> unix
    core --> windows
    zerocopy --> core
    
    classDef foundation fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef platform fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    
    class core,zerocopy foundation
    class unix,windows platform
```

### Storage Layer Dependencies

```mermaid
graph TB
    subgraph "Storage Implementation"
        graph_db["`**codegraph-graph**
        RocksDB Integration
        Column Families
        Batch Operations
        Graph Persistence`"]
        
        vector["`**codegraph-vector**
        FAISS Integration
        Vector Embeddings
        Similarity Search
        OpenAI Client`"]
        
        cache["`**codegraph-cache**
        Memory Cache
        Dashboard
        Profiler
        Optimization`"]
    end
    
    subgraph "Core Dependencies"
        core[codegraph-core]
        zerocopy[codegraph-zerocopy]
    end
    
    subgraph "External Libraries"
        rocksdb[RocksDB 0.22]
        faiss[FAISS 0.12]
        openai[OpenAI Client]
        memmap[memmap2]
        dashmap[DashMap 6.0]
    end
    
    %% Core dependencies
    graph_db --> core
    vector --> core
    cache --> core
    
    %% Zero-copy dependencies
    graph_db --> zerocopy
    vector --> zerocopy
    cache --> zerocopy
    
    %% External dependencies
    graph_db --> rocksdb
    vector --> faiss
    vector --> openai
    cache --> memmap
    cache --> dashmap
    
    %% Cross-storage dependencies
    vector -.-> graph_db
    cache -.-> graph_db
    
    classDef storage fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef core_dep fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef external fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    
    class graph_db,vector,cache storage
    class core,zerocopy core_dep
    class rocksdb,faiss,openai,memmap,dashmap external
```

### Processing Layer Dependencies

```mermaid
graph TB
    subgraph "Code Processing"
        parser["`**codegraph-parser**
        Tree-sitter AST
        Language Support
        Code Analysis
        Multi-language`"]
        
        git["`**codegraph-git**
        Repository Handling
        Git Integration
        Change Detection
        Branch Management`"]
    end
    
    subgraph "Concurrent Processing"
        concurrent["`**codegraph-concurrent**
        Parallel Processing
        Thread Management
        Work Distribution
        Performance`"]
        
        queue["`**codegraph-queue**
        Task Queue
        Async Processing
        Job Scheduling
        Backpressure`"]
    end
    
    subgraph "Language Parsers"
        rust_parser[tree-sitter-rust 0.24]
        python_parser[tree-sitter-python 0.23]
        js_parser[tree-sitter-javascript 0.25]
        ts_parser[tree-sitter-typescript 0.23]
        go_parser[tree-sitter-go 0.23]
        java_parser[tree-sitter-java 0.23]
        cpp_parser[tree-sitter-cpp 0.23]
    end
    
    subgraph "Core Dependencies"
        core[codegraph-core]
    end
    
    %% Core dependencies
    parser --> core
    git --> core
    concurrent --> core
    queue --> core
    
    %% Language parser dependencies
    parser --> rust_parser
    parser --> python_parser
    parser --> js_parser
    parser --> ts_parser
    parser --> go_parser
    parser --> java_parser
    parser --> cpp_parser
    
    %% Processing dependencies
    parser -.-> concurrent
    queue --> concurrent
    git -.-> parser
    
    classDef processing fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef concurrent_proc fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef language fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef core_dep fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    
    class parser,git processing
    class concurrent,queue concurrent_proc
    class rust_parser,python_parser,js_parser,ts_parser,go_parser,java_parser,cpp_parser language
    class core core_dep
```

### API Layer Dependencies

```mermaid
graph TB
    subgraph "API Services"
        api["`**codegraph-api**
        Axum Framework
        REST + GraphQL
        WebSocket Support
        Authentication`"]
        
        mcp["`**codegraph-mcp**
        MCP Protocol
        Server Implementation
        Client Integration
        Transport Layer`"]
        
        lb["`**codegraph-lb**
        Load Balancer
        Health Checks
        Traffic Distribution
        Failover`"]
    end
    
    subgraph "RAG Server"
        rag_server["`**core-rag-mcp-server**
        RAG Implementation
        MCP Integration
        Document Processing
        Query Engine`"]
    end
    
    subgraph "All Lower Layers"
        core[codegraph-core]
        graph_db[codegraph-graph]
        vector[codegraph-vector]
        cache[codegraph-cache]
        parser[codegraph-parser]
        queue[codegraph-queue]
        git[codegraph-git]
    end
    
    subgraph "Web Framework Stack"
        axum[Axum 0.7]
        tower[Tower 0.4]
        hyper[Hyper 1.0]
        graphql[async-graphql 7.0]
        websocket[tokio-tungstenite]
    end
    
    %% Core dependencies
    api --> core
    mcp --> core
    lb --> core
    rag_server --> core
    
    %% Service layer dependencies
    api --> graph_db
    api --> vector
    api --> cache
    api --> parser
    api --> queue
    
    mcp --> graph_db
    mcp --> vector
    mcp --> parser
    
    lb --> api
    
    rag_server --> mcp
    rag_server --> parser
    rag_server --> vector
    
    %% Framework dependencies
    api --> axum
    api --> tower
    api --> hyper
    api --> graphql
    api --> websocket
    
    mcp --> axum
    
    classDef api_service fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef rag fill:#fce4ec,stroke:#c2185b,stroke-width:2px
    classDef lower fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef framework fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    
    class api,mcp,lb api_service
    class rag_server rag
    class core,graph_db,vector,cache,parser,queue,git lower
    class axum,tower,hyper,graphql,websocket framework
```

## Dependency Depth Analysis

```mermaid
flowchart TD
    subgraph "Depth 0 - Foundation"
        D0["`**Level 0**
        codegraph-core
        (No dependencies)`"]
    end
    
    subgraph "Depth 1 - Core Extensions"
        D1A["`**Level 1A**
        codegraph-zerocopy`"]
        D1B["`**Level 1B**
        codegraph-concurrent`"]
        D1C["`**Level 1C**
        codegraph-git`"]
    end
    
    subgraph "Depth 2 - Storage & Processing"
        D2A["`**Level 2A**
        codegraph-graph
        codegraph-vector
        codegraph-cache`"]
        D2B["`**Level 2B**
        codegraph-parser
        codegraph-queue`"]
    end
    
    subgraph "Depth 3 - Services"
        D3A["`**Level 3A**
        codegraph-mcp`"]
        D3B["`**Level 3B**
        codegraph-api`"]
        D3C["`**Level 3C**
        codegraph-lb`"]
    end
    
    subgraph "Depth 4 - Applications"
        D4["`**Level 4**
        core-rag-mcp-server`"]
    end
    
    %% Dependencies flow
    D0 --> D1A
    D0 --> D1B
    D0 --> D1C
    
    D1A --> D2A
    D1B --> D2B
    D0 --> D2A
    D0 --> D2B
    
    D2A --> D3A
    D2A --> D3B
    D2B --> D3A
    D2B --> D3B
    D3B --> D3C
    D0 --> D3A
    D0 --> D3B
    D0 --> D3C
    
    D3A --> D4
    D2B --> D4
    D2A --> D4
    D0 --> D4
    
    classDef depth0 fill:#e3f2fd,stroke:#1976d2,stroke-width:3px
    classDef depth1 fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef depth2 fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef depth3 fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef depth4 fill:#fce4ec,stroke:#c2185b,stroke-width:2px
    
    class D0 depth0
    class D1A,D1B,D1C depth1
    class D2A,D2B depth2
    class D3A,D3B,D3C depth3
    class D4 depth4
```

## Feature Dependency Matrix

```mermaid
flowchart LR
    subgraph "Optional Features"
        F1["`**FAISS Support**
        Feature: faiss
        Vector search capabilities`"]
        
        F2["`**GPU Acceleration**
        Feature: gpu
        CUDA/OpenCL support`"]
        
        F3["`**Persistent Storage**
        Feature: persistent
        Memory mapping`"]
        
        F4["`**OpenAI Integration**
        Feature: openai
        External embeddings`"]
        
        F5["`**Memory Leak Detection**
        Feature: leak-detect
        Development profiling`"]
    end
    
    subgraph "Crates"
        vector[codegraph-vector]
        api[codegraph-api]
        cache[codegraph-cache]
    end
    
    %% Feature dependencies
    vector --> F1
    F1 --> F2
    vector --> F3
    vector --> F4
    api --> F5
    
    classDef feature fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef crate fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    
    class F1,F2,F3,F4,F5 feature
    class vector,api,cache crate
```

## Cross-Cutting Concerns

```mermaid
graph TB
    subgraph "Observability"
        O1["`**Tracing**
        All crates use tracing
        Structured logging
        Performance monitoring`"]
        
        O2["`**Metrics**
        Prometheus integration
        API performance
        Resource usage`"]
        
        O3["`**Error Handling**
        thiserror + anyhow
        Consistent errors
        Context propagation`"]
    end
    
    subgraph "Async Runtime"
        A1["`**Tokio Runtime**
        Async/await support
        Task scheduling
        I/O operations`"]
        
        A2["`**Futures**
        Stream processing
        Combinators
        Async traits`"]
    end
    
    subgraph "Serialization"
        S1["`**Serde**
        JSON serialization
        API communication
        Configuration`"]
        
        S2["`**rkyv**
        Zero-copy archives
        High performance
        Memory efficiency`"]
    end
    
    subgraph "Concurrency"
        C1["`**Parking Lot**
        Efficient mutexes
        Reader-writer locks
        Lower overhead`"]
        
        C2["`**Crossbeam**
        Channels
        Atomic operations
        Lock-free structures`"]
        
        C3["`**Rayon**
        Data parallelism
        Work stealing
        Parallel iterators`"]
    end
    
    %% All crates depend on these concerns
    O1 -.-> A1
    O1 -.-> S1
    A1 -.-> C1
    S2 -.-> C1
    C3 -.-> C2
    
    classDef observability fill:#e3f2fd,stroke:#1976d2,stroke-width:2px
    classDef async fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef serialization fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef concurrency fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    
    class O1,O2,O3 observability
    class A1,A2 async
    class S1,S2 serialization
    class C1,C2,C3 concurrency
```

---

*Generated by CodeGraph Documentation Specialist - Component Dependencies Analysis*