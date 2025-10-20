# CodeGraph System Architecture

This document provides a detailed overview of the CodeGraph system architecture, including a component-level dependency diagram.

## 1. Interactive Architecture Diagram

An interactive and animated version of the architecture diagram is available in [`architecture.html`](architecture.html).

To view it, open the HTML file in your browser. For the interactivity to work, you will first need to generate the `architecture.svg` file by rendering the Mermaid diagram below using the [Mermaid Live Editor](https://mermaid.live).

## 2. Component-Level Dependency Architecture

The following diagram illustrates the dependencies between the various crates (components) in the CodeGraph system.

```mermaid
graph TD
    subgraph "Core"
        A[codegraph-core]
    end

    subgraph "Data Processing"
        B[codegraph-parser] --> A
        C[codegraph-graph] --> A
        D[codegraph-vector] --> A
        E[codegraph-ai] --> A
        E --> C
        E --> D
    end

    subgraph "Application Logic"
        F[codegraph-mcp] --> A
        F --> B
        F --> C
        F --> D
        F --> E
        G[codegraph-api] --> A
        G --> B
        G --> C
        G --> D
        H[core-rag-mcp-server] --> A
        H --> B
        H --> D
        H --> I[codegraph-cache]
    end

    subgraph "Utilities"
        I[codegraph-cache] --> A
        J[codegraph-concurrent] --> A
        K[codegraph-git] --> A
        L[codegraph-queue] --> A
        L --> J
        M[codegraph-lb]
        N[codegraph-zerocopy] --> A
    end

    style F fill:#f9f,stroke:#333,stroke-width:2px
    style G fill:#f9f,stroke:#333,stroke-width:2px
    style H fill:#f9f,stroke:#333,stroke-width:2px
```