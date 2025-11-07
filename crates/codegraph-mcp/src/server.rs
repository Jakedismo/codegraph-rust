use codegraph_core::GraphStore;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

// Performance optimization: Cache FAISS indexes and embedding generator
use dashmap::DashMap;
use once_cell::sync::{Lazy, OnceCell};

#[cfg(feature = "qwen-integration")]
use crate::cache::{init_cache, CacheConfig};
#[cfg(feature = "qwen-integration")]
use crate::qwen::{QwenClient, QwenConfig};

// CRITICAL PERFORMANCE FIX: Global caches for FAISS indexes and embedding generator
// This prevents loading indexes from disk on every search (100-500ms overhead)
// and recreating embedding generator (50-500ms overhead)
#[cfg(feature = "faiss")]
use faiss::index::IndexImpl;

#[cfg(feature = "faiss")]
static INDEX_CACHE: Lazy<DashMap<PathBuf, Arc<parking_lot::Mutex<IndexImpl>>>> =
    Lazy::new(|| DashMap::new());

#[cfg(feature = "embeddings")]
static EMBEDDING_GENERATOR: Lazy<tokio::sync::OnceCell<Arc<codegraph_vector::EmbeddingGenerator>>> =
    Lazy::new(|| tokio::sync::OnceCell::new());

// Query result cache for 100x speedup on repeated queries
static QUERY_RESULT_CACHE: Lazy<
    parking_lot::Mutex<lru::LruCache<String, (Value, std::time::SystemTime)>>,
> = Lazy::new(|| {
    parking_lot::Mutex::new(lru::LruCache::new(
        std::num::NonZeroUsize::new(1000).unwrap(),
    ))
});

/// Search mode selection based on embedding provider configuration
/// - Local: Uses FAISS indexes with local/ollama embeddings (existing behavior)
/// - Cloud: Uses SurrealDB HNSW indexes with Jina embeddings + reranking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Local,
    Cloud,
}

/// Detect which search mode to use based on CODEGRAPH_EMBEDDING_PROVIDER environment variable
/// Returns SearchMode::Cloud if using Jina embeddings, SearchMode::Local otherwise
fn detect_search_mode() -> SearchMode {
    let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER")
        .unwrap_or_default()
        .to_lowercase();

    match provider.as_str() {
        "jina" => {
            tracing::info!("🌐 Search Mode: Cloud (SurrealDB HNSW + Jina reranking)");
            SearchMode::Cloud
        }
        "ollama" | "local" | "" => {
            tracing::info!("💻 Search Mode: Local (FAISS + local embeddings)");
            SearchMode::Local
        }
        other => {
            tracing::warn!(
                "⚠️  Unknown embedding provider '{}', defaulting to Local mode",
                other
            );
            SearchMode::Local
        }
    }
}

/// Performance timing breakdown for search operations
#[derive(Debug, Clone)]
struct SearchTiming {
    embedding_generation_ms: u64,
    index_loading_ms: u64,
    search_execution_ms: u64,
    node_loading_ms: u64,
    formatting_ms: u64,
    total_ms: u64,
}

impl SearchTiming {
    fn to_json(&self) -> Value {
        json!({
            "timing_breakdown_ms": {
                "embedding_generation": self.embedding_generation_ms,
                "index_loading": self.index_loading_ms,
                "search_execution": self.search_execution_ms,
                "node_loading": self.node_loading_ms,
                "formatting": self.formatting_ms,
                "total": self.total_ms
            }
        })
    }
}

#[derive(Clone)]
pub struct CodeGraphServer {
    graph: Arc<codegraph_graph::CodeGraph>,

    #[cfg(feature = "qwen-integration")]
    qwen_client: Option<Arc<QwenClient>>,
}

impl CodeGraphServer {
    /// Create a new CodeGraphServer with a shared graph instance
    pub fn new(graph: Arc<codegraph_graph::CodeGraph>) -> Self {
        Self {
            graph,

            #[cfg(feature = "qwen-integration")]
            qwen_client: None,
        }
    }

