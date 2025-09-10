# CodeGraph Technology Stack and Deployment Architecture

```mermaid
graph LR
    subgraph "Deployment Environment"
        Cloud[Cloud Provider (e.g., AWS, GCP, Azure)]
        K8s[Kubernetes Cluster]
        Docker[Docker Containers]
    end

    subgraph "CodeGraph Application"
        CGApp[CodeGraph Binary]
        subgraph "Internal Components"
            RAG[RAG Engine]
            API[API Layer (REST, GraphQL, MCP)]
            Parser[Code Parser]
            Analyzer[Semantic Analyzer]
            Embedder[Embedding System]
            GraphStore[Graph Store]
            VectorStore[Vector Store]
        end
    end

    subgraph "External Services"
        OpenAI[OpenAI API]
        Monitoring[Prometheus/Grafana]
        Logging[ELK Stack/Loki]
    end

    Cloud --> K8s
    K8s --> Docker
    Docker --> CGApp

    CGApp --> RAG
    CGApp --> API
    CGApp --> Parser
    CGApp --> Analyzer
    CGApp --> Embedder
    CGApp --> GraphStore
    CGApp --> VectorStore

    RAG --> OpenAI
    Embedder --> OpenAI

    GraphStore --> RocksDB[(RocksDB)]
    VectorStore --> FAISS[(FAISS Index)]

    CGApp --> Monitoring
    CGApp --> Logging

    style Cloud fill:#f9f,stroke:#333,stroke-width:2px
    style K8s fill:#bbf,stroke:#333,stroke-width:2px
    style Docker fill:#bfb,stroke:#333,stroke-width:2px
    style CGApp fill:#fbb,stroke:#333,stroke-width:2px
    style RAG fill:#f9f,stroke:#333,stroke-width:2px
    style API fill:#ccf,stroke:#333,stroke-width:2px
    style Parser fill:#bfb,stroke:#333,stroke-width:2px
    style Analyzer fill:#bfb,stroke:#333,stroke-width:2px
    style Embedder fill:#bbf,stroke:#333,stroke-width:2px
    style GraphStore fill:#ffb,stroke:#333,stroke-width:2px
    style VectorStore fill:#ffb,stroke:#333,stroke-width:2px
    style OpenAI fill:#ddd,stroke:#333,stroke-width:2px
    style Monitoring fill:#ddd,stroke:#333,stroke-width:2px
    style Logging fill:#ddd,stroke:#333,stroke-width:2px
    style RocksDB fill:#eee,stroke:#333,stroke-width:2px
    style FAISS fill:#eee,stroke:#333,stroke-width:2px
```
