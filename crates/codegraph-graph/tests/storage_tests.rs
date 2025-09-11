use codegraph_core::{CodeNode, GraphStore, Language, Location, NodeType, Result};
use codegraph_graph::{BulkInsertStats, HighPerformanceRocksDbStorage, SerializableEdge};
use tempfile::TempDir;
use uuid::Uuid;

fn make_node_named(name: &str) -> CodeNode {
    CodeNode::new(
        name.to_string(),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location {
            file_path: "dummy.rs".to_string(),
            line: 1,
            column: 1,
            end_line: Some(1),
            end_column: Some(10),
        },
    )
}

fn make_edge(from: Uuid, to: Uuid, id: u64) -> SerializableEdge {
    use std::collections::HashMap;
    SerializableEdge {
        id,
        from,
        to,
        edge_type: "ref".to_string(),
        weight: 1.0,
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn test_cf_initialization_four_families() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let cfs = storage.list_cf_names().unwrap();
    assert!(cfs.contains(&"default".to_string()) || cfs.contains(&"nodes".to_string()));
    assert!(cfs.contains(&"nodes".to_string()));
    assert!(cfs.contains(&"edges".to_string()));
    assert!(cfs.contains(&"metadata".to_string()));
    assert!(cfs.contains(&"indices".to_string()));
    assert_eq!(
        cfs.iter()
            .filter(|n| *n == "nodes" || *n == "edges" || *n == "metadata" || *n == "indices")
            .count(),
        4
    );
}

#[tokio::test]
async fn test_add_get_node() {
    let tmp = TempDir::new().unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let node = make_node_named("alpha");
    let id = node.id;
    storage.add_node(node).await.unwrap();
    let got = storage.get_node(id).await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().name, "alpha");
}

#[tokio::test]
async fn test_find_nodes_by_name() {
    let tmp = TempDir::new().unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    storage.add_node(make_node_named("a")).await.unwrap();
    storage.add_node(make_node_named("b")).await.unwrap();
    storage.add_node(make_node_named("a")).await.unwrap();
    let a_nodes = storage.find_nodes_by_name("a").await.unwrap();
    assert!(a_nodes.len() >= 2);
}

#[tokio::test]
async fn test_remove_node() {
    let tmp = TempDir::new().unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let node = make_node_named("remove_me");
    let id = node.id;
    storage.add_node(node).await.unwrap();
    storage.remove_node(id).await.unwrap();
    assert!(storage.get_node(id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_add_get_edges_from() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let from = Uuid::new_v4();
    let to1 = Uuid::new_v4();
    let to2 = Uuid::new_v4();
    storage.add_edge(make_edge(from, to1, 1)).await.unwrap();
    storage.add_edge(make_edge(from, to2, 2)).await.unwrap();
    storage.flush_batch_writes().unwrap();
    let edges = storage.get_edges_from(from).await.unwrap();
    assert!(edges.iter().any(|e| e.to == to1));
    assert!(edges.iter().any(|e| e.to == to2));
}

#[tokio::test]
async fn test_transaction_commit_node() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let tx = storage.begin();
    let node = make_node_named("tx_node");
    let id = node.id;
    storage.add_node_tx(tx, node).await.unwrap();
    // Not visible until commit
    assert!(storage.get_node(id).await.unwrap().is_none());
    storage.commit(tx).unwrap();
    assert!(storage.get_node(id).await.unwrap().is_some());
}

#[tokio::test]
async fn test_transaction_rollback_node() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let tx = storage.begin();
    let node = make_node_named("tx_rollback");
    let id = node.id;
    storage.add_node_tx(tx, node).await.unwrap();
    storage.rollback(tx).unwrap();
    assert!(storage.get_node(id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_transaction_commit_edge() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let tx = storage.begin();
    storage
        .add_edge_tx(tx, make_edge(from, to, 42))
        .await
        .unwrap();
    // before commit, should not be visible
    assert!(storage.get_edges_from(from).await.unwrap().is_empty());
    storage.commit(tx).unwrap();
    let edges = storage.get_edges_from(from).await.unwrap();
    assert!(edges.iter().any(|e| e.id == 42));
}

#[tokio::test]
async fn test_bulk_insert_nodes_chunksize() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let mut nodes = Vec::new();
    for i in 0..1500 {
        nodes.push(make_node_named(&format!("n{}", i)));
    }
    let stats = storage.bulk_insert_nodes(nodes).await.unwrap();
    assert!(stats.batches >= 2);
}

#[tokio::test]
async fn test_bulk_insert_edges_chunksize() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let from = Uuid::new_v4();
    let mut edges = Vec::new();
    for i in 0..1200u64 {
        edges.push(make_edge(from, Uuid::new_v4(), i + 1));
    }
    let stats = storage.bulk_insert_edges(edges).await.unwrap();
    assert!(stats.batches >= 2);
}

