use crate::edge::CodeEdge;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use surrealdb::{engine::any::Any, opt::auth::Root, Error as SurrealError, Surreal};
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
        let connection = env::var("CODEGRAPH_SURREALDB_URL")
            .unwrap_or_else(|_| "ws://localhost:3004".to_string());
        let namespace =
            env::var("CODEGRAPH_SURREALDB_NAMESPACE").unwrap_or_else(|_| "ouroboros".to_string());
        let database =
            env::var("CODEGRAPH_SURREALDB_DATABASE").unwrap_or_else(|_| "codegraph".to_string());
        let username = env::var("CODEGRAPH_SURREALDB_USERNAME")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let password = env::var("CODEGRAPH_SURREALDB_PASSWORD")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            connection,
            namespace,
            database,
            username,
            password,
            strict_mode: false,
            auto_migrate: true,
            cache_enabled: true,
        }
    }
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

        info!("SurrealDB storage initialized successfully (schema management disabled)");
        Ok(storage)
    }

    /// Initialize database schema with flexible design (unused when schema managed externally)
    #[allow(dead_code)]
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
            DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record<nodes>;
            DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record<nodes>;
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

    /// Run database migrations (unused when schema managed externally)
    #[allow(dead_code)]
    async fn migrate(&self) -> Result<()> {
        let _current_version = *self.schema_version.read().unwrap();
        info!("Running migrations from version {}", _current_version);
        // TODO: Fix lifetime issues with async closures in migrations
        // Migrations temporarily disabled
        Ok(())
    }

    /// Vector search using SurrealDB HNSW indexes
    /// Returns node IDs and similarity scores
    pub async fn vector_search_knn(
        &self,
        embedding_column: &str,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
    ) -> Result<Vec<(String, f32)>> {
        let column = normalized_embedding_column(embedding_column).ok_or_else(|| {
            CodeGraphError::Configuration(format!(
                "Unsupported embedding column '{}'",
                embedding_column
            ))
        })?;

        info!(
            "Executing HNSW vector search on {} with limit={}, ef_search={}",
            column, limit, ef_search
        );

        // Convert f32 to f64 for SurrealDB
        let query_vec: Vec<f64> = query_embedding.iter().map(|&f| f as f64).collect();

        // SurrealDB HNSW search using <|K,EF|> operator
        // vector::distance::knn() reuses pre-computed distance from HNSW
        let query = format!(
            r#"
            SELECT id, vector::distance::knn() AS score
            FROM nodes
            WHERE {column} <|$limit,$ef_search|> $query_embedding
            ORDER BY score ASC
            LIMIT $limit
        "#
        );

        let mut result = self
            .db
            .query(&query)
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
        embedding_column: &str,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
        node_type: Option<String>,
        language: Option<String>,
        file_path_pattern: Option<String>,
    ) -> Result<Vec<(String, f32)>> {
        let column = normalized_embedding_column(embedding_column).ok_or_else(|| {
            CodeGraphError::Configuration(format!(
                "Unsupported embedding column '{}'",
                embedding_column
            ))
        })?;

        info!(
            "Executing filtered HNSW search on {}: type={:?}, lang={:?}, path={:?}",
            column, node_type, language, file_path_pattern
        );

        let query_vec: Vec<f64> = query_embedding.iter().map(|&f| f as f64).collect();

        // Build dynamic WHERE clause
        let mut where_clauses = vec![format!("{column} <|$limit,$ef_search|> $query_embedding")];

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
    fn node_to_surreal(&self, node: &CodeNode) -> Result<SurrealNodeRecord> {
        let metadata = if node.metadata.attributes.is_empty() {
            None
        } else {
            Some(node.metadata.attributes.clone())
        };

        let (
            embedding_384,
            embedding_768,
            embedding_1024,
            embedding_2048,
            embedding_2560,
            embedding_4096,
        ) = if let Some(values) = &node.embedding {
            let embedding_vec: Vec<f64> = values.iter().map(|&f| f as f64).collect();
            match values.len() {
                384 => (Some(embedding_vec), None, None, None, None, None),
                768 => (None, Some(embedding_vec), None, None, None, None),
                1024 => (None, None, Some(embedding_vec), None, None, None),
                2560 => (None, None, None, None, Some(embedding_vec), None),
                4096 => (None, None, None, None, None, Some(embedding_vec)),
                _ => (None, None, None, Some(embedding_vec), None, None),
            }
        } else {
            (None, None, None, None, None, None)
        };

        let embedding_model = node.metadata.attributes.get("embedding_model").cloned();

        Ok(SurrealNodeRecord {
            id: node.id.to_string(),
            name: node.name.to_string(),
            node_type: node.node_type.as_ref().map(|value| format!("{:?}", value)),
            language: node.language.as_ref().map(|value| format!("{:?}", value)),
            content: node.content.as_ref().map(|c| c.to_string()),
            file_path: node.location.file_path.to_string(),
            start_line: node.location.line,
            end_line: node.location.end_line,
            embedding_384,
            embedding_768,
            embedding_1024,
            embedding_2048,
            embedding_2560,
            embedding_4096,
            embedding_model,
            complexity: node.complexity,
            metadata,
            project_id: node.metadata.attributes.get("project_id").cloned(),
            organization_id: node.metadata.attributes.get("organization_id").cloned(),
            repository_url: node.metadata.attributes.get("repository_url").cloned(),
            domain: node.metadata.attributes.get("domain").cloned(),
        })
    }

    pub async fn upsert_nodes_batch(&mut self, nodes: &[CodeNode]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let mut records = Vec::with_capacity(nodes.len());
        for node in nodes {
            records.push(self.node_to_surreal(node)?);
        }

        self.db
            .query(UPSERT_NODES_QUERY)
            .bind(("data", records.clone()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to upsert node batch ({} items): {}",
                    records.len(),
                    truncate_surreal_error(&e)
                ))
            })?;

        if self.config.cache_enabled {
            for node in nodes {
                self.node_cache.insert(node.id, node.clone());
            }
        }

        Ok(())
    }

    pub async fn upsert_edges_batch(&mut self, edges: &[CodeEdge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }

        let payloads: Vec<JsonValue> = edges
            .iter()
            .map(|record| {
                let metadata_value =
                    serde_json::to_value(&record.metadata).unwrap_or_else(|_| JsonValue::Null);
                json!({
                    "id": record.id.to_string(),
                    "from": record.from.to_string(),
                    "to": record.to.to_string(),
                    "edge_type": record.edge_type.to_string(),
                    "weight": record.weight,
                    "metadata": metadata_value,
                })
            })
            .collect();

        self.db
            .query(UPSERT_EDGES_QUERY)
            .bind(("data", payloads))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to upsert edge batch ({} items): {}",
                    edges.len(),
                    truncate_surreal_error(&e)
                ))
            })?;

        Ok(())
    }

    pub async fn upsert_symbol_embeddings_batch(
        &self,
        records: &[SymbolEmbeddingRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        self.db
            .query(UPSERT_SYMBOL_EMBEDDINGS_QUERY)
            .bind(("data", records.to_vec()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to upsert symbol embedding batch ({} items): {}",
                    records.len(),
                    truncate_surreal_error(&e)
                ))
            })?;

        Ok(())
    }

    pub async fn update_node_embeddings_batch(
        &self,
        records: &[NodeEmbeddingRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        for record in records {
            let column = normalized_embedding_column(record.column).ok_or_else(|| {
                CodeGraphError::Database(format!(
                    "Unsupported embedding column '{}'",
                    record.column
                ))
            })?;

            let query = format!(
                "UPDATE type::thing('nodes', $id) SET {} = $embedding, updated_at = time::now();",
                column
            );

            self.db
                .query(query)
                .bind(("id", record.id.clone()))
                .bind(("embedding", record.embedding.clone()))
                .await
                .map_err(|e| {
                    CodeGraphError::Database(format!(
                        "Failed to update node embedding {}: {}",
                        record.id,
                        truncate_surreal_error(&e)
                    ))
                })?;
        }

        Ok(())
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

        let embedding = [
            "embedding_4096",
            "embedding_2048",
            "embedding_1024",
            "embedding_384",
        ]
        .iter()
        .find_map(|field| data.get(*field).and_then(|v| v.as_array()))
        .map(|arr| {
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

    pub async fn add_code_edge(&mut self, edge: CodeEdge) -> Result<()> {
        self.upsert_edges_batch(std::slice::from_ref(&edge)).await
    }

    pub async fn add_code_edges(&mut self, edges: Vec<CodeEdge>) -> Result<()> {
        self.upsert_edges_batch(&edges).await
    }

    pub async fn upsert_symbol_embedding(&self, record: SymbolEmbeddingUpsert<'_>) -> Result<()> {
        let owned = SymbolEmbeddingRecord::from(&record);
        self.upsert_symbol_embeddings_batch(std::slice::from_ref(&owned))
            .await
    }

    pub async fn upsert_project_metadata(&self, record: ProjectMetadataRecord) -> Result<()> {
        let payload = json!({
            "project_id": record.project_id,
            "name": record.name,
            "root_path": record.root_path,
            "primary_language": record.primary_language,
            "file_count": record.file_count,
            "node_count": record.node_count,
            "edge_count": record.edge_count,
            "avg_coverage_score": record.avg_coverage_score,
            "last_analyzed": record.last_analyzed,
            "codegraph_version": record.codegraph_version,
            "organization_id": record.organization_id,
            "domain": record.domain,
            "metadata": json!({}),
        });

        let _: Option<HashMap<String, JsonValue>> = self
            .db
            .update(("project_metadata", record.project_id.clone()))
            .content(payload)
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to upsert project metadata: {}", e))
            })?;

        Ok(())
    }

    /// Upsert single file metadata record
    pub async fn upsert_file_metadata(&self, record: &FileMetadataRecord) -> Result<()> {
        let record_id = record.id();
        let payload = json!({
            "file_path": record.file_path,
            "project_id": record.project_id,
            "content_hash": record.content_hash,
            "modified_at": record.modified_at,
            "file_size": record.file_size,
            "last_indexed_at": record.last_indexed_at,
            "node_count": record.node_count,
            "edge_count": record.edge_count,
            "language": record.language,
            "parse_errors": record.parse_errors,
        });

        let _: Option<HashMap<String, JsonValue>> = self
            .db
            .update(("file_metadata", record_id))
            .content(payload)
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to upsert file metadata: {}", e))
            })?;

        Ok(())
    }

    /// Batch upsert file metadata records
    pub async fn upsert_file_metadata_batch(&self, records: &[FileMetadataRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let query = r#"
            LET $batch = $data;
            FOR $doc IN $batch {
                UPSERT type::thing('file_metadata', $doc.id) CONTENT $doc;
            }
        "#;

        let payloads: Vec<JsonValue> = records
            .iter()
            .map(|r| {
                json!({
                    "id": r.id(),
                    "file_path": r.file_path,
                    "project_id": r.project_id,
                    "content_hash": r.content_hash,
                    "modified_at": r.modified_at,
                    "file_size": r.file_size,
                    "last_indexed_at": r.last_indexed_at,
                    "node_count": r.node_count,
                    "edge_count": r.edge_count,
                    "language": r.language,
                    "parse_errors": r.parse_errors,
                })
            })
            .collect();

        self.db
            .query(query)
            .bind(("data", payloads))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to batch upsert file metadata ({} items): {}",
                    records.len(),
                    truncate_surreal_error(&e)
                ))
            })?;

        // Verify writes actually succeeded by checking at least one record exists
        // SurrealDB FOR loops can silently skip operations if schema/table missing
        // Use the first record's project_id to count records for that project
        let project_id = &records[0].project_id;
        let verify_query = "SELECT VALUE count() FROM file_metadata WHERE project_id = $project_id";
        let mut verify_resp = self
            .db
            .query(verify_query)
            .bind(("project_id", project_id.clone()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to verify file metadata writes: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        let written_count: Option<i64> = verify_resp.take(0).map_err(|e| {
            CodeGraphError::Database(format!("Failed to extract verification count: {}", e))
        })?;
        let written = written_count.unwrap_or(0) as usize;

        if written < records.len() {
            return Err(CodeGraphError::Database(format!(
                "File metadata batch upsert verification failed: wrote {} of {} records. \
                 Check that file_metadata table exists and schema is applied.",
                written,
                records.len()
            )));
        }

        debug!("Batch upserted {} file metadata records", records.len());
        Ok(())
    }

    /// Get all file metadata for a project
    pub async fn get_file_metadata_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<FileMetadataRecord>> {
        let query = "SELECT * FROM file_metadata WHERE project_id = $project_id";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to query file metadata: {}", e))
            })?;

        let records: Vec<FileMetadataRecord> = result.take(0).map_err(|e| {
            CodeGraphError::Database(format!("Failed to extract file metadata results: {}", e))
        })?;

        debug!(
            "Retrieved {} file metadata records for project {}",
            records.len(),
            project_id
        );
        Ok(records)
    }

    /// Delete file metadata records for specific files
    pub async fn delete_file_metadata_for_files(
        &self,
        project_id: &str,
        file_paths: &[String],
    ) -> Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }

        let query =
            "DELETE file_metadata WHERE project_id = $project_id AND file_path IN $file_paths";
        self.db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .bind(("file_paths", file_paths.to_vec()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to delete file metadata: {}", e))
            })?;

        debug!(
            "Deleted {} file metadata records for project {}",
            file_paths.len(),
            project_id
        );
        Ok(())
    }

    /// Delete all nodes for a project
    pub async fn delete_nodes_for_project(&self, project_id: &str) -> Result<usize> {
        let query = "DELETE nodes WHERE project_id = $project_id RETURN BEFORE";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to delete nodes: {}", e)))?;

        let deleted: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let count = deleted.len();

        // Clear cache for deleted nodes
        if self.config.cache_enabled {
            for data in deleted {
                if let Some(id_str) = data.get("id").and_then(|v| v.as_str()) {
                    if let Ok(id) = NodeId::parse_str(id_str) {
                        self.node_cache.remove(&id);
                    }
                }
            }
        }

        info!("Deleted {} nodes for project {}", count, project_id);
        Ok(count)
    }

    /// Delete all edges where from or to node belongs to project
    pub async fn delete_edges_for_project(&self, project_id: &str) -> Result<usize> {
        // First get all node IDs for the project
        let query = "SELECT VALUE id FROM nodes WHERE project_id = $project_id";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to query node IDs: {}", e)))?;

        let node_ids: Vec<String> = result.take(0).unwrap_or_default();

        if node_ids.is_empty() {
            return Ok(0);
        }

        // Delete edges where from or to is in node_ids
        let delete_query = r#"
            LET $node_ids = $ids;
            DELETE edges WHERE
                string::split(string::trim(from), ':')[1] IN $node_ids OR
                string::split(string::trim(to), ':')[1] IN $node_ids
            RETURN BEFORE
        "#;

        let mut result = self
            .db
            .query(delete_query)
            .bind(("ids", node_ids))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to delete edges: {}", e)))?;

        let deleted: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let count = deleted.len();

        info!("Deleted {} edges for project {}", count, project_id);
        Ok(count)
    }

    /// Delete all symbol embeddings for a project
    pub async fn delete_symbol_embeddings_for_project(&self, project_id: &str) -> Result<usize> {
        let query = "DELETE symbol_embeddings WHERE project_id = $project_id RETURN BEFORE";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to delete symbol embeddings: {}", e))
            })?;

        let deleted: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let count = deleted.len();

        info!(
            "Deleted {} symbol embeddings for project {}",
            count, project_id
        );
        Ok(count)
    }

    /// Delete all file metadata for a project
    pub async fn delete_file_metadata_for_project(&self, project_id: &str) -> Result<usize> {
        let query = "DELETE file_metadata WHERE project_id = $project_id RETURN BEFORE";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to delete file metadata: {}", e))
            })?;

        let deleted: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let count = deleted.len();

        info!(
            "Deleted {} file metadata records for project {}",
            count, project_id
        );
        Ok(count)
    }

    /// Clean slate: Delete ALL data for a project (nodes, edges, embeddings, file metadata)
    /// Used when --force flag is set
    pub async fn clean_project_data(&self, project_id: &str) -> Result<()> {
        info!(
            "🧹 Starting clean slate deletion for project: {}",
            project_id
        );

        // Delete in order: edges first (referential integrity), then nodes, then metadata
        let edges_deleted = self.delete_edges_for_project(project_id).await?;
        let nodes_deleted = self.delete_nodes_for_project(project_id).await?;
        let symbols_deleted = self
            .delete_symbol_embeddings_for_project(project_id)
            .await?;
        let files_deleted = self.delete_file_metadata_for_project(project_id).await?;

        info!(
            "🧹 Clean slate complete: {} edges, {} nodes, {} symbols, {} files deleted",
            edges_deleted, nodes_deleted, symbols_deleted, files_deleted
        );

        Ok(())
    }

    pub async fn update_node_embedding(&self, node_id: NodeId, embedding: &[f32]) -> Result<()> {
        let embedding_column = surreal_embedding_column_for_dimension(embedding.len());
        let record = NodeEmbeddingRecord {
            id: node_id.to_string(),
            column: embedding_column,
            embedding: embedding.iter().map(|&f| f as f64).collect(),
            updated_at: Utc::now(),
        };
        self.update_node_embeddings_batch(std::slice::from_ref(&record))
            .await
    }
}

