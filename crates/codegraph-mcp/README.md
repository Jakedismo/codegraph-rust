# CodeGraph MCP

An async, type-safe Rust implementation for the Model Context Protocol (MCP) with a comprehensive CLI for server management and project indexing.

Features
- Async/await with Tokio
- JSON-RPC 2.0 message handling
- MCP initialize handshake with version negotiation
- WebSocket transport (tokio-tungstenite)
- Heartbeat with websocket ping/pong
- Connection pooling with least-busy selection
- Comprehensive error types

Quick Start
```rust
use codegraph_mcp::{connection::McpClientConfig, McpConnection};
use url::Url;

#[tokio::main]
async fn main() -> codegraph_mcp::Result<()> {
    let url = Url::parse("wss://localhost:8081/mcp").unwrap();
    let cfg = McpClientConfig::new(url);
    let client = McpConnection::connect(&cfg).await?;

    // Typed request example
    #[derive(serde::Serialize)]
    struct EchoParams { value: String }
    #[derive(serde::Deserialize)]
    struct EchoResult { echoed: String }

    let res: EchoResult = client
        .send_request_typed("codegraph/echo", &EchoParams { value: "hello".into() })
        .await?;
    println!("echoed={}", res.echoed);

    client.close().await?;
    Ok(())
}
```

Notes
- Supported protocol versions: 2024-11-05, 2025-03-26 (default)
- Uses websocket ping/pong for heartbeat; integrates with HeartbeatManager
- Requests are timed out; responses routed via in-flight map

## CLI Usage

The `codegraph` CLI provides comprehensive tools for managing MCP servers and indexing projects.

### Installation

```bash
cargo install --path .
```

### Server Management

```bash
# Start MCP server with STDIO transport
codegraph start stdio

# Start with HTTP transport
codegraph start http --host 127.0.0.1 --port 3000

# Start with dual transport (STDIO + HTTP)
codegraph start dual --port 3000

# Check server status
codegraph status --detailed

# Stop server
codegraph stop
```

### Project Indexing

```bash
# Index current directory
codegraph index .

# Index with specific languages
codegraph index . --languages rust,python,typescript

# Watch for changes and auto-reindex
codegraph index . --watch

# Force reindex with multiple workers
codegraph index . --force --workers 8
```

### Code Search

```bash
# Semantic search
codegraph search "authentication handler"

# Exact match
codegraph search "fn process_data" --search-type exact

# Regex search
codegraph search "fn \w+_handler" --search-type regex

# Output as JSON
codegraph search "database" --format json --limit 20
```

### Configuration

```bash
# Show configuration
codegraph config show

# Set configuration values
codegraph config set embedding_model openai
codegraph config set vector_dimension 1536

# Validate configuration
codegraph config validate
```

### Statistics

```bash
# Show all statistics
codegraph stats

# Index statistics only
codegraph stats --index --format json
```

### Project Initialization

```bash
# Initialize new project
codegraph init --name my-project
```

### Cleanup

```bash
# Clean all resources
codegraph clean --all --yes
```

For more detailed documentation, run `codegraph --help` or `codegraph <command> --help`.

