# CodeGraph MCP (`codegraph-mcp`)

## Overview
`codegraph-mcp` is the orchestrator crate. It implements the Model Context Protocol (MCP) server and coordinates the indexing pipeline.

## key Components

### `Indexer` (`indexer.rs`)
The heart of the system.
1.  **Orchestration**: Calls `codegraph-parser` to get raw nodes.
2.  **Enrichment**: Calls `codegraph-vector` to add embeddings.
3.  **Resolution**: Resolves abstract edges (names) to concrete IDs.
4.  **Persistence**: Calls `codegraph-graph` to save to DB.

### MCP Server
Implements the standardized protocol to allow IDEs (VS Code, Cursor) and AI assistants to query the graph.
- **Tools**: Exposes tools like `search_code`, `find_references`.
- **Resources**: Exposes graph data as MCP resources.

## Architecture
This crate ties everything together. It has the highest **efferent coupling** (depends on almost everything) but provides the unified interface to the outside world.
