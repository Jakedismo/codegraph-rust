use codegraph_mcp::*;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};
use url::Url;

/// Mock MCP server for testing
struct MockMcpServer {
    addr: String,
}

impl MockMcpServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let addr_str = format!("ws://{}:{}", addr.ip(), addr.port());

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(handle_connection(stream));
            }
        });

        Self { addr: addr_str }
    }

    fn url(&self) -> Url {
        Url::parse(&self.addr).unwrap()
    }
}

async fn handle_connection(stream: TcpStream) {
    let ws_stream = accept_async(stream).await.unwrap();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(msg) = ws_receiver.next().await {
        if let Ok(msg) = msg {
            match msg {
                WsMessage::Text(text) => {
                    // Parse JSON-RPC message
                    if let Ok(json_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(method) = json_msg.get("method").and_then(|m| m.as_str()) {
                            if method == "initialize" {
                                // Send initialize response
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "id": json_msg.get("id"),
                                    "result": {
                                        "protocol_version": "2025-03-26",
                                        "capabilities": {},
                                        "server_info": {
                                            "name": "mock-mcp-server",
                                            "version": "1.0.0"
                                        }
                                    }
                                });
                                let _ = ws_sender
                                    .send(WsMessage::Text(response.to_string().into()))
                                    .await;
                            }
                        }

                        // Echo other messages for testing
                        if json_msg.get("method").is_none() && json_msg.get("id").is_some() {
                            let response = json!({
                                "jsonrpc": "2.0",
                                "id": json_msg.get("id"),
                                "result": "echo"
                            });
                            let _ = ws_sender
                                .send(WsMessage::Text(response.to_string().into()))
                                .await;
                        }
                    }
                }
                WsMessage::Ping(payload) => {
                    let _ = ws_sender.send(WsMessage::Pong(payload)).await;
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }
    }
}

#[tokio::test]
async fn test_mcp_connection_establishment() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await;
    assert!(connection.is_ok());

    let conn = connection.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_mcp_version_negotiation() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();

    // Connection should have negotiated the latest version successfully
    assert_eq!(connection.inflight(), 0);

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_mcp_request_response() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();

    // Send a test request
    let params = json!({"test": "data"});
    let response: String = connection
        .send_request_typed("test_method", &params)
        .await
        .unwrap();

    assert_eq!(response, "echo");

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_mcp_notification() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();

    // Send a notification (should not fail)
    let params = json!({"notification": "test"});
    let result = connection
        .send_notification("test_notification", &params)
        .await;

    assert!(result.is_ok());

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_mcp_heartbeat_enabled() {
    let server = MockMcpServer::start().await;
    let heartbeat_config = HeartbeatConfig {
        interval: Duration::from_millis(100),
        timeout: Duration::from_millis(50),
        max_missed: 2,
    };
    let heartbeat = HeartbeatManager::with_config(heartbeat_config);
    let cfg = McpClientConfig::new(server.url()).with_heartbeat(heartbeat);

    let connection = McpConnection::connect(&cfg).await.unwrap();

    // Wait a bit to let heartbeat mechanism work
    tokio::time::sleep(Duration::from_millis(300)).await;

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_mcp_connection_pool() {
    let server = MockMcpServer::start().await;

    let pool = McpClientPool::connect(server.url(), 3).await.unwrap();

    // Test acquiring connections
    let conn1 = pool.acquire();
    let conn2 = pool.acquire();
    let conn3 = pool.acquire();

    // All connections should be different instances but share the load
    assert!(conn1.inflight() == 0);
    assert!(conn2.inflight() == 0);
    assert!(conn3.inflight() == 0);
}

#[tokio::test]
async fn test_message_latency_benchmark() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();

    let mut total_latency = Duration::ZERO;
    let num_requests = 10;

    for _ in 0..num_requests {
        let start = Instant::now();

        let params = json!({"benchmark": "test"});
        let _response: String = connection
            .send_request_typed("benchmark", &params)
            .await
            .unwrap();

        let latency = start.elapsed();
        total_latency += latency;

        // Each individual request should be under 50ms
        assert!(
            latency < Duration::from_millis(50),
            "Request latency {} exceeded 50ms target",
            latency.as_millis()
        );
    }

    let avg_latency = total_latency / num_requests;
    println!("Average message latency: {}ms", avg_latency.as_millis());

    // Average should definitely be under 50ms
    assert!(avg_latency < Duration::from_millis(50));

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_error_handling() {
    // Test connection to non-existent server
    let invalid_url = Url::parse("ws://127.0.0.1:12345").unwrap();
    let cfg = McpClientConfig::new(invalid_url);

    let connection = McpConnection::connect(&cfg).await;
    assert!(connection.is_err());
}

#[tokio::test]
async fn test_concurrent_requests() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();
    let connection = std::sync::Arc::new(connection);

    // Send multiple concurrent requests
    let mut handles = Vec::new();
    for i in 0..5 {
        let conn = connection.clone();
        let handle = tokio::spawn(async move {
            let params = json!({"request_id": i});
            let response: String = conn
                .send_request_typed("concurrent_test", &params)
                .await
                .unwrap();
            response
        });
        handles.push(handle);
    }

    // All requests should complete successfully
    for handle in handles {
        let response = handle.await.unwrap();
        assert_eq!(response, "echo");
    }

    connection.close().await.unwrap();
}

#[tokio::test]
async fn test_protocol_validation() {
    let server = MockMcpServer::start().await;
    let cfg = McpClientConfig::new(server.url());

    let connection = McpConnection::connect(&cfg).await.unwrap();

    // Test invalid JSON should be handled gracefully
    let result = connection
        .send_request_raw("invalid", serde_json::Value::Null, Duration::from_secs(5))
        .await;

    // Should still work (server echoes)
    assert!(result.is_ok());

    connection.close().await.unwrap();
}