#[async_trait]
impl GraphStore for SurrealDbStorage {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        self.upsert_nodes_batch(std::slice::from_ref(&node)).await
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

fn symbol_embedding_record_id(project_id: &str, normalized_symbol: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    hasher.update(b":");
    hasher.update(normalized_symbol.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct SymbolEmbeddingUpsert<'a> {
    pub symbol: &'a str,
    pub normalized_symbol: &'a str,
    pub project_id: &'a str,
    pub organization_id: Option<&'a str>,
    pub embedding: &'a [f32],
    pub embedding_model: &'a str,
    pub embedding_column: &'a str,
    pub node_id: Option<&'a str>,
    pub source_edge_id: Option<&'a str>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolEmbeddingRecord {
    pub id: String,
    pub symbol: String,
    pub normalized_symbol: String,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_384: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_768: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_1024: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_1536: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_2048: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_2560: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_3072: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_4096: Option<Vec<f64>>,
    pub embedding_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_edge_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonValue>,
    pub last_computed_at: DateTime<Utc>,
    pub access_count: i64,
}

impl SymbolEmbeddingRecord {
    pub fn new(
        project_id: &str,
        organization_id: Option<&str>,
        symbol: &str,
        normalized_symbol: &str,
        embedding: &[f32],
        embedding_model: &str,
        embedding_column: &str,
        node_id: Option<&str>,
        source_edge_id: Option<&str>,
        metadata: Option<JsonValue>,
    ) -> Self {
        let embedding_vec: Vec<f64> = embedding.iter().map(|&f| f as f64).collect();
        let (
            embedding_384,
            embedding_768,
            embedding_1024,
            embedding_1536,
            embedding_2048,
            embedding_2560,
            embedding_3072,
            embedding_4096,
        ) = match embedding_column {
            SURR_EMBEDDING_COLUMN_384 => (
                Some(embedding_vec),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_768 => (
                None,
                Some(embedding_vec),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_1024 => (
                None,
                None,
                Some(embedding_vec),
                None,
                None,
                None,
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_1536 => (
                None,
                None,
                None,
                Some(embedding_vec),
                None,
                None,
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_2048 => (
                None,
                None,
                None,
                None,
                Some(embedding_vec),
                None,
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_2560 => (
                None,
                None,
                None,
                None,
                None,
                Some(embedding_vec),
                None,
                None,
            ),
            SURR_EMBEDDING_COLUMN_3072 => (
                None,
                None,
                None,
                None,
                None,
                None,
                Some(embedding_vec),
                None,
            ),
            SURR_EMBEDDING_COLUMN_4096 => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(embedding_vec),
            ),
            _ => (None, None, None, None, None, None, None, None),
        };

        SymbolEmbeddingRecord {
            id: symbol_embedding_record_id(project_id, normalized_symbol),
            symbol: symbol.to_string(),
            normalized_symbol: normalized_symbol.to_string(),
            project_id: project_id.to_string(),
            organization_id: organization_id.map(|s| s.to_string()),
            embedding_384,
            embedding_768,
            embedding_1024,
            embedding_1536,
            embedding_2048,
            embedding_2560,
            embedding_3072,
            embedding_4096,
            embedding_model: embedding_model.to_string(),
            node_id: node_id.map(|s| s.to_string()),
            source_edge_id: source_edge_id.map(|s| s.to_string()),
            metadata,
            last_computed_at: Utc::now(),
            access_count: 0,
        }
    }
}

impl<'a> From<&SymbolEmbeddingUpsert<'a>> for SymbolEmbeddingRecord {
    fn from(record: &SymbolEmbeddingUpsert<'a>) -> Self {
        SymbolEmbeddingRecord::new(
            record.project_id,
            record.organization_id,
            record.symbol,
            record.normalized_symbol,
            record.embedding,
            record.embedding_model,
            record.embedding_column,
            record.node_id,
            record.source_edge_id,
            record.metadata.clone(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProjectMetadataRecord {
    pub project_id: String,
    pub name: String,
    pub root_path: String,
    pub primary_language: Option<String>,
    pub file_count: i64,
    pub node_count: i64,
    pub edge_count: i64,
    pub avg_coverage_score: f32,
    pub last_analyzed: DateTime<Utc>,
    pub codegraph_version: String,
    pub organization_id: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataRecord {
    pub file_path: String,
    pub project_id: String,
    pub content_hash: String,
    pub modified_at: DateTime<Utc>,
    pub file_size: i64,
    pub last_indexed_at: DateTime<Utc>,
    pub node_count: i64,
    pub edge_count: i64,
    pub language: Option<String>,
    pub parse_errors: Option<Vec<String>>,
}

impl FileMetadataRecord {
    /// Generate a unique record ID based on project_id and file_path
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.project_id.as_bytes());
        hasher.update(b":");
        hasher.update(self.file_path.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, Serialize)]
struct SurrealNodeRecord {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    file_path: String,
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_384: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_768: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_1024: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_2048: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_2560: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_4096: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complexity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
}

pub const SURR_EMBEDDING_COLUMN_384: &str = "embedding_384";
pub const SURR_EMBEDDING_COLUMN_768: &str = "embedding_768";
pub const SURR_EMBEDDING_COLUMN_1024: &str = "embedding_1024";
pub const SURR_EMBEDDING_COLUMN_1536: &str = "embedding_1536";
pub const SURR_EMBEDDING_COLUMN_2048: &str = "embedding_2048";
pub const SURR_EMBEDDING_COLUMN_2560: &str = "embedding_2560";
pub const SURR_EMBEDDING_COLUMN_3072: &str = "embedding_3072";
pub const SURR_EMBEDDING_COLUMN_4096: &str = "embedding_4096";

fn normalized_embedding_column(column: &str) -> Option<&'static str> {
    match column {
        SURR_EMBEDDING_COLUMN_384 => Some(SURR_EMBEDDING_COLUMN_384),
        SURR_EMBEDDING_COLUMN_768 => Some(SURR_EMBEDDING_COLUMN_768),
        SURR_EMBEDDING_COLUMN_1024 => Some(SURR_EMBEDDING_COLUMN_1024),
        SURR_EMBEDDING_COLUMN_1536 => Some(SURR_EMBEDDING_COLUMN_1536),
        SURR_EMBEDDING_COLUMN_2048 => Some(SURR_EMBEDDING_COLUMN_2048),
        SURR_EMBEDDING_COLUMN_2560 => Some(SURR_EMBEDDING_COLUMN_2560),
        SURR_EMBEDDING_COLUMN_3072 => Some(SURR_EMBEDDING_COLUMN_3072),
        SURR_EMBEDDING_COLUMN_4096 => Some(SURR_EMBEDDING_COLUMN_4096),
        _ => None,
    }
}

pub fn surreal_embedding_column_for_dimension(dim: usize) -> &'static str {
    match dim {
        384 => SURR_EMBEDDING_COLUMN_384,
        768 => SURR_EMBEDDING_COLUMN_768,
        1024 => SURR_EMBEDDING_COLUMN_1024,
        1536 => SURR_EMBEDDING_COLUMN_1536,
        2048 => SURR_EMBEDDING_COLUMN_2048,
        2560 => SURR_EMBEDDING_COLUMN_2560,
        3072 => SURR_EMBEDDING_COLUMN_3072,
        4096 => SURR_EMBEDDING_COLUMN_4096,
        _ => {
            // Fallback to 2048 for unsupported dimensions
            // TODO: Consider returning Result<&'static str, Error> instead of silent fallback
            tracing::warn!(
                "Unsupported embedding dimension {}, falling back to 2048. \
                 Supported dimensions: 384, 768, 1024, 1536, 2048, 2560, 3072, 4096",
                dim
            );
            SURR_EMBEDDING_COLUMN_2048
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeEmbeddingRecord {
    pub id: String,
    pub column: &'static str,
    pub embedding: Vec<f64>,
    pub updated_at: DateTime<Utc>,
}

const UPSERT_NODES_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::thing('nodes', $doc.id) SET
        name = $doc.name,
        node_type = $doc.node_type,
        language = $doc.language,
        content = $doc.content,
        file_path = $doc.file_path,
        start_line = $doc.start_line,
        end_line = $doc.end_line,
        embedding_384 = $doc.embedding_384,
        embedding_768 = $doc.embedding_768,
        embedding_1024 = $doc.embedding_1024,
        embedding_1536 = $doc.embedding_1536,
        embedding_2048 = $doc.embedding_2048,
        embedding_2560 = $doc.embedding_2560,
        embedding_3072 = $doc.embedding_3072,
        embedding_4096 = $doc.embedding_4096,
        embedding_model = $doc.embedding_model,
        complexity = $doc.complexity,
        metadata = $doc.metadata,
        project_id = $doc.project_id,
        organization_id = $doc.organization_id,
        repository_url = $doc.repository_url,
        domain = $doc.domain,
        updated_at = time::now();
}
"#;

const UPSERT_EDGES_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::thing('edges', $doc.id) SET
        from = type::thing('nodes', $doc.from),
        to = type::thing('nodes', $doc.to),
        edge_type = $doc.edge_type,
        weight = $doc.weight,
        metadata = $doc.metadata,
        created_at = time::now();
}
"#;

const UPSERT_SYMBOL_EMBEDDINGS_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::thing('symbol_embeddings', $doc.id) SET
        symbol = $doc.symbol,
        normalized_symbol = $doc.normalized_symbol,
        project_id = $doc.project_id,
        organization_id = $doc.organization_id,
        embedding_384 = $doc.embedding_384,
        embedding_768 = $doc.embedding_768,
        embedding_1024 = $doc.embedding_1024,
        embedding_2048 = $doc.embedding_2048,
        embedding_2560 = $doc.embedding_2560,
        embedding_4096 = $doc.embedding_4096,
        embedding_model = $doc.embedding_model,
        node_id = $doc.node_id,
        source_edge_id = $doc.source_edge_id,
        metadata = $doc.metadata,
        access_count = $doc.access_count;
}
"#;

fn truncate_surreal_error(e: &SurrealError) -> String {
    const MAX_LEN: usize = 512;
    let mut msg = e.to_string();
    if msg.len() > MAX_LEN {
        msg.truncate(MAX_LEN);
        msg.push_str("…");
    }
    msg
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

    #[test]
    fn normalized_embedding_column_supports_2560() {
        assert_eq!(
            normalized_embedding_column(SURR_EMBEDDING_COLUMN_2560),
            Some(SURR_EMBEDDING_COLUMN_2560)
        );
    }

    #[test]
    fn normalized_embedding_column_rejects_unknown_column() {
        assert!(normalized_embedding_column("embedding_9999").is_none());
    }
}
