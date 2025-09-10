# CodeGraph Architecture Diagrams

This directory contains various architecture diagrams for the CodeGraph project, providing visual representations of the system's structure, components, data flow, technology stack, deployment, and performance optimization layers.

## Diagrams

- [System Architecture Overview](./system_architecture.md)
- [Component Dependencies](./component_dependencies.md)
- [Data Flow Diagram](./data_flow.md)
- [Technology Stack and Deployment Architecture](./tech_stack_deployment.md)
- [Performance Flow and Optimization Layers](./performance_flow.md)

## Viewing Diagrams

These diagrams are created using [Mermaid](https://mermaid-js.github.io/mermaid/). You can view them directly in any Markdown viewer that supports Mermaid rendering (e.g., GitHub, VS Code).

## Generating Interactive HTML Reports

For interactive HTML reports with clickable elements and drill-down capabilities, you can use the [Mermaid Live Editor](https://mermaid.live/) or integrate Mermaid rendering into a documentation site generator (e.g., [MkDocs with `mkdocs-mermaid2-plugin`](https://github.com/mkdocs/mkdocs-mermaid2-plugin)).

### Using Mermaid Live Editor

1. Open the `.md` file of the diagram you want to convert.
2. Copy the Mermaid code block (the content within ````mermaid ... ````).
3. Go to the [Mermaid Live Editor](https://mermaid.live/).
4. Paste the code into the editor on the left.
5. The interactive diagram will be rendered on the right.
6. You can then export the diagram as an SVG, PNG, or copy the HTML code.

### Example HTML Structure (Conceptual)

```html
<!DOCTYPE html>
<html>
<head>
    <title>CodeGraph System Architecture</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
</head>
<body>
    <h1>CodeGraph System Architecture Overview</h1>
    <div class="mermaid">
        graph TB
            subgraph "Application Layer"
                CLI[CLI Tools]
                Web[Web Interface]
                API[API Gateway]
                MCP[MCP[MCP Server]]
            end
            
            subgraph "AI/ML Intelligence Layer"
                RAG[RAG Engine]
                EMB[Embedding Service]
                NLP[NLP Processing]
                CTX[Context Manager]
            end
            
            subgraph "API Protocol Layer"
                GQL[GraphQL API]
                REST[REST API]
                WSS[WebSocket/SSE]
                SDK[Client SDKs]
            end
            
            subgraph "Processing Layer"
                PAR[Code Parser]
                ANA[Semantic Analyzer]
                IND[Indexing Engine]
                QUE[Query Engine]
            end
            
            subgraph "Storage Layer"
                GS[Graph Store]
                VS[Vector Store]
                FS[File System]
                CACHE[Cache Layer]
            end
            
            subgraph "Infrastructure Layer"
                RDB[(RocksDB)]
                FAISS[(FAISS Index)]
                MEM[Memory Pool]
                NET[Network Layer]
            end
            
            CLI --> API
            Web --> API
            API --> RAG
            MCP --> RAG
            
            RAG --> EMB
            RAG --> CTX
            EMB --> NLP
            
            GQL --> QUE
            REST --> QUE
            WSS --> QUE
            
            PAR --> IND
            ANA --> IND
            IND --> VS
            QUE --> GS
            
            GS --> RDB
            VS --> FAISS
            CACHE --> MEM
            
            NET --> ALL
    </div>
    <script>
        mermaid.initialize({ startOnLoad: true });
    </script>
</body>
</html>
```
