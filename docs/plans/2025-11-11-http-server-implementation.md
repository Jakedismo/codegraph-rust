# HTTP Server Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement production-ready HTTP transport for CodeGraph MCP server using rmcp's StreamableHttpService

**Architecture:** Session-based stateful HTTP server with SSE streaming for real-time progress notifications during agentic tool execution. Uses Tower service pattern with POST /mcp for requests and GET /sse for streaming responses.

**Tech Stack:**
- rmcp 0.7.0 (transport-streamable-http-server feature)
- axum (HTTP framework)
- Tower (service abstraction)
- tokio (async runtime)
- SSE (Server-Sent Events) for progress streaming

---

## Task 1: Add HTTP Server Dependencies and Feature

**Files:**
- Modify: `crates/codegraph-mcp/Cargo.toml:91`

**Step 1: Verify dependencies are already present**

Check lines 65-66 and 91 in Cargo.toml:
```toml
axum = { workspace = true, optional = true }
hyper = { workspace = true, optional = true }
server-http = ["dep:axum", "dep:hyper"]
```

Expected: Dependencies already configured (no changes needed)

**Step 2: Update build documentation**

Add to README.md build examples:
```bash
# HTTP server with AutoAgents
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"
```

**Step 3: Commit**

```bash
git add crates/codegraph-mcp/Cargo.toml README.md
git commit -m "docs: add HTTP server build instructions"
```

---

## Task 2: Create HTTP Server Configuration Module

**Files:**
- Create: `crates/codegraph-mcp/src/http_config.rs`

**Step 1: Write failing test**

Create test file:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_http_config() {
        let config = HttpServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.keep_alive_seconds, 15);
    }

    #[test]
    fn test_bind_address() {
        let config = HttpServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            keep_alive_seconds: 30,
        };
        assert_eq!(config.bind_address(), "0.0.0.0:8080");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p codegraph-mcp --lib http_config`
Expected: FAIL with "module not found"

**Step 3: Implement configuration struct**

```rust
// ABOUTME: HTTP server configuration for CodeGraph MCP server
// ABOUTME: Handles host, port, and SSE keep-alive settings for session-based HTTP transport

use serde::{Deserialize, Serialize};

/// Configuration for HTTP server transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Host address to bind to (default: "127.0.0.1")
    pub host: String,
    /// Port to listen on (default: 3000)
    pub port: u16,
    /// SSE keep-alive interval in seconds (default: 15)
    pub keep_alive_seconds: u64,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            keep_alive_seconds: 15,
        }
    }
}

impl HttpServerConfig {
    /// Get the bind address as host:port string
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Parse from environment variables with CODEGRAPH_HTTP_ prefix
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("CODEGRAPH_HTTP_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("CODEGRAPH_HTTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            keep_alive_seconds: std::env::var("CODEGRAPH_HTTP_KEEP_ALIVE")
                .ok()
                .and_then(|k| k.parse().ok())
                .unwrap_or(15),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_http_config() {
        let config = HttpServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.keep_alive_seconds, 15);
    }

    #[test]
    fn test_bind_address() {
        let config = HttpServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            keep_alive_seconds: 30,
        };
        assert_eq!(config.bind_address(), "0.0.0.0:8080");
    }
}
```

**Step 4: Add module to lib.rs**

Add to `crates/codegraph-mcp/src/lib.rs`:
```rust
#[cfg(feature = "server-http")]
pub mod http_config;
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p codegraph-mcp --features server-http --lib http_config`
Expected: PASS (2 tests)

**Step 6: Commit**

```bash
git add crates/codegraph-mcp/src/http_config.rs crates/codegraph-mcp/src/lib.rs
git commit -m "feat: add HTTP server configuration module"
```

---

## Task 3: Implement HTTP Server Handler Module

**Files:**
- Create: `crates/codegraph-mcp/src/http_server.rs`

**Step 1: Write structure for HTTP handler**

```rust
// ABOUTME: HTTP server implementation using rmcp StreamableHttpService
// ABOUTME: Provides session-based HTTP transport with SSE streaming for progress notifications

