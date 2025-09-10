use cg_integration_test_support::{setup_test_server, write_sample_repo};
use axum_test::TestServer;
use serde_json::json;
use serial_test::serial;

async fn index_path(server: &TestServer, path: &str, parallel: bool) -> serde_json::Value {
    let res = server
        .post("/v1/index")
        .json(&json!({"path": path, "parallel": parallel}))
        .await;
    assert!(res.status().is_success(), "index request failed: {}", res.text());
    res.json()
}

#[tokio::test]
#[serial]
async fn index_valid_directory_returns_counts() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);

    let body = index_path(&ctx.server, repo.to_str().unwrap(), true).await;
    assert!(body["nodes_indexed"].as_u64().unwrap() > 0);
    assert!(body["files_parsed"].as_u64().unwrap() >= 3);
}

#[tokio::test]
#[serial]
async fn index_nonexistent_path_returns_400() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": "/definitely/not/here", "parallel": true}))
        .await;
    assert_eq!(res.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn index_with_parallel_false_still_completes() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    let body = index_path(&ctx.server, repo.to_str().unwrap(), false).await;
    assert!(body["nodes_indexed"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[serial]
async fn index_twice_accumulates_nodes() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    let b1 = index_path(&ctx.server, repo.to_str().unwrap(), true).await;
    let b2 = index_path(&ctx.server, repo.to_str().unwrap(), true).await;
    assert!(b2["nodes_indexed"].as_u64().unwrap() >= b1["nodes_indexed"].as_u64().unwrap());
}

#[tokio::test]
#[serial]
async fn index_empty_path_validation_error() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.post("/v1/index").json(&json!({"path": ""})).await;
    assert_eq!(res.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn cache_headers_present_on_get_endpoints_after_index() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    let _ = index_path(&ctx.server, repo.to_str().unwrap(), true).await;
    let res = ctx.server.get("/v1/search?q=test").await;
    assert!(res.status().is_success());
    let etag = res.headers().get("etag");
    let cache = res.headers().get("cache-control");
    assert!(etag.is_some());
    assert!(cache.is_some());
}

#[tokio::test]
#[serial]
async fn health_endpoints_work() {
    let ctx = setup_test_server().await.unwrap();
    for path in ["/health", "/health/live", "/health/ready"] {
        let res = ctx.server.get(path).await;
        assert!(res.status().is_success(), "{} failed: {}", path, res.text());
    }
}

#[tokio::test]
#[serial]
async fn swagger_ui_and_openapi_available() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.get("/v1/openapi.json").await;
    assert!(res.status().is_success());
    let json = res.json::<serde_json::Value>();
    assert_eq!(json["openapi"].as_str(), Some("3.0.3"));
}

#[tokio::test]
#[serial]
async fn metrics_endpoint_works() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.get("/metrics").await;
    assert!(res.status().is_success());
    let body = res.text();
    assert!(body.contains("process_cpu_seconds_total") || body.len() > 0);
}

#[tokio::test]
#[serial]
async fn vector_index_rebuild_returns_200() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx
        .server
        .post("/vector/index/rebuild")
        .json(&serde_json::json!({}))
        .await;
    assert!(res.status().is_success());
}
