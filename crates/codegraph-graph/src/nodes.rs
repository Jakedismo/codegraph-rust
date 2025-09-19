use async_trait::async_trait;
use dashmap::DashMap;
use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, DBWithThreadMode,
    IteratorMode, MultiThreaded, Options, ReadOptions, WriteBatch, WriteOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use codegraph_core::{CodeGraphError, Result};

type DB = DBWithThreadMode<MultiThreaded>;

const NODES_CF: &str = "cg_nodes";
const LABEL_INDEX_CF: &str = "cg_label_idx";
const PROP_INDEX_CF: &str = "cg_prop_idx";
const HISTORY_CF: &str = "cg_history";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: Uuid,
    pub properties: HashMap<String, JsonValue>,
    pub labels: Vec<String>,
    pub version: u64,
}

impl Node {
    pub fn new(labels: Vec<String>, properties: HashMap<String, JsonValue>) -> Self {
        Self {
            id: Uuid::new_v4(),
            labels,
            properties,
            version: 1,
        }
    }
}

#[async_trait]
pub trait NodeStore: Send + Sync {
    async fn create(&self, node: Node) -> Result<()>;
    async fn read(&self, id: Uuid) -> Result<Option<Node>>;
    async fn update(&self, node: Node) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;

    async fn batch_create(&self, nodes: Vec<Node>) -> Result<()>;
    async fn batch_update(&self, nodes: Vec<Node>) -> Result<()>;

    async fn find_by_label(&self, label: &str) -> Result<Vec<Node>>;
    async fn find_by_property(&self, name: &str, value: &JsonValue) -> Result<Vec<Node>>;
    async fn history(&self, id: Uuid) -> Result<Vec<Node>>;
}

pub struct RocksNodeStore {
    db: Arc<DB>,
    write_options: WriteOptions,
    read_options: ReadOptions,
    read_cache: Arc<DashMap<Uuid, Arc<Node>>>,
}

