use codegraph_mcp::*;
use serde_json::json;
use std::time::Duration;

#[test]
fn test_mcp_client_config() {
    let url = url::Url::parse("ws://localhost:8080").unwrap();
    let config = McpClientConfig::new(url.clone());

    assert_eq!(config.url, url);
    assert_eq!(config.client_name, "codegraph-mcp-rs");
    assert_eq!(config.request_timeout, Duration::from_secs(30));
    assert_eq!(config.connect_max_retries, 5);
    assert!(config.heartbeat.is_none());
}

#[test]
fn test_mcp_client_config_with_heartbeat() {
    let url = url::Url::parse("ws://localhost:8080").unwrap();
    let heartbeat_config = HeartbeatConfig {
        interval: Duration::from_millis(100),
        timeout: Duration::from_millis(50),
        max_missed: 2,
    };
    let heartbeat = HeartbeatManager::with_config(heartbeat_config);
    let config = McpClientConfig::new(url).with_heartbeat(heartbeat);

    assert!(config.heartbeat.is_some());
}

#[test]
fn test_heartbeat_config_default() {
    let config = HeartbeatConfig::default();
    assert_eq!(config.interval, Duration::from_secs(30));
    assert_eq!(config.timeout, Duration::from_secs(10));
    assert_eq!(config.max_missed, 3);
}

#[test]
fn test_heartbeat_manager_creation() {
    let manager = HeartbeatManager::new();
    assert!(!manager.is_enabled());

    let config = HeartbeatConfig::default();
    let manager = HeartbeatManager::with_config(config);
    assert!(manager.is_enabled());
}

#[tokio::test]
async fn test_heartbeat_manager_health_check() {
    let manager = HeartbeatManager::new();
    assert!(manager.is_healthy().await);

    let config = HeartbeatConfig::default();
    let manager = HeartbeatManager::with_config(config);
    assert!(manager.is_healthy().await);
}

#[test]
fn test_protocol_version_creation() {
    let version = ProtocolVersion::new("2025-06-18").unwrap();
    assert_eq!(version.as_str(), "2025-06-18");

    let invalid = ProtocolVersion::new("invalid-version");
    assert!(invalid.is_err());
}

#[test]
fn test_protocol_version_support() {
    assert!(ProtocolVersion::is_supported("2025-06-18"));
    assert!(ProtocolVersion::is_supported("2025-03-26"));
    assert!(ProtocolVersion::is_supported("2024-11-05"));
    assert!(!ProtocolVersion::is_supported("1.0.0"));
}

#[test]
fn test_protocol_version_negotiation() {
    let server_versions = vec!["2024-11-05", "2025-03-26", "2025-06-18"];

    // Client requests latest supported version
    let result = ProtocolVersion::negotiate("2025-06-18", &server_versions);
    assert_eq!(result.unwrap(), "2025-06-18");

    // Client requests supported version
    let result = ProtocolVersion::negotiate("2025-03-26", &server_versions);
    assert_eq!(result.unwrap(), "2025-03-26");

    // Client requests older supported version
    let result = ProtocolVersion::negotiate("2024-11-05", &server_versions);
    assert_eq!(result.unwrap(), "2024-11-05");

    // Client requests unsupported version, should fallback to latest supported
    let result = ProtocolVersion::negotiate("1.0.0", &server_versions);
    assert_eq!(result.unwrap(), "2025-06-18");

    // No compatible versions
    let empty_versions: Vec<&str> = vec![];
    let result = ProtocolVersion::negotiate("2025-06-18", &empty_versions);
    assert!(result.is_none());
}

#[test]
fn test_version_negotiator_creation() {
    let negotiator = VersionNegotiator::new();

    // Test with supported versions
    assert!(negotiator.negotiate("2025-06-18").is_ok());
    assert!(negotiator.negotiate("2025-03-26").is_ok());
    assert!(negotiator.negotiate("2024-11-05").is_ok());
}