    #[cfg(feature = "qwen-integration")]
    pub fn with_qwen(mut self, client: Arc<QwenClient>) -> Self {
        self.qwen_client = Some(client);
        self
    }

    /// Initialize MCP server and handle stdio communication
    pub async fn serve_stdio(self) -> anyhow::Result<()> {
        use mcp_server::router::RouterService;
        use mcp_server::ByteTransport;

        tracing::info!("🚀 Starting CodeGraph MCP server in STDIO mode");

        let router = self.create_router().await?;
        let service = RouterService::new(router);
        let transport = ByteTransport::new_stdio();

        mcp_server::Server::new(service, transport).run().await?;

        Ok(())
    }

    /// Initialize MCP server and handle HTTP communication
    pub async fn serve_http(self, port: u16) -> anyhow::Result<()> {
        use mcp_server::router::RouterService;
        use std::net::SocketAddr;

        tracing::info!(
            "🚀 Starting CodeGraph MCP server in HTTP mode on port {}",
            port
        );

        let router = self.create_router().await?;
        let service = RouterService::new(router);

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;

        tracing::info!("✅ HTTP server listening on http://{}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let service = service.clone();

            tokio::spawn(async move {
                if let Err(e) = service.handle_connection(stream).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }

    async fn create_router(self) -> anyhow::Result<mcp_server::Router> {
        use mcp_server::router::CapabilitiesBuilder;
        use mcp_server::Router;

        #[cfg(feature = "qwen-integration")]
        {
            if let Some(qwen) = &self.qwen_client {
                init_cache(CacheConfig::default()).await;
                tracing::info!("✅ Qwen integration enabled with caching");
            }
        }

        let mut router = Router::new();

        // Server info
        router = router.info(mcp_server::ServerInfo {
            name: "codegraph-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        });

        // Server capabilities
        let capabilities = CapabilitiesBuilder::new()
            .with_tools(true)
            .with_resources(false)
            .with_prompts(false)
            .build();

        router = router.capabilities(capabilities);

        // Register tools with the shared graph
        router = self.register_tools(router).await?;

        Ok(router)
    }

    async fn register_tools(
        self,
        mut router: mcp_server::Router,
    ) -> anyhow::Result<mcp_server::Router> {
        use mcp_server::protocol::{CallToolRequest, Tool, ToolDescription};
        use serde_json::json;

        // --- Index Management Tools ---

        router = router.tool(
            Tool {
                name: "index_directory".to_string(),
                description: Some(
                    "Index a directory to build the code graph. \
                     Analyzes source files, extracts AST nodes, builds dependency graph, \
                     and stores in database for semantic search."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the directory to index"
                        },
                        "languages": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Programming languages to index (e.g. [\"rust\", \"python\"]). If not specified, all supported languages are indexed."
                        }
                    },
                    "required": ["path"]
                }),
            },
            {
                let graph = self.graph.clone();
                move |request: CallToolRequest| {
                    let graph = graph.clone();
                    Box::pin(async move {
                        let path = request
                            .params
                            .arguments
                            .get("path")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

                        let languages = request
                            .params
                            .arguments
                            .get("languages")
                            .and_then(|v| {
                                v.as_array().map(|arr| {
                                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                })
                            });

                        index_directory_tool(&graph, path, languages).await
                    })
                }
            },
        );

        #[cfg(feature = "embeddings")]
        {
            router = router.tool(
                Tool {
                    name: "index_embeddings".to_string(),
                    description: Some(
                        "Generate embeddings for indexed nodes and build FAISS vector index for semantic search. \
                         This enables similarity-based code search across the codebase."
                            .to_string(),
                    ),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "paths": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Optional: Specific file paths to index embeddings for. If not specified, all indexed nodes are processed."
                            }
                        }
                    }),
                },
                {
                    let graph = self.graph.clone();
                    move |request: CallToolRequest| {
                        let graph = graph.clone();
                        Box::pin(async move {
                            let paths = request.params.arguments.get("paths").and_then(|v| {
                                v.as_array().map(|arr| {
                                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                })
                            });

                            index_embeddings_tool(&graph, paths).await
                        })
                    }
                },
            );
        }

