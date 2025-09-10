CodeGraph MCP (Rust)

An async, type-safe Rust client SDK for the Model Context Protocol (MCP).

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

