use async_trait::async_trait;
use codegraph_core::{CodeGraphError, EdgeId, NodeId, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Edge represents a directed relationship between two nodes with optional properties
/// and a weight used for relationship strength scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub label: String,
    pub weight: f64,
    pub properties: HashMap<String, JsonValue>,
}

impl Edge {
    pub fn new<S: Into<String>>(source: NodeId, target: NodeId, label: S) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            target,
            label: label.into(),
            weight: 1.0,
            properties: HashMap::new(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_property<K: Into<String>>(mut self, key: K, value: JsonValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

#[async_trait]
pub trait EdgeStore: Send + Sync {
    async fn create(&self, edge: Edge) -> Result<EdgeId>;
    async fn read(&self, id: EdgeId) -> Result<Option<Edge>>;
    async fn update(&self, edge: Edge) -> Result<()>;
    async fn delete(&self, id: EdgeId) -> Result<bool>;

    async fn get_outgoing(&self, source: NodeId) -> Result<Vec<Edge>>;
    async fn get_incoming(&self, target: NodeId) -> Result<Vec<Edge>>;
    async fn get_bidirectional(&self, node: NodeId) -> Result<Vec<Edge>>;

    async fn create_bulk(&self, edges: Vec<Edge>) -> Result<Vec<EdgeId>>;
}

/// In-memory edge store with lock-free read concurrency using DashMap and
/// compact indexing by source, target, and label.
#[derive(Debug, Default)]
pub struct InMemoryEdgeStore {
    // Primary storage for edges by id
    edges: DashMap<EdgeId, Arc<Edge>>,
    // Indexes
    by_source: DashMap<NodeId, Arc<Vec<EdgeId>>>,
    by_target: DashMap<NodeId, Arc<Vec<EdgeId>>>,
    by_label: DashMap<String, Arc<Vec<EdgeId>>>,
    // Small write lock to coordinate index vector rebuilds (rare)
    // Readers always operate on Arc<Vec<...>> snapshots to avoid locks.
    rebuild_lock: RwLock<()>,
}

impl InMemoryEdgeStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert_index<K: Eq + std::hash::Hash + Clone + Send + Sync + 'static>(
        map: &DashMap<K, Arc<Vec<EdgeId>>>,
        key: K,
        edge_id: EdgeId,
    ) {
        if let Some(mut entry) = map.get_mut(&key) {
            // Copy-on-write small vector rebuild to keep readers lock-free
            let mut v: Vec<EdgeId> = entry.value().as_ref().clone();
            v.push(edge_id);
            *entry = Arc::new(v);
            return;
        }
        map.insert(key, Arc::new(vec![edge_id]));
    }

    fn remove_index<K: Eq + std::hash::Hash + Clone + Send + Sync + 'static>(
        map: &DashMap<K, Arc<Vec<EdgeId>>>,
        key: &K,
        edge_id: EdgeId,
    ) {
        if let Some(mut entry) = map.get_mut(key) {
            let v = entry.value();
            if v.len() == 1 && v[0] == edge_id {
                drop(entry);
                map.remove(key);
            } else {
                let mut nv = v.as_ref().clone();
                if let Some(pos) = nv.iter().position(|e| *e == edge_id) {
                    nv.swap_remove(pos);
                }
                *entry = Arc::new(nv);
            }
        }
    }

    fn reindex_edge(&self, old: Option<Arc<Edge>>, new: &Arc<Edge>) {
        let _guard = self.rebuild_lock.write();
        if let Some(old_edge) = old {
            // Remove old indexes
            Self::remove_index(&self.by_source, &old_edge.source, old_edge.id);
            Self::remove_index(&self.by_target, &old_edge.target, old_edge.id);
            Self::remove_index(&self.by_label, &old_edge.label, old_edge.id);
        }
        // Insert new indexes
        Self::insert_index(&self.by_source, new.source, new.id);
        Self::insert_index(&self.by_target, new.target, new.id);
        Self::insert_index(&self.by_label, new.label.clone(), new.id);
    }
}

