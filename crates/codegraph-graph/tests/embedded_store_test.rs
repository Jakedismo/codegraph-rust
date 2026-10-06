// ABOUTME: Opens a project-local embedded SurrealKV store through SurrealDbStorage.
// ABOUTME: Checks the bundled schema is applied once and the handle is shared in-process.
#![cfg(feature = "surrealdb")]

use codegraph_graph::{SurrealDbConfig, SurrealDbStorage, is_embedded_connection};
use serde_json::Value;

#[tokio::test]
async fn embedded_store_applies_schema_once_and_shares_handle() {
    let dir = tempfile::tempdir().unwrap();
    let config = SurrealDbConfig::embedded(dir.path());
    assert!(config.is_embedded(), "{}", config.connection);
    assert!(config.connection.starts_with("surrealkv://"));

    let first = SurrealDbStorage::new(config.clone()).await.unwrap();
    assert!(dir.path().join(".codegraph").join("db").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".codegraph").join(".gitignore")).unwrap(),
        "*\n"
    );

    // The bundled schema defines the nodes table and records itself.
    let mut response = first
        .db()
        .query("SELECT name, version FROM schema_versions:bundled")
        .await
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "v2");
    assert_eq!(rows[0]["version"], 0);

    // A second open of the same store from this process reuses the engine
    // instead of failing on the directory lock, and does not reapply the schema.
    let second = SurrealDbStorage::new(config).await.unwrap();
    let mut response = second
        .db()
        .query("SELECT count() AS n FROM schema_versions GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    assert_eq!(rows[0]["n"], 1);
}

#[tokio::test]
async fn reopening_refreshes_functions_from_an_older_schema_revision() {
    let dir = tempfile::tempdir().unwrap();
    let config = SurrealDbConfig::embedded(dir.path());
    let first = SurrealDbStorage::new(config.clone()).await.unwrap();

    // Simulate a store created by an earlier binary: a stale function body and checksum.
    first
        .db()
        .query(
            "DEFINE FUNCTION OVERWRITE fn::edge_types() { RETURN ['stale']; } PERMISSIONS FULL; \
             UPDATE schema_versions:bundled SET checksum = 'older-revision'; \
             CREATE nodes:keep SET name = 'kept', project_id = 'p';",
        )
        .await
        .unwrap()
        .check()
        .unwrap();

    let reopened = SurrealDbStorage::new(config).await.unwrap();
    let mut response = reopened
        .db()
        .query(
            "RETURN { types: array::len(fn::edge_types()), \
             checksum: (SELECT VALUE checksum FROM ONLY schema_versions:bundled), \
             kept: (SELECT VALUE name FROM ONLY nodes:keep), \
             versions: (SELECT count() AS n FROM schema_versions GROUP ALL)[0].n }",
        )
        .await
        .unwrap();
    let state: Option<Value> = response.take(0).unwrap();
    let state = state.unwrap();
    assert_eq!(state["types"], 21, "function body restored: {state}");
    assert_ne!(
        state["checksum"], "older-revision",
        "checksum updated: {state}"
    );
    assert_eq!(state["kept"], "kept", "data untouched: {state}");
    assert_eq!(state["versions"], 1);
}

#[test]
fn connection_scheme_classification() {
    assert!(is_embedded_connection("surrealkv:///tmp/x"));
    assert!(is_embedded_connection("mem://"));
    assert!(!is_embedded_connection("ws://localhost:3004"));
    assert!(!is_embedded_connection("wss://cloud.surrealdb.com"));
}