#[cfg(feature = "server-http")]
use crate::http_config::HttpServerConfig;
use crate::official_server::CodeGraphMCPServer;
use axum::{routing::get, Router};
use rmcp::transport::streamable_http_server::{
    session::LocalSessionManager, StreamableHttpService,
};
use std::net::SocketAddr;
use tracing::{info, warn};

/// Start HTTP server with CodeGraph MCP service
pub async fn start_http_server(
    server: CodeGraphMCPServer,
    config: HttpServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server at {}", config.bind_address());

    // Create session manager for stateful HTTP connections
    let session_manager = LocalSessionManager::new();

    // Create streamable HTTP service with CodeGraph MCP server
    let http_service = StreamableHttpService::new(
        server,
        session_manager,
        std::time::Duration::from_secs(config.keep_alive_seconds),
    );

    // Build Axum router with MCP endpoints
    let app = Router::new()
        .route("/mcp", axum::routing::post(handle_mcp_request))
        .route("/sse", get(handle_sse_stream))
        .route("/health", get(health_check))
        .with_state(http_service);

    // Parse bind address
    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .map_err(|e| format!("Invalid bind address: {}", e))?;

    info!("CodeGraph MCP HTTP server listening on http://{}", addr);
    info!("Endpoints:");
    info!("  POST http://{}/mcp - Send MCP requests", addr);
    info!("  GET  http://{}/sse - Connect to SSE stream (requires Mcp-Session-Id header)", addr);
    info!("  GET  http://{}/health - Health check", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Handle MCP POST requests
async fn handle_mcp_request(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl axum::response::IntoResponse {
    // Implementation placeholder - will be completed in next task
    axum::http::StatusCode::NOT_IMPLEMENTED
}

/// Handle SSE streaming connections
async fn handle_sse_stream(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    // Implementation placeholder - will be completed in next task
    axum::http::StatusCode::NOT_IMPLEMENTED
}
```

**Step 2: Add module to lib.rs**

Add to `crates/codegraph-mcp/src/lib.rs`:
```rust
#[cfg(feature = "server-http")]
pub mod http_server;
```

**Step 3: Build to verify structure compiles**

Run: `cargo build -p codegraph-mcp --features "server-http,ai-enhanced,faiss,ollama"`
Expected: Compile success (warnings about unused variables OK)

**Step 4: Commit**

```bash
git add crates/codegraph-mcp/src/http_server.rs crates/codegraph-mcp/src/lib.rs
git commit -m "feat: add HTTP server handler structure"
```

---

## Task 4: Implement MCP Request Handler

**Files:**
- Modify: `crates/codegraph-mcp/src/http_server.rs:71-78`

**Step 1: Study rmcp Tower service integration**

Read: `vendor/rmcp/crates/rmcp-server-sdk/src/transport/streamable_http_server/tower.rs:200-250`

Key patterns:
- Use `tower::ServiceExt::call()` to invoke the service
- Extract session ID from `Mcp-Session-Id` header
- Handle request body as JSON-RPC message
- Return SSE response stream

**Step 2: Implement handle_mcp_request**

Replace placeholder with:
```rust
/// Handle MCP POST requests
async fn handle_mcp_request(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl axum::response::IntoResponse {
    use axum::response::sse::Event;
    use futures::stream::StreamExt;
    use tower::ServiceExt;

    // Extract session ID from header (optional for first request)
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Parse request body as JSON-RPC
    let request = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            warn!("Invalid JSON-RPC request: {}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Invalid JSON-RPC: {}", e),
            )
                .into_response();
        }
    };

    // Create HTTP request for Tower service
    let mut http_request = axum::http::Request::builder()
        .uri("/mcp")
        .method("POST")
        .header("content-type", "application/json");

    // Add session ID header if present
    if let Some(sid) = session_id {
        http_request = http_request.header("Mcp-Session-Id", sid);
    }

    let http_request = match http_request.body(request) {
        Ok(req) => req,
        Err(e) => {
            warn!("Failed to build HTTP request: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Request build error: {}", e),
            )
                .into_response();
        }
    };

    // Call Tower service
    let response = match service.oneshot(http_request).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Service call failed: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Service error: {}", e),
            )
                .into_response();
        }
    };

    // Extract session ID from response headers
    let new_session_id = response
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok());

    // Convert response body stream to SSE events
    let body_stream = response.into_body();
    let sse_stream = body_stream.map(|chunk| {
        chunk
            .map(|data| {
                let json_str = String::from_utf8_lossy(&data);
                Event::default().data(json_str.to_string())
            })
            .map_err(|e| {
                warn!("Stream error: {}", e);
                e
            })
    });

    // Build SSE response
    let mut sse_response = axum::response::Sse::new(sse_stream).into_response();

    // Add session ID header to response
    if let Some(sid) = new_session_id {
        sse_response.headers_mut().insert(
            "Mcp-Session-Id",
            axum::http::HeaderValue::from_str(sid).unwrap(),
        );
    }

    sse_response
}
```

**Step 3: Build to verify implementation**

Run: `cargo build -p codegraph-mcp --features "server-http,ai-enhanced,faiss,ollama"`
Expected: Compile success

**Step 4: Commit**

```bash
git add crates/codegraph-mcp/src/http_server.rs
git commit -m "feat: implement MCP POST request handler with SSE response"
```

---

## Task 5: Implement SSE Stream Reconnection Handler

**Files:**
- Modify: `crates/codegraph-mcp/src/http_server.rs:80-88`

**Step 1: Implement handle_sse_stream**

Replace placeholder with:
```rust
/// Handle SSE streaming connections (reconnection support)
async fn handle_sse_stream(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    use axum::response::sse::Event;
    use futures::stream::StreamExt;
    use tower::ServiceExt;

    // Extract session ID (required for SSE reconnection)
    let session_id = match headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
    {
        Some(sid) => sid,
        None => {
            warn!("SSE connection missing Mcp-Session-Id header");
            return (
                axum::http::StatusCode::BAD_REQUEST,
                "Missing Mcp-Session-Id header",
            )
                .into_response();
        }
    };

    // Extract Last-Event-Id for resumption (optional)
    let last_event_id = headers
        .get("Last-Event-Id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    info!(
        "SSE reconnection request for session {} (last_event_id: {:?})",
        session_id, last_event_id
    );

    // Create GET request for SSE endpoint
    let mut http_request = axum::http::Request::builder()
        .uri("/sse")
        .method("GET")
        .header("Mcp-Session-Id", session_id);

    if let Some(event_id) = last_event_id {
        http_request = http_request.header("Last-Event-Id", event_id);
    }

    let http_request = match http_request.body(serde_json::Value::Null) {
        Ok(req) => req,
        Err(e) => {
            warn!("Failed to build SSE request: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Request build error: {}", e),
            )
                .into_response();
        }
    };

    // Call Tower service
    let response = match service.oneshot(http_request).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("SSE service call failed: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Service error: {}", e),
            )
                .into_response();
        }
    };

    // Convert response stream to SSE events
    let body_stream = response.into_body();
    let sse_stream = body_stream.map(|chunk| {
        chunk
            .map(|data| {
                let json_str = String::from_utf8_lossy(&data);
                Event::default().data(json_str.to_string())
            })
            .map_err(|e| {
                warn!("SSE stream error: {}", e);
                e
            })
    });

    axum::response::Sse::new(sse_stream).into_response()
}
```

**Step 2: Build to verify implementation**

Run: `cargo build -p codegraph-mcp --features "server-http,ai-enhanced,faiss,ollama"`
Expected: Compile success

**Step 3: Commit**

```bash
git add crates/codegraph-mcp/src/http_server.rs
git commit -m "feat: implement SSE stream reconnection handler"
```

---

## Task 6: Integrate HTTP Server into Main Binary

**Files:**
- Modify: `crates/codegraph-mcp/src/bin/codegraph-official.rs:86-89`

**Step 1: Add imports for HTTP server**

Add at top of file after existing imports:
```rust
#[cfg(feature = "server-http")]
use codegraph_mcp::{http_config::HttpServerConfig, http_server::start_http_server};
```

**Step 2: Replace HTTP transport stub**

Replace lines 86-89 with:
```rust
"http" => {
    #[cfg(feature = "server-http")]
    {
        info!("Using HTTP transport with SSE streaming (official MCP protocol)");

        let http_config = HttpServerConfig {
            host: _host,
            port: _port,
            keep_alive_seconds: 15,
        };

        start_http_server(server, http_config).await?;
    }

    #[cfg(not(feature = "server-http"))]
    {
        return Err("HTTP transport requires 'server-http' feature. Build with: cargo build --features server-http".into());
    }
}
```

**Step 3: Build with HTTP feature**

Run: `cargo build -p codegraph-mcp --bin codegraph-official --features "server-http,ai-enhanced,faiss,ollama"`
Expected: Compile success

**Step 4: Test binary accepts http transport**

Run: `./target/debug/codegraph-official serve --transport http --help`
Expected: Help output shows http transport option

**Step 5: Commit**

```bash
git add crates/codegraph-mcp/src/bin/codegraph-official.rs
git commit -m "feat: integrate HTTP server into main binary"
```

---

## Task 7: Update Environment Variables Documentation

**Files:**
- Modify: `.env.example`

**Step 1: Add HTTP server configuration variables**

Add new section after existing content:
```bash
# ============================================================================
# HTTP Server Configuration (when using --transport http)
# ============================================================================

