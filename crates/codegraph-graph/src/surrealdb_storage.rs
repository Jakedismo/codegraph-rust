use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::{engine::any::Any, opt::auth::Root, Surreal};
use tracing::{debug, info};

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
            connection: "ws://localhost:3004".to_string(),
            namespace: "ouroboros".to_string(),
            database: "codegraph".to_string(),
            username: Some("root".to_string()),
            password: Some("root".to_string()),
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
    /// Get the underlying SurrealDB connection
    /// This is useful for advanced operations like graph functions
    pub fn db(&self) -> Arc<Surreal<Any>> {
        Arc::clone(&self.db)
    }

    /// Create a new SurrealDB storage instance
    pub async fn new(config: SurrealDbConfig) -> Result<Self> {
        info!(
            "Initializing SurrealDB storage with connection: {}",
            config.connection
        );

        // Connect to SurrealDB
        let db: Surreal<Any> = Surreal::init();
        db.connect(&config.connection)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to connect: {}", e)))?;

        // Authenticate if credentials provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            db.signin(Root { username, password })
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
        self.db.query(node_schema).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to create nodes schema: {}", e))
        })?;

        self.db.query(edge_schema).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to create edges schema: {}", e))
        })?;

        self.db.query(version_schema).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to create versions schema: {}", e))
        })?;

        self.db.query(metadata_schema).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to create metadata schema: {}", e))
        })?;

        // Initialize schema version if not exists
        let _: Option<SchemaVersion> = self
            .db
            .create(("schema_versions", "current"))
            .content(SchemaVersion {
                version: 1,
                applied_at: chrono::Utc::now().to_rfc3339(),
                description: "Initial schema".to_string(),
            })
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to set schema version: {}", e))
            })?;

        *self.schema_version.write().unwrap() = 1;

        info!("Schema initialized successfully");
        Ok(())
    }

    /// Run database migrations
    async fn migrate(&self) -> Result<()> {
        let _current_version = *self.schema_version.read().unwrap();
        info!("Running migrations from version {}", _current_version);
        // TODO: Fix lifetime issues with async closures in migrations
        // Migrations temporarily disabled
        Ok(())
    }

    /*
    async fn _migrate_disabled(&self) -> Result<()> {
        let current_version = *self.schema_version.read().unwrap();
        info!("Running migrations from version {}", current_version);

        // Define migrations as functions for easy addition
        async fn migration_2(db: &Surreal<Any>) -> Result<()> {
            db.query(
                r#"
                DEFINE INDEX IF NOT EXISTS idx_nodes_created_at ON TABLE nodes COLUMNS created_at;
                DEFINE INDEX IF NOT EXISTS idx_edges_created_at ON TABLE edges COLUMNS created_at;
            "#,
            )
            .await
            .map_err(|e| CodeGraphError::Database(format!("Migration 2 failed: {}", e)))?;
            Ok(())
        }

        let migrations: Vec<(u32, &str, fn(&Surreal<Any>) -> _)> = vec![
            // Migration 2: Add indexes for performance
            (2u32, "Add performance indexes", |db| {
                Box::pin(migration_2(db)) as _
            }),
        ];

        for (version, description, migration) in migrations {
            if version > current_version {
                info!("Applying migration {}: {}", version, description);
                migration(&self.db).await?;

                // Record migration
                let _: Option<SchemaVersion> = self
                    .db
                    .create(("schema_versions", format!("v{}", version)))
                    .content(SchemaVersion {
                        version,
                        applied_at: chrono::Utc::now().to_rfc3339(),
                        description: description.to_string(),
                    })
                    .await
                    .map_err(|e| {
                        CodeGraphError::Database(format!("Failed to record migration: {}", e))
                    })?;

                *self.schema_version.write().unwrap() = version;
            }
        }

        info!("Migrations completed successfully");
        Ok(())
    }
    */

    /// Vector search using SurrealDB HNSW indexes
    /// Returns node IDs and similarity scores
    pub async fn vector_search_knn(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
    ) -> Result<Vec<(String, f32)>> {
        info!(
            "Executing HNSW vector search with limit={}, ef_search={}",
            limit, ef_search
        );

        // Convert f32 to f64 for SurrealDB
        let query_vec: Vec<f64> = query_embedding.iter().map(|&f| f as f64).collect();

        // SurrealDB HNSW search using <|K,EF|> operator
        // vector::distance::knn() reuses pre-computed distance from HNSW
        let query = r#"
            SELECT id, vector::distance::knn() AS score
            FROM nodes
            WHERE embedding <|$limit,$ef_search|> $query_embedding
            ORDER BY score ASC
            LIMIT $limit
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("query_embedding", query_vec))
            .bind(("limit", limit))
            .bind(("ef_search", ef_search))
            .await
            .map_err(|e| CodeGraphError::Database(format!("HNSW search failed: {}", e)))?;

        #[derive(Deserialize)]
        struct SearchResult {
            id: String,
            score: f64,
        }

        let results: Vec<SearchResult> = result.take(0).map_err(|e| {
            CodeGraphError::Database(format!("Failed to extract search results: {}", e))
        })?;

        Ok(results
            .into_iter()
            .map(|r| (r.id, r.score as f32))
            .collect())
    }

    /// Vector search with metadata filtering
    pub async fn vector_search_with_metadata(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
        node_type: Option<String>,
        language: Option<String>,
        file_path_pattern: Option<String>,
    ) -> Result<Vec<(String, f32)>> {
        info!(
            "Executing filtered HNSW search: type={:?}, lang={:?}, path={:?}",
            node_type, language, file_path_pattern
        );

        let query_vec: Vec<f64> = query_embedding.iter().map(|&f| f as f64).collect();

        // Build dynamic WHERE clause
        let mut where_clauses =
            vec!["embedding <|$limit,$ef_search|> $query_embedding".to_string()];

        if let Some(ref nt) = node_type {
            where_clauses.push(format!("node_type = '{}'", nt));
        }

        if let Some(ref lang) = language {
            where_clauses.push(format!("language = '{}'", lang));
        }

        if let Some(ref path) = file_path_pattern {
            // Support OR patterns like "src/|lib/"
            if path.contains('|') {
                let patterns: Vec<String> = path
                    .split('|')
                    .map(|p| format!("file_path CONTAINS '{}'", p))
                    .collect();
                where_clauses.push(format!("({})", patterns.join(" OR ")));
            } else {
                where_clauses.push(format!("file_path CONTAINS '{}'", path));
            }
        }

        let where_clause = where_clauses.join(" AND ");

        let query = format!(
            r#"
            SELECT id, vector::distance::knn() AS score
            FROM nodes
            WHERE {}
            ORDER BY score ASC
            LIMIT $limit
        "#,
            where_clause
        );

        let mut result = self
            .db
            .query(&query)
            .bind(("query_embedding", query_vec))
            .bind(("limit", limit))
            .bind(("ef_search", ef_search))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Filtered HNSW search failed: {}", e)))?;

        #[derive(Deserialize)]
        struct SearchResult {
            id: String,
            score: f64,
        }

        let results: Vec<SearchResult> = result.take(0).map_err(|e| {
            CodeGraphError::Database(format!("Failed to extract filtered results: {}", e))
        })?;

        Ok(results
            .into_iter()
            .map(|r| (r.id, r.score as f32))
            .collect())
    }

    /// Get multiple nodes by their IDs in one query
    pub async fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<CodeNode>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        debug!("Getting {} nodes by IDs", ids.len());

        // Check cache first for all IDs
        let mut nodes = Vec::new();
        let mut missing_ids = Vec::new();

        if self.config.cache_enabled {
            for id_str in ids {
                if let Ok(id) = NodeId::parse_str(id_str) {
                    if let Some(cached) = self.node_cache.get(&id) {
                        nodes.push(cached.clone());
                    } else {
                        missing_ids.push(id_str.clone());
                    }
                }
            }
        } else {
            missing_ids = ids.to_vec();
        }

        // Fetch missing nodes from database
        if !missing_ids.is_empty() {
            let query = "SELECT * FROM nodes WHERE id IN $ids";
            let mut result = self
                .db
                .query(query)
                .bind(("ids", missing_ids))
                .await
                .map_err(|e| CodeGraphError::Database(format!("Failed to query nodes: {}", e)))?;

            let db_nodes: Vec<HashMap<String, JsonValue>> = result.take(0).map_err(|e| {
                CodeGraphError::Database(format!("Failed to extract query results: {}", e))
            })?;

            for data in db_nodes {
                let node = self.surreal_to_node(data)?;

                // Update cache
                if self.config.cache_enabled {
                    self.node_cache.insert(node.id, node.clone());
                }

                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    /// Convert CodeNode to SurrealDB-compatible format
    fn node_to_surreal(&self, node: &CodeNode) -> Result<HashMap<String, JsonValue>> {
        let mut data = HashMap::new();

        data.insert("id".to_string(), JsonValue::String(node.id.to_string()));
        data.insert("name".to_string(), JsonValue::String(node.name.to_string()));

        if let Some(node_type) = &node.node_type {
            data.insert(
                "node_type".to_string(),
                JsonValue::String(format!("{:?}", node_type)),
            );
        }

        if let Some(language) = &node.language {
            data.insert(
                "language".to_string(),
                JsonValue::String(format!("{:?}", language)),
            );
        }

        if let Some(content) = &node.content {
            data.insert(
                "content".to_string(),
                JsonValue::String(content.to_string()),
            );
        }

        data.insert(
            "file_path".to_string(),
            JsonValue::String(node.location.file_path.to_string()),
        );
        data.insert(
            "start_line".to_string(),
            JsonValue::Number(node.location.line.into()),
        );
        if let Some(end_line) = node.location.end_line {
            data.insert("end_line".to_string(), JsonValue::Number(end_line.into()));
        }

        if let Some(embedding) = &node.embedding {
            let emb_values: Vec<JsonValue> = embedding
                .iter()
                .map(|&f| JsonValue::from(f as f64))
                .collect();
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
        data.insert(
            "metadata".to_string(),
            serde_json::to_value(metadata_obj).unwrap(),
        );

        Ok(data)
    }

    /// Convert SurrealDB result to CodeNode
    fn surreal_to_node(&self, data: HashMap<String, JsonValue>) -> Result<CodeNode> {
        use codegraph_core::{Language, Location, Metadata, NodeType, SharedStr};

        let id_str = data
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeGraphError::Parse("Missing node id".to_string()))?;
        let id = NodeId::parse_str(id_str)
            .map_err(|e| CodeGraphError::Parse(format!("Invalid node id: {}", e)))?;

        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeGraphError::Parse("Missing node name".to_string()))?;

        let node_type = data
            .get("node_type")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<NodeType>(&format!("\"{}\"", s)).ok());

        let language = data
            .get("language")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<Language>(&format!("\"{}\"", s)).ok());

        let content = data
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| SharedStr::from(s));

        let file_path = data.get("file_path").and_then(|v| v.as_str()).unwrap_or("");

        let start_line = data.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let end_line = data.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let embedding = data.get("embedding").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect()
        });

        let complexity = data
            .get("complexity")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);

        let mut attributes = std::collections::HashMap::new();
        if let Some(meta_obj) = data.get("metadata").and_then(|v| v.as_object()) {
            for (key, value) in meta_obj {
                if let Some(val_str) = value.as_str() {
                    attributes.insert(key.clone(), val_str.to_string());
                }
            }
        }

        let metadata = Metadata {
            attributes,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        Ok(CodeNode {
            id,
            name: SharedStr::from(name),
            node_type,
            language,
            location: Location {
                file_path: file_path.to_string(),
                line: start_line as u32,
                column: 0,
                end_line: if end_line > 0 {
                    Some(end_line as u32)
                } else {
                    None
                },
                end_column: None,
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

        let _: Option<HashMap<String, JsonValue>> = self
            .db
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
        let result: Option<HashMap<String, JsonValue>> = self
            .db
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

        let _: Option<HashMap<String, JsonValue>> = self
            .db
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
        let _: Option<HashMap<String, JsonValue>> = self
            .db
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
        let name_owned = name.to_string();
        let mut result = self
            .db
            .query(query)
            .bind(("name", name_owned))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to query nodes: {}", e)))?;

        let nodes: Vec<HashMap<String, JsonValue>> = result.take(0).map_err(|e| {
            CodeGraphError::Database(format!("Failed to extract query results: {}", e))
        })?;

        nodes
            .into_iter()
            .map(|data| self.surreal_to_node(data))
            .collect()
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
