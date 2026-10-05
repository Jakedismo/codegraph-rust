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
    assert_eq!(rows[0]["name"], "main");
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

#[test]
fn connection_scheme_classification() {
    assert!(is_embedded_connection("surrealkv:///tmp/x"));
    assert!(is_embedded_connection("mem://"));
    assert!(!is_embedded_connection("ws://localhost:3004"));
    assert!(!is_embedded_connection("wss://cloud.surrealdb.com"));
}