# Host address to bind HTTP server (default: 127.0.0.1)
# Use 0.0.0.0 to allow external connections
CODEGRAPH_HTTP_HOST=127.0.0.1

# Port for HTTP server (default: 3000)
CODEGRAPH_HTTP_PORT=3000

# SSE keep-alive interval in seconds (default: 15)
# Prevents proxy timeouts for long-running agentic operations
CODEGRAPH_HTTP_KEEP_ALIVE=15
```

**Step 2: Commit**

```bash
git add .env.example
git commit -m "docs: add HTTP server environment variables"
```

---

## Task 8: Create HTTP Server Integration Test

**Files:**
- Create: `tests/http_server_integration.rs`

**Step 1: Write integration test**

```rust
//! Integration test for HTTP server with MCP protocol
#![cfg(feature = "server-http")]

use codegraph_mcp::{
    http_config::HttpServerConfig,
    http_server::start_http_server,
    official_server::CodeGraphMCPServer,
};
use serde_json::json;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_http_server_health_check() {
    // Start server in background
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13000, // Use non-standard port for testing
        keep_alive_seconds: 5,
    };

    let server_handle = tokio::spawn(async move {
        start_http_server(server, config).await
    });

    // Wait for server to start
    sleep(Duration::from_millis(100)).await;

    // Test health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:13000/health")
        .send()
        .await
        .expect("Failed to send health check request");

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");

    // Cleanup
    server_handle.abort();
}

