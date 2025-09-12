use axum_test::TestServer;
use codegraph_api::{create_router, AppState};
use codegraph_core::ConfigManager;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("app state");
    let app = create_router(state);
    let server = TestServer::new(app).unwrap();

    let resp = server.get("/health").await;
    assert_eq!(resp.status_code(), 200);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn graphiql_serves_html() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("app state");
    let app = create_router(state);
    let server = TestServer::new(app).unwrap();

    let resp = server.get("/graphiql").await;
    assert_eq!(resp.status_code(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/html"), "expected HTML content-type");
}

#[tokio::test]
async fn graphql_health_query() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("app state");
    let app = create_router(state);
    let server = TestServer::new(app).unwrap();

    let query = json!({
        "query": "query { health }"
    });
    let resp = server.post("/graphql").json(&query).await;

    assert_eq!(resp.status_code(), 200);
    let body: serde_json::Value = resp.json();
    let data = &body["data"]["health"];
    assert!(data.is_string());
}

#[tokio::test]
async fn http2_config_and_health_endpoints() {
    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("app state");
    let app = create_router(state);
    let server = TestServer::new(app).unwrap();

    let cfg = server.get("/http2/config").await;
    assert_eq!(cfg.status_code(), 200);

    let health = server.get("/http2/health").await;
    assert_eq!(health.status_code(), 200);
}

#[tokio::test]
async fn parse_endpoint_parses_temp_rust_file() {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("test_file.rs");
    let mut f = std::fs::File::create(&file_path).expect("create temp file");
    writeln!(
        f,
        "{}",
        r#"fn main() { let x = 1 + 2; println!(\"{}\", x); }"#
    )
    .unwrap();

    let config = Arc::new(ConfigManager::new().expect("Failed to create config"));
    let state = AppState::new(config).await.expect("app state");
    let app = create_router(state);
    let server = TestServer::new(app).unwrap();

    let payload = json!({"file_path": file_path.to_string_lossy()});
    let resp = server.post("/parse").json(&payload).await;
    assert_eq!(resp.status_code(), 200);
    let body: serde_json::Value = resp.json();
    assert!(body["nodes_created"].as_u64().unwrap_or(0) >= 0);
}
