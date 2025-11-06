use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::{
    engine::any::Any,
    opt::auth::Root,
    Surreal,
};
use tracing::{debug, error, info, warn};

/// SurrealDB storage implementation with flexible schema support
#[derive(Clone)]
pub struct SurrealDbStorage {
    db: Arc<Surreal<Any>>,
    config: SurrealDbConfig,
    // In-memory cache for performance
    node_cache: Arc<DashMap<NodeId, CodeNode>>,
    schema_version: Arc<std::sync::RwLock<u32>>,
}

#[derive(Debug, Clone)]
pub struct SurrealDbConfig {
    pub connection: String,
    pub namespace: String,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub strict_mode: bool,
    pub auto_migrate: bool,
    pub cache_enabled: bool,
}

impl Default for SurrealDbConfig {
    fn default() -> Self {
        Self {
            connection: "file://data/graph.db".to_string(),
            namespace: "codegraph".to_string(),
            database: "graph".to_string(),
            username: None,
            password: None,
            strict_mode: false,
            auto_migrate: true,
            cache_enabled: true,
        }
    }
}

/// Flexible node representation for SurrealDB
/// Uses serde_json::Value for schema flexibility
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurrealNode {
    id: String,
    #[serde(flatten)]
    data: HashMap<String, JsonValue>,
}

/// Edge representation for graph relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurrealEdge {
    id: String,
    from: String,
    to: String,
    edge_type: String,
    weight: f64,
    #[serde(flatten)]
    metadata: HashMap<String, JsonValue>,
}

/// Schema version tracking for migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchemaVersion {
    version: u32,
    applied_at: String,
    description: String,
}