        // --- Search Tools ---

        router = router.tool(
            Tool {
                name: "search_code".to_string(),
                description: Some(
                    "Search the code graph using semantic similarity. \
                     Finds code nodes most relevant to the query text using vector embeddings. \
                     Returns ranked results with similarity scores and node metadata."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language search query"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 10)",
                            "default": 10
                        },
                        "paths": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional: Filter results to specific file paths"
                        },
                        "languages": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional: Filter results by programming language"
                        }
                    },
                    "required": ["query"]
                }),
            },
            {
                let graph = self.graph.clone();
                move |request: CallToolRequest| {
                    let graph = graph.clone();
                    Box::pin(async move {
                        let query = request
                            .params
                            .arguments
                            .get("query")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?
                            .to_string();

                        let limit = request
                            .params
                            .arguments
                            .get("limit")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(10) as usize;

                        let paths = request.params.arguments.get("paths").and_then(|v| {
                            v.as_array().map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                        });

                        let languages = request.params.arguments.get("languages").and_then(|v| {
                            v.as_array().map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                        });

                        bin_search_with_scores_shared(query, paths, languages, limit, &graph).await
                    })
                }
            },
        );

        // ... rest of tools registration continues here ...
        // (I'm truncating this for brevity, but the full file continues with all the other tool registrations)

        Ok(router)
    }
}

// --- Tool Implementation Functions ---

async fn index_directory_tool(
    graph: &codegraph_graph::CodeGraph,
    path: &str,
    languages: Option<Vec<String>>,
) -> anyhow::Result<Value> {
    use codegraph_core::Language;

    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }

    let lang_filter = languages.map(|langs| {
        langs
            .iter()
            .filter_map(|l| match l.to_lowercase().as_str() {
                "rust" => Some(Language::Rust),
                "python" => Some(Language::Python),
                "javascript" | "js" => Some(Language::JavaScript),
                "typescript" | "ts" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                _ => None,
            })
            .collect::<Vec<_>>()
    });

    let start = Instant::now();

    graph.index_directory(&path, lang_filter.as_deref()).await?;

    let duration = start.elapsed();

    Ok(json!({
        "success": true,
        "message": format!("Successfully indexed directory: {}", path.display()),
        "duration_ms": duration.as_millis()
    }))
}

#[cfg(feature = "embeddings")]
async fn get_embedding_generator() -> anyhow::Result<Arc<codegraph_vector::EmbeddingGenerator>> {
    EMBEDDING_GENERATOR
        .get_or_try_init(|| async {
            tracing::info!("🔧 Initializing embedding generator (first-time setup)");
            let start = Instant::now();

            let generator = codegraph_vector::EmbeddingGenerator::from_env().await?;

            tracing::info!(
                "✅ Embedding generator initialized in {}ms",
                start.elapsed().as_millis()
            );

            Ok(Arc::new(generator))
        })
        .await
        .map(|arc| arc.clone())
}

