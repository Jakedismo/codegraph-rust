use crate::edge::CodeEdge;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use surrealdb::types::SurrealValue;
use surrealdb::{
    Error as SurrealError, Surreal,
    engine::any::Any,
    opt::auth::{Database, Root},
};
use tracing::{debug, info, warn};

/// Bundled schemas, applied automatically to a fresh embedded store.
const BUNDLED_SCHEMA_MAIN: &str = include_str!("../../../schema/codegraph.surql");
const BUNDLED_SCHEMA_EXPERIMENTAL: &str =
    include_str!("../../../schema/codegraph_graph_experimental.surql");

/// Directory under the project root that holds the embedded store.
pub const EMBEDDED_DB_DIR: &str = ".codegraph";
/// Name of the embedded store inside [`EMBEDDED_DB_DIR`].
pub const EMBEDDED_DB_NAME: &str = "db";

/// Namespace and database used for embedded stores (one store per project,
/// so these never need to vary).
const EMBEDDED_NAMESPACE: &str = "codegraph";
const EMBEDDED_DATABASE: &str = "codegraph";
const EMBEDDED_GRAPH_DATABASE: &str = "codegraph_graph";

/// Connection schemes handled by an in-process engine rather than a server.
const EMBEDDED_SCHEMES: [&str; 4] = ["surrealkv://", "file://", "rocksdb://", "mem://"];

/// True when the connection string opens an in-process engine.
pub fn is_embedded_connection(connection: &str) -> bool {
    EMBEDDED_SCHEMES
        .iter()
        .any(|scheme| connection.starts_with(scheme))
}

/// True for in-process engines backed by a directory. These lock the
/// directory, so a process must share one handle per store, and they get
/// the bundled schema on first open. `mem://` is excluded: every open is a
/// fresh, empty database, which tests rely on.
fn is_persistent_embedded_connection(connection: &str) -> bool {
    is_embedded_connection(connection) && !connection.starts_with("mem://")
}

/// Handles to embedded stores already opened by this process, keyed by connection string.
static EMBEDDED_HANDLES: OnceLock<tokio::sync::Mutex<HashMap<String, Surreal<Any>>>> =
    OnceLock::new();

/// Drop top-level `null` keys from batch payloads. A JSON `null` binds as
/// SurrealQL `NULL`, which `option<T>` fields reject; a missing key reads as
/// `NONE`, which they accept.
fn strip_null_fields(payloads: Vec<JsonValue>) -> Vec<JsonValue> {
    payloads
        .into_iter()
        .map(|mut payload| {
            if let JsonValue::Object(map) = &mut payload {
                map.retain(|_, v| !v.is_null());
            }
            payload
        })
        .collect()
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

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

impl SurrealDbConfig {
    /// Connection settings for a project.
    ///
    /// With `CODEGRAPH_SURREALDB_URL` set, this targets that server using the
    /// `CODEGRAPH_SURREALDB_*` settings. Otherwise it opens the project's own
    /// embedded SurrealKV store at `<project_root>/.codegraph/db`.
    pub fn for_project(project_root: &Path) -> Self {
        match env_value("CODEGRAPH_SURREALDB_URL") {
            Some(connection) => Self::remote(connection),
            None => Self::embedded(project_root),
        }
    }

    /// Settings for the embedded store under `project_root`.
    pub fn embedded(project_root: &Path) -> Self {
        let root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        Self {
            connection: format!("surrealkv://{}", Self::embedded_db_path(&root).display()),
            namespace: EMBEDDED_NAMESPACE.to_string(),
            database: if env_flag("CODEGRAPH_USE_GRAPH_SCHEMA") {
                EMBEDDED_GRAPH_DATABASE.to_string()
            } else {
                EMBEDDED_DATABASE.to_string()
            },
            username: None,
            password: None,
            strict_mode: false,
            auto_migrate: true,
            cache_enabled: true,
        }
    }

    /// Where the embedded store for `project_root` lives.
    pub fn embedded_db_path(project_root: &Path) -> PathBuf {
        project_root.join(EMBEDDED_DB_DIR).join(EMBEDDED_DB_NAME)
    }

    /// Settings for a SurrealDB server, with namespace, database and
    /// credentials taken from the `CODEGRAPH_SURREALDB_*` variables.
    pub fn remote(connection: String) -> Self {
        let namespace =
            env_value("CODEGRAPH_SURREALDB_NAMESPACE").unwrap_or_else(|| "ouroboros".to_string());

        // Optional toggle to point at experimental graph schema DB without changing table names.
        let database = if env_flag("CODEGRAPH_USE_GRAPH_SCHEMA") {
            env_value("CODEGRAPH_GRAPH_DB_DATABASE")
                .unwrap_or_else(|| EMBEDDED_GRAPH_DATABASE.to_string())
        } else {
            env_value("CODEGRAPH_SURREALDB_DATABASE").unwrap_or_else(|| "codegraph".to_string())
        };

        Self {
            connection,
            namespace,
            database,
            username: env_value("CODEGRAPH_SURREALDB_USERNAME"),
            password: env_value("CODEGRAPH_SURREALDB_PASSWORD"),
            strict_mode: false,
            auto_migrate: true,
            cache_enabled: true,
        }
    }

    /// True when this configuration opens an in-process engine.
    pub fn is_embedded(&self) -> bool {
        is_embedded_connection(&self.connection)
    }
}

impl Default for SurrealDbConfig {
    /// Settings for the project in the current working directory (see [`Self::for_project`]).
    fn default() -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::for_project(&cwd)
    }
}