#[tokio::test]
async fn test_http_mcp_initialize_request() {
    // Start server in background
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13001,
        keep_alive_seconds: 5,
    };

    let server_handle = tokio::spawn(async move {
        start_http_server(server, config).await
    });

    sleep(Duration::from_millis(100)).await;

    // Send initialize request
    let client = reqwest::Client::new();
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    let response = client
        .post("http://127.0.0.1:13001/mcp")
        .header("Content-Type", "application/json")
        .json(&initialize_request)
        .send()
        .await
        .expect("Failed to send initialize request");

    // Should get SSE stream with session ID
    assert!(response.headers().contains_key("mcp-session-id"));
    assert_eq!(response.headers().get("content-type").unwrap(), "text/event-stream");

    // Cleanup
    server_handle.abort();
}
```

**Step 2: Add reqwest to dev-dependencies**

Verify `crates/codegraph-mcp/Cargo.toml` has reqwest (already present at line 28)

**Step 3: Run integration tests**

Run: `cargo test -p codegraph-mcp --test http_server_integration --features "server-http,ai-enhanced,faiss,ollama"`
Expected: Both tests PASS

**Step 4: Commit**

```bash
git add tests/http_server_integration.rs
git commit -m "test: add HTTP server integration tests"
```

---

## Task 9: Create Python HTTP MCP Client Test

**Files:**
- Create: `test_http_mcp_client.py`

**Step 1: Write Python test using SSE client**

```python
#!/usr/bin/env python3
"""
Test HTTP MCP server with SSE streaming for agentic tools.

REQUIREMENTS:
  - Binary built with: cargo build --release --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"
  - Start server: ./target/release/codegraph-official serve --transport http --port 3000

Usage:
  python3 test_http_mcp_client.py
"""

