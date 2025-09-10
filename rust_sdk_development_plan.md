## Development Context

- Feature: Native Rust client SDK for MCP (Model Context Protocol), providing async WebSocket transport, JSON-RPC 2.0 message handling, versioned MCP handshake, heartbeat monitoring, and connection pooling for efficient, concurrent use.
- Technical Stack: Rust 1.79+, Tokio async runtime, tokio-tungstenite WebSocket client, Serde/serde_json for JSON, tracing for logging, thiserror/anyhow for error handling, DashMap for concurrency-safe maps.
- Constraints: Idiomatic async/await design, type-safe request/response APIs, non-blocking I/O, avoid clones and extra allocations where possible, integrate heartbeat and reconnection with backoff, support MCP protocol versions 2024-11-05 and 2025-03-26.
- Success Criteria: 
  - Asynchronous connect + initialize (handshake) succeeds against compliant MCP server
  - Typed request/response helpers work with strong typing and timeouts
  - Connection pool multiplexes requests efficiently with least-busy selection
  - Heartbeat and reconnection logic recover from drops
  - Comprehensive error types and structured logging

## Library Research (Context7 fallback)

Context7 library fetch failed in this environment; used docs.rs/homepages as references:
- tokio (async runtime) – docs.rs/tokio
- tokio-tungstenite (WebSocket client) – docs.rs/tokio-tungstenite
- serde/serde_json (serialization) – docs.rs/serde, docs.rs/serde_json
- tracing (logging) – docs.rs/tracing
- thiserror/anyhow (error handling) – docs.rs/thiserror, docs.rs/anyhow
- dashmap (concurrent map) – docs.rs/dashmap
- url (URL parsing) – docs.rs/url

Patterns applied:
- Split sink/stream with `futures::StreamExt`/`SinkExt` for WebSocket
- JSON-RPC 2.0 envelope with typed helpers for safe decode
- Version negotiation using predeclared supported versions
- Exponential backoff with jitter for reconnect
- Heartbeat via WebSocket ping/pong frames, integrated with state monitor

Security and robustness:
- Validate incoming messages before routing
- Timeouts for requests; pending map cleanup on error
- Avoid blocking operations; all I/O async
- Minimal cloning and zero-copy where reasonable (e.g., reusing strings/values)