/// Schema version tracking for migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchemaVersion {
    version: u32,
    applied_at: String,
    description: String,
}

/// Statistics from orphan cleanup operation
#[derive(Debug, Clone, Default)]
pub struct OrphanCleanupStats {
    pub chunks_deleted: usize,
    pub edges_deleted: usize,
    pub symbols_deleted: usize,
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

        let persistent_embedded = is_persistent_embedded_connection(&config.connection);
        let db = if persistent_embedded {
            Self::open_embedded(&config).await?
        } else {
            Self::connect_remote(&config).await?
        };

        // Select namespace and database
        db.use_ns(&config.namespace)
            .use_db(&config.database)
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to select namespace/database: {}", e))
            })?;

        if persistent_embedded && config.auto_migrate {
            Self::ensure_bundled_schema(&db).await?;
        }

        let storage = Self {
            db: Arc::new(db),
            config: config.clone(),
            node_cache: Arc::new(DashMap::new()),
            schema_version: Arc::new(std::sync::RwLock::new(0)),
        };

        info!("SurrealDB storage initialized successfully");
        Ok(storage)
    }

    /// Connect to a SurrealDB server (or a fresh `mem://` engine) and authenticate if credentials are set.
    async fn connect_remote(config: &SurrealDbConfig) -> Result<Surreal<Any>> {
        let db: Surreal<Any> = Surreal::init();
        db.connect(&config.connection)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to connect: {}", e)))?;

        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            let auth_result = if config.connection.starts_with("wss://") {
                // Prefer database-scoped auth for hosted / cloud setups
                db.signin(Database {
                    namespace: config.namespace.clone(),
                    database: config.database.clone(),
                    username: username.clone(),
                    password: password.clone(),
                })
                .await
            } else {
                db.signin(Root {
                    username: username.clone(),
                    password: password.clone(),
                })
                .await
            };

            auth_result
                .map_err(|e| CodeGraphError::Database(format!("Authentication failed: {}", e)))?;
        }

        Ok(db)
    }

    /// Open an embedded store, reusing the handle if this process already has it open.
    ///
    /// The engine locks its directory, so a second open of the same path from
    /// this process would fail; a second open from another process does fail.
    async fn open_embedded(config: &SurrealDbConfig) -> Result<Surreal<Any>> {
        let handles = EMBEDDED_HANDLES.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
        let mut handles = handles.lock().await;

        if let Some(db) = handles.get(&config.connection) {
            debug!(
                "Reusing embedded SurrealDB handle for {}",
                config.connection
            );
            return Ok(db.clone());
        }

        if let Some(path) = config.connection.strip_prefix("surrealkv://") {
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CodeGraphError::Database(format!(
                        "Failed to create {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
                Self::write_gitignore(parent);
            }
        }

        let db: Surreal<Any> = Surreal::init();
        db.connect(&config.connection).await.map_err(|e| {
            let message = e.to_string();
            if message.contains("already locked") {
                CodeGraphError::Database(format!(
                    "The embedded database {} is open in another process. Stop the other \
                     codegraph process (for example a running `codegraph start`) or set \
                     CODEGRAPH_SURREALDB_URL to use a shared SurrealDB server. ({})",
                    config.connection, message
                ))
            } else {
                CodeGraphError::Database(format!("Failed to open embedded database: {}", message))
            }
        })?;

        handles.insert(config.connection.clone(), db.clone());
        Ok(db)
    }

    /// Keep the store out of version control: `.codegraph/.gitignore` ignoring everything.
    fn write_gitignore(dir: &Path) {
        let gitignore = dir.join(".gitignore");
        if gitignore.exists() {
            return;
        }
        if let Err(e) = std::fs::write(&gitignore, "*\n") {
            warn!("Could not write {}: {}", gitignore.display(), e);
        }
    }

    /// Apply the bundled schema to a store that has none, and warn when the
    /// store was created from a different schema revision.
    async fn ensure_bundled_schema(db: &Surreal<Any>) -> Result<()> {
        let (name, schema) = if env_flag("CODEGRAPH_USE_GRAPH_SCHEMA") {
            ("experimental", BUNDLED_SCHEMA_EXPERIMENTAL)
        } else {
            ("main", BUNDLED_SCHEMA_MAIN)
        };
        let checksum = Sha256::digest(schema.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        if Self::has_table(db, "nodes").await? {
            Self::check_schema_checksum(db, name, &checksum).await;
            return Ok(());
        }

        info!("Applying bundled {} schema to new embedded database", name);
        db.query(schema)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Schema parsing failed: {}", e)))?
            .check()
            .map_err(|e| CodeGraphError::Database(format!("Schema application failed: {}", e)))?;

        db.query(
            "CREATE schema_versions:bundled SET version = 0, name = $name, checksum = $checksum, \
             description = 'Bundled schema applied on first open';",
        )
        .bind(("name", name.to_string()))
        .bind(("checksum", checksum))
        .await
        .map_err(|e| CodeGraphError::Database(format!("Failed to record schema: {}", e)))?
        .check()
        .map_err(|e| CodeGraphError::Database(format!("Failed to record schema: {}", e)))?;

        Ok(())
    }

    async fn has_table(db: &Surreal<Any>, table: &str) -> Result<bool> {
        let mut response = db
            .query("INFO FOR DB")
            .await
            .map_err(|e| CodeGraphError::Database(format!("INFO FOR DB failed: {}", e)))?;
        let info: Option<JsonValue> = response
            .take(0)
            .map_err(|e| CodeGraphError::Database(format!("INFO FOR DB unreadable: {}", e)))?;
        Ok(info
            .and_then(|v| v.get("tables").cloned())
            .and_then(|t| t.get(table).cloned())
            .is_some())
    }

    async fn check_schema_checksum(db: &Surreal<Any>, name: &str, checksum: &str) {
        let recorded = async {
            let mut response = db
                .query("SELECT name, checksum FROM schema_versions:bundled")
                .await
                .ok()?;
            let rows: Vec<JsonValue> = response.take(0).ok()?;
            let row = rows.into_iter().next()?;
            Some((
                row.get("name")?.as_str()?.to_string(),
                row.get("checksum")?.as_str()?.to_string(),
            ))
        }
        .await;

        match recorded {
            Some((recorded_name, recorded_checksum))
                if recorded_name == name && recorded_checksum == checksum => {}
            Some((recorded_name, _)) => warn!(
                "Embedded database was created from the {} schema at a different revision than \
                 this binary bundles ({}). Re-index with `codegraph index --force` to rebuild it.",
                recorded_name, name
            ),
            None => debug!("Embedded database has no bundled-schema record; skipping check"),
        }
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

        #[derive(Serialize, Deserialize, surrealdb::types::SurrealValue)]
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

        #[derive(Serialize, Deserialize, surrealdb::types::SurrealValue)]
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
                    match self.node_cache.get(&id) {
                        Some(cached) => {
                            nodes.push(cached.clone());
                        }
                        _ => {
                            missing_ids.push(id_str.clone());
                        }
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
            let metadata_json =
                serde_json::to_value(&node.metadata.attributes).unwrap_or(JsonValue::Null);
            let compressed = codegraph_core::compress_json(&metadata_json);
            Some(JsonValue::String(compressed))
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

        let content = node
            .content
            .as_ref()
            .map(|c| codegraph_core::compress_to_string(&c));

        Ok(SurrealNodeRecord {
            id: node.id.to_string(),
            name: node.name.to_string(),
            node_type: node.node_type.as_ref().map(|value| format!("{:?}", value)),
            language: node.language.as_ref().map(|value| format!("{:?}", value)),
            content,
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
            chunk_count: node
                .metadata
                .attributes
                .get("chunk_count")
                .and_then(|v| v.parse::<i64>().ok()),
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
            })?
            .check()
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Node batch upsert returned error ({} items): {}",
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
                    "project_id": record.project_id.clone(),
                })
            })
            .collect();

        self.db
            .query(UPSERT_EDGES_QUERY)
            .bind(("data", strip_null_fields(payloads)))
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

    pub async fn upsert_chunk_embeddings_batch(
        &self,
        records: &[ChunkEmbeddingRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        // Serialize to owned values to satisfy static requirement of Surreal bindings
        let owned: Vec<ChunkEmbeddingRecord> = records.to_vec();

        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.db
                .query(UPSERT_CHUNK_EMBEDDINGS_QUERY)
                .bind(("data", owned)),
        )
        .await
        .map_err(|_| {
            CodeGraphError::Database(format!(
                "Chunk upsert timed out after 10s ({} items)",
                records.len()
            ))
        })?
        .map_err(|e| {
            CodeGraphError::Database(format!(
                "Failed to upsert chunk embedding batch ({} items): {}",
                records.len(),
                truncate_surreal_error(&e)
            ))
        })?;

        // Ensure Surreal didn’t return per-statement errors that would otherwise be hidden
        resp.check().map_err(|e| {
            CodeGraphError::Database(format!(
                "Surreal chunk batch returned error ({} items): {}",
                records.len(),
                truncate_surreal_error(&e)
            ))
        })?;

        Ok(())
    }

    /// Resilient chunk upsert: on Surreal computation depth or connection reset, split batch and retry.
    pub async fn upsert_chunk_embeddings_resilient(
        &self,
        records: &[ChunkEmbeddingRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        // First, try a bulk INSERT (shallow query, no FOR loop)
        if let Err(err) = self.insert_chunk_embeddings_batch(records).await {
            let msg = err.to_string();
            let duplicate = msg.to_lowercase().contains("duplicate");
            let depth_hit = msg.contains("excessive computation depth")
                || msg.contains("ComputationDepth")
                || msg.contains("connection reset");

            // Only fall through to upsert/backoff on duplicate or depth issues; otherwise fail fast
            if !duplicate && !depth_hit {
                return Err(err);
            }
        } else {
            return Ok(());
        }

        // Iterative backoff to avoid recursive async futures
        let mut queue: Vec<(Vec<ChunkEmbeddingRecord>, u8)> = vec![(records.to_vec(), 3)];

        while let Some((mut batch, remaining)) = queue.pop() {
            if batch.is_empty() {
                continue;
            }

            match self.upsert_chunk_embeddings_batch(&batch).await {
                Ok(()) => {}
                Err(err) => {
                    let msg = err.to_string();
                    let depth_hit = msg.contains("excessive computation depth")
                        || msg.contains("ComputationDepth")
                        || msg.contains("connection reset");

                    if depth_hit && remaining > 0 && batch.len() > 1 {
                        let mid = batch.len() / 2;
                        let right = batch.split_off(mid);
                        queue.push((right, remaining - 1));
                        queue.push((batch, remaining - 1));
                    } else if depth_hit {
                        // Fall back to per-record upserts to keep queries shallow
                        for rec in batch {
                            self.upsert_chunk_embedding_single(&rec).await?;
                        }
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }

    async fn insert_chunk_embeddings_batch(&self, records: &[ChunkEmbeddingRecord]) -> Result<()> {
        let owned: Vec<ChunkEmbeddingRecord> = records.to_vec();
        let resp = self
            .db
            .query(INSERT_CHUNK_EMBEDDINGS_QUERY)
            .bind(("batch", owned))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to insert chunk embedding batch ({} items): {}",
                    records.len(),
                    truncate_surreal_error(&e)
                ))
            })?;

        resp.check().map_err(|e| {
            CodeGraphError::Database(format!(
                "Surreal chunk insert returned error ({} items): {}",
                records.len(),
                truncate_surreal_error(&e)
            ))
        })?;
        Ok(())
    }

    async fn upsert_chunk_embedding_single(&self, record: &ChunkEmbeddingRecord) -> Result<()> {
        let owned = record.clone();
        let resp = self
            .db
            .query(UPSERT_CHUNK_EMBEDDING_SINGLE_QUERY)
            .bind(("doc", owned))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to upsert chunk embedding: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        resp.check().map_err(|e| {
            CodeGraphError::Database(format!(
                "Surreal chunk upsert returned error: {}",
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
                "UPDATE type::record('nodes', $id) SET {} = $embedding, updated_at = time::now();",
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
            span: None,
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
        let query = r#"
            UPSERT type::record('project_metadata', $id) SET
                project_id = $pid,
                name = $name,
                root_path = $root,
                primary_language = $lang,
                file_count = $files,
                node_count = $nodes,
                edge_count = $edges,
                avg_coverage_score = $cov,
                last_analyzed = <datetime>$last_analyzed,
                codegraph_version = $ver,
                organization_id = $org,
                domain = $dom,
                metadata = $meta,
                updated_at = time::now();
        "#;

        self.db
            .query(query)
            .bind(("id", record.project_id.clone()))
            .bind(("pid", record.project_id))
            .bind(("name", record.name))
            .bind(("root", record.root_path))
            .bind(("lang", record.primary_language))
            .bind(("files", record.file_count))
            .bind(("nodes", record.node_count))
            .bind(("edges", record.edge_count))
            .bind(("cov", record.avg_coverage_score))
            .bind(("last_analyzed", record.last_analyzed))
            .bind(("ver", record.codegraph_version))
            .bind(("org", record.organization_id))
            .bind(("dom", record.domain))
            .bind(("meta", json!({})))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to upsert project metadata: {}", e))
            })?
            .check()
            .map_err(|e| {
                CodeGraphError::Database(format!("Project metadata upsert error: {}", e))
            })?;

        Ok(())
    }

    /// Upsert single file metadata record
    pub async fn upsert_file_metadata(&self, record: &FileMetadataRecord) -> Result<()> {
        let query = r#"
            UPSERT type::record('file_metadata', $id) SET
                file_path = $file_path,
                project_id = $project_id,
                content_hash = $content_hash,
                modified_at = <datetime>$modified_at,
                file_size = $file_size,
                last_indexed_at = <datetime>$last_indexed_at,
                node_count = $node_count,
                edge_count = $edge_count,
                language = $language,
                parse_errors = $parse_errors,
                updated_at = time::now();
        "#;

        let resp = self
            .db
            .query(query)
            .bind(("id", record.id()))
            .bind(("file_path", record.file_path.clone()))
            .bind(("project_id", record.project_id.clone()))
            .bind(("content_hash", record.content_hash.clone()))
            .bind(("modified_at", record.modified_at))
            .bind(("file_size", record.file_size))
            .bind(("last_indexed_at", record.last_indexed_at))
            .bind(("node_count", record.node_count))
            .bind(("edge_count", record.edge_count))
            .bind(("language", record.language.clone()))
            .bind(("parse_errors", record.parse_errors.clone()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("Failed to upsert file metadata: {}", e))
            })?;

        resp.check().map_err(|e| {
            CodeGraphError::Database(format!("Failed to upsert file metadata: {}", e))
        })?;

        Ok(())
    }

    /// Batch upsert file metadata records
    pub async fn upsert_file_metadata_batch(&self, records: &[FileMetadataRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        // Use SET syntax (like nodes/edges) instead of CONTENT to avoid ID conflicts
        // Cast datetime strings to SurrealDB datetime type using <datetime>
        let query = r#"
            LET $batch = $data;
            FOR $doc IN $batch {
                UPSERT type::record('file_metadata', $doc.id) SET
                    file_path = $doc.file_path,
                    project_id = $doc.project_id,
                    content_hash = $doc.content_hash,
                    modified_at = IF $doc.modified_at != NONE THEN <datetime>$doc.modified_at ELSE time::now() END,
                    file_size = $doc.file_size,
                    last_indexed_at = IF $doc.last_indexed_at != NONE THEN <datetime>$doc.last_indexed_at ELSE time::now() END,
                    node_count = $doc.node_count,
                    edge_count = $doc.edge_count,
                    language = $doc.language,
                    parse_errors = $doc.parse_errors,
                    updated_at = time::now();
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
            .bind(("data", strip_null_fields(payloads)))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to batch upsert file metadata ({} items): {}",
                    records.len(),
                    truncate_surreal_error(&e)
                ))
            })?
            .check()
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "File metadata batch upsert returned error: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        let project_id = records[0].project_id.clone();

        // Verify writes with retry and diagnostics
        let written = self
            .verify_project_file_count(
                &project_id,
                records.len(),
                3,
                std::time::Duration::from_millis(50),
            )
            .await?;

        if written < records.len() {
            // Fallback: attempt per-record upserts to ensure persistence and surface exact error
            warn!(
                "File metadata batch upsert verification shortfall: wrote {} of {}. Falling back to per-record upsert.",
                written,
                records.len()
            );
            for record in records {
                self.upsert_file_metadata(record).await.map_err(|e| {
                    CodeGraphError::Database(format!(
                        "File metadata fallback upsert failed for {}: {}",
                        record.file_path, e
                    ))
                })?;
            }

            // Re-verify after fallback
            self.verify_project_file_count(
                &project_id,
                records.len(),
                3,
                std::time::Duration::from_millis(100),
            )
            .await?;
        }

        debug!("Batch upserted {} file metadata records", records.len());
        Ok(())
    }

    async fn verify_project_file_count(
        &self,
        project_id: &str,
        expected: usize,
        attempts: usize,
        delay: std::time::Duration,
    ) -> Result<usize> {
        if let Ok(reached) = verify_with_retry(expected, attempts, delay, || async {
            self.get_project_file_count(project_id).await
        })
        .await
        {
            return Ok(reached);
        }

        let last_count = self.get_project_file_count(project_id).await.unwrap_or(0);
        let samples = self
            .sample_file_paths(project_id, 5)
            .await
            .unwrap_or_default();
        Err(CodeGraphError::Database(format!(
            "File metadata count {} is less than expected {} for project {}. Sample file_paths: {:?}",
            last_count, expected, project_id, samples
        )))
    }

    async fn get_project_file_count(&self, project_id: &str) -> Result<i64> {
        // Primary: use COUNT grouped to avoid serialization issues with VALUE count() in Surreal.
        let mut resp = self
            .db
            .query(
                "SELECT count() AS count FROM file_metadata WHERE project_id = $project_id GROUP ALL;",
            )
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to query file_metadata count: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        let rows: Vec<JsonValue> = resp.take(0).map_err(|e| {
            CodeGraphError::Database(format!(
                "Failed to extract file_metadata count row: {}",
                truncate_surreal_error(&e)
            ))
        })?;

        if let Some(count) = rows
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
        {
            return Ok(count);
        }

        // Fallback: enumerate rows to derive count directly if count() shape is unexpected.
        let mut fallback = self
            .db
            .query(
                "SELECT file_path FROM file_metadata WHERE project_id = $project_id LIMIT 200000;",
            )
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to fetch file_metadata rows for fallback count: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        let rows: Vec<JsonValue> = fallback.take(0).map_err(|e| {
            CodeGraphError::Database(format!(
                "Failed to extract file_metadata rows for fallback count: {}",
                truncate_surreal_error(&e)
            ))
        })?;

        Ok(rows.len() as i64)
    }

    async fn sample_file_paths(&self, project_id: &str, limit: usize) -> Result<Vec<String>> {
        let mut resp = self
            .db
            .query(
                "SELECT file_path FROM file_metadata WHERE project_id = $project_id LIMIT $limit",
            )
            .bind(("project_id", project_id.to_string()))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to fetch sample file_metadata rows: {}",
                    truncate_surreal_error(&e)
                ))
            })?;

        let rows: Vec<JsonValue> = resp.take(0).map_err(|e| {
            CodeGraphError::Database(format!(
                "Failed to extract sample file_metadata rows: {}",
                truncate_surreal_error(&e)
            ))
        })?;

        let mut paths = Vec::new();
        for row in rows {
            if let Some(path) = row
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                paths.push(path);
            }
        }
        Ok(paths)
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

    /// Delete all chunks for a project
    pub async fn delete_chunks_for_project(&self, project_id: &str) -> Result<usize> {
        let query = "DELETE chunks WHERE project_id = $project_id RETURN BEFORE";
        let mut result = self
            .db
            .query(query)
            .bind(("project_id", project_id.to_string()))
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to delete chunks: {}", e)))?;

        let deleted: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let count = deleted.len();

        info!("Deleted {} chunks for project {}", count, project_id);
        Ok(count)
    }

    /// Clean slate: Delete ALL data for a project (nodes, edges, chunks, embeddings, file metadata)
    /// Used when --force flag is set
    pub async fn clean_project_data(&self, project_id: &str) -> Result<()> {
        info!(
            "🧹 Starting clean slate deletion for project: {}",
            project_id
        );

        // Delete in order: edges first, then chunks (both reference nodes), then nodes, then metadata
        let edges_deleted = self.delete_edges_for_project(project_id).await?;
        let chunks_deleted = self.delete_chunks_for_project(project_id).await?;
        let nodes_deleted = self.delete_nodes_for_project(project_id).await?;
        let symbols_deleted = self
            .delete_symbol_embeddings_for_project(project_id)
            .await?;
        let files_deleted = self.delete_file_metadata_for_project(project_id).await?;

        info!(
            "🧹 Clean slate complete: {} edges, {} chunks, {} nodes, {} symbols, {} files deleted",
            edges_deleted, chunks_deleted, nodes_deleted, symbols_deleted, files_deleted
        );

        Ok(())
    }

    /// Clean up orphaned records (chunks without parent nodes, edges with missing endpoints)
    /// Returns counts of deleted orphans
    pub async fn cleanup_orphans(&self) -> Result<OrphanCleanupStats> {
        info!("🧹 Starting orphan cleanup");

        // Delete chunks where parent_node doesn't exist in nodes table
        let orphan_chunks_query =
            "DELETE chunks WHERE parent_node NOT IN (SELECT VALUE id FROM nodes) RETURN BEFORE";
        let mut result = self.db.query(orphan_chunks_query).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to delete orphan chunks: {}", e))
        })?;
        let orphan_chunks: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let chunks_deleted = orphan_chunks.len();

        // Delete edges where from node doesn't exist
        let orphan_edges_from_query =
            "DELETE edges WHERE from NOT IN (SELECT VALUE id FROM nodes) RETURN BEFORE";
        let mut result = self.db.query(orphan_edges_from_query).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to delete orphan edges (from): {}", e))
        })?;
        let orphan_edges_from: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();

        // Delete edges where to node doesn't exist
        let orphan_edges_to_query =
            "DELETE edges WHERE to NOT IN (SELECT VALUE id FROM nodes) RETURN BEFORE";
        let mut result = self.db.query(orphan_edges_to_query).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to delete orphan edges (to): {}", e))
        })?;
        let orphan_edges_to: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let edges_deleted = orphan_edges_from.len() + orphan_edges_to.len();

        // Delete symbol_embeddings where node doesn't exist
        let orphan_symbols_query = "DELETE symbol_embeddings WHERE node_id NOT IN (SELECT VALUE id FROM nodes) RETURN BEFORE";
        let mut result = self.db.query(orphan_symbols_query).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to delete orphan symbol embeddings: {}", e))
        })?;
        let orphan_symbols: Vec<HashMap<String, JsonValue>> = result.take(0).unwrap_or_default();
        let symbols_deleted = orphan_symbols.len();

        let stats = OrphanCleanupStats {
            chunks_deleted,
            edges_deleted,
            symbols_deleted,
        };

        info!(
            "🧹 Orphan cleanup complete: {} chunks, {} edges, {} symbols deleted",
            stats.chunks_deleted, stats.edges_deleted, stats.symbols_deleted
        );

        Ok(stats)
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
            .select(("nodes", node_id.as_str()))
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
            .update(("nodes", node_id.as_str()))
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
            .delete(("nodes", node_id.as_str()))
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
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurrealNodeRecord {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>, // Back to String (Base64 encoded if compressed)
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
    metadata: Option<JsonValue>, // JsonValue but contains strings if compressed
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunk_count: Option<i64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkEmbeddingRecord {
    pub id: String,
    pub parent_node: String,
    pub chunk_index: i32,
    pub text: String, // Back to String (Base64 encoded if compressed)
    pub project_id: String,
    pub embedding_384: Option<Vec<f64>>,
    pub embedding_768: Option<Vec<f64>>,
    pub embedding_1024: Option<Vec<f64>>,
    pub embedding_1536: Option<Vec<f64>>,
    pub embedding_2048: Option<Vec<f64>>,
    pub embedding_2560: Option<Vec<f64>>,
    pub embedding_3072: Option<Vec<f64>>,
    pub embedding_4096: Option<Vec<f64>>,
    pub embedding_model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChunkEmbeddingRecord {
    pub fn new(
        parent_node: &str,
        chunk_index: usize,
        text: String,
        embedding: &[f32],
        embedding_model: &str,
        embedding_column: &str,
        project_id: &str,
    ) -> Self {
        let embedding_vec: Vec<f64> = embedding.iter().map(|&f| f as f64).collect();

        // Use Base64 encoding for compression to satisfy String type
        let text_val = codegraph_core::compress_to_string(&text);

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
            // ... match arms ...
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

        let id = {
            let mut hasher = Sha256::new();
            hasher.update(parent_node.as_bytes());
            hasher.update(b":");
            hasher.update(chunk_index.to_le_bytes());
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };

        Self {
            id,
            parent_node: parent_node.to_string(),
            chunk_index: chunk_index as i32,
            text: text_val,
            project_id: project_id.to_string(),
            embedding_384,
            embedding_768,
            embedding_1024,
            embedding_1536,
            embedding_2048,
            embedding_2560,
            embedding_3072,
            embedding_4096,
            embedding_model: embedding_model.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

const UPSERT_NODES_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::record('nodes', $doc.id) SET
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
        chunk_count = $doc.chunk_count,
        updated_at = time::now();
}
"#;

const UPSERT_EDGES_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::record('edges', $doc.id) SET
        from = type::record('nodes', $doc.from),
        to = type::record('nodes', $doc.to),
        edge_type = $doc.edge_type,
        weight = $doc.weight,
        metadata = $doc.metadata,
        project_id = $doc.project_id,
        created_at = time::now();
}
"#;

const UPSERT_SYMBOL_EMBEDDINGS_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    LET $node_rec = if $doc.node_id != NONE { type::record('nodes', $doc.node_id) } else { NONE };
    LET $edge_rec = if $doc.source_edge_id != NONE { type::record('edges', $doc.source_edge_id) } else { NONE };
    UPSERT type::record('symbol_embeddings', $doc.id) SET
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
        node_id = $node_rec,
        source_edge_id = $edge_rec,
        metadata = $doc.metadata,
        access_count = $doc.access_count;
}
"#;

const UPSERT_CHUNK_EMBEDDINGS_QUERY: &str = r#"
LET $batch = $data;
FOR $doc IN $batch {
    UPSERT type::record('chunks', $doc.id) SET
        parent_node = type::record('nodes', $doc.parent_node),
        chunk_index = $doc.chunk_index,
        text = $doc.text,
        project_id = $doc.project_id,
        embedding_384 = $doc.embedding_384,
        embedding_768 = $doc.embedding_768,
        embedding_1024 = $doc.embedding_1024,
        embedding_1536 = $doc.embedding_1536,
        embedding_2048 = $doc.embedding_2048,
        embedding_2560 = $doc.embedding_2560,
        embedding_3072 = $doc.embedding_3072,
        embedding_4096 = $doc.embedding_4096,
        embedding_model = $doc.embedding_model,
        created_at = time::now(),
        updated_at = time::now();
}
"#;

const INSERT_CHUNK_EMBEDDINGS_QUERY: &str = r#"
LET $batch = array::map($batch, |$doc| {
    id: type::record('chunks', $doc.id),
    parent_node: type::record('nodes', $doc.parent_node),
    chunk_index: $doc.chunk_index,
    text: $doc.text,
    project_id: $doc.project_id,
    embedding_384: $doc.embedding_384,
    embedding_768: $doc.embedding_768,
    embedding_1024: $doc.embedding_1024,
    embedding_1536: $doc.embedding_1536,
    embedding_2048: $doc.embedding_2048,
    embedding_2560: $doc.embedding_2560,
    embedding_3072: $doc.embedding_3072,
    embedding_4096: $doc.embedding_4096,
    embedding_model: $doc.embedding_model,
    created_at: time::now(),
    updated_at: time::now(),
});
INSERT INTO chunks $batch ON DUPLICATE KEY UPDATE
    parent_node = $input.parent_node,
    chunk_index = $input.chunk_index,
    text = $input.text,
    embedding_384 = $input.embedding_384,
    embedding_768 = $input.embedding_768,
    embedding_1024 = $input.embedding_1024,
    embedding_1536 = $input.embedding_1536,
    embedding_2048 = $input.embedding_2048,
    embedding_2560 = $input.embedding_2560,
    embedding_3072 = $input.embedding_3072,
    embedding_4096 = $input.embedding_4096,
    embedding_model = $input.embedding_model,
    updated_at = time::now()
RETURN NONE;
"#;

const UPSERT_CHUNK_EMBEDDING_SINGLE_QUERY: &str = r#"
LET $doc = $doc;
UPSERT type::record('chunks', $doc.id) SET
    parent_node = type::record('nodes', $doc.parent_node),
    chunk_index = $doc.chunk_index,
    text = $doc.text,
    project_id = $doc.project_id,
    embedding_384 = $doc.embedding_384,
    embedding_768 = $doc.embedding_768,
    embedding_1024 = $doc.embedding_1024,
    embedding_1536 = $doc.embedding_1536,
    embedding_2048 = $doc.embedding_2048,
    embedding_2560 = $doc.embedding_2560,
    embedding_3072 = $doc.embedding_3072,
    embedding_4096 = $doc.embedding_4096,
    embedding_model = $doc.embedding_model,
    created_at = time::now(),
    updated_at = time::now();
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

async fn verify_with_retry<F, Fut>(
    expected: usize,
    attempts: usize,
    delay: std::time::Duration,
    mut get_count: F,
) -> Result<usize>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<i64>>,
{
    if attempts == 0 {
        return Err(CodeGraphError::Database(
            "verify_with_retry requires at least one attempt".to_string(),
        ));
    }

    for attempt in 1..=attempts {
        let count = get_count().await?;
        if count >= expected as i64 {
            return Ok(count as usize);
        }

        if attempt < attempts {
            tokio::time::sleep(delay).await;
        }
    }

    Err(CodeGraphError::Database(format!(
        "Count did not reach expected {} after {} attempts",
        expected, attempts
    )))
}

#[allow(dead_code)]
fn parse_count(values: Vec<JsonValue>) -> Result<i64> {
    let Some(first) = values.into_iter().next() else {
        return Ok(0);
    };

    match first {
        JsonValue::Number(n) => n.as_i64().ok_or_else(|| {
            CodeGraphError::Database(format!("Count value is not an integer: {}", n))
        }),
        JsonValue::Object(map) => map.get("count").and_then(|v| v.as_i64()).ok_or_else(|| {
            CodeGraphError::Database("Count object missing integer 'count' field".to_string())
        }),
        other => Err(CodeGraphError::Database(format!(
            "Unexpected count shape: {}",
            other
        ))),
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

    #[tokio::test]
    async fn upsert_project_metadata_creates_row_with_project_id() {
        let config = SurrealDbConfig {
            connection: "mem://".to_string(),
            ..Default::default()
        };

        let storage = SurrealDbStorage::new(config).await.unwrap();

        storage
            .db
            .query(
                r#"
                DEFINE TABLE project_metadata TYPE NORMAL SCHEMAFULL PERMISSIONS FULL;
                DEFINE FIELD project_id ON project_metadata TYPE string PERMISSIONS FULL;
                DEFINE FIELD name ON project_metadata TYPE string PERMISSIONS FULL;
                DEFINE FIELD root_path ON project_metadata TYPE string PERMISSIONS FULL;
                DEFINE FIELD primary_language ON project_metadata TYPE option<string> PERMISSIONS FULL;
                DEFINE FIELD file_count ON project_metadata TYPE int PERMISSIONS FULL;
                DEFINE FIELD node_count ON project_metadata TYPE int PERMISSIONS FULL;
                DEFINE FIELD edge_count ON project_metadata TYPE int PERMISSIONS FULL;
                DEFINE FIELD avg_coverage_score ON project_metadata TYPE float PERMISSIONS FULL;
                DEFINE FIELD last_analyzed ON project_metadata TYPE option<datetime> PERMISSIONS FULL;
                DEFINE FIELD codegraph_version ON project_metadata TYPE option<string> PERMISSIONS FULL;
                DEFINE FIELD organization_id ON project_metadata TYPE option<string> PERMISSIONS FULL;
                DEFINE FIELD domain ON project_metadata TYPE option<string> PERMISSIONS FULL;
                DEFINE FIELD metadata ON project_metadata TYPE option<object> FLEXIBLE PERMISSIONS FULL;
                DEFINE FIELD created_at ON project_metadata TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
                DEFINE FIELD updated_at ON project_metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;
            "#,
            )
            .await
            .unwrap()
            .check()
            .unwrap();

        let project_id = "project-a".to_string();
        storage
            .upsert_project_metadata(ProjectMetadataRecord {
                project_id: project_id.clone(),
                name: "Project A".to_string(),
                root_path: "/tmp/project-a".to_string(),
                primary_language: Some("rust".to_string()),
                file_count: 3,
                node_count: 10,
                edge_count: 20,
                avg_coverage_score: 0.5,
                last_analyzed: chrono::Utc::now(),
                codegraph_version: "0.0.0-test".to_string(),
                organization_id: None,
                domain: None,
            })
            .await
            .unwrap();

        let mut resp = storage
            .db
            .query(
                "SELECT count() AS count FROM project_metadata WHERE project_id = $project_id GROUP ALL;",
            )
            .bind(("project_id", project_id))
            .await
            .unwrap();
        let rows: Vec<JsonValue> = resp.take(0).unwrap();
        let count = rows
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert_eq!(count, 1);
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

    #[tokio::test]
    async fn verify_with_retry_succeeds_on_second_attempt() {
        let counts = std::sync::Arc::new(std::sync::Mutex::new(vec![1_i64, 4_i64]));
        let expected = 3usize;
        let counts_clone = counts.clone();
        let result = verify_with_retry(
            expected,
            3,
            std::time::Duration::from_millis(1),
            move || {
                let counts_inner = counts_clone.clone();
                async move {
                    let mut data = counts_inner.lock().unwrap();
                    let value = data.remove(0);
                    Ok(value)
                }
            },
        )
        .await;

        assert!(result.is_ok(), "expected success on second attempt");
        assert_eq!(result.unwrap(), 4);
    }

    #[tokio::test]
    async fn verify_with_retry_fails_after_attempts() {
        let counts = std::sync::Arc::new(std::sync::Mutex::new(vec![1_i64, 1_i64, 2_i64]));
        let expected = 5usize;
        let counts_clone = counts.clone();
        let result = verify_with_retry(
            expected,
            3,
            std::time::Duration::from_millis(1),
            move || {
                let counts_inner = counts_clone.clone();
                async move {
                    let mut data = counts_inner.lock().unwrap();
                    let value = data.remove(0);
                    Ok(value)
                }
            },
        )
        .await;

        assert!(
            result.is_err(),
            "should fail when counts never reach expected"
        );
    }
}

crate::impl_surreal_serde!(
    SchemaVersion,
    SymbolEmbeddingRecord,
    FileMetadataRecord,
    SurrealNodeRecord,
    ChunkEmbeddingRecord
);