import json
import requests
import sseclient
from typing import Optional

MCP_URL = "http://127.0.0.1:3000/mcp"
SSE_URL = "http://127.0.0.1:3000/sse"

def send_mcp_request(request: dict, session_id: Optional[str] = None):
    """Send MCP request and handle SSE response stream."""
    headers = {"Content-Type": "application/json"}
    if session_id:
        headers["Mcp-Session-Id"] = session_id

    response = requests.post(MCP_URL, json=request, headers=headers, stream=True)
    response.raise_for_status()

    # Extract session ID from response
    new_session_id = response.headers.get("Mcp-Session-Id")

    # Parse SSE stream
    client = sseclient.SSEClient(response)
    results = []
    for event in client.events():
        if event.data:
            results.append(json.loads(event.data))

    return results, new_session_id

def test_initialize():
    """Test MCP initialize handshake."""
    print("Testing initialize...")

    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test-http-client",
                "version": "1.0.0"
            }
        }
    }

    results, session_id = send_mcp_request(request)
    assert session_id, "No session ID returned"
    assert results, "No response received"

    print(f"✓ Initialize successful (session: {session_id})")
    return session_id

def test_list_tools(session_id: str):
    """Test listing available tools."""
    print("Testing list tools...")

    request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }

    results, _ = send_mcp_request(request, session_id)
    assert results, "No tools returned"

    tools = results[0].get("result", {}).get("tools", [])
    print(f"✓ Found {len(tools)} tools")

    # Verify agentic tools present
    agentic_tools = [t for t in tools if t["name"].startswith("agentic_")]
    print(f"✓ Found {len(agentic_tools)} agentic tools")

    return tools

def test_vector_search(session_id: str):
    """Test vector search tool."""
    print("Testing vector search...")

    request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "vector_search",
            "arguments": {
                "query": "graph database",
                "limit": 3
            }
        }
    }

    results, _ = send_mcp_request(request, session_id)
    assert results, "No search results"

    print(f"✓ Vector search returned {len(results)} events")

def main():
    """Run all HTTP MCP tests."""
    print("=" * 72)
    print("CodeGraph HTTP MCP Server Test")
    print("=" * 72)

    try:
        # Test initialize handshake
        session_id = test_initialize()

        # Test list tools
        test_list_tools(session_id)

        # Test vector search
        test_vector_search(session_id)

        print("\n" + "=" * 72)
        print("All tests passed! ✓")
        print("=" * 72)

    except Exception as e:
        print(f"\n✗ Test failed: {e}")
        raise

if __name__ == "__main__":
    main()
```

**Step 2: Add Python dependencies**

Add to `requirements-test.txt`:
```
sseclient-py>=1.8.0
```

**Step 3: Install dependencies and run test**

Run:
```bash
pip install sseclient-py
python3 test_http_mcp_client.py
```
Expected: All tests pass when server is running

**Step 4: Commit**

```bash
git add test_http_mcp_client.py requirements-test.txt
git commit -m "test: add Python HTTP MCP client test"
```

---

## Task 10: Update CLAUDE.md Documentation

**Files:**
- Modify: `CLAUDE.md:86-90`

**Step 1: Update HTTP transport status**

Replace:
```markdown
**Note on HTTP Transport:**
- HTTP transport is **not yet implemented** with the official rmcp SDK
- STDIO transport is the recommended and fully-supported mode
- Use `codegraph start stdio` for Claude Desktop and other MCP clients
- HTTP implementation is planned but currently incomplete
```

With:
```markdown
**HTTP Transport (Experimental):**
- HTTP transport with SSE streaming is now available
- Requires `server-http` feature flag
- Build: `cargo build --release --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"`
- Start: `./target/release/codegraph-official serve --transport http --port 3000`
- Endpoints:
  - `POST /mcp` - Send MCP requests (returns SSE stream)
  - `GET /sse` - Reconnect to existing session
  - `GET /health` - Health check
