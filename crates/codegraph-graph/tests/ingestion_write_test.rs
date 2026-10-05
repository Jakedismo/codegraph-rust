// ABOUTME: Exercises repeatable bulk ingestion and project-scoped cleanup against real schemas.
// ABOUTME: Runs offline using embedded SurrealDB, without optional service skips.
#![cfg(feature = "surrealdb")]

use codegraph_core::{CodeNode, EdgeType, Language, Location, NodeType};
use codegraph_graph::{
    ChunkEmbeddingRecord, FileMetadataRecord, SurrealDbConfig, SurrealDbStorage,
    SymbolEmbeddingRecord, edge::CodeEdge,
};
use serde_json::Value;

async fn storage(schema: &str) -> SurrealDbStorage {
    let dir = tempfile::tempdir().unwrap();
    let mut config = SurrealDbConfig::embedded(dir.path());
    config.connection = "mem://".into();
    config.database = format!(
        "ingestion_{}",
        dir.path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace('.', "_")
    );
    config.auto_migrate = false;
    let storage = SurrealDbStorage::new(config).await.unwrap();
    storage
        .db()
        .query(schema.to_string())
        .await
        .unwrap()
        .check()
        .unwrap();
    storage
}

fn node(project: &str, name: &str) -> CodeNode {
    let mut node = CodeNode::new(
        name.to_string(),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location {
            file_path: "shared.rs".into(),
            line: 1,
            column: 1,
            end_line: Some(1),
            end_column: None,
        },
    );
    node.metadata
        .attributes
        .insert("project_id".into(), project.into());
    node.set_deterministic_id(project);
    node
}

async fn count(storage: &SurrealDbStorage, table: &str, project: &str) -> usize {
    let mut response = storage
        .db()
        .query(format!(
            "SELECT count() AS n FROM {table} WHERE project_id = $project GROUP ALL"
        ))
        .bind(("project", project.to_string()))
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    rows.first().and_then(|row| row["n"].as_u64()).unwrap_or(0) as usize
}

#[tokio::test]
async fn bulk_ingestion_is_repeatable_and_cleanup_is_typed_and_scoped() {
    for schema in [
        include_str!("../../../schema/codegraph.surql"),
        include_str!("../../../schema/codegraph_v2.surql"),
    ] {
        let mut store = storage(schema).await;
        for project in ["one", "two"] {
            let nodes = [node(project, "caller"), node(project, "callee")];
            let mut edge = CodeEdge::new(nodes[0].id, nodes[1].id, EdgeType::Calls);
            edge.set_deterministic_id(project);
            let vector = vec![0.1; 384];
            let chunk = ChunkEmbeddingRecord::new(
                &nodes[0].id.to_string(),
                0,
                "fn caller() {}".into(),
                &vector,
                "fixture",
                "embedding_384",
                project,
            );
            let symbol = SymbolEmbeddingRecord::new(
                project,
                None,
                "caller",
                "caller",
                &vector,
                "fixture",
                "embedding_384",
                Some(&nodes[0].id.to_string()),
                None,
                None,
            );
            let file = FileMetadataRecord {
                file_path: "shared.rs".into(),
                project_id: project.into(),
                content_hash: "test".into(),
                modified_at: chrono::Utc::now(),
                file_size: 10,
                last_indexed_at: chrono::Utc::now(),
                node_count: 2,
                edge_count: 1,
                language: Some("rust".into()),
                parse_errors: None,
            };
            for _ in 0..2 {
                store.upsert_nodes_batch(&nodes).await.unwrap();
                store
                    .upsert_edges_batch(std::slice::from_ref(&edge))
                    .await
                    .unwrap();
                store
                    .upsert_chunk_embeddings_batch(std::slice::from_ref(&chunk))
                    .await
                    .unwrap();
                store
                    .upsert_symbol_embeddings_batch(std::slice::from_ref(&symbol))
                    .await
                    .unwrap();
                store
                    .upsert_file_metadata_batch(std::slice::from_ref(&file))
                    .await
                    .unwrap();
            }
            assert_eq!(count(&store, "nodes", project).await, 2);
            for table in ["edges", "chunks", "symbol_embeddings", "file_metadata"] {
                assert_eq!(count(&store, table, project).await, 1, "{table}");
            }
        }
        store
            .delete_data_for_files("one", &["shared.rs".into()])
            .await
            .unwrap();
        for table in [
            "nodes",
            "edges",
            "chunks",
            "symbol_embeddings",
            "file_metadata",
        ] {
            assert_eq!(count(&store, table, "one").await, 0, "{table}");
            assert!(count(&store, table, "two").await > 0, "{table}");
        }
    }
}

#[tokio::test]
async fn statement_errors_reach_ingestion_callers() {
    let mut store = storage(include_str!("../../../schema/codegraph_v2.surql")).await;
    let nodes = [node("test", "a"), node("test", "b")];
    store.upsert_nodes_batch(&nodes).await.unwrap();
    let mut edge = CodeEdge::new(nodes[0].id, nodes[1].id, EdgeType::Calls);
    edge.set_deterministic_id("test");
    store
        .db()
        .query("DEFINE FIELD OVERWRITE weight ON edges TYPE float ASSERT $value >= 0")
        .await
        .unwrap()
        .check()
        .unwrap();
    edge.weight = -1.0;
    assert!(store.upsert_edges_batch(&[edge]).await.is_err());
    let bad = SymbolEmbeddingRecord::new(
        "test",
        None,
        "bad",
        "bad",
        &[0.1; 3],
        "fixture",
        "embedding_384",
        None,
        None,
        None,
    );
    assert!(store.upsert_symbol_embeddings_batch(&[bad]).await.is_err());
}