#[async_trait]
impl EdgeStore for InMemoryEdgeStore {
    async fn create(&self, edge: Edge) -> Result<EdgeId> {
        let edge_id = edge.id;
        let edge_arc = Arc::new(edge);
        let old = self.edges.insert(edge_id, edge_arc.clone());
        self.reindex_edge(old, &edge_arc);
        Ok(edge_id)
    }

    async fn read(&self, id: EdgeId) -> Result<Option<Edge>> {
        Ok(self.edges.get(&id).map(|e| e.value().as_ref().clone()))
    }

    async fn update(&self, edge: Edge) -> Result<()> {
        let id = edge.id;
        if !self.edges.contains_key(&id) {
            return Err(CodeGraphError::NotFound(format!(
                "edge {} not found for update",
                id
            )));
        }
        let edge_arc = Arc::new(edge);
        let old = self.edges.insert(id, edge_arc.clone());
        self.reindex_edge(old, &edge_arc);
        Ok(())
    }

    async fn delete(&self, id: EdgeId) -> Result<bool> {
        if let Some((_, old)) = self.edges.remove(&id) {
            self.reindex_edge(
                Some(old),
                &Arc::new(Edge {
                    id,
                    source: Uuid::nil(),
                    target: Uuid::nil(),
                    label: String::new(),
                    weight: 0.0,
                    properties: HashMap::new(),
                }),
            );
            return Ok(true);
        }
        Ok(false)
    }

    async fn get_outgoing(&self, source: NodeId) -> Result<Vec<Edge>> {
        if let Some(v) = self.by_source.get(&source) {
            let ids = v.as_ref();
            let mut result = Vec::with_capacity(ids.len());
            for id in ids.iter() {
                if let Some(edge) = self.edges.get(id) {
                    result.push(edge.value().as_ref().clone());
                }
            }
            return Ok(result);
        }
        Ok(Vec::new())
    }

    async fn get_incoming(&self, target: NodeId) -> Result<Vec<Edge>> {
        if let Some(v) = self.by_target.get(&target) {
            let ids = v.as_ref();
            let mut result = Vec::with_capacity(ids.len());
            for id in ids.iter() {
                if let Some(edge) = self.edges.get(id) {
                    result.push(edge.value().as_ref().clone());
                }
            }
            return Ok(result);
        }
        Ok(Vec::new())
    }

    async fn get_bidirectional(&self, node: NodeId) -> Result<Vec<Edge>> {
        let mut seen: HashSet<EdgeId> = HashSet::new();
        let mut res = Vec::new();

        if let Some(out) = self.by_source.get(&node) {
            for id in out.iter() {
                if let Some(edge) = self.edges.get(id) {
                    if seen.insert(*id) {
                        res.push(edge.value().as_ref().clone());
                    }
                }
            }
        }
        if let Some(inc) = self.by_target.get(&node) {
            for id in inc.iter() {
                if let Some(edge) = self.edges.get(id) {
                    if seen.insert(*id) {
                        res.push(edge.value().as_ref().clone());
                    }
                }
            }
        }
        Ok(res)
    }