- **Production Status**: Experimental - use STDIO for production
- **Best For**: Web integrations, multi-client scenarios, debugging
```

**Step 2: Add HTTP server section**

Add new section after "Running the MCP Server":
```markdown
### HTTP Server Mode

```bash
# Build with HTTP support
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"

# Start HTTP server (default: http://127.0.0.1:3000)
./target/release/codegraph-official serve --transport http

# Custom host and port
./target/release/codegraph-official serve --transport http --host 0.0.0.0 --port 8080

# Test with curl
curl http://127.0.0.1:3000/health  # Should return "OK"

# Send MCP initialize request
curl -X POST http://127.0.0.1:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "capabilities": {},
      "clientInfo": {"name": "curl", "version": "1.0"}
    }
  }'
```

**Environment Variables:**
```bash
CODEGRAPH_HTTP_HOST=127.0.0.1  # Bind address
CODEGRAPH_HTTP_PORT=3000        # Listen port
CODEGRAPH_HTTP_KEEP_ALIVE=15   # SSE keep-alive seconds
```
```

**Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with HTTP server documentation"
```

---

## Task 11: Add Makefile Target for HTTP Server

**Files:**
- Modify: `Makefile`

**Step 1: Add HTTP server build target**

Add after existing build targets:
```makefile
.PHONY: build-mcp-http
build-mcp-http:
	cargo build --release -p codegraph-mcp --bin codegraph-official --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"

.PHONY: run-http-server
run-http-server: build-mcp-http
	./target/release/codegraph-official serve --transport http --port 3000
```

**Step 2: Test makefile target**

Run: `make build-mcp-http`
Expected: Binary builds successfully

**Step 3: Commit**

```bash
git add Makefile
git commit -m "build: add Makefile targets for HTTP server"
```

---

## Task 12: Final Testing and Documentation

**Files:**
- Modify: `README.md`

**Step 1: Add HTTP server quick start**

Add section to README.md:
```markdown
### HTTP Server Mode (Experimental)

For web integrations and multi-client scenarios:

```bash
# Build with HTTP support
make build-mcp-http

# Start server
./target/release/codegraph-official serve --transport http

# Test endpoints
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
```

**Features:**
- ✅ Session-based stateful connections
- ✅ SSE streaming for real-time progress
- ✅ Automatic session management
- ✅ Reconnection support with Last-Event-Id

**Use Cases:**
- Web-based code analysis dashboards
- Multi-client collaborative environments
- API integrations
- Development/debugging (easier to inspect than STDIO)

**Note:** For production use with Claude Desktop, use STDIO mode.
```

**Step 2: Run full test suite**

Run:
```bash
cargo test -p codegraph-mcp --features "server-http,ai-enhanced,faiss,ollama"
```
Expected: All tests pass

**Step 3: Manual end-to-end test**

```bash
# Terminal 1: Start server
./target/release/codegraph-official serve --transport http

# Terminal 2: Run Python test
python3 test_http_mcp_client.py
```
Expected: All Python tests pass

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add HTTP server quick start to README"
```

---

## Summary

**Implementation Complete:**
✅ HTTP server configuration module
✅ Tower service integration with rmcp StreamableHttpService
✅ POST /mcp handler with SSE response streaming
✅ GET /sse reconnection handler with Last-Event-Id support
✅ GET /health endpoint
✅ Integration with main binary (feature-gated)
✅ Environment variable configuration
✅ Rust integration tests
✅ Python client test suite
✅ Documentation updates (CLAUDE.md, README.md)
✅ Makefile build targets

**Architecture:**
- Session-based HTTP server using rmcp's StreamableHttpService
- SSE streaming for real-time progress during agentic operations
- Tower service pattern for composability
- Graceful degradation (feature flags)
- Environment-based configuration

**Testing:**
- Unit tests: HttpServerConfig (2 tests)
- Integration tests: Health check, initialize handshake (2 tests)
- End-to-end: Python client with SSE parsing (3 tests)

**Total Tasks:** 12
**Estimated Time:** 60-90 minutes (bite-sized 5-minute tasks)
