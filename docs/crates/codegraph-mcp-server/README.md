# CodeGraph MCP Family

This documentation covers the suite of MCP-related crates that modularize the server functionality.

## `codegraph-mcp-core`
Core traits and types for the MCP protocol implementation, shared by server and tools.

## `codegraph-mcp-server`
The actual binary/library setup for running the MCP server. Handles connection lifecycle (Stdio/SSE).

## `codegraph-mcp-daemon`
Background service implementation.
- **File Watching**: Monitors filesystem for changes to trigger incremental re-indexing.
- **State Management**: Holds the long-running reference to the graph.

## `codegraph-mcp-tools`
Collection of specific MCP tools exposed to clients.
- **Search**: Semantic and exact search.
- **Graph Traversal**: Tools to "walk" the graph.

## `codegraph-mcp-autoagents`
Experimental crate for autonomous agents that can use the graph to solve tasks.
- **Agent Loop**: Observation -> Thought -> Action loop using the graph tools.

## `codegraph-mcp-rig`
Testing rig and scaffolding for developing new MCP tools and features.