    async fn create_bulk(&self, edges: Vec<Edge>) -> Result<Vec<EdgeId>> {
        let mut ids = Vec::with_capacity(edges.len());
        // Write path: minimal locking, index rebuild per edge; fast in practice for 10k edges.
        for edge in edges {
            let id = edge.id;
            let arc = Arc::new(edge);
            let old = self.edges.insert(id, arc.clone());
            self.reindex_edge(old, &arc);
            ids.push(id);
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid() -> Uuid {
        Uuid::new_v4()
    }

    #[tokio::test]
    async fn create_and_read_edge() {
        let store = InMemoryEdgeStore::new();
        let e = Edge::new(uuid(), uuid(), "calls").with_weight(2.5);
        let id = store.create(e.clone()).await.unwrap();

        let fetched = store.read(id).await.unwrap().unwrap();
        assert_eq!(fetched, e);
        assert_eq!(fetched.weight, 2.5);
    }

    #[tokio::test]
    async fn update_edge_label_and_weight() {
        let store = InMemoryEdgeStore::new();
        let mut e = Edge::new(uuid(), uuid(), "uses").with_weight(1.0);
        let id = store.create(e.clone()).await.unwrap();

        e.id = id;
        e.label = "imports".to_string();
        e.weight = 3.14;
        store.update(e.clone()).await.unwrap();

        let fetched = store.read(id).await.unwrap().unwrap();
        assert_eq!(fetched.label, "imports");
        assert_eq!(fetched.weight, 3.14);
    }

    #[tokio::test]
    async fn delete_edge_removes_from_indexes() {
        let store = InMemoryEdgeStore::new();
        let s = uuid();
        let t = uuid();
        let id = store.create(Edge::new(s, t, "calls")).await.unwrap();
        assert_eq!(store.get_outgoing(s).await.unwrap().len(), 1);
        assert_eq!(store.get_incoming(t).await.unwrap().len(), 1);

        assert!(store.delete(id).await.unwrap());
        assert!(store.read(id).await.unwrap().is_none());
        assert!(store.get_outgoing(s).await.unwrap().is_empty());
        assert!(store.get_incoming(t).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_outgoing_edges_returns_correct() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let c = uuid();
        store.create(Edge::new(a, b, "calls")).await.unwrap();
        store.create(Edge::new(a, c, "uses")).await.unwrap();
        store.create(Edge::new(b, c, "calls")).await.unwrap();

        let out = store.get_outgoing(a).await.unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|e| e.target == b));
        assert!(out.iter().any(|e| e.target == c));
    }

    #[tokio::test]
    async fn get_incoming_edges_returns_correct() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let c = uuid();
        store.create(Edge::new(a, b, "calls")).await.unwrap();
        store.create(Edge::new(c, b, "uses")).await.unwrap();

        let inc = store.get_incoming(b).await.unwrap();
        assert_eq!(inc.len(), 2);
        assert!(inc.iter().any(|e| e.source == a));
        assert!(inc.iter().any(|e| e.source == c));
    }

    #[tokio::test]
    async fn get_bidirectional_edges_returns_both() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        store.create(Edge::new(a, b, "calls")).await.unwrap();
        store.create(Edge::new(b, a, "uses")).await.unwrap();