impl RocksNodeStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_compression_type(DBCompressionType::Lz4);
        db_opts.set_bottommost_compression_type(DBCompressionType::Zstd);
        db_opts.set_max_background_jobs(num_cpus::get() as i32);
        db_opts.set_max_subcompactions(4);

        let block_cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&block_cache);
        block_opts.set_block_size(32 * 1024);
        block_opts.set_bloom_filter(10.0, false);
        db_opts.set_block_based_table_factory(&block_opts);

        let cf_descriptors = vec![
            Self::nodes_cf_descriptor(),
            Self::labels_cf_descriptor(),
            Self::props_cf_descriptor(),
            Self::history_cf_descriptor(),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cf_descriptors)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database: {}", e)))?;

        let mut write_options = WriteOptions::default();
        write_options.set_sync(false);
        write_options.disable_wal(false);

        let mut read_options = ReadOptions::default();
        read_options.set_verify_checksums(false);
        read_options.fill_cache(true);

        Ok(Self {
            db: Arc::new(db),
            write_options,
            read_options,
            read_cache: Arc::new(DashMap::with_capacity(100_000)),
        })
    }

    fn nodes_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        let cache = Cache::new_lru_cache(128 * 1024 * 1024);
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(32 * 1024);
        opts.set_block_based_table_factory(&block_opts);
        opts.set_compression_type(DBCompressionType::Lz4);
        ColumnFamilyDescriptor::new(NODES_CF, opts)
    }

    fn labels_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        let cache = Cache::new_lru_cache(64 * 1024 * 1024);
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(15.0, false);
        opts.set_block_based_table_factory(&block_opts);
        opts.set_compression_type(DBCompressionType::Lz4);
        ColumnFamilyDescriptor::new(LABEL_INDEX_CF, opts)
    }

    fn props_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        let cache = Cache::new_lru_cache(64 * 1024 * 1024);
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(15.0, false);
        opts.set_block_based_table_factory(&block_opts);
        opts.set_compression_type(DBCompressionType::Lz4);
        ColumnFamilyDescriptor::new(PROP_INDEX_CF, opts)
    }

    fn history_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        let cache = Cache::new_lru_cache(32 * 1024 * 1024);
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(32 * 1024);
        opts.set_block_based_table_factory(&block_opts);
        opts.set_compression_type(DBCompressionType::Zstd);
        ColumnFamilyDescriptor::new(HISTORY_CF, opts)
    }

    fn cf(&self, name: &str) -> Result<std::sync::Arc<rocksdb::BoundColumnFamily<'_>>> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", name)))
    }

    #[inline]
    fn id_key(id: Uuid) -> [u8; 16] {
        *id.as_bytes()
    }

    #[inline]
    fn label_index_key(label: &str, id: Uuid) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + label.len() + 16);
        v.extend_from_slice(b"lbl:");
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(Self::id_key(id).as_slice());
        v
    }

    #[inline]
    fn prop_canonical(value: &JsonValue) -> Vec<u8> {
        // Canonical serialization for stable indexing
        serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec())
    }

    #[inline]
    fn prop_index_prefix(name: &str, value: &JsonValue) -> Vec<u8> {
        let mut v = Vec::with_capacity(5 + name.len() + 1 + 32);
        v.extend_from_slice(b"prop:");
        v.extend_from_slice(name.as_bytes());
        v.push(0x00);
        v.extend_from_slice(&Self::prop_canonical(value));
        v
    }

    #[inline]
    fn prop_index_key(name: &str, value: &JsonValue, id: Uuid) -> Vec<u8> {
        let mut v = Self::prop_index_prefix(name, value);
        v.extend_from_slice(Self::id_key(id).as_slice());
        v
    }

    #[inline]
    fn history_key(id: Uuid, version: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(5 + 16 + 8);
        v.extend_from_slice(b"hist:");
        v.extend_from_slice(Self::id_key(id).as_slice());
        v.extend_from_slice(&version.to_be_bytes());
        v
    }

    fn write_node_indices(
        batch: &mut WriteBatch,
        label_cf: &Arc<rocksdb::BoundColumnFamily<'_>>,
        prop_cf: &Arc<rocksdb::BoundColumnFamily<'_>>,
        node: &Node,
    ) {
        // Deduplicate labels to avoid duplicate index entries
        let mut seen = HashSet::new();
        for label in &node.labels {
            if seen.insert(label) {
                batch.put_cf(label_cf, Self::label_index_key(label, node.id), b"");
            }
        }
        for (k, v) in &node.properties {
            batch.put_cf(prop_cf, Self::prop_index_key(k, v, node.id), b"");
        }
    }

    fn delete_node_indices(
        batch: &mut WriteBatch,
        label_cf: &Arc<rocksdb::BoundColumnFamily<'_>>,
        prop_cf: &Arc<rocksdb::BoundColumnFamily<'_>>,
        node: &Node,
    ) {
        let mut seen = HashSet::new();
        for label in &node.labels {
            if seen.insert(label) {
                batch.delete_cf(label_cf, Self::label_index_key(label, node.id));
            }
        }
        for (k, v) in &node.properties {
            batch.delete_cf(prop_cf, Self::prop_index_key(k, v, node.id));
        }
    }
}

