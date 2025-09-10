use cg_integration_test_support::{setup_test_server, write_sample_repo};
use serde_json::json;
use serial_test::serial;
use futures::future::join_all;

#[tokio::test]
#[serial]
async fn integrity_and_recovery_endpoints() {
    let ctx = setup_test_server().await.unwrap();
    let res1 = ctx.server.get("/stats/recovery").await;
    assert!(res1.status().is_success());
    let res2 = ctx.server.post("/integrity/check").await;
    assert!(res2.status().is_success());
}

#[tokio::test]
#[serial]
async fn service_registry_register_discover_heartbeat_deregister() {
    let ctx = setup_test_server().await.unwrap();

    // Register
    let reg = ctx
        .server
        .post("/services")
        .json(&json!({
            "service_name": "test-svc",
            "version": "1.0.0",
            "address": "127.0.0.1",
            "port": 8081,
            "tags": ["http"],
            "ttl_seconds": 30
        }))
        .await;
    assert!(reg.status().is_success());
    let body = reg.json::<serde_json::Value>();
    let service_id = body["service_id"].as_str().unwrap().to_string();

    // Heartbeat
    let hb = ctx
        .server
        .post("/services/heartbeat")
        .json(&json!({"service_id": service_id}))
        .await;
    assert!(hb.status().is_success());

    // Discover/list
    let list = ctx.server.get("/services").await;
    assert!(list.status().is_success());
    let discover = ctx.server.get("/services/discover?service_name=test-svc").await;
    assert!(discover.status().is_success());

    // Deregister
    let dereg = ctx.server.delete(&format!("/services/{}", body["service_id"].as_str().unwrap())).await;
    assert!(dereg.status().is_success());
}

#[tokio::test]
#[serial]
async fn concurrent_index_requests_do_not_conflict() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    let path = repo.to_str().unwrap().to_string();

    // Fire a few concurrent indexing requests
    let futs = (0..4).map(|_| {
        ctx.server
            .post("/v1/index")
            .json(&json!({"path": path}))
    });
    let res = join_all(futs).await;
    for r in res {
        assert!(r.status().is_success(), "one of the concurrent index requests failed: {}", r.text());
    }
}

#[tokio::test]
#[serial]
async fn bad_inputs_return_clear_errors() {
    let ctx = setup_test_server().await.unwrap();

    // Missing required fields
    let r1 = ctx.server.post("/v1/index").json(&json!({})).await;
    assert_eq!(r1.status().as_u16(), 400);

    // Invalid UUID formats
    let r2 = ctx.server.get("/v1/node/invalid-uuid").await;
    assert_eq!(r2.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn similar_nodes_invalid_id_400() {
    let ctx = setup_test_server().await.unwrap();
    let res = ctx.server.get("/nodes/not-a-uuid/similar").await;
    assert_eq!(res.status().as_u16(), 400);
}

#[tokio::test]
#[serial]
async fn streaming_metadata_and_http2_metrics_available() {
    let ctx = setup_test_server().await.unwrap();
    let meta = ctx.server.get("/stream/123/metadata").await;
    assert!(meta.status().is_success());
    let m = ctx.server.get("/http2/metrics").await;
    assert!(m.status().is_success());
}
