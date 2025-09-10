# CodeGraph Component Dependencies

This document provides detailed dependency analysis and component interaction diagrams for the CodeGraph system.

## Crate Dependency Graph (Detailed)

```mermaid
graph TB
    subgraph "Foundation Layer (Tier 0)"
        Core[codegraph-core<br/>📦 Types, Traits, Config<br/>🔗 No dependencies]
    end
    
    subgraph "Serialization Layer (Tier 1)"
        ZeroCopy[codegraph-zerocopy<br/>📦 rkyv, Zero-copy<br/>🔗 → core]
    end
    
    subgraph "Storage Layer (Tier 2)"
        Graph[codegraph-graph<br/>📦 RocksDB, Storage<br/>🔗 → core, zerocopy]
        Vector[codegraph-vector<br/>📦 FAISS, Embeddings<br/>🔗 → core, zerocopy]
        Cache[codegraph-cache<br/>📦 Memory Cache<br/>🔗 → core, zerocopy]
    end
    
    subgraph "Processing Layer (Tier 3)"
        Parser[codegraph-parser<br/>📦 Tree-sitter<br/>🔗 → core]
        Queue[codegraph-queue<br/>📦 Task Processing<br/>🔗 → core]
        Git[codegraph-git<br/>📦 Git Integration<br/>🔗 → core]
        Concurrent[codegraph-concurrent<br/>📦 Parallelism<br/>🔗 → core]
    end
    
    subgraph "Integration Layer (Tier 4)"
        MCP[codegraph-mcp<br/>📦 MCP Protocol<br/>🔗 → core, graph, vector, parser]
        MCP_Server[core-rag-mcp-server<br/>📦 RAG MCP Server<br/>🔗 → mcp, api]
    end
    
    subgraph "Service Layer (Tier 5)"
        API[codegraph-api<br/>📦 REST + GraphQL<br/>🔗 → ALL layers]
        LB[codegraph-lb<br/>📦 Load Balancer<br/>🔗 → core, api]
    end
    
    %% Tier 0 → Tier 1
    Core --> ZeroCopy
    
    %% Tier 1 → Tier 2
    ZeroCopy --> Graph
    ZeroCopy --> Vector
    ZeroCopy --> Cache
    Core --> Graph
    Core --> Vector
    Core --> Cache
    
    %% Tier 0 → Tier 3 (Direct)
    Core --> Parser
    Core --> Queue
    Core --> Git
    Core --> Concurrent
    
    %% Tier 2 → Tier 3 (Optional)
    Graph -.-> Vector
    Graph -.-> Cache
    
    %% Tier 3 → Parser Enhancement
    Concurrent -.-> Parser
    Queue -.-> Git
    
    %% Tier 3 → Tier 4
    Core --> MCP
    Graph --> MCP
    Vector --> MCP
    Parser --> MCP
    MCP --> MCP_Server
    
    %% Tier 4 → Tier 5
    Core --> API
    Graph --> API
    Vector --> API
    Cache --> API
    Parser --> API
    Queue --> API
    MCP --> API
    MCP_Server -.-> API
    
    Core --> LB
    API --> LB
    
    %% Styling by tier
    classDef tier0 fill:#e8eaf6,stroke:#3f51b5,stroke-width:3px
    classDef tier1 fill:#e1f5fe,stroke:#0277bd,stroke-width:2px
    classDef tier2 fill:#e8f5e8,stroke:#388e3c,stroke-width:2px
    classDef tier3 fill:#fff3e0,stroke:#f57c00,stroke-width:2px
    classDef tier4 fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    classDef tier5 fill:#ffebee,stroke:#c62828,stroke-width:3px
    
    class Core tier0
    class ZeroCopy tier1
    class Graph,Vector,Cache tier2
    class Parser,Queue,Git,Concurrent tier3
    class MCP,MCP_Server tier4
    class API,LB tier5
```

## API Layer Dependencies

