# Core RAG MCP Server

A production-ready MCP (Model Context Protocol) server that exposes CodeGraph's RAG (Retrieval-Augmented Generation) functionality through standardized MCP tools.

## Features

- **Dual Transport Support**: Both STDIO and HTTP streaming protocols
- **MCP 2024-06-18 Compliance**: Follows the latest MCP specification
- **CodeGraph RAG Integration**: Semantic code search and analysis
- **Production Ready**: Comprehensive error handling and configuration

## Available Tools

### `search_code`
Search for code patterns, functions, and concepts using vector similarity.

**Parameters:**
- `query` (string): Search query
- `limit` (optional, number): Maximum results (default: 10, max: 100)
- `threshold` (optional, number): Similarity threshold (default: 0.7, range: 0.0-1.0)

### `get_code_details`
Get detailed information about a specific code node by its ID.

**Parameters:**
- `node_id` (string): Unique identifier for the code node

### `analyze_relationships`
Analyze relationships and dependencies for a given code node.

**Parameters:**
- `node_id` (string): Node ID to analyze
- `depth` (optional, number): Analysis depth (default: 2, max: 5)

### `get_repo_stats`
Get statistics and overview of the CodeGraph repository.

**Parameters:** None

### `semantic_search`
Perform semantic search using natural language queries.

**Parameters:**
- `query` (string): Natural language search query
- `limit` (optional, number): Maximum results (default: 10, max: 50)

## Installation

### Build from Source

```bash
# Build STDIO server
cargo build --release --bin core-rag-mcp-server-stdio

# Build HTTP server  
cargo build --release --bin core-rag-mcp-server-http
```

### Run STDIO Server

```bash
# Basic usage
./target/release/core-rag-mcp-server-stdio

# With environment configuration
CORE_RAG_DB_PATH=/path/to/db \
CORE_RAG_MAX_RESULTS=50 \
RUST_LOG=info \
./target/release/core-rag-mcp-server-stdio
```

### Run HTTP Server

```bash
# Basic usage (default: 127.0.0.1:8080)
./target/release/core-rag-mcp-server-http

# Custom host/port
CORE_RAG_HOST=0.0.0.0 \
CORE_RAG_PORT=3000 \
./target/release/core-rag-mcp-server-http
```

## Configuration

### Environment Variables

- `CORE_RAG_CONFIG`: Path to JSON configuration file
- `CORE_RAG_DB_PATH`: CodeGraph database path
- `CORE_RAG_HOST`: HTTP server host (default: 127.0.0.1)
- `CORE_RAG_PORT`: HTTP server port (default: 8080)
- `CORE_RAG_MAX_RESULTS`: Maximum search results
- `CORE_RAG_THRESHOLD`: Default similarity threshold
- `CORE_RAG_CACHE_SIZE_MB`: Cache size in megabytes
- `CORE_RAG_WORKERS`: Number of worker threads
- `RUST_LOG`: Logging level (debug, info, warn, error)

### Configuration File

Create `core-rag-config.json`:

```json
{
  "database_path": "./codegraph.db",
  "vector_config": {
    "max_results": 100,
    "default_threshold": 0.7,
    "dimension": 768,
    "index_type": "IVFFlat"
  },
  "cache_config": {
    "cache_size_mb": 256,
    "ttl_seconds": 3600,
    "enable_lru": true
  },
  "parser_config": {
    "file_extensions": ["rs", "py", "js", "ts", "go", "java"],
    "max_file_size": 10485760,
    "incremental": true
  },
  "performance": {
    "worker_threads": 8,
    "batch_size": 100,
    "connection_pool_size": 10,
    "enable_parallel": true
  }
}
```

## Claude Code Integration

### STDIO Configuration

Add to your Claude Code configuration:

```json
{
  "mcpServers": {
    "core-rag": {
      "command": "/path/to/core-rag-mcp-server-stdio",
      "args": [],
      "env": {
        "CORE_RAG_DB_PATH": "/path/to/your/codegraph.db",
        "RUST_LOG": "info"
      }
    }
  }
}
```

### HTTP Configuration

For HTTP-based clients:

```json
{
  "mcpServers": {
    "core-rag": {
      "transport": "http",
      "url": "http://localhost:8080/mcp",
      "headers": {
        "Content-Type": "application/json"
      }
    }
  }
}
```

## API Endpoints (HTTP Mode)

- `GET /health` - Health check
- `GET /status` - Server status and metrics
- `POST /mcp` - MCP JSON-RPC requests
- `GET /mcp` - MCP session management (stateful mode)
- `DELETE /mcp` - Close MCP session

## Testing

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific module
cargo test config
```

## Development

### Project Structure

```
src/
├── lib.rs              # Main server implementation
├── config.rs           # Configuration management
├── error.rs            # Error handling
├── rag_tools.rs         # RAG functionality
├── tests.rs            # Unit tests
└── bin/
    ├── stdio_server.rs  # STDIO transport server
    └── http_server.rs   # HTTP transport server
```

### Adding New Tools

1. Add tool method to `CoreRagMcpServer` with `#[tool]` attribute
2. Implement the functionality in `RagTools`
3. Add appropriate error handling
4. Update documentation

### Architecture

The server follows the official MCP Rust SDK patterns:

- **Tool Router**: Uses `#[tool_router]` macro for automatic tool discovery
- **Server Handler**: Implements `ServerHandler` trait for MCP lifecycle
- **Dual Transport**: Supports both STDIO and HTTP streaming
- **Error Handling**: Comprehensive error types with MCP integration

## License

This project is licensed under the same terms as the CodeGraph project.