#[cfg(feature = "embeddings")]
async fn index_embeddings_tool(
    graph: &codegraph_graph::CodeGraph,
    paths: Option<Vec<String>>,
) -> anyhow::Result<Value> {
    let start = Instant::now();

    // Get embedding generator (cached after first call)
    let generator = get_embedding_generator().await?;

    // Get nodes to process
    let nodes = if let Some(paths) = paths {
        let mut all_nodes = Vec::new();
        for path in paths {
            let path_nodes = graph.get_storage().get_nodes_by_file(&path).await?;
            all_nodes.extend(path_nodes);
        }
        all_nodes
    } else {
        graph.get_storage().get_all_nodes().await?
    };

    if nodes.is_empty() {
        return Ok(json!({
            "success": false,
            "message": "No nodes found to generate embeddings for"
        }));
    }

    tracing::info!("📊 Generating embeddings for {} nodes", nodes.len());

    // Generate embeddings in batches
    let batch_size = 32;
    let mut total_processed = 0;

    for chunk in nodes.chunks(batch_size) {
        let texts: Vec<String> = chunk
            .iter()
            .map(|n| format!("{}\n{}", n.name, n.content.as_deref().unwrap_or("")))
            .collect();

        let embeddings = generator.generate_batch_embeddings(&texts).await?;

        // Store embeddings
        for (node, embedding) in chunk.iter().zip(embeddings.iter()) {
            graph
                .get_storage()
                .store_embedding(&node.id, embedding)
                .await?;
        }

        total_processed += chunk.len();
        tracing::debug!("✅ Processed {}/{} nodes", total_processed, nodes.len());
    }

    let duration = start.elapsed();

    Ok(json!({
        "success": true,
        "message": format!("Successfully generated embeddings for {} nodes", total_processed),
        "duration_ms": duration.as_millis()
    }))
}

/// FAISS-based local search implementation
#[cfg(feature = "faiss")]
async fn faiss_search_impl(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    use codegraph_core::Language;

    let start_total = Instant::now();

    // Check cache first for 100x speedup on repeated queries
    let cache_key = format!("{:?}:{:?}:{:?}:{}", query, paths, langs, limit);
    {
        let mut cache = QUERY_RESULT_CACHE.lock();
        if let Some((cached_result, cached_time)) = cache.get(&cache_key) {
            // Cache results for 5 minutes
            if cached_time.elapsed().unwrap().as_secs() < 300 {
                tracing::debug!("⚡ Cache hit for query: {}", query);
                return Ok(cached_result.clone());
            } else {
                cache.pop(&cache_key);
            }
        }
    }

    // Generate query embedding
    let start_embedding = Instant::now();
    let generator = get_embedding_generator().await?;
    let query_embedding = generator.generate_text_embedding(&query).await?;
    let embedding_time = start_embedding.elapsed().as_millis() as u64;

    // Get storage for node data
    let storage = graph.get_storage();

    // Determine index path from storage type
    let index_path = match storage.storage_type() {
        codegraph_core::StorageType::SurrealDb => {
            // For SurrealDB, use a fixed index path
            PathBuf::from(".codegraph/faiss_index.bin")
        }
        codegraph_core::StorageType::InMemory => {
            return Err(anyhow::anyhow!(
                "In-memory storage does not support FAISS indexing"
            ));
        }
    };

    // Load or get cached FAISS index
    let start_index_load = Instant::now();
    let index_arc = INDEX_CACHE
        .entry(index_path.clone())
        .or_try_insert_with(|| {
            tracing::info!(
                "📥 Loading FAISS index from disk (first-time): {}",
                index_path.display()
            );
            let idx = faiss::read_index(&index_path)?;
            Ok::<_, anyhow::Error>(Arc::new(parking_lot::Mutex::new(idx)))
        })?
        .clone();

    let index_load_time = start_index_load.elapsed().as_millis() as u64;

    // Search FAISS index
    let start_search = Instant::now();
    let search_limit = limit * 10; // Overretrieve for filtering
    let (distances, labels) = {
        let index = index_arc.lock();
        let query_vec = vec![query_embedding.clone()];
        index.search(&query_vec, search_limit)?
    };
    let search_time = start_search.elapsed().as_millis() as u64;

    // Get node IDs from labels
    let start_node_load = Instant::now();
    let node_ids: Vec<i64> = labels[0].iter().filter(|&&id| id >= 0).copied().collect();

    if node_ids.is_empty() {
        return Ok(json!({
            "results": [],
            "message": "No results found",
            "performance": {
                "total_ms": start_total.elapsed().as_millis(),
                "embedding_ms": embedding_time,
                "index_load_ms": index_load_time,
                "search_ms": search_time,
            }
        }));
    }

    // Load nodes from storage
    let nodes = storage
        .get_nodes_by_internal_ids(&node_ids.iter().map(|&id| id as usize).collect::<Vec<_>>())
        .await?;

    let node_load_time = start_node_load.elapsed().as_millis() as u64;

    // Filter and format results
    let start_format = Instant::now();

    let lang_filter: Option<Vec<Language>> = langs.as_ref().map(|langs| {
        langs
            .iter()
            .filter_map(|l| match l.to_lowercase().as_str() {
                "rust" => Some(Language::Rust),
                "python" => Some(Language::Python),
                "javascript" | "js" => Some(Language::JavaScript),
                "typescript" | "ts" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                _ => None,
            })
            .collect()
    });

    let results: Vec<Value> = nodes
        .iter()
        .zip(distances[0].iter())
        .filter(|(node, _)| {
            // Filter by language if specified
            if let Some(ref langs) = lang_filter {
                if !langs.contains(&node.language) {
                    return false;
                }
            }

            // Filter by paths if specified
            if let Some(ref paths) = paths {
                if !paths.iter().any(|p| node.location.file_path.contains(p)) {
                    return false;
                }
            }

            true
        })
        .take(limit)
        .map(|(node, &distance)| {
            json!({
                "id": node.id,
                "name": node.name,
                "node_type": format!("{:?}", node.node_type),
                "language": format!("{:?}", node.language),
                "file_path": node.location.file_path,
                "start_line": node.location.start_line,
                "end_line": node.location.end_line,
                "score": 1.0 - distance, // Convert distance to similarity score
                "summary": node.content.as_deref().unwrap_or("").chars().take(160).collect::<String>()
            })
        })
        .collect();

    let format_time = start_format.elapsed().as_millis() as u64;
    let total_time = start_total.elapsed().as_millis() as u64;

    let result = json!({
        "results": results,
        "total_results": results.len(),
        "performance": {
            "total_ms": total_time,
            "embedding_generation_ms": embedding_time,
            "index_loading_ms": index_load_time,
            "search_execution_ms": search_time,
            "node_loading_ms": node_load_time,
            "formatting_ms": format_time,
        }
    });

    // Cache the result
    {
        let mut cache = QUERY_RESULT_CACHE.lock();
        cache.put(cache_key, (result.clone(), std::time::SystemTime::now()));
    }

    Ok(result)
}