```mermaid
graph LR
    subgraph "API Service (codegraph-api)"
        REST[REST Endpoints<br/>Axum Routes]
        GraphQL[GraphQL Schema<br/>async-graphql]
        WS[WebSocket<br/>Real-time Events]
        Stream[Streaming<br/>Large Datasets]
    end
    
    subgraph "Core Dependencies"
        CoreTypes[Core Types<br/>Error Handling]
        Config[Configuration<br/>Settings]
        Auth[Authentication<br/>JWT, Security]
    end
    
    subgraph "Storage Dependencies"
        GraphStore[Graph Storage<br/>RocksDB Access]
        VectorStore[Vector Store<br/>FAISS Queries]
        CacheStore[Cache Store<br/>Memory Cache]
    end
    
    subgraph "Processing Dependencies"
        ParseService[Parser Service<br/>Code Analysis]
        QueueService[Queue Service<br/>Async Tasks]
        GitService[Git Service<br/>Repository Ops]
    end
    
    subgraph "External Systems"
        Prometheus[Prometheus<br/>Metrics]
        Tracing[Tracing<br/>Observability]
        HealthCheck[Health Checks<br/>Monitoring]
    end
    
    %% API internal connections
    REST --> GraphQL
    GraphQL --> WS
    WS --> Stream
    
    %% Core dependencies
    REST --> CoreTypes
    GraphQL --> CoreTypes
    REST --> Config
    GraphQL --> Config
    REST --> Auth
    GraphQL --> Auth
    
    %% Storage access
    REST --> GraphStore
    GraphQL --> GraphStore
    REST --> VectorStore
    GraphQL --> VectorStore
    REST --> CacheStore
    GraphQL --> CacheStore
    
    %% Processing integration
    REST --> ParseService
    GraphQL --> ParseService
    REST --> QueueService
    GraphQL --> QueueService
    WS --> QueueService
    REST --> GitService
    
    %% External systems
    REST --> Prometheus
    GraphQL --> Prometheus
    REST --> Tracing
    GraphQL --> Tracing
    REST --> HealthCheck
    
    %% Styling
    classDef api fill:#e3f2fd
    classDef core fill:#e8f5e8
    classDef storage fill:#fff3e0
    classDef processing fill:#f3e5f5
    classDef external fill:#fce4ec
    
    class REST,GraphQL,WS,Stream api
    class CoreTypes,Config,Auth core
    class GraphStore,VectorStore,CacheStore storage
    class ParseService,QueueService,GitService processing
    class Prometheus,Tracing,HealthCheck external
```

## Parser Dependencies & Language Support

```mermaid
graph TB
    subgraph "Parser Core (codegraph-parser)"
        ParseEngine[Parse Engine<br/>AST Generation]
        Visitor[Visitor Pattern<br/>Tree Traversal]
        NodeFactory[Node Factory<br/>Type Creation]
    end
    
    subgraph "Tree-sitter Languages"
        Rust_TS[tree-sitter-rust<br/>v0.24]
        Python_TS[tree-sitter-python<br/>v0.23]
        JS_TS[tree-sitter-javascript<br/>v0.25]
        TS_TS[tree-sitter-typescript<br/>v0.23]
        Go_TS[tree-sitter-go<br/>v0.23]
        Java_TS[tree-sitter-java<br/>v0.23]
        CPP_TS[tree-sitter-cpp<br/>v0.23]
    end
    
    subgraph "AST Node Types (from core)"
        Function[Function Node]
        Class[Class Node]
        Variable[Variable Node]
        Import[Import Node]
        Comment[Comment Node]
        Generic[Generic Node]
    end
    
    subgraph "Output Targets"
        GraphNodes[Graph Nodes<br/>→ codegraph-graph]
        VectorEmbed[Vector Embeddings<br/>→ codegraph-vector]
        CacheData[Cache Data<br/>→ codegraph-cache]
    end
    
    subgraph "Parallel Processing"
        WorkerPool[Worker Pool<br/>→ codegraph-concurrent]
        TaskQueue[Task Queue<br/>Async Processing]
    end
    
    %% Language to parser engine
    Rust_TS --> ParseEngine
    Python_TS --> ParseEngine
    JS_TS --> ParseEngine
    TS_TS --> ParseEngine
    Go_TS --> ParseEngine
    Java_TS --> ParseEngine
    CPP_TS --> ParseEngine
    
    %% Parser engine to components
    ParseEngine --> Visitor
    ParseEngine --> NodeFactory
    Visitor --> NodeFactory
    
    %% Node factory to AST types
    NodeFactory --> Function
    NodeFactory --> Class
    NodeFactory --> Variable
    NodeFactory --> Import
    NodeFactory --> Comment
    NodeFactory --> Generic
    
    %% AST to outputs
    Function --> GraphNodes
    Class --> GraphNodes
    Variable --> GraphNodes
    Import --> GraphNodes
    Function --> VectorEmbed
    Class --> VectorEmbed
    Comment --> VectorEmbed
    
    %% Caching
    ParseEngine --> CacheData
    NodeFactory --> CacheData
    
    %% Parallel processing
    ParseEngine --> WorkerPool
    ParseEngine --> TaskQueue
    WorkerPool --> TaskQueue
    
    %% Styling
    classDef parser fill:#e3f2fd
    classDef language fill:#e8f5e8
    classDef ast fill:#fff3e0
    classDef output fill:#f3e5f5
    classDef parallel fill:#fce4ec
    
    class ParseEngine,Visitor,NodeFactory parser
    class Rust_TS,Python_TS,JS_TS,TS_TS,Go_TS,Java_TS,CPP_TS language
    class Function,Class,Variable,Import,Comment,Generic ast
    class GraphNodes,VectorEmbed,CacheData output
    class WorkerPool,TaskQueue parallel
```