#[tokio::test]
async fn test_backup_snapshot_and_restore() {
    use std::path::PathBuf;
    let tmp = TempDir::new().unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let node = make_node_named("backup");
    let id = node.id;
    storage.add_node(node).await.unwrap();
    storage.flush_batch_writes().unwrap();

    let backups_root = PathBuf::from("target/rocksdb/backups");
    let snapshot = storage.backup_snapshot(&backups_root).unwrap();
    assert!(snapshot.exists());

    // restore to a new location
    let dest = TempDir::new().unwrap();
    HighPerformanceRocksDbStorage::restore_from_snapshot(&snapshot, dest.path()).unwrap();
    let mut storage_restored = HighPerformanceRocksDbStorage::new(dest.path()).unwrap();
    let got = storage_restored.get_node(id).await.unwrap();
    assert!(got.is_some());
}

#[tokio::test]
async fn test_db_path_exposed() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    assert_eq!(storage.db_path(), tmp.path());
}

#[tokio::test]
async fn test_persist_in_target_dir() {
    use std::fs;
    use std::path::PathBuf;
    let base = PathBuf::from("target/rocksdb/persist_test");
    if base.exists() {
        let _ = fs::remove_dir_all(&base);
    }
    fs::create_dir_all(&base).unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(&base).unwrap();
    let node = make_node_named("persist");
    storage.add_node(node).await.unwrap();
    storage.flush_batch_writes().unwrap();
    // Expect some RocksDB files in directory
    let entries: Vec<_> = fs::read_dir(&base).unwrap().collect();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_rollback_unknown_tx_errors() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let err = storage.rollback(999999);
    assert!(err.is_err());
}

#[tokio::test]
async fn test_flush_batch_writes_noop() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    storage.flush_batch_writes().unwrap();
}

#[tokio::test]
async fn test_add_node_tx_commit_visibility() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let tx = storage.begin();
    let node = make_node_named("visible");
    let id = node.id;
    storage.add_node_tx(tx, node).await.unwrap();
    storage.commit(tx).unwrap();
    assert!(storage.get_node(id).await.unwrap().is_some());
}

#[tokio::test]
async fn test_add_edge_tx_commit_visibility() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let tx = storage.begin();
    storage
        .add_edge_tx(tx, make_edge(from, to, 7))
        .await
        .unwrap();
    storage.commit(tx).unwrap();
    let edges = storage.get_edges_from(from).await.unwrap();
    assert!(edges.iter().any(|e| e.id == 7));
}

#[tokio::test]
async fn test_list_cf_names_contains_expected() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let names = storage.list_cf_names().unwrap();
    assert!(names.contains(&"indices".to_string()));
}

#[tokio::test]
async fn test_indices_prefix_scan_limits() {
    // ensure scanning from: prefix stops correctly when prefix changes
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let from = Uuid::new_v4();
    storage
        .add_edge(make_edge(from, Uuid::new_v4(), 100))
        .await
        .unwrap();
    storage
        .add_edge(make_edge(Uuid::new_v4(), Uuid::new_v4(), 101))
        .await
        .unwrap();
    storage.flush_batch_writes().unwrap();
    let edges = storage.get_edges_from(from).await.unwrap();
    assert!(edges.iter().any(|e| e.id == 100));
    assert!(!edges.iter().any(|e| e.id == 101));
}

#[tokio::test]
async fn test_large_batch_2500_nodes() {
    let tmp = TempDir::new().unwrap();
    let storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let mut nodes = Vec::new();
    for i in 0..2500 {
        nodes.push(make_node_named(&format!("x{}", i)));
    }
    let stats = storage.bulk_insert_nodes(nodes).await.unwrap();
    assert!(stats.batches >= 3);
}

#[tokio::test]
async fn test_remove_node_clears_index_lookup() {
    let tmp = TempDir::new().unwrap();
    let mut storage = HighPerformanceRocksDbStorage::new(tmp.path()).unwrap();
    let n = make_node_named("gone");
    let id = n.id;
    storage.add_node(n).await.unwrap();
    let cnt = storage.find_nodes_by_name("gone").await.unwrap().len();
    assert!(cnt >= 1);
    storage.remove_node(id).await.unwrap();
    let cnt2 = storage.find_nodes_by_name("gone").await.unwrap().len();
    assert!(cnt2 <= cnt - 1);
}