#[async_trait]
impl NodeStore for RocksNodeStore {
    async fn create(&self, mut node: Node) -> Result<()> {
        if node.id.is_nil() {
            node.id = Uuid::new_v4();
        }
        if node.version == 0 {
            node.version = 1;
        }

        let nodes_cf = self.cf(NODES_CF)?;
        let labels_cf = self.cf(LABEL_INDEX_CF)?;
        let props_cf = self.cf(PROP_INDEX_CF)?;
        let history_cf = self.cf(HISTORY_CF)?;

        let node_key = Self::id_key(node.id);
        let node_bytes =
            serde_json::to_vec(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let mut batch = WriteBatch::default();
        batch.put_cf(&nodes_cf, node_key, &node_bytes);
        Self::write_node_indices(&mut batch, &labels_cf, &props_cf, &node);
        // Write initial history entry
        batch.put_cf(
            &history_cf,
            Self::history_key(node.id, node.version),
            &node_bytes,
        );

        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.read_cache.insert(node.id, Arc::new(node));
        Ok(())
    }

    async fn read(&self, id: Uuid) -> Result<Option<Node>> {
        if let Some(cached) = self.read_cache.get(&id) {
            let node = cached.value().as_ref().clone();
            return Ok(Some(node));
        }
        let nodes_cf = self.cf(NODES_CF)?;
        let key = Self::id_key(id);
        let data = self
            .db
            .get_cf_opt(&nodes_cf, &key, &self.read_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        if let Some(bytes) = data {
            let node: Node = serde_json::from_slice::<Node>(&bytes)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            self.read_cache.insert(id, Arc::new(node.clone()));
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, mut node: Node) -> Result<()> {
        let Some(prev) = self.read(node.id).await? else {
            return Err(CodeGraphError::NodeNotFound(node.id.to_string()));
        };
        // Increment version
        let new_version = prev.version + 1;
        node.version = new_version;

        let nodes_cf = self.cf(NODES_CF)?;
        let labels_cf = self.cf(LABEL_INDEX_CF)?;
        let props_cf = self.cf(PROP_INDEX_CF)?;
        let history_cf = self.cf(HISTORY_CF)?;

        let node_key = Self::id_key(node.id);
        let node_bytes =
            serde_json::to_vec(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let mut batch = WriteBatch::default();
        // Update main record
        batch.put_cf(&nodes_cf, node_key, &node_bytes);
        // Update indices: remove old, add new
        Self::delete_node_indices(&mut batch, &labels_cf, &props_cf, &prev);
        Self::write_node_indices(&mut batch, &labels_cf, &props_cf, &node);
        // Append history for new version
        batch.put_cf(
            &history_cf,
            Self::history_key(node.id, node.version),
            &node_bytes,
        );

        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.read_cache.insert(node.id, Arc::new(node));
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let Some(node) = self.read(id).await? else {
            return Ok(());
        };

        let nodes_cf = self.cf(NODES_CF)?;
        let labels_cf = self.cf(LABEL_INDEX_CF)?;
        let props_cf = self.cf(PROP_INDEX_CF)?;

        let mut batch = WriteBatch::default();
        batch.delete_cf(&nodes_cf, Self::id_key(id));
        Self::delete_node_indices(&mut batch, &labels_cf, &props_cf, &node);

        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.read_cache.remove(&id);
        Ok(())
    }

    async fn batch_create(&self, mut nodes: Vec<Node>) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        let nodes_cf = self.cf(NODES_CF)?;
        let labels_cf = self.cf(LABEL_INDEX_CF)?;
        let props_cf = self.cf(PROP_INDEX_CF)?;
        let history_cf = self.cf(HISTORY_CF)?;
        let mut batch = WriteBatch::default();
        for node in nodes.iter_mut() {
            if node.id.is_nil() {
                node.id = Uuid::new_v4();
            }
            if node.version == 0 {
                node.version = 1;
            }
            let key = Self::id_key(node.id);
            let bytes =
                serde_json::to_vec(&*node).map_err(|e| CodeGraphError::Database(e.to_string()))?;
            batch.put_cf(&nodes_cf, key, &bytes);
            Self::write_node_indices(&mut batch, &labels_cf, &props_cf, node);
            batch.put_cf(
                &history_cf,
                Self::history_key(node.id, node.version),
                &bytes,
            );
        }
        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        for n in nodes.iter() {
            self.read_cache.insert(n.id, Arc::new(n.clone()));
        }
        Ok(())
    }

    async fn batch_update(&self, nodes: Vec<Node>) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        let mut batch = WriteBatch::default();
        for mut node in nodes.into_iter() {
            let Some(prev) = self.read(node.id).await? else {
                return Err(CodeGraphError::NodeNotFound(node.id.to_string()));
            };
            node.version = prev.version + 1;
            // Obtain CF handles after await to avoid holding non-Send references across .await
            let nodes_cf = self.cf(NODES_CF)?;
            let labels_cf = self.cf(LABEL_INDEX_CF)?;
            let props_cf = self.cf(PROP_INDEX_CF)?;
            let history_cf = self.cf(HISTORY_CF)?;

            let key = Self::id_key(node.id);
            let bytes =
                serde_json::to_vec(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;
            batch.put_cf(&nodes_cf, key, &bytes);
            Self::delete_node_indices(&mut batch, &labels_cf, &props_cf, &prev);
            Self::write_node_indices(&mut batch, &labels_cf, &props_cf, &node);
            batch.put_cf(
                &history_cf,
                Self::history_key(node.id, node.version),
                &bytes,
            );
            self.read_cache.insert(node.id, Arc::new(node));
        }
        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))
    }

    async fn find_by_label(&self, label: &str) -> Result<Vec<Node>> {
        let prefix = {
            let mut p = Vec::with_capacity(4 + label.len());
            p.extend_from_slice(b"lbl:");
            p.extend_from_slice(label.as_bytes());
            p
        };

        // Collect all matching IDs first, without holding the column family reference
        let node_ids = {
            let labels_cf = self.cf(LABEL_INDEX_CF)?;
            let iter = self.db.iterator_cf_opt(
                &labels_cf,
                ReadOptions::default(),
                IteratorMode::From(&prefix, rocksdb::Direction::Forward),
            );

            let mut ids = Vec::new();
            for item in iter {
                let (key, _val) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
                if !key.starts_with(&prefix) {
                    break;
                }
                if key.len() >= prefix.len() + 16 {
                    let id_bytes = &key[key.len() - 16..];
                    if let Ok(id) = Uuid::from_slice(id_bytes) {
                        ids.push(id);
                    }
                }
            }
            ids
        };

        // Now fetch the actual nodes
        let mut out = Vec::new();
        for id in node_ids {
            if let Some(n) = self.read(id).await? {
                out.push(n);
            }
        }
        Ok(out)
    }

    async fn find_by_property(&self, name: &str, value: &JsonValue) -> Result<Vec<Node>> {
        let prefix = Self::prop_index_prefix(name, value);

        // Collect all matching IDs first, without holding the column family reference
        let node_ids = {
            let props_cf = self.cf(PROP_INDEX_CF)?;
            let iter = self.db.iterator_cf_opt(
                &props_cf,
                ReadOptions::default(),
                IteratorMode::From(&prefix, rocksdb::Direction::Forward),
            );

            let mut ids = Vec::new();
            for item in iter {
                let (key, _val) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
                if !key.starts_with(&prefix) {
                    break;
                }
                if key.len() >= prefix.len() + 16 {
                    let id_bytes = &key[key.len() - 16..];
                    if let Ok(id) = Uuid::from_slice(id_bytes) {
                        ids.push(id);
                    }
                }
            }
            ids
        };

        // Now fetch the actual nodes
        let mut out = Vec::new();
        for id in node_ids {
            if let Some(n) = self.read(id).await? {
                out.push(n);
            }
        }
        Ok(out)
    }

    async fn history(&self, id: Uuid) -> Result<Vec<Node>> {
        let history_cf = self.cf(HISTORY_CF)?;
        let mut prefix = Vec::with_capacity(5 + 16);
        prefix.extend_from_slice(b"hist:");
        prefix.extend_from_slice(Self::id_key(id).as_slice());
        let iter = self.db.iterator_cf_opt(
            &history_cf,
            ReadOptions::default(),
            IteratorMode::From(&prefix, rocksdb::Direction::Forward),
        );
        let mut out = Vec::new();
        for item in iter {
            let (key, val) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            if !key.starts_with(&prefix) {
                break;
            }
            let node: Node =
                serde_json::from_slice::<Node>(&val).map_err(|e| CodeGraphError::Database(e.to_string()))?;
            out.push(node);
        }
        // Already ordered by version due to suffix
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio_test::block_on;

    fn store() -> RocksNodeStore {
        let dir = tempdir().unwrap();
        RocksNodeStore::new(dir.path()).unwrap()
    }

    fn node_with(label: &str, (k, v): (&str, JsonValue)) -> Node {
        let mut props = HashMap::new();
        props.insert(k.to_string(), v);
        Node::new(vec![label.to_string()], props)
    }

    #[test]
    fn create_and_read_node() {
        let s = store();
        let n = node_with("User", ("name", JsonValue::String("alice".into())));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            let got = s.read(id).await.unwrap().unwrap();
            assert_eq!(got.id, id);
            assert_eq!(got.version, 1);
            assert_eq!(got.labels, vec!["User".to_string()]);
            assert_eq!(
                got.properties.get("name").unwrap(),
                &JsonValue::String("alice".into())
            );
        });
    }