impl SurrealDbStorage {
    /// Create a new SurrealDB storage instance
    pub async fn new(config: SurrealDbConfig) -> Result<Self> {
        info!(
            "Initializing SurrealDB storage with connection: {}",
            config.connection
        );

        // Connect to SurrealDB
        let db = Surreal::new::<Any>(&config.connection)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to connect: {}", e)))?;

        // Authenticate if credentials provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            db.signin(Root {
                username,
                password,
            })
            .await
            .map_err(|e| CodeGraphError::Database(format!("Authentication failed: {}", e)))?;
        }

        // Select namespace and database
        db.use_ns(&config.namespace)
            .use_db(&config.database)
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to select namespace/database: {}", e))
            })?;

        let storage = Self {
            db: Arc::new(db),
            config: config.clone(),
            node_cache: Arc::new(DashMap::new()),
            schema_version: Arc::new(std::sync::RwLock::new(0)),
        };

        // Initialize schema
        storage.initialize_schema().await?;

        // Auto-migrate if enabled
        if config.auto_migrate {
            storage.migrate().await?;
        }

        info!("SurrealDB storage initialized successfully");
        Ok(storage)
    }

    /// Initialize database schema with flexible design
    async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing SurrealDB schema");

        // Define flexible schema for nodes table
        // This schema is intentionally loose to allow for easy modifications
        let node_schema = r#"
            DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS id ON TABLE nodes TYPE string;
            DEFINE FIELD IF NOT EXISTS name ON TABLE nodes TYPE string;
            DEFINE FIELD IF NOT EXISTS node_type ON TABLE nodes TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS language ON TABLE nodes TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS content ON TABLE nodes TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS file_path ON TABLE nodes TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS start_line ON TABLE nodes TYPE option<number>;
            DEFINE FIELD IF NOT EXISTS end_line ON TABLE nodes TYPE option<number>;
            DEFINE FIELD IF NOT EXISTS embedding ON TABLE nodes TYPE option<array<float>>;
            DEFINE FIELD IF NOT EXISTS complexity ON TABLE nodes TYPE option<float>;
            DEFINE FIELD IF NOT EXISTS metadata ON TABLE nodes TYPE option<object>;
            DEFINE FIELD IF NOT EXISTS created_at ON TABLE nodes TYPE datetime DEFAULT time::now();
            DEFINE FIELD IF NOT EXISTS updated_at ON TABLE nodes TYPE datetime DEFAULT time::now();

            -- Indexes for efficient queries
            DEFINE INDEX IF NOT EXISTS idx_nodes_id ON TABLE nodes COLUMNS id UNIQUE;
            DEFINE INDEX IF NOT EXISTS idx_nodes_name ON TABLE nodes COLUMNS name;
            DEFINE INDEX IF NOT EXISTS idx_nodes_type ON TABLE nodes COLUMNS node_type;
            DEFINE INDEX IF NOT EXISTS idx_nodes_language ON TABLE nodes COLUMNS language;
            DEFINE INDEX IF NOT EXISTS idx_nodes_file_path ON TABLE nodes COLUMNS file_path;
        "#;

        // Define edges table for relationships
        let edge_schema = r#"
            DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS id ON TABLE edges TYPE string;
            DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record(nodes);
            DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record(nodes);
            DEFINE FIELD IF NOT EXISTS edge_type ON TABLE edges TYPE string;
            DEFINE FIELD IF NOT EXISTS weight ON TABLE edges TYPE float DEFAULT 1.0;
            DEFINE FIELD IF NOT EXISTS metadata ON TABLE edges TYPE option<object>;
            DEFINE FIELD IF NOT EXISTS created_at ON TABLE edges TYPE datetime DEFAULT time::now();

            -- Indexes for graph traversal
            DEFINE INDEX IF NOT EXISTS idx_edges_from ON TABLE edges COLUMNS from;
            DEFINE INDEX IF NOT EXISTS idx_edges_to ON TABLE edges COLUMNS to;
            DEFINE INDEX IF NOT EXISTS idx_edges_type ON TABLE edges COLUMNS edge_type;
        "#;

        // Schema version tracking
        let version_schema = r#"
            DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS version ON TABLE schema_versions TYPE number;
            DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now();
            DEFINE FIELD IF NOT EXISTS description ON TABLE schema_versions TYPE string;

            DEFINE INDEX IF NOT EXISTS idx_schema_version ON TABLE schema_versions COLUMNS version UNIQUE;
        "#;

        // Metadata table for system information
        let metadata_schema = r#"
            DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS key ON TABLE metadata TYPE string;
            DEFINE FIELD IF NOT EXISTS value ON TABLE metadata TYPE option<string | number | bool | object | array>;
            DEFINE FIELD IF NOT EXISTS updated_at ON TABLE metadata TYPE datetime DEFAULT time::now();

            DEFINE INDEX IF NOT EXISTS idx_metadata_key ON TABLE metadata COLUMNS key UNIQUE;
        "#;

        // Execute schema definitions
        self.db
            .query(node_schema)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create nodes schema: {}", e)))?;

        self.db
            .query(edge_schema)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create edges schema: {}", e)))?;

        self.db
            .query(version_schema)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create versions schema: {}", e)))?;

        self.db
            .query(metadata_schema)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create metadata schema: {}", e)))?;

        // Initialize schema version if not exists
        let _: Option<SchemaVersion> = self.db
            .create(("schema_versions", "current"))
            .content(SchemaVersion {
                version: 1,
                applied_at: chrono::Utc::now().to_rfc3339(),
                description: "Initial schema".to_string(),
            })
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to set schema version: {}", e)))?;

        *self.schema_version.write().unwrap() = 1;

        info!("Schema initialized successfully");
        Ok(())
    }

    /// Run database migrations
    async fn migrate(&self) -> Result<()> {
        let current_version = *self.schema_version.read().unwrap();
        info!("Running migrations from version {}", current_version);

        // Define migrations as functions for easy addition
        let migrations = vec![
            // Migration 2: Add indexes for performance
            (2u32, "Add performance indexes", |db: &Surreal<Any>| async move {
                db.query(r#"
                    DEFINE INDEX IF NOT EXISTS idx_nodes_created_at ON TABLE nodes COLUMNS created_at;
                    DEFINE INDEX IF NOT EXISTS idx_edges_created_at ON TABLE edges COLUMNS created_at;
                "#)
                .await
                .map_err(|e| CodeGraphError::Database(format!("Migration 2 failed: {}", e)))
            }),
        ];

        for (version, description, migration) in migrations {
            if version > current_version {
                info!("Applying migration {}: {}", version, description);
                migration(&self.db).await?;

                // Record migration
                let _: Option<SchemaVersion> = self.db
                    .create(("schema_versions", format!("v{}", version)))
                    .content(SchemaVersion {
                        version,
                        applied_at: chrono::Utc::now().to_rfc3339(),
                        description: description.to_string(),
                    })
                    .await
                    .map_err(|e| CodeGraphError::Database(format!("Failed to record migration: {}", e)))?;

                *self.schema_version.write().unwrap() = version;
            }
        }

        info!("Migrations completed successfully");
        Ok(())
    }

    /// Convert CodeNode to SurrealDB-compatible format
    fn node_to_surreal(&self, node: &CodeNode) -> Result<HashMap<String, JsonValue>> {
        let mut data = HashMap::new();

        data.insert("id".to_string(), JsonValue::String(node.id.to_string()));
        data.insert("name".to_string(), JsonValue::String(node.name.to_string()));

        if let Some(node_type) = &node.node_type {
            data.insert("node_type".to_string(), JsonValue::String(format!("{:?}", node_type)));
        }

        if let Some(language) = &node.language {
            data.insert("language".to_string(), JsonValue::String(format!("{:?}", language)));
        }

        if let Some(content) = &node.content {
            data.insert("content".to_string(), JsonValue::String(content.to_string()));
        }

        data.insert("file_path".to_string(), JsonValue::String(node.location.file_path.to_string()));
        data.insert("start_line".to_string(), JsonValue::Number(node.location.start_line.into()));
        data.insert("end_line".to_string(), JsonValue::Number(node.location.end_line.into()));

        if let Some(embedding) = &node.embedding {
            let emb_values: Vec<JsonValue> = embedding.iter().map(|&f| JsonValue::from(f as f64)).collect();
            data.insert("embedding".to_string(), JsonValue::Array(emb_values));
        }

        if let Some(complexity) = node.complexity {
            data.insert("complexity".to_string(), JsonValue::from(complexity as f64));
        }

        // Store metadata as nested object
        let mut metadata_obj = HashMap::new();
        for (key, value) in &node.metadata.attributes {
            metadata_obj.insert(key.clone(), JsonValue::String(value.clone()));
        }
        data.insert("metadata".to_string(), serde_json::to_value(metadata_obj).unwrap());

        Ok(data)
    }

    /// Convert SurrealDB result to CodeNode
    fn surreal_to_node(&self, data: HashMap<String, JsonValue>) -> Result<CodeNode> {
        use codegraph_core::{Language, Location, Metadata, NodeType, SharedStr};

        let id_str = data.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeGraphError::Deserialization("Missing node id".to_string()))?;
        let id = NodeId::parse_str(id_str)
            .map_err(|e| CodeGraphError::Deserialization(format!("Invalid node id: {}", e)))?;

        let name = data.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeGraphError::Deserialization("Missing node name".to_string()))?;

        let node_type = data.get("node_type")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<NodeType>(&format!("\"{}\"", s)).ok());

        let language = data.get("language")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<Language>(&format!("\"{}\"", s)).ok());

        let content = data.get("content")
            .and_then(|v| v.as_str())
            .map(|s| SharedStr::from(s));

        let file_path = data.get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let start_line = data.get("start_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let end_line = data.get("end_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let embedding = data.get("embedding")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect());

        let complexity = data.get("complexity")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);

        let mut metadata = Metadata::new();
        if let Some(meta_obj) = data.get("metadata").and_then(|v| v.as_object()) {
            for (key, value) in meta_obj {
                if let Some(val_str) = value.as_str() {
                    metadata.attributes.insert(key.clone(), val_str.to_string());
                }
            }
        }

        Ok(CodeNode {
            id,
            name: SharedStr::from(name),
            node_type,
            language,
            location: Location {
                file_path: SharedStr::from(file_path),
                start_line,
                end_line,
            },
            content,
            metadata,
            embedding,
            complexity,
        })
    }
}