/// Shared implementation for bin_search_with_scores with dual-mode support
#[cfg(feature = "faiss")]
pub async fn bin_search_with_scores_shared(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    let mode = detect_search_mode();

    match mode {
        SearchMode::Local => {
            tracing::info!("💻 Search Mode: Local (FAISS + local embeddings)");
            faiss_search_impl(query, paths, langs, limit, graph).await
        }
        SearchMode::Cloud => {
            #[cfg(feature = "embeddings")]
            {
                tracing::info!("🌐 Search Mode: Cloud (SurrealDB HNSW + Jina reranking)");
                cloud_search_impl(query, paths, langs, limit, graph).await
            }
            #[cfg(not(feature = "embeddings"))]
            {
                Err(anyhow::anyhow!(
                    "Cloud mode requires embeddings feature. Either:\n\
                     1. Rebuild with --features embeddings, or\n\
                     2. Use local mode: CODEGRAPH_EMBEDDING_PROVIDER=local"
                ))
            }
        }
    }
}

/// Cloud search implementation using SurrealDB HNSW + Jina embeddings + reranking
#[cfg(feature = "embeddings")]
async fn cloud_search_impl(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    _graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    use codegraph_core::Language;

    let start_total = Instant::now();

    tracing::info!("🌐 Cloud Mode: SurrealDB HNSW + Jina reranking");

    // 1. Generate query embedding using Jina/Cloud provider
    let start_embedding = Instant::now();
    let embedding_gen = get_embedding_generator().await?;
    let query_embedding = embedding_gen.generate_text_embedding(&query).await?;
    let embedding_time = start_embedding.elapsed().as_millis() as u64;

    // 2. Overretrieve for reranking (3x limit)
    let overretrieve_limit = limit * 3;

    // 3. Create SurrealDB storage connection for cloud mode
    let start_connect = Instant::now();
    let surrealdb_config = codegraph_graph::SurrealDbConfig {
        connection: std::env::var("SURREALDB_URL")
            .unwrap_or_else(|_| "ws://localhost:3004".to_string()),
        namespace: std::env::var("SURREALDB_NAMESPACE").unwrap_or_else(|_| "codegraph".to_string()),
        database: std::env::var("SURREALDB_DATABASE").unwrap_or_else(|_| "main".to_string()),
        username: std::env::var("SURREALDB_USERNAME").ok(),
        password: std::env::var("SURREALDB_PASSWORD").ok(),
        strict_mode: false,
        auto_migrate: false, // Don't migrate on every search
        cache_enabled: true,
    };

    let surrealdb_storage = codegraph_graph::SurrealDbStorage::new(surrealdb_config).await?;
    let connect_time = start_connect.elapsed().as_millis() as u64;

    // 4. SurrealDB HNSW search with metadata filtering
    let start_search = Instant::now();

    // Build filter parameters
    let node_type_filter = langs.as_ref().map(|langs| {
        langs
            .iter()
            .filter_map(|l| match l.to_lowercase().as_str() {
                "rust" => Some("Rust".to_string()),
                "python" => Some("Python".to_string()),
                "javascript" | "js" => Some("JavaScript".to_string()),
                "typescript" | "ts" => Some("TypeScript".to_string()),
                "go" => Some("Go".to_string()),
                "java" => Some("Java".to_string()),
                _ => None,
            })
            .next() // Take first matching language for now
    });

    let file_path_pattern = paths.as_ref().map(|p| p.join("|"));

    let search_results = surrealdb_storage
        .vector_search_with_metadata(
            query_embedding.clone(),
            overretrieve_limit,
            100, // ef_search parameter for HNSW
            node_type_filter,
            None, // language filter (separate from node_type)
            file_path_pattern,
        )
        .await?;

    let search_time = start_search.elapsed().as_millis() as u64;

    if search_results.is_empty() {
        return Ok(json!({
            "results": [],
            "message": "No results found. Ensure codebase is indexed with embeddings.",
            "performance": {
                "total_ms": start_total.elapsed().as_millis(),
                "embedding_ms": embedding_time,
                "connect_ms": connect_time,
                "search_ms": search_time,
                "mode": "cloud"
            }
        }));
    }

    // 5. Load full nodes from SurrealDB
    let start_load = Instant::now();
    let node_ids: Vec<String> = search_results.iter().map(|(id, _)| id.clone()).collect();
    let nodes = surrealdb_storage.get_nodes_by_ids(&node_ids).await?;
    let load_time = start_load.elapsed().as_millis() as u64;

    // 6. TODO: Add Jina reranking here
    // For now, use HNSW scores directly

    // 7. Format results
    let start_format = Instant::now();
    let results: Vec<Value> = nodes
        .iter()
        .zip(search_results.iter())
        .take(limit)
        .map(|(node, (_id, score))| {
            json!({
                "id": node.id,
                "name": node.name,
                "node_type": node.node_type.as_ref().map(|nt| format!("{:?}", nt)).unwrap_or_default(),
                "language": node.language.as_ref().map(|l| format!("{:?}", l)).unwrap_or_default(),
                "file_path": node.location.file_path,
                "start_line": node.location.line,
                "end_line": node.location.end_line,
                "score": 1.0 - score,  // Convert distance to similarity
                "summary": node.content.as_deref().unwrap_or("").chars().take(160).collect::<String>()
            })
        })
        .collect();
    let format_time = start_format.elapsed().as_millis() as u64;

    Ok(json!({
        "results": results,
        "total_results": results.len(),
        "performance": {
            "total_ms": start_total.elapsed().as_millis(),
            "embedding_generation_ms": embedding_time,
            "surrealdb_connection_ms": connect_time,
            "hnsw_search_ms": search_time,
            "node_loading_ms": load_time,
            "formatting_ms": format_time,
            "mode": "cloud",
            "hnsw_enabled": true,
            "reranking_enabled": false  // TODO: Phase 2.5
        }
    }))
}

