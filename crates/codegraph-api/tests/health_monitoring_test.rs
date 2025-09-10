use axum_test::TestServer;
use codegraph_api::{create_router, AppState};
use codegraph_core::ConfigManager;
use std::sync::Arc;

#[tokio::test]
async fn test_health_endpoint() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    // Test basic health endpoint
    let response = server.get("/health").await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("status").is_some());
    assert!(body.get("version").is_some());
    assert!(body.get("components").is_some());
    assert!(body.get("metrics").is_some());
}

#[tokio::test]
async fn test_liveness_endpoint() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    let response = server.get("/health/live").await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert_eq!(body.get("status").unwrap(), "alive");
    assert!(body.get("timestamp").is_some());
    assert!(body.get("uptime_seconds").is_some());
}

#[tokio::test]
async fn test_readiness_endpoint() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    let response = server.get("/health/ready").await;
    // Readiness might fail if dependencies aren't ready, so we check for valid responses
    assert!(response.status_code() == 200 || response.status_code() == 503);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("status").is_some());
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    let response = server.get("/metrics").await;
    assert_eq!(response.status_code(), 200);
    
    let body = response.text();
    // Check for some standard Prometheus metrics
    assert!(body.contains("# HELP"));
    assert!(body.contains("# TYPE"));
}

#[tokio::test]
async fn test_service_registry_endpoints() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    // Test service registration
    let registration_payload = serde_json::json!({
        "service_name": "test-service",
        "version": "1.0.0",
        "address": "127.0.0.1",
        "port": 8080,
        "tags": ["http", "api"],
        "health_check_url": "http://127.0.0.1:8080/health",
        "ttl_seconds": 60
    });

    let response = server
        .post("/services")
        .json(&registration_payload)
        .await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("service_id").is_some());
    let service_id = body.get("service_id").unwrap().as_str().unwrap();

    // Test service listing
    let response = server.get("/services").await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("services").is_some());
    assert!(body.get("total").is_some());

    // Test service discovery
    let response = server
        .get("/services/discover?service_name=test-service")
        .await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    let services = body.get("services").unwrap().as_array().unwrap();
    assert_eq!(services.len(), 1);

    // Test getting specific service
    let response = server
        .get(&format!("/services/{}", service_id))
        .await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert_eq!(body.get("service_name").unwrap(), "test-service");

    // Test heartbeat
    let heartbeat_payload = serde_json::json!({
        "service_id": service_id
    });

    let response = server
        .post("/services/heartbeat")
        .json(&heartbeat_payload)
        .await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert_eq!(body.get("success").unwrap(), true);

    // Test service deregistration
    let response = server
        .delete(&format!("/services/{}", service_id))
        .await;
    assert_eq!(response.status_code(), 200);
}

#[tokio::test]
async fn test_health_components() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    let response = server.get("/health").await;
    let body: serde_json::Value = response.json();
    
    let components = body.get("components").unwrap();
    
    // Check that all required components are present
    assert!(components.get("database").is_some());
    assert!(components.get("vector_search").is_some());
    assert!(components.get("parser").is_some());
    assert!(components.get("memory").is_some());
    assert!(components.get("storage").is_some());
    
    // Check component structure
    let database = components.get("database").unwrap();
    assert!(database.get("status").is_some());
    assert!(database.get("last_check").is_some());
}

#[tokio::test]
async fn test_metrics_integration() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    // Make a few requests to generate metrics
    let _ = server.get("/health").await;
    let _ = server.get("/health/live").await;
    let _ = server.get("/health/ready").await;

    let response = server.get("/metrics").await;
    let body = response.text();
    
    // Check for HTTP metrics
    assert!(body.contains("http_requests_total"));
    assert!(body.contains("http_request_duration_seconds"));
    
    // Check for system metrics
    assert!(body.contains("system_cpu_usage_percent"));
    assert!(body.contains("system_memory_usage_bytes"));
    
    // Check for application metrics
    assert!(body.contains("application_uptime_seconds"));
    assert!(body.contains("build_info"));
}

#[cfg(feature = "leak-detect")]
#[tokio::test]
async fn test_memory_leak_detection() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    // Test memory stats endpoint
    let response = server.get("/memory/stats").await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("total_allocations").is_some());
    assert!(body.get("active_allocations").is_some());
    assert!(body.get("leaked_allocations").is_some());

    // Test leak report export
    let response = server.get("/memory/leaks").await;
    assert_eq!(response.status_code(), 200);
    
    let body: serde_json::Value = response.json();
    assert!(body.get("exported").is_some());
    assert!(body.get("path").is_some());
}

#[tokio::test]
async fn test_concurrent_health_checks() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("Failed to create app state");
    let app = create_router(state);
    let server = TestServer::new(app).expect("Failed to create test server");

    // Run multiple health checks concurrently
    let tasks = (0..10).map(|_| {
        let server = &server;
        async move {
            let response = server.get("/health").await;
            assert_eq!(response.status_code(), 200);
            response
        }
    });

    let results = futures::future::join_all(tasks).await;
    assert_eq!(results.len(), 10);
    
    // All health checks should succeed
    for result in results {
        let body: serde_json::Value = result.json();
        assert!(body.get("status").is_some());
    }
}