## Vector Search Dependencies

```mermaid
graph TB
    subgraph "Vector Engine (codegraph-vector)"
        Embedder[Embedding Generator<br/>OpenAI/Local Models]
        IndexManager[Index Manager<br/>FAISS Operations]
        SearchEngine[Search Engine<br/>Similarity Queries]
        Serializer[Serializer<br/>rkyv Zero-copy]
    end
    
    subgraph "FAISS Index Types"
        IndexFlat[IndexFlatL2<br/>Exact Search]
        IndexIVF[IndexIVFFlat<br/>Inverted File]
        IndexPQ[IndexPQ<br/>Product Quantization]
        IndexHNSW[IndexHNSW<br/>Hierarchical NSW]
    end
    
    subgraph "Vector Data Sources"
        CodeBlocks[Code Blocks<br/>Functions, Classes]
        Comments[Comments<br/>Documentation]
        Identifiers[Identifiers<br/>Variables, Types]
        Strings[String Literals<br/>Text Content]
    end
    
    subgraph "Storage Integration"
        GraphLink[Graph Linkage<br/>→ codegraph-graph]
        CacheLayer[Cache Layer<br/>→ codegraph-cache]
        ZeroCopyStore[Zero-copy Store<br/>→ codegraph-zerocopy]
    end
    
    subgraph "External APIs"
        OpenAI[OpenAI API<br/>text-embedding-ada-002]
        LocalModel[Local Models<br/>sentence-transformers]
        CustomModel[Custom Models<br/>Domain-specific]
    end
    
    %% Vector engine components
    Embedder --> IndexManager
    IndexManager --> SearchEngine
    SearchEngine --> Serializer
    
    %% FAISS index selection
    IndexManager --> IndexFlat
    IndexManager --> IndexIVF
    IndexManager --> IndexPQ
    IndexManager --> IndexHNSW
    
    %% Data source processing
    CodeBlocks --> Embedder
    Comments --> Embedder
    Identifiers --> Embedder
    Strings --> Embedder
    
    %% Storage integration
    IndexManager --> GraphLink
    SearchEngine --> GraphLink
    Embedder --> CacheLayer
    SearchEngine --> CacheLayer
    Serializer --> ZeroCopyStore
    IndexManager --> ZeroCopyStore
    
    %% External API integration
    Embedder --> OpenAI
    Embedder --> LocalModel
    Embedder --> CustomModel
    
    %% Performance annotations
    IndexFlat -.->|"O(n) exact"| SearchEngine
    IndexIVF -.->|"O(nprobe)"| SearchEngine
    IndexPQ -.->|"O(1) approx"| SearchEngine
    IndexHNSW -.->|"O(log n)"| SearchEngine
    
    %% Styling
    classDef vector fill:#e3f2fd
    classDef index fill:#e8f5e8
    classDef source fill:#fff3e0
    classDef storage fill:#f3e5f5
    classDef external fill:#fce4ec
    
    class Embedder,IndexManager,SearchEngine,Serializer vector
    class IndexFlat,IndexIVF,IndexPQ,IndexHNSW index
    class CodeBlocks,Comments,Identifiers,Strings source
    class GraphLink,CacheLayer,ZeroCopyStore storage
    class OpenAI,LocalModel,CustomModel external
```