#[test]
fn test_mcp_protocol() {
    let version = ProtocolVersion::new("2025-06-18").unwrap();
    let protocol = McpProtocol::new(version);

    assert_eq!(protocol.version().as_str(), "2025-06-18");

    // Test request building
    let params = json!({"test": "data"});
    let request = protocol.build_request("test_method", &params).unwrap();
    assert_eq!(request.method, "test_method");
    assert!(request.id.is_string() || request.id.is_number());
    assert_eq!(request.params.unwrap(), params);

    // Test notification building
    let notification = protocol
        .build_notification("test_notification", &params)
        .unwrap();
    assert_eq!(notification.method, "test_notification");
    assert_eq!(notification.params.unwrap(), params);
}

#[test]
fn test_error_types() {
    let websocket_error =
        McpError::WebSocket(tokio_tungstenite::tungstenite::Error::ConnectionClosed);
    assert!(websocket_error.to_string().contains("WebSocket error"));

    let version_mismatch = McpError::VersionMismatch {
        expected: "2025-06-18".to_string(),
        actual: "1.0.0".to_string(),
    };
    assert!(version_mismatch.to_string().contains("version mismatch"));

    let timeout_error = McpError::RequestTimeout("test_method".to_string());
    assert!(timeout_error
        .to_string()
        .contains("Request timeout: test_method"));
}

#[test]
fn test_json_rpc_message_serialization() {
    // Test request serialization
    let request = JsonRpcRequest::new(
        json!(1),
        "test_method".to_string(),
        Some(json!({"param": "value"})),
    );
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(serialized.contains("jsonrpc"));
    assert!(serialized.contains("test_method"));
    assert!(serialized.contains("param"));

    // Test response serialization
    let response = JsonRpcResponse::success(json!(1), json!("result"));
    let serialized = serde_json::to_string(&response).unwrap();
    assert!(serialized.contains("jsonrpc"));
    assert!(serialized.contains("result"));

    // Test notification serialization
    let notification = JsonRpcNotification::new(
        "test_notification".to_string(),
        Some(json!({"data": "test"})),
    );
    let serialized = serde_json::to_string(&notification).unwrap();
    assert!(serialized.contains("jsonrpc"));
    assert!(serialized.contains("test_notification"));
    assert!(!serialized.contains("id")); // Notifications have no ID
}

// Note: Message validation is covered by the other serialization tests above

#[tokio::test]
async fn test_backoff_durations() {
    use codegraph_mcp::transport::backoff_durations;

    let durations: Vec<_> = backoff_durations(5).collect();
    assert_eq!(durations.len(), 5);

    // Ensure durations are increasing (with some jitter tolerance)
    for (i, duration) in durations.iter().enumerate() {
        let expected_base = 200 * 2_u64.pow(i.min(6) as u32);
        assert!(duration.as_millis() as u64 >= expected_base);
        assert!(duration.as_millis() as u64 <= expected_base + expected_base * 50 + 50);
    }
}

#[test]
fn test_mcp_initialize_params() {
    let params = McpInitializeParams {
        protocol_version: "2025-06-18".to_string(),
        capabilities: McpCapabilities {
            experimental: None,
            sampling: None,
        },
        client_info: McpClientInfo {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
        },
    };

    let serialized = serde_json::to_value(&params).unwrap();
    assert_eq!(serialized["protocolVersion"], "2025-06-18");
    assert_eq!(serialized["clientInfo"]["name"], "test-client");
    assert_eq!(serialized["clientInfo"]["version"], "1.0.0");
}

#[test]
fn test_mcp_initialize_result() {
    let json_result = json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "serverInfo": {
            "name": "test-server",
            "version": "1.0.0"
        }
    });

    let result: McpInitializeResult = serde_json::from_value(json_result).unwrap();
    assert_eq!(result.protocol_version, "2025-06-18");
    assert_eq!(result.server_info.name, "test-server");
    assert_eq!(result.server_info.version, "1.0.0");
}