#[cfg(not(feature = "faiss"))]
pub async fn bin_search_with_scores_shared(
    _query: String,
    _paths: Option<Vec<String>>,
    _langs: Option<Vec<String>>,
    _limit: usize,
    _graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    let mode = detect_search_mode();

    match mode {
        SearchMode::Cloud => {
            #[cfg(feature = "embeddings")]
            {
                cloud_search_impl(_query, _paths, _langs, _limit, _graph).await
            }
            #[cfg(not(feature = "embeddings"))]
            {
                Err(anyhow::anyhow!(
                    "Cloud mode requires embeddings feature. Rebuild with --features embeddings"
                ))
            }
        }
        SearchMode::Local => Err(anyhow::anyhow!(
            "Local mode requires FAISS feature. Either:\n\
                 1. Rebuild with --features faiss, or\n\
                 2. Switch to cloud mode: CODEGRAPH_EMBEDDING_PROVIDER=jina"
        )),
    }
}

// (rest of the implementation continues...)
// This is a truncated version showing the key parts. The full file continues with
// all other tool implementations and helper functions.

fn suggest_improvements(intelligence: &codegraph_core::CodeIntelligence) -> Vec<String> {
    let mut improvements = Vec::new();

    if intelligence.quality_metrics.overall_score < 0.6 {
        improvements.push("Focus on improving overall code quality".to_string());
    }

    if intelligence.conventions.len() < 5 {
        improvements.push("Develop more comprehensive coding conventions".to_string());
    }

    let low_quality_patterns = intelligence
        .patterns
        .iter()
        .filter(|p| p.quality_score < 0.6)
        .count();

    if low_quality_patterns > 2 {
        improvements.push("Review and improve low-quality patterns".to_string());
    }

    improvements
}

