use axum_test::TestServer;
use cg_integration_test_support::{setup_test_server, write_sample_repo};
use serde_json::json;
use serial_test::serial;
use std::fs;

#[tokio::test]
#[serial]
async fn adding_new_file_and_reindex_updates_counts() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);

    let res1 = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": repo.to_str().unwrap()}))
        .await;
    let b1 = res1.json::<serde_json::Value>();

    // Add a new source file
    fs::write(
        repo.join("new.rs"),
        "pub fn sub(a: i32, b: i32) -> i32 { a - b }\n",
    )
    .unwrap();

    let res2 = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": repo.to_str().unwrap()}))
        .await;
    let b2 = res2.json::<serde_json::Value>();

    assert!(b2["files_parsed"].as_u64().unwrap() >= b1["files_parsed"].as_u64().unwrap());
    assert!(b2["nodes_indexed"].as_u64().unwrap() >= b1["nodes_indexed"].as_u64().unwrap());
}

#[tokio::test]
#[serial]
async fn modifying_file_then_reindex_still_succeeds() {
    let ctx = setup_test_server().await.unwrap();
    let repo = write_sample_repo(&ctx.tmpdir);
    let _ = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": repo.to_str().unwrap()}))
        .await;

    // Modify content
    let lib = repo.join("lib.rs");
    fs::write(&lib, "pub fn add(a:i32,b:i32)->i32{a+b}//changed\n").unwrap();

    let res = ctx
        .server
        .post("/v1/index")
        .json(&json!({"path": repo.to_str().unwrap()}))
        .await;
    assert!(res.status().is_success());
}

#[tokio::test]
#[serial]
async fn etag_is_consistent_for_same_query_and_changes_with_query() {
    let ctx = setup_test_server().await.unwrap();
    let e1 = ctx
        .server
        .get("/v1/search?q=a")
        .await
        .headers()
        .get("etag")
        .cloned();
    let e2 = ctx
        .server
        .get("/v1/search?q=a")
        .await
        .headers()
        .get("etag")
        .cloned();
    assert_eq!(e1, e2);
    let e3 = ctx
        .server
        .get("/v1/search?q=b")
        .await
        .headers()
        .get("etag")
        .cloned();
    assert_ne!(e1, e3);
}

#[tokio::test]
#[serial]
async fn http2_config_roundtrip() {
    let ctx = setup_test_server().await.unwrap();
    let get_res = ctx.server.get("/http2/config").await;
    assert!(get_res.status().is_success());

    let update = ctx
        .server
        .post("/http2/config")
        .json(&json!({"max_concurrent_streams": 128}))
        .await;
    assert!(update.status().is_success());
}

#[tokio::test]
#[serial]
async fn versioning_endpoints_basic_flow() {
    let ctx = setup_test_server().await.unwrap();

    // Begin transaction
    let tx = ctx
        .server
        .post("/transactions")
        .json(&json!({"isolation_level": "read_committed"}))
        .await;
    assert!(tx.status().is_success());
    let tx_id = tx.json::<serde_json::Value>()["transaction_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Commit transaction
    let commit = ctx
        .server
        .post(&format!("/transactions/{}/commit", tx_id))
        .await;
    assert!(commit.status().is_success());

    // Create version
    let ver = ctx
        .server
        .post("/versions")
        .json(&json!({
            "name": "v1",
            "description": "test version",
            "author": "tester",
            "parent_versions": []
        }))
        .await;
    assert!(ver.status().is_success());

    // List versions
    let list = ctx.server.get("/versions").await;
    assert!(list.status().is_success());

    // Create and list branches
    let br = ctx
        .server
        .post("/branches")
        .json(&json!({
            "name": "main",
            "from_version": "root",
            "author": "tester"
        }))
        .await;
    assert!(br.status().is_success());
    let br_list = ctx.server.get("/branches").await;
    assert!(br_list.status().is_success());
}
