use codegraph_mcp::*;
use serde_json::json;
use std::time::Instant;

#[tokio::test]
async fn benchmark_message_serialization() {
    let iterations = 1000;
    let mut total_duration = std::time::Duration::ZERO;

    println!(
        "Running message serialization benchmark with {} iterations",
        iterations
    );

    for i in 0..iterations {
        let start = Instant::now();

        // Create a complex request message
        let params = json!({
            "method": "complex_operation",
            "data": {
                "files": vec![
                    format!("file_{}.txt", i),
                    format!("config_{}.json", i),
                    format!("metadata_{}.xml", i),
                ],
                "options": {
                    "recursive": true,
                    "follow_symlinks": false,
                    "max_depth": 10,
                    "patterns": ["*.rs", "*.js", "*.ts", "*.py"]
                },
                "context": {
                    "session_id": format!("session_{}", i),
                    "user_id": format!("user_{}", i % 100),
                    "workspace": format!("/workspace/project_{}", i % 50)
                }
            }
        });

        // Create request
        let request = JsonRpcRequest::new(json!(i), "complex_operation".to_string(), Some(params));
        let message = JsonRpcMessage::V2(JsonRpcV2Message::Request(request));

        // Serialize
        let serialized = serde_json::to_string(&message).unwrap();

        // Deserialize back
        let _deserialized: JsonRpcMessage = serde_json::from_str(&serialized).unwrap();

        total_duration += start.elapsed();
    }

    let avg_duration = total_duration / iterations as u32;
    println!(
        "Average message serialization/deserialization time: {}μs",
        avg_duration.as_micros()
    );

    // Assert that average time is well under our 50ms target (should be microseconds, not milliseconds)
    assert!(
        avg_duration.as_millis() < 1,
        "Message processing should be under 1ms, got {}μs",
        avg_duration.as_micros()
    );
}

#[tokio::test]
async fn benchmark_protocol_operations() {
    let iterations = 1000;
    let protocol = McpProtocol::default();

    println!(
        "Running protocol operations benchmark with {} iterations",
        iterations
    );

    let start = Instant::now();

    for i in 0..iterations {
        let params = json!({
            "operation_id": i,
            "data": format!("test_data_{}", i)
        });

        // Build request
        let _request = protocol.build_request("test_method", &params).unwrap();

        // Build notification
        let _notification = protocol
            .build_notification("test_notification", &params)
            .unwrap();
    }

    let total_duration = start.elapsed();
    let avg_duration = total_duration / iterations as u32;

    println!(
        "Average protocol operation time: {}μs",
        avg_duration.as_micros()
    );

    // Should be very fast - well under 1ms per operation
    assert!(
        avg_duration.as_millis() < 1,
        "Protocol operations should be under 1ms, got {}μs",
        avg_duration.as_micros()
    );
}

#[tokio::test]
async fn benchmark_version_negotiation() {
    let iterations = 10000; // More iterations since this should be very fast
    let negotiator = VersionNegotiator::new();

    println!(
        "Running version negotiation benchmark with {} iterations",
        iterations
    );

    let start = Instant::now();

    for i in 0..iterations {
        let version = if i % 2 == 0 {
            "2025-03-26"
        } else {
            "2024-11-05"
        };
        let _result = negotiator.negotiate(version).unwrap();
    }

    let total_duration = start.elapsed();
    let avg_duration = total_duration / iterations as u32;

    println!(
        "Average version negotiation time: {}ns",
        avg_duration.as_nanos()
    );

    // Version negotiation should be extremely fast - nanoseconds
    assert!(
        avg_duration.as_micros() < 100,
        "Version negotiation should be under 100μs, got {}ns",
        avg_duration.as_nanos()
    );
}

#[test]
fn benchmark_heartbeat_creation() {
    let iterations = 1000;

    println!(
        "Running heartbeat creation benchmark with {} iterations",
        iterations
    );

    let start = Instant::now();

    for _i in 0..iterations {
        let config = HeartbeatConfig::default();
        let _manager = HeartbeatManager::with_config(config);
    }

    let total_duration = start.elapsed();
    let avg_duration = total_duration / iterations as u32;

    println!(
        "Average heartbeat manager creation time: {}μs",
        avg_duration.as_micros()
    );

    // Heartbeat creation should be fast
    assert!(
        avg_duration.as_millis() < 1,
        "Heartbeat creation should be under 1ms, got {}μs",
        avg_duration.as_micros()
    );
}

#[test]
fn benchmark_error_creation() {
    let iterations = 10000;

    println!(
        "Running error creation benchmark with {} iterations",
        iterations
    );

    let start = Instant::now();

    for i in 0..iterations {
        let _error = McpError::RequestTimeout(format!("method_{}", i));
    }

    let total_duration = start.elapsed();
    let avg_duration = total_duration / iterations as u32;

    println!("Average error creation time: {}ns", avg_duration.as_nanos());

    // Error creation should be very fast
    assert!(
        avg_duration.as_micros() < 10,
        "Error creation should be under 10μs, got {}ns",
        avg_duration.as_nanos()
    );
}

// Performance test to verify overall system responsiveness
#[tokio::test]
async fn performance_system_responsiveness() {
    println!("Running system responsiveness test");

    // Test various operations in sequence to ensure system remains responsive
    let start = Instant::now();

    // Create protocol
    let protocol = McpProtocol::default();

    // Create heartbeat manager
    let config = HeartbeatConfig::default();
    let _heartbeat = HeartbeatManager::with_config(config);

    // Create version negotiator
    let negotiator = VersionNegotiator::new();
    let _version = negotiator.negotiate("2025-03-26").unwrap();

    // Create complex message
    let params = json!({
        "complex_data": {
            "arrays": vec![1, 2, 3, 4, 5],
            "nested": {
                "deep": {
                    "values": ["a", "b", "c"]
                }
            },
            "metadata": {
                "timestamp": 1234567890,
                "version": "1.0.0"
            }
        }
    });

    let request = protocol
        .build_request("complex_operation", &params)
        .unwrap();
    let message = JsonRpcMessage::V2(JsonRpcV2Message::Request(request));
    let serialized = serde_json::to_string(&message).unwrap();
    let _deserialized: JsonRpcMessage = serde_json::from_str(&serialized).unwrap();

    let total_duration = start.elapsed();

    println!(
        "Total system responsiveness test duration: {}μs",
        total_duration.as_micros()
    );

    // Entire sequence should complete in well under 1ms
    assert!(
        total_duration.as_millis() < 1,
        "System responsiveness test should complete under 1ms, got {}μs",
        total_duration.as_micros()
    );
}