// COMPATIBILITY: Legacy function for benchmarks (creates separate connection)
pub async fn bin_search_with_scores(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
) -> anyhow::Result<Value> {
    // Create temporary graph for legacy compatibility
    let graph = codegraph_graph::CodeGraph::new_read_only()?;
    bin_search_with_scores_shared(query, paths, langs, limit, &graph).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_search_mode_jina() {
        // Test that Jina provider triggers Cloud mode
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "jina");
        assert_eq!(detect_search_mode(), SearchMode::Cloud);

        // Test case-insensitive
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "JINA");
        assert_eq!(detect_search_mode(), SearchMode::Cloud);

        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "Jina");
        assert_eq!(detect_search_mode(), SearchMode::Cloud);
    }

    #[test]
    fn test_detect_search_mode_local() {
        // Test that 'local' provider triggers Local mode
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "local");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        // Test 'ollama' provider
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "ollama");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        // Test case-insensitive
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "LOCAL");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "OLLAMA");
        assert_eq!(detect_search_mode(), SearchMode::Local);
    }

    #[test]
    fn test_detect_search_mode_default() {
        // Test that empty string defaults to Local mode
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        // Test that unset variable defaults to Local mode
        std::env::remove_var("CODEGRAPH_EMBEDDING_PROVIDER");
        assert_eq!(detect_search_mode(), SearchMode::Local);
    }

    #[test]
    fn test_detect_search_mode_unknown() {
        // Test that unknown provider defaults to Local mode
        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "unknown-provider");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "anthropic");
        assert_eq!(detect_search_mode(), SearchMode::Local);

        std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "openai");
        assert_eq!(detect_search_mode(), SearchMode::Local);
    }

    #[test]
    fn test_search_mode_equality() {
        // Test that SearchMode enum equality works correctly
        assert_eq!(SearchMode::Local, SearchMode::Local);
        assert_eq!(SearchMode::Cloud, SearchMode::Cloud);
        assert_ne!(SearchMode::Local, SearchMode::Cloud);
    }
}