## MCP Integration Architecture

```mermaid
graph TB
    subgraph "MCP Server (codegraph-mcp)"
        MCPCore[MCP Core<br/>Protocol Handler]
        Tools[Tool Registry<br/>Available Operations]
        Resources[Resource Manager<br/>Code Access]
        Prompts[Prompt Templates<br/>AI Interactions]
    end
    
    subgraph "RAG MCP Server (core-rag-mcp-server)"
        RAGCore[RAG Core<br/>Retrieval Logic]
        VectorRAG[Vector Retrieval<br/>Semantic Search]
        GraphRAG[Graph Retrieval<br/>Structural Search]
        HybridRAG[Hybrid Retrieval<br/>Combined Approach]
    end
    
    subgraph "MCP Protocol Stack"
        Transport[Transport Layer<br/>STDIO/HTTP]
        Serialization[Message Serialization<br/>JSON-RPC]
        Authentication[Authentication<br/>Capability-based]
        Streaming[Streaming Support<br/>Large Responses]
    end
    
    subgraph "CodeGraph Integration"
        GraphAccess[Graph Access<br/>→ codegraph-graph]
        VectorAccess[Vector Access<br/>→ codegraph-vector]
        ParserAccess[Parser Access<br/>→ codegraph-parser]
        APIAccess[API Access<br/>→ codegraph-api]
    end
    
    subgraph "Client Integration"
        Claude[Claude Desktop<br/>AI Assistant]
        VSCode[VS Code Extension<br/>IDE Integration]
        CLI[CLI Tools<br/>Command Line]
        WebApps[Web Applications<br/>Browser-based]
    end
    
    %% MCP Server internal
    MCPCore --> Tools
    MCPCore --> Resources
    MCPCore --> Prompts
    
    %% RAG Server components
    RAGCore --> VectorRAG
    RAGCore --> GraphRAG
    RAGCore --> HybridRAG
    MCPCore --> RAGCore
    
    %% Protocol stack
    MCPCore --> Transport
    MCPCore --> Serialization
    MCPCore --> Authentication
    MCPCore --> Streaming
    
    %% CodeGraph integration
    Tools --> GraphAccess
    Tools --> VectorAccess
    Tools --> ParserAccess
    Resources --> GraphAccess
    Resources --> VectorAccess
    VectorRAG --> VectorAccess
    GraphRAG --> GraphAccess
    HybridRAG --> VectorAccess
    HybridRAG --> GraphAccess
    
    %% API integration
    MCPCore --> APIAccess
    RAGCore --> APIAccess
    
    %% Client connections
    Transport --> Claude
    Transport --> VSCode
    Transport --> CLI
    Transport --> WebApps
    
    %% Data flow annotations
    VectorRAG -.->|"Semantic similarity"| VectorAccess
    GraphRAG -.->|"Structural relationships"| GraphAccess
    HybridRAG -.->|"Combined ranking"| VectorAccess
    HybridRAG -.->|"Graph context"| GraphAccess
    
    %% Styling
    classDef mcp fill:#e3f2fd
    classDef rag fill:#e8f5e8
    classDef protocol fill:#fff3e0
    classDef integration fill:#f3e5f5
    classDef client fill:#fce4ec
    
    class MCPCore,Tools,Resources,Prompts mcp
    class RAGCore,VectorRAG,GraphRAG,HybridRAG rag
    class Transport,Serialization,Authentication,Streaming protocol
    class GraphAccess,VectorAccess,ParserAccess,APIAccess integration
    class Claude,VSCode,CLI,WebApps client
```

