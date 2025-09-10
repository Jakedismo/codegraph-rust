use cg_integration_test_support::{setup_test_server, write_sample_repo};
use axum_test::TestServer;
use serde_json::json;
use serial_test::serial;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn rest_search_returns_200_and_json() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    // Index first to have graph populated
    let _ = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": repo.to_str().unwrap()}))
        .await;

    let res = ctx.server.get("/v1/search?q=function").await;
    assert!(res.status().is_success());
    let body = res.json::<serde_json::Value>();
    assert!(body["results"].is_array());
}

#[tokio::test]
#[serial]
async fn rest_get_node_invalid_id_returns_400() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.get("/v1/node/not-a-uuid").await;
    assert_eq!(res.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn rest_get_node_unknown_id_returns_404() {
    let ctx = setup_test_server().await.unwrap();
    let random_id = Uuid::new_v4();
    let res = ctx.server.get(&format!("/v1/node/{}", random_id)).await;
    assert_eq!(res.status().as_u16(), 404);
}

#[tokio::test]
#[serial]
async fn rest_neighbors_unknown_id_returns_404() {
    let ctx = setup_test_server().await.unwrap();
    let random_id = Uuid::new_v4();
    let res = ctx
        .server
        .get(&format!("/v1/graph/neighbors?id={}", random_id))
        .await;
    assert_eq!(res.status().as_u16(), 404);
}

#[tokio::test]
#[serial]
async fn graphql_health_and_version() {
    let ctx = setup_test_server().await.unwrap();
    let q = json!({"query": "{ health version }"});
    let res = ctx.server.post("/graphql").json(&q).await;
    assert!(res.status().is_success());
    let body = res.json::<serde_json::Value>();
    assert_eq!(body["errors"], serde_json::Value::Null);
    let data = &body["data"];
    assert_eq!(data["health"].as_str(), Some("GraphQL API is running"));
    assert_eq!(data["version"].as_str(), Some("1.0.0"));
}

#[tokio::test]
#[serial]
async fn graphql_node_query_returns_null_for_unknown() {
    let ctx = setup_test_server().await.unwrap();
    let id = Uuid::new_v4();
    let q = json!({"query": format!("{{ node(id: \"{}\") {{ id name }} }}", id)});
    let res = ctx.server.post("/graphql").json(&q).await;
    assert!(res.status().is_success());
    let body = res.json::<serde_json::Value>();
    assert_eq!(body["errors"], serde_json::Value::Null);
    assert_eq!(body["data"]["node"], serde_json::Value::Null);
}

#[tokio::test]
#[serial]
async fn vector_index_endpoints_are_accessible() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.get("/vector/index/stats").await;
    assert!(res.status().is_success());
    let res2 = ctx.server.get("/vector/index/config").await;
    assert!(res2.status().is_success());
}

#[tokio::test]
#[serial]
async fn vector_search_requires_embedding() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx
        .server
        .post("/vector/search")
        .json(&json!({"query_embedding": [], "k": 5}))
        .await;
    assert_eq!(res.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn streaming_endpoints_are_reachable() {
    let ctx = setup_test_server().await.unwrap();
    let res1 = ctx.server.get("/stream/stats").await;
    assert!(res1.status().is_success());
    let res2 = ctx.server.get("/stream/search?query=hello&limit=10").await;
    assert!(res2.status().is_success());
    let res3 = ctx.server.get("/stream/csv?query=hello&limit=10").await;
    assert!(res3.status().is_success());
}

#[tokio::test]
#[serial]
async fn concurrent_search_requests_succeed() {
    let ctx = setup_test_server().await.unwrap();
    let futs = (0..8).map(|i| ctx.server.get(&format!("/v1/search?q=item{}", i)));
    let results = futures::future::join_all(futs).await;
    for r in results { assert!(r.status().is_success()); }
}