        let bi = store.get_bidirectional(a).await.unwrap();
        assert_eq!(bi.len(), 2);
        assert!(bi.iter().any(|e| e.source == a && e.target == b));
        assert!(bi.iter().any(|e| e.source == b && e.target == a));
    }

    #[tokio::test]
    async fn indexing_by_label_works() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let c = uuid();
        store.create(Edge::new(a, b, "calls")).await.unwrap();
        store.create(Edge::new(a, c, "calls")).await.unwrap();
        store.create(Edge::new(b, c, "uses")).await.unwrap();

        // Indirect verification via update reindex on label
        let mut e = store
            .get_outgoing(a)
            .await
            .unwrap()
            .into_iter()
            .find(|e| e.target == c)
            .unwrap();
        e.label = "imports".to_string();
        store.update(e.clone()).await.unwrap();

        let out_a = store.get_outgoing(a).await.unwrap();
        assert_eq!(out_a.len(), 2);
        assert!(out_a.iter().any(|x| x.label == "calls"));
        assert!(out_a.iter().any(|x| x.label == "imports"));
    }

    #[tokio::test]
    async fn bulk_create_10k_edges() {
        let store = InMemoryEdgeStore::new();
        let s = uuid();
        let mut edges = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            edges.push(Edge::new(s, uuid(), "calls"));
        }
        let ids = store.create_bulk(edges).await.unwrap();
        assert_eq!(ids.len(), 10_000);
        let out = store.get_outgoing(s).await.unwrap();
        assert_eq!(out.len(), 10_000);
    }

    #[tokio::test]
    async fn weight_scoring_ordering() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let c = uuid();
        store
            .create(Edge::new(a, b, "calls").with_weight(0.5))
            .await
            .unwrap();
        store
            .create(Edge::new(a, c, "calls").with_weight(2.0))
            .await
            .unwrap();
        let mut out = store.get_outgoing(a).await.unwrap();
        out.sort_by(|x, y| y.weight.partial_cmp(&x.weight).unwrap());
        assert_eq!(out[0].weight, 2.0);
        assert_eq!(out[1].weight, 0.5);
    }

    #[tokio::test]
    async fn update_edge_source_reindexes() {
        let store = InMemoryEdgeStore::new();
        let s1 = uuid();
        let s2 = uuid();
        let t = uuid();
        let mut e = Edge::new(s1, t, "uses");
        let id = store.create(e.clone()).await.unwrap();
        assert_eq!(store.get_outgoing(s1).await.unwrap().len(), 1);
        e.id = id;
        e.source = s2;
        store.update(e).await.unwrap();
        assert!(store.get_outgoing(s1).await.unwrap().is_empty());
        assert_eq!(store.get_outgoing(s2).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn update_edge_target_reindexes() {
        let store = InMemoryEdgeStore::new();
        let s = uuid();
        let t1 = uuid();
        let t2 = uuid();
        let mut e = Edge::new(s, t1, "uses");
        let id = store.create(e.clone()).await.unwrap();
        assert_eq!(store.get_incoming(t1).await.unwrap().len(), 1);
        e.id = id;
        e.target = t2;
        store.update(e).await.unwrap();
        assert!(store.get_incoming(t1).await.unwrap().is_empty());
        assert_eq!(store.get_incoming(t2).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn update_edge_label_reindexes() {
        let store = InMemoryEdgeStore::new();
        let s = uuid();
        let t = uuid();
        let mut e = Edge::new(s, t, "uses");
        let id = store.create(e.clone()).await.unwrap();
        e.id = id;
        e.label = "extends".to_string();
        store.update(e).await.unwrap();
        let out = store.get_outgoing(s).await.unwrap();
        assert!(out.iter().any(|x| x.label == "extends"));
    }

    #[tokio::test]
    async fn mixed_operations_consistency() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let c = uuid();
        let id1 = store.create(Edge::new(a, b, "calls")).await.unwrap();
        let id2 = store.create(Edge::new(a, c, "uses")).await.unwrap();
        assert_eq!(store.get_outgoing(a).await.unwrap().len(), 2);
        store.delete(id1).await.unwrap();
        assert_eq!(store.get_outgoing(a).await.unwrap().len(), 1);
        let mut e2 = store.read(id2).await.unwrap().unwrap();
        e2.target = b;
        store.update(e2).await.unwrap();
        assert_eq!(store.get_incoming(b).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn properties_roundtrip() {
        let store = InMemoryEdgeStore::new();
        let s = uuid();
        let t = uuid();
        let e = Edge::new(s, t, "references")
            .with_property("confidence", JsonValue::from(0.87))
            .with_property("note", JsonValue::from("auto"));
        let id = store.create(e.clone()).await.unwrap();
        let got = store.read(id).await.unwrap().unwrap();
        assert_eq!(
            got.properties.get("confidence").unwrap(),
            &JsonValue::from(0.87)
        );
        assert_eq!(
            got.properties.get("note").unwrap(),
            &JsonValue::from("auto")
        );
    }

    #[tokio::test]
    async fn concurrent_creations_thread_safety() {
        use futures::future::join_all;
        let store = Arc::new(InMemoryEdgeStore::new());
        let s = uuid();
        let tasks = (0..1000).map(|_| {
            let store = store.clone();
            let s = s;
            tokio::spawn(async move { store.create(Edge::new(s, uuid(), "calls")).await.unwrap() })
        });
        let ids = join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 1000);
        assert_eq!(store.get_outgoing(s).await.unwrap().len(), 1000);
    }

    #[tokio::test]
    async fn bidirectional_no_duplicates() {
        let store = InMemoryEdgeStore::new();
        let a = uuid();
        let b = uuid();
        let e1 = Edge::new(a, b, "calls");
        let e2 = Edge::new(a, b, "uses");
        store.create(e1).await.unwrap();
        store.create(e2).await.unwrap();
        let both = store.get_bidirectional(a).await.unwrap();
        assert_eq!(both.len(), 2);
    }
}