## Dependency Complexity Matrix

```ascii
╔═══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                    CodeGraph Dependency Complexity Analysis                                          ║
╠═══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                                                       ║
║  Complexity Levels:                                    Dependency Types:                                             ║
║  🟢 Simple (0-2 deps)     🟡 Moderate (3-5 deps)       ● Required (runtime)                                         ║
║  🟠 Complex (6-8 deps)    🔴 High (9+ deps)            ○ Optional (feature-gated)                                    ║
║                                                         ◐ Weak (no strong coupling)                                  ║
║                                                                                                                       ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                        Crate Complexity Map                                                   │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                                       ║
║           🟢 core                        🟡 parser ────────○ concurrent                                              ║
║              │                              │                                                                        ║
║              ↓                              ↓                                                                        ║
║           🟢 zerocopy                    🟡 queue ─────────◐ git                                                      ║
║              │                              │                                                                        ║
║              ↓                              ↓                                                                        ║
║     ┌────────┴────────┐                     │                                                                        ║
║     ↓                 ↓                     ↓                                                                        ║
║  🟡 graph          🟡 vector            🟠 mcp ─────────● graph, vector, parser                                      ║
║     │                 │                     │                                                                        ║
║     ↓                 ↓                     ↓                                                                        ║
║  🟡 cache ────────○ graph               🔴 api ─────────● ALL previous layers                                        ║
║                                             │                                                                        ║
║                                             ↓                                                                        ║
║                                         🟡 lb ──────────● api                                                        ║
║                                                                                                                       ║
║  ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐   ║
║  │                                      Complexity Metrics                                                        │   ║
║  ├─────────────────────────────────────────────────────────────────────────────────────────────────────────────┤   ║
║  │  Crate               │ Direct Deps │ Transitive │ Complexity │ Risk Level │ Refactoring Priority              │   ║
║  ├─────────────────────────────────────────────────────────────────────────────────────────────────────────────┤   ║
║  │  codegraph-core      │      0      │      0     │    🟢      │    Low     │ Stable (foundation)               │   ║
║  │  codegraph-zerocopy  │      1      │      0     │    🟢      │    Low     │ Low (serialization)               │   ║
║  │  codegraph-graph     │      2      │      0     │    🟡      │   Medium   │ Medium (storage evolution)        │   ║
║  │  codegraph-vector    │      2      │      0     │    🟡      │   Medium   │ Medium (ML integration)           │   ║
║  │  codegraph-cache     │      3      │      1     │    🟡      │   Medium   │ Low (isolated optimization)       │   ║
║  │  codegraph-parser    │      1      │      1     │    🟡      │   Medium   │ High (language support)           │   ║
║  │  codegraph-queue     │      1      │      1     │    🟡      │   Medium   │ Medium (async architecture)       │   ║
║  │  codegraph-git       │      1      │      0     │    🟢      │    Low     │ Low (stable integration)          │   ║
║  │  codegraph-concurrent│      1      │      0     │    🟢      │    Low     │ Low (performance utility)         │   ║
║  │  codegraph-mcp       │      4      │      6     │    🟠      │    High    │ High (protocol evolution)         │   ║
║  │  core-rag-mcp-server │      2      │      8     │    🟠      │    High    │ High (RAG architecture)           │   ║
║  │  codegraph-api       │      7      │     12     │    🔴      │  Very High │ Very High (API stability)         │   ║
║  │  codegraph-lb        │      2      │     14     │    🟡      │    High    │ Medium (infrastructure)           │   ║
║  └─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                                                       ║
║  Critical Dependency Paths (for failure analysis):                                                                   ║
║  1. core → ALL (foundation failure affects everything)                                                               ║
║  2. api → graph|vector|cache|parser (service failure affects core functionality)                                     ║
║  3. mcp → graph|vector|parser (protocol failure affects integration)                                                 ║
║  4. graph → vector (storage failure affects search)                                                                  ║
║                                                                                                                       ║
╚═══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

*Generated by CodeGraph Documentation Specialist - Component Dependency Analysis*