    #[test]
    fn update_increments_version_and_tracks_history() {
        let s = store();
        let mut n = node_with("File", ("path", JsonValue::String("/a".into())));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            // update
            n.properties
                .insert("path".into(), JsonValue::String("/b".into()));
            s.update(n.clone()).await.unwrap();
            let got = s.read(id).await.unwrap().unwrap();
            assert_eq!(got.version, 2);
            let hist = s.history(id).await.unwrap();
            assert_eq!(hist.len(), 2);
            assert_eq!(hist[0].version, 1);
            assert_eq!(hist[1].version, 2);
        });
    }

    #[test]
    fn delete_removes_node_and_indices() {
        let s = store();
        let n = node_with("Tag", ("key", JsonValue::String("v".into())));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            // find by label and prop
            let by_label = s.find_by_label("Tag").await.unwrap();
            assert!(by_label.iter().any(|x| x.id == id));
            let by_prop = s
                .find_by_property("key", &JsonValue::String("v".into()))
                .await
                .unwrap();
            assert!(by_prop.iter().any(|x| x.id == id));

            s.delete(id).await.unwrap();
            assert!(s.read(id).await.unwrap().is_none());
            assert!(!s
                .find_by_label("Tag")
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
            assert!(!s
                .find_by_property("key", &JsonValue::String("v".into()))
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
        });
    }

    #[test]
    fn batch_create_1000_and_query() {
        let s = store();
        let mut nodes = Vec::new();
        for i in 0..1000 {
            let mut props = HashMap::new();
            props.insert("idx".into(), JsonValue::from(i as i64));
            let label = if i % 2 == 0 { "Even" } else { "Odd" };
            nodes.push(Node::new(vec![label.into()], props));
        }
        block_on(async {
            s.batch_create(nodes.clone()).await.unwrap();
            let evens = s.find_by_label("Even").await.unwrap();
            assert!(evens.len() >= 400); // rough check
                                         // spot check reads
            for n in evens.iter().take(5) {
                let got = s.read(n.id).await.unwrap();
                assert!(got.is_some());
            }
        });
    }

    #[test]
    fn batch_update_1000() {
        let s = store();
        let mut nodes = Vec::new();
        for i in 0..1000 {
            let mut props = HashMap::new();
            props.insert("idx".into(), JsonValue::from(i as i64));
            nodes.push(Node::new(vec!["Group".into()], props));
        }
        block_on(async {
            s.batch_create(nodes.clone()).await.unwrap();
            // modify
            let updated: Vec<Node> = s
                .find_by_label("Group")
                .await
                .unwrap()
                .into_iter()
                .map(|mut n| {
                    n.labels = vec!["Updated".into()];
                    n
                })
                .collect();
            s.batch_update(updated.clone()).await.unwrap();
            let updated_nodes = s.find_by_label("Updated").await.unwrap();
            assert!(updated_nodes.len() >= 800);
            // versions should be 2
            assert!(updated_nodes.iter().all(|n| n.version == 2));
        });
    }

    #[test]
    fn index_by_property_string() {
        let s = store();
        let a = node_with("User", ("role", JsonValue::String("admin".into())));
        let b = node_with("User", ("role", JsonValue::String("user".into())));
        block_on(async {
            s.batch_create(vec![a.clone(), b.clone()]).await.unwrap();
            let admins = s
                .find_by_property("role", &JsonValue::String("admin".into()))
                .await
                .unwrap();
            assert!(admins.iter().any(|x| x.id == a.id));
            assert!(!admins.iter().any(|x| x.id == b.id));
        });
    }

    #[test]
    fn index_by_property_number() {
        let s = store();
        let a = node_with("Item", ("price", JsonValue::from(10)));
        let b = node_with("Item", ("price", JsonValue::from(20)));
        block_on(async {
            s.batch_create(vec![a.clone(), b.clone()]).await.unwrap();
            let cheap = s
                .find_by_property("price", &JsonValue::from(10))
                .await
                .unwrap();
            assert!(cheap.iter().any(|x| x.id == a.id));
            assert!(!cheap.iter().any(|x| x.id == b.id));
        });
    }

    #[test]
    fn properties_roundtrip_nested() {
        let s = store();
        let mut props = HashMap::new();
        props.insert("meta".into(), serde_json::json!({ "a": 1, "b": [1,2,3] }));
        let n = Node::new(vec!["Doc".into()], props);
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            let got = s.read(id).await.unwrap().unwrap();
            assert_eq!(
                got.properties.get("meta").unwrap(),
                &serde_json::json!({"a":1, "b": [1,2,3]})
            );
        });
    }

    #[test]
    fn history_is_ordered() {
        let s = store();
        let mut n = node_with("X", ("v", JsonValue::from(1)));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            for i in 2..=5 {
                n.properties.insert("v".into(), JsonValue::from(i));
                s.update(n.clone()).await.unwrap();
            }
            let hist = s.history(id).await.unwrap();
            assert_eq!(hist.len(), 5);
            for (i, hn) in hist.iter().enumerate() {
                assert_eq!(hn.version as usize, i + 1);
            }
        });
    }

    #[test]
    fn duplicate_labels_are_deduped_in_index() {
        let s = store();
        let mut n = node_with("L", ("k", JsonValue::from(1)));
        n.labels.push("L".into());
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            let by_label = s.find_by_label("L").await.unwrap();
            // should only appear once
            let count = by_label.iter().filter(|x| x.id == id).count();
            assert_eq!(count, 1);
        });
    }

    #[test]
    fn update_removes_old_label_index() {
        let s = store();
        let mut n = node_with("Old", ("k", JsonValue::from(1)));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            n.labels = vec!["New".into()];
            s.update(n.clone()).await.unwrap();
            assert!(!s
                .find_by_label("Old")
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
            assert!(s
                .find_by_label("New")
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
        });
    }

    #[test]
    fn update_removes_old_property_index() {
        let s = store();
        let mut n = node_with("N", ("k", JsonValue::from(1)));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            assert!(s
                .find_by_property("k", &JsonValue::from(1))
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
            n.properties.insert("k".into(), JsonValue::from(2));
            s.update(n.clone()).await.unwrap();
            assert!(!s
                .find_by_property("k", &JsonValue::from(1))
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
            assert!(s
                .find_by_property("k", &JsonValue::from(2))
                .await
                .unwrap()
                .iter()
                .any(|x| x.id == id));
        });
    }

    #[test]
    fn large_batch_create_10k() {
        let s = store();
        let mut nodes = Vec::new();
        for i in 0..10_000usize {
            let mut props = HashMap::new();
            props.insert("i".into(), JsonValue::from(i as i64));
            nodes.push(Node::new(vec!["B".into()], props));
        }
        block_on(async {
            s.batch_create(nodes).await.unwrap();
        });
    }

    #[test]
    fn history_survives_delete() {
        let s = store();
        let mut n = node_with("H", ("k", JsonValue::from(1)));
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            n.properties.insert("k".into(), JsonValue::from(3));
            s.update(n.clone()).await.unwrap();
            s.delete(id).await.unwrap();
            let hist = s.history(id).await.unwrap();
            assert_eq!(hist.len(), 2);
        });
    }

    #[test]
    fn create_with_nil_id_assigns_new() {
        let s = store();
        let mut n = node_with("X", ("a", JsonValue::from(1)));
        n.id = Uuid::nil();
        block_on(async {
            let id_nil = n.id;
            s.create(n.clone()).await.unwrap();
            let hist = s.history(id_nil).await.unwrap();
            assert!(hist.is_empty());
        });
    }
}