#[async_trait]
impl GraphStore for SurrealDbStorage {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        debug!("Adding node: {}", node.id);

        let data = self.node_to_surreal(&node)?;
        let node_id = node.id.to_string();

        let _: Option<HashMap<String, JsonValue>> = self.db
            .create(("nodes", &node_id))
            .content(data)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to create node: {}", e)))?;

        // Update cache
        if self.config.cache_enabled {
            self.node_cache.insert(node.id, node);
        }

        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        // Check cache first
        if self.config.cache_enabled {
            if let Some(cached) = self.node_cache.get(&id) {
                return Ok(Some(cached.clone()));
            }
        }

        let node_id = id.to_string();
        let result: Option<HashMap<String, JsonValue>> = self.db
            .select(("nodes", &node_id))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to get node: {}", e)))?;

        match result {
            Some(data) => {
                let node = self.surreal_to_node(data)?;

                // Update cache
                if self.config.cache_enabled {
                    self.node_cache.insert(id, node.clone());
                }

                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        debug!("Updating node: {}", node.id);

        let data = self.node_to_surreal(&node)?;
        let node_id = node.id.to_string();

        let _: Option<HashMap<String, JsonValue>> = self.db
            .update(("nodes", &node_id))
            .content(data)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to update node: {}", e)))?;

        // Update cache
        if self.config.cache_enabled {
            self.node_cache.insert(node.id, node);
        }

        Ok(())
    }

    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        debug!("Removing node: {}", id);

        let node_id = id.to_string();
        let _: Option<HashMap<String, JsonValue>> = self.db
            .delete(("nodes", &node_id))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to delete node: {}", e)))?;

        // Remove from cache
        if self.config.cache_enabled {
            self.node_cache.remove(&id);
        }

        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        debug!("Finding nodes by name: {}", name);

        let query = "SELECT * FROM nodes WHERE name = $name";
        let mut result = self.db
            .query(query)
            .bind(("name", name))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to query nodes: {}", e)))?;

        let nodes: Vec<HashMap<String, JsonValue>> = result
            .take(0)
            .map_err(|e| CodeGraphError::Database(format!("Failed to extract query results: {}", e)))?;

        nodes.into_iter().map(|data| self.surreal_to_node(data)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_surrealdb_storage_creation() {
        let config = SurrealDbConfig {
            connection: "mem://".to_string(),
            ..Default::default()
        };

        let storage = SurrealDbStorage::new(config).await;
        assert!(storage.is_ok());
    }
}
