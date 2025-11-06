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
pub struct ServerState {
    pub graph: Arc<tokio::sync::Mutex<codegraph_graph::CodeGraph>>,
    #[cfg(feature = "qwen-integration")]
    pub qwen_client: Option<QwenClient>,
}

// CRITICAL PERFORMANCE FIX: Helper functions for cached access
/// Get or initialize the cached embedding generator (10-100x speedup)
#[cfg(feature = "embeddings")]
async fn get_embedding_generator() -> Arc<codegraph_vector::EmbeddingGenerator> {
    EMBEDDING_GENERATOR
        .get_or_init(|| async {
            tracing::info!("Initializing embedding generator (first time only)");
            let gen = codegraph_vector::EmbeddingGenerator::with_auto_from_env().await;
            Arc::new(gen)
        })
        .await
        .clone()
}

/// Get or load a cached FAISS index (10-50x speedup)
#[cfg(feature = "faiss")]
fn get_cached_index(index_path: &Path) -> anyhow::Result<Arc<parking_lot::Mutex<IndexImpl>>> {
    use faiss::index::io::read_index;

    // Check if index is already cached
    if let Some(cached) = INDEX_CACHE.get(index_path) {
        tracing::debug!("Cache hit for index: {:?}", index_path);
        return Ok(cached.clone());
    }

    // Load index from disk if not cached
    tracing::debug!("Loading index from disk: {:?}", index_path);
    let index = read_index(index_path.to_string_lossy())?;
    let arc_index: Arc<parking_lot::Mutex<IndexImpl>> = Arc::new(parking_lot::Mutex::new(index));

    // Cache for future use
    INDEX_CACHE.insert(index_path.to_path_buf(), arc_index.clone());

    Ok(arc_index)
}

/// Clear index cache (useful for testing or when indexes are updated)
#[cfg(feature = "faiss")]
#[allow(dead_code)]
pub fn clear_index_cache() {
    INDEX_CACHE.clear();
    tracing::info!("Index cache cleared");
}

/// Get cache statistics
#[cfg(feature = "faiss")]
#[allow(dead_code)]
pub fn get_cache_stats() -> (usize, usize) {
    let cached_indexes = INDEX_CACHE.len();
    let estimated_memory_mb = cached_indexes * 60; // Rough estimate: 60MB per index
    (cached_indexes, estimated_memory_mb)
}

/// Generate cache key for query result caching
fn generate_cache_key(
    query: &str,
    paths: &Option<Vec<String>>,
    langs: &Option<Vec<String>>,
    limit: usize,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    if let Some(p) = paths {
        hasher.update(format!("{:?}", p).as_bytes());
    }
    if let Some(l) = langs {
        hasher.update(format!("{:?}", l).as_bytes());
    }
    hasher.update(limit.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Get cached query result if available (5 minute TTL)
fn get_cached_query_result(cache_key: &str) -> Option<Value> {
    let cache = QUERY_RESULT_CACHE.lock();
    if let Some((result, timestamp)) = cache.peek(cache_key) {
        // Check if cache entry is still valid (5 minute TTL)
        let elapsed = std::time::SystemTime::now()
            .duration_since(*timestamp)
            .ok()?;
        if elapsed.as_secs() < 300 {
            tracing::debug!("Query cache hit: {}", cache_key);
            return Some(result.clone());
        }
    }
    None
}

/// Cache query result
fn cache_query_result(cache_key: String, result: Value) {
    let mut cache = QUERY_RESULT_CACHE.lock();
    cache.put(cache_key, (result, std::time::SystemTime::now()));
}

/// Clear query result cache
#[allow(dead_code)]
pub fn clear_query_cache() {
    let mut cache = QUERY_RESULT_CACHE.lock();
    cache.clear();
    tracing::info!("Query result cache cleared");
}

/// Get query cache statistics
#[allow(dead_code)]
pub fn get_query_cache_stats() -> (usize, usize) {
    let cache = QUERY_RESULT_CACHE.lock();
    (cache.len(), cache.cap().get())
}

// Shared dispatcher for both HTTP and STDIO transports
async fn dispatch(state: &ServerState, method: &str, params: Value) -> Result<Value, String> {
    match method {
        "vector.search" => vector_search(state, params).await,
        "graph.neighbors" => graph_neighbors(state, params).await,
        "graph.traverse" => graph_traverse(state, params).await,
        "code.read" => code_read(params).await,
        "code.patch" => code_patch(params).await,
        "test.run" => test_run(params).await,
        "tools/list" => tools_list(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.semantic_intelligence" => semantic_intelligence(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.enhanced_search" => enhanced_search(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.performance_metrics" => performance_metrics(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.impact_analysis" => impact_analysis(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.cache_stats" => cache_stats(state, params).await,
        "codegraph.pattern_detection" => pattern_detection(state, params).await,
        _ => Err(format!("Unknown method: {}", method)),
    }
}

// Handlers
pub async fn vector_search(state: &ServerState, params: Value) -> Result<Value, String> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("missing query")?
        .to_string();
    let paths = params.get("paths").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
    });
    let langs = params.get("langs").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
    });
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    // REVOLUTIONARY: Use shared database connection to prevent lock conflicts
    let graph = state.graph.lock().await;
    let res = bin_search_with_scores_shared(query, paths, langs, limit, &graph)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res)
}

pub async fn graph_neighbors(state: &ServerState, params: Value) -> Result<Value, String> {
    let node_str = params
        .get("node")
        .and_then(|v| v.as_str())
        .ok_or("missing node")?;
    let id = uuid::Uuid::parse_str(node_str).map_err(|e| e.to_string())?;
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let graph = state.graph.lock().await;
    let neighbors = graph.get_neighbors(id).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for nb in neighbors.into_iter().take(limit) {
        if let Some(n) = graph.get_node(nb).await.map_err(|e| e.to_string())? {
            let node_type = n
                .node_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".into());
            let language = n
                .language
                .as_ref()
                .map(|l| format!("{:?}", l))
                .unwrap_or_else(|| "unknown".into());
            out.push(json!({
                "id": nb,
                "name": n.name,
                "path": n.location.file_path,
                "node_type": node_type,
                "language": language,
                "depth": 1
            }));
        }
    }
    Ok(json!({"neighbors": out}))
}

pub async fn graph_traverse(state: &ServerState, params: Value) -> Result<Value, String> {
    use std::collections::{HashSet, VecDeque};
    let start_str = params
        .get("start")
        .and_then(|v| v.as_str())
        .ok_or("missing start")?;
    let start = uuid::Uuid::parse_str(start_str).map_err(|e| e.to_string())?;
    let depth = params.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let graph = state.graph.lock().await;
    let mut seen: HashSet<codegraph_core::NodeId> = HashSet::new();
    let mut q: VecDeque<(codegraph_core::NodeId, usize)> = VecDeque::new();
    q.push_back((start, 0));
    seen.insert(start);
    let mut out = Vec::new();
    while let Some((nid, d)) = q.pop_front() {
        if out.len() >= limit {
            break;
        }
        if let Some(n) = graph.get_node(nid).await.map_err(|e| e.to_string())? {
            let node_type = n
                .node_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".into());
            let language = n
                .language
                .as_ref()
                .map(|l| format!("{:?}", l))
                .unwrap_or_else(|| "unknown".into());
            out.push(json!({
                "id": nid,
                "name": n.name,
                "path": n.location.file_path,
                "node_type": node_type,
                "language": language,
                "depth": d
            }));
        }
        if d >= depth {
            continue;
        }
        for nb in graph.get_neighbors(nid).await.map_err(|e| e.to_string())? {
            if seen.insert(nb) {
                q.push_back((nb, d + 1));
            }
        }
    }
    Ok(json!({"nodes": out}))
}

async fn code_read(params: Value) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("missing path")?;
    let start = params.get("start").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let end = params.get("end").and_then(|v| v.as_u64());
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let total = text.lines().count();
    let e = end.unwrap_or(total as u64) as usize;
    let mut lines = Vec::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .skip(start.saturating_sub(1))
        .take(e.saturating_sub(start.saturating_sub(1)))
    {
        lines.push(json!({"line": i + 1, "text": line}));
    }
    Ok(json!({"path": path, "start": start, "end": e, "lines": lines}))
}

async fn code_patch(params: Value) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("missing path")?;
    let find = params
        .get("find")
        .and_then(|v| v.as_str())
        .ok_or("missing find")?;
    let replace = params
        .get("replace")
        .and_then(|v| v.as_str())
        .ok_or("missing replace")?;
    let dry = params
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let replacements = text.matches(find).count();
    if dry {
        return Ok(json!({
            "path": path,
            "find": find,
            "replace": replace,
            "replacements": replacements,
            "dry_run": true
        }));
    }
    let new = text.replace(find, replace);
    std::fs::write(path, new).map_err(|e| e.to_string())?;
    Ok(json!({
        "path": path,
        "find": find,
        "replace": replace,
        "replacements": replacements,
        "dry_run": false
    }))
}

async fn test_run(params: Value) -> Result<Value, String> {
    use tokio::process::Command;
    let package = params.get("package").and_then(|v| v.as_str());
    let mut args = vec!["test".to_string()];
    if let Some(pkg) = package {
        args.push("-p".into());
        args.push(pkg.into());
    }
    if let Some(extra) = params.get("args").and_then(|v| v.as_array()) {
        for a in extra {
            if let Some(s) = a.as_str() {
                args.push(s.to_string());
            }
        }
    }
    let output = Command::new("cargo")
        .args(&args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(json!({"status": status, "stdout": stdout, "stderr": stderr}))
}

// MCP protocol: tools list
async fn tools_list(_state: &ServerState, _params: Value) -> Result<Value, String> {
    #[cfg(feature = "qwen-integration")]
    {
        Ok(json!({
            "tools": crate::tools_schema::get_tools_list()
        }))
    }
    #[cfg(not(feature = "qwen-integration"))]
    {
        // Basic tools without Qwen integration
        Ok(json!({
            "tools": [
                {
                    "name": "vector.search",
                    "description": "Basic vector similarity search",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"},
                            "limit": {"type": "integer", "default": 10}
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "graph.neighbors",
                    "description": "Get neighboring nodes in code graph",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "node": {"type": "string"},
                            "limit": {"type": "integer", "default": 20}
                        },
                        "required": ["node"]
                    }
                }
            ]
        }))
    }
}

// REVOLUTIONARY: Vector search with query caching, parallel shard search, and performance timing
pub async fn bin_search_with_scores_shared(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
    graph: &codegraph_graph::CodeGraph,
) -> anyhow::Result<Value> {
    #[cfg(feature = "faiss")]
    {
        use faiss::index::Index as _;
        use rayon::prelude::*;
        use std::path::Path;

        let start_total = Instant::now();

        // PERFORMANCE FIX #1: Check query result cache first (100x speedup on cache hit)
        let cache_key = generate_cache_key(&query, &paths, &langs, limit);
        if let Some(cached_result) = get_cached_query_result(&cache_key) {
            tracing::info!("Query cache hit - returning cached result");
            return Ok(cached_result);
        }

        // PERFORMANCE FIX #2: Use cached embedding generator
        let start_embedding = Instant::now();
        let emb = {
            #[cfg(feature = "embeddings")]
            {
                let embedding_gen = get_embedding_generator().await;
                let e = embedding_gen.generate_text_embedding(&query).await?;
                crate::indexer::normalize(&e)
            }
            #[cfg(not(feature = "embeddings"))]
            {
                let dimension = 384;
                let e = crate::indexer::simple_text_embedding(&query, dimension);
                crate::indexer::normalize(&e)
            }
        };
        let embedding_time = start_embedding.elapsed().as_millis() as u64;

        // Collect all index paths to search
        let start_index_loading = Instant::now();
        let mut index_paths: Vec<(PathBuf, PathBuf, usize)> = Vec::new();

        if let Some(prefs) = &paths {
            for p in prefs {
                let seg = p.trim_start_matches("./").split('/').next().unwrap_or("");
                if !seg.is_empty() {
                    let idx = Path::new(".codegraph/shards/path").join(format!("{}.index", seg));
                    let ids = Path::new(".codegraph/shards/path").join(format!("{}_ids.json", seg));
                    if idx.exists() && ids.exists() {
                        index_paths.push((idx, ids, limit * 5));
                    }
                }
            }
        }

        if let Some(l) = &langs {
            for lang in l {
                let norm = lang.to_lowercase();
                let idx = Path::new(".codegraph/shards/lang").join(format!("{}.index", norm));
                let ids = Path::new(".codegraph/shards/lang").join(format!("{}_ids.json", norm));
                if idx.exists() && ids.exists() {
                    index_paths.push((idx, ids, limit * 5));
                }
            }
        }

        if index_paths.is_empty() {
            // Search all available shards
            for dir_path in &[
                Path::new(".codegraph/shards/lang"),
                Path::new(".codegraph/shards/path"),
            ] {
                if dir_path.exists() {
                    if let Ok(entries) = std::fs::read_dir(dir_path) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().and_then(|s| s.to_str()) == Some("index") {
                                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                    let ids_path = dir_path.join(format!("{}_ids.json", stem));
                                    if ids_path.exists() {
                                        index_paths.push((path, ids_path, limit * 2));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Always search main index
        let main_index_path = Path::new(".codegraph/faiss.index");
        let main_ids_path = Path::new(".codegraph/faiss_ids.json");
        if main_index_path.exists() && main_ids_path.exists() {
            index_paths.push((
                main_index_path.to_path_buf(),
                main_ids_path.to_path_buf(),
                limit * 3,
            ));
        }

        // PERFORMANCE FIX #3: Parallel shard searching (2-3x speedup)
        let start_search = Instant::now();
        let scored: Vec<(codegraph_core::NodeId, f32)> = index_paths
            .par_iter()
            .flat_map(|(index_path, ids_path, topk)| {
                let mut results = Vec::new();
                if let Ok(index) = get_cached_index(index_path) {
                    if let Ok(mapping_raw) = std::fs::read_to_string(ids_path) {
                        if let Ok(mapping) =
                            serde_json::from_str::<Vec<codegraph_core::NodeId>>(&mapping_raw)
                        {
                            let mut index_lock = index.lock();
                            if let Ok(res) = index_lock.search(&emb, *topk) {
                                for (i, label) in res.labels.into_iter().enumerate() {
                                    if let Some(idx_val) = label.get() {
                                        let idx = idx_val as usize;
                                        if idx < mapping.len() {
                                            results.push((mapping[idx], res.distances[i]));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                results
            })
            .collect();

        let mut scored = scored;
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.dedup_by_key(|(id, _)| *id);
        let top: Vec<(codegraph_core::NodeId, f32)> = scored.into_iter().take(limit).collect();

        let index_and_search_time = start_search.elapsed().as_millis() as u64;
        let index_loading_time = start_index_loading.elapsed().as_millis() as u64;

        // Load nodes from graph
        let start_node_loading = Instant::now();
        let mut out = Vec::new();
        for (id, score) in top {
            if let Some(node) = graph.get_node(id).await? {
                let summary = node
                    .content
                    .as_deref()
                    .map(|s| {
                        let mut t = s.trim().replace('\n', " ");
                        if t.len() > 160 {
                            t.truncate(160);
                            t.push_str("...");
                        }
                        t
                    })
                    .unwrap_or_default();
                let node_type = node
                    .node_type
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "unknown".into());
                let language = node
                    .language
                    .as_ref()
                    .map(|l| format!("{:?}", l))
                    .unwrap_or_else(|| "unknown".into());
                out.push(json!({
                    "id": id,
                    "name": node.name,
                    "path": node.location.file_path,
                    "node_type": node_type,
                    "language": language,
                    "depth": 0,
                    "summary": summary,
                    "score": score
                }));
            }
        }
        let node_loading_time = start_node_loading.elapsed().as_millis() as u64;

        let start_formatting = Instant::now();
        let timing = SearchTiming {
            embedding_generation_ms: embedding_time,
            index_loading_ms: index_loading_time,
            search_execution_ms: index_and_search_time,
            node_loading_ms: node_loading_time,
            formatting_ms: 0, // Will be set after
            total_ms: start_total.elapsed().as_millis() as u64,
        };

        let mut result = json!({"results": out});
        result["performance"] = timing.to_json();
        let formatting_time = start_formatting.elapsed().as_millis() as u64;
        result["performance"]["timing_breakdown_ms"]["formatting"] = json!(formatting_time);

        // PERFORMANCE FIX #4: Cache the result for future queries
        cache_query_result(cache_key, result.clone());

        tracing::info!(
            "Search completed in {}ms (embedding: {}ms, index+search: {}ms, nodes: {}ms)",
            timing.total_ms,
            embedding_time,
            index_and_search_time,
            node_loading_time
        );

        return Ok(result);
    }

    #[allow(unused_variables)]
    {
        // Fallback when FAISS is not enabled: return empty result list
        Ok(json!({"results": Value::Array(Vec::new())}))
    }
}

// HTTP transport using shared dispatcher
#[cfg(feature = "server-http")]
pub async fn serve_http(host: String, port: u16) -> crate::Result<()> {
    use axum::{extract::State, routing::post, Json, Router};
    use std::net::SocketAddr;

    let state = ServerState {
        graph: Arc::new(tokio::sync::Mutex::new(
            codegraph_graph::CodeGraph::new()
                .map_err(|e| crate::McpError::Transport(e.to_string()))?,
        )),
        #[cfg(feature = "qwen-integration")]
        qwen_client: init_qwen_client().await,
    };
    async fn handle(State(state): State<ServerState>, Json(payload): Json<Value>) -> Json<Value> {
        let id = payload.get("id").cloned().unwrap_or(json!(null));
        let method = payload.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = payload.get("params").cloned().unwrap_or(json!({}));
        let body = match dispatch(&state, method, params).await {
            Ok(r) => json!({"jsonrpc":"2.0","id": id, "result": r}),
            Err(e) => json!({"jsonrpc":"2.0","id": id, "error": {"code": -32000, "message": e}}),
        };
        Json(body)
    }

    let app = Router::new().route("/mcp", post(handle)).with_state(state);
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e: std::net::AddrParseError| crate::McpError::Transport(e.to_string()))?;
    tracing::info!("HTTP MCP listening at http://{}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| crate::McpError::Transport(e.to_string()))?,
        app,
    )
    .await
    .map_err(|e| crate::McpError::Transport(e.to_string()))?;
    Ok(())
}

// STDIO transport using shared dispatcher
pub async fn serve_stdio(_buffer_size: usize) -> crate::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let state = ServerState {
        graph: Arc::new(tokio::sync::Mutex::new(
            codegraph_graph::CodeGraph::new()
                .map_err(|e| crate::McpError::Transport(e.to_string()))?,
        )),
        #[cfg(feature = "qwen-integration")]
        qwen_client: init_qwen_client().await,
    };
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader
            .read_line(&mut buf)
            .await
            .map_err(|e| crate::McpError::Transport(e.to_string()))?;
        if n == 0 {
            break;
        }
        let response = match serde_json::from_str::<Value>(&buf) {
            Ok(v) => {
                let id = v.get("id").cloned().unwrap_or(json!(null));
                let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let params = v.get("params").cloned().unwrap_or(json!({}));
                match dispatch(&state, method, params).await {
                    Ok(r) => json!({"jsonrpc":"2.0","id": id, "result": r}),
                    Err(e) => {
                        json!({"jsonrpc":"2.0","id": id, "error": {"code": -32000, "message": e}})
                    }
                }
            }
            Err(e) => {
                json!({"jsonrpc":"2.0","id": null, "error": {"code": -32700, "message": format!("parse error: {}", e)}})
            }
        };
        let line = serde_json::to_string(&response).unwrap();
        stdout.write_all(line.as_bytes()).await.unwrap();
        stdout.write_all(b"\n").await.unwrap();
        stdout.flush().await.unwrap();
    }
    Ok(())
}

// Qwen2.5-Coder integration functions
#[cfg(feature = "qwen-integration")]
pub async fn init_qwen_client() -> Option<QwenClient> {
    // Initialize intelligent cache
    let cache_config = CacheConfig::default();
    init_cache(cache_config);
    // Use eprintln for STDIO to avoid polluting stdout
    eprintln!("✅ Intelligent response cache initialized");

    // Note: Cache warming will happen on first requests

    let config = QwenConfig::default();
    let client = QwenClient::new(config.clone());

    match client.check_availability().await {
        Ok(true) => {
            eprintln!("✅ Qwen2.5-Coder-14B-128K available for CodeGraph intelligence");
            Some(client)
        }
        Ok(false) => {
            eprintln!(
                "⚠️ Qwen2.5-Coder model not found. Install with: ollama pull {}",
                config.model_name
            );
            None
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to Qwen2.5-Coder: {}", e);
            None
        }
    }
}

// Enhanced semantic search with Qwen2.5-Coder intelligence
#[cfg(feature = "qwen-integration")]
pub async fn enhanced_search(state: &ServerState, params: Value) -> Result<Value, String> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("missing query")?
        .to_string();

    let include_analysis = params
        .get("include_analysis")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let max_results = params
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    // 1. Perform standard vector search using shared database connection
    let graph = state.graph.lock().await;
    let search_results =
        bin_search_with_scores_shared(query.clone(), None, None, max_results * 2, &graph)
            .await
            .map_err(|e| e.to_string())?;

    // 2. If Qwen analysis is requested and available, enhance results
    if include_analysis {
        if let Some(qwen_client) = &state.qwen_client {
            // Check cache first
            let search_context = build_search_context(&search_results, &query);

            if let Some(cached_response) =
                crate::cache::get_cached_response(&query, &search_context).await
            {
                tracing::info!("🚀 Cache hit for enhanced search: {}", query);
                return Ok(cached_response);
            }

            // Use Qwen2.5-Coder for intelligent analysis
            match qwen_client.analyze_codebase(&query, &search_context).await {
                Ok(qwen_result) => {
                    // Record performance metrics
                    crate::performance::record_qwen_operation(
                        "enhanced_search",
                        qwen_result.processing_time,
                        qwen_result.context_tokens,
                        qwen_result.completion_tokens,
                        qwen_result.confidence_score,
                    );

                    // Build response
                    let response = json!({
                        "search_results": search_results["results"],
                        "ai_analysis": qwen_result.text,
                        "intelligence_metadata": {
                            "model_used": qwen_result.model_used,
                            "processing_time_ms": qwen_result.processing_time.as_millis(),
                            "context_tokens": qwen_result.context_tokens,
                            "completion_tokens": qwen_result.completion_tokens,
                            "confidence_score": qwen_result.confidence_score,
                            "context_window_used": state.qwen_client.as_ref().unwrap().config.context_window
                        },
                        "generation_guidance": crate::prompts::extract_enhanced_generation_guidance(&qwen_result.text),
                        "quality_assessment": crate::prompts::extract_enhanced_quality_assessment(&qwen_result.text)
                    });

                    // Cache the response for future use
                    let _ = crate::cache::cache_response(
                        &query,
                        &search_context,
                        response.clone(),
                        qwen_result.confidence_score,
                        qwen_result.processing_time,
                        qwen_result.context_tokens,
                        qwen_result.completion_tokens,
                    )
                    .await;

                    return Ok(response);
                }
                Err(e) => {
                    tracing::error!("Qwen analysis failed: {}", e);
                    // Fall back to basic search results
                }
            }
        }
    }

    // Return basic search results if no analysis or Qwen not available
    Ok(search_results)
}

// Comprehensive semantic intelligence using Qwen2.5-Coder
#[cfg(feature = "qwen-integration")]
pub async fn semantic_intelligence(state: &ServerState, params: Value) -> Result<Value, String> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("missing query")?
        .to_string();

    let task_type = params
        .get("task_type")
        .and_then(|v| v.as_str())
        .unwrap_or("semantic_search");

    let max_context_tokens = params
        .get("max_context_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(80000) as usize;

    // Check if Qwen is available
    let qwen_client = state
        .qwen_client
        .as_ref()
        .ok_or("Qwen2.5-Coder not available. Please install: ollama pull qwen2.5-coder-14b-128k")?;

    // 1. Gather comprehensive codebase context
    let codebase_context = build_comprehensive_context(state, &query, max_context_tokens)
        .await
        .map_err(|e| e.to_string())?;

    // Check cache first for semantic intelligence
    if let Some(cached_response) =
        crate::cache::get_cached_response(&query, &codebase_context).await
    {
        tracing::info!("🚀 Cache hit for semantic intelligence: {}", query);
        return Ok(cached_response);
    }

    // 2. Use Qwen2.5-Coder for comprehensive analysis
    let analysis_result = qwen_client
        .analyze_codebase(&query, &codebase_context)
        .await
        .map_err(|e| e.to_string())?;

    // Record performance metrics
    crate::performance::record_qwen_operation(
        "semantic_intelligence",
        analysis_result.processing_time,
        analysis_result.context_tokens,
        analysis_result.completion_tokens,
        analysis_result.confidence_score,
    );

    // 3. Structure response for MCP-calling LLMs
    let response = json!({
        "task_type": task_type,
        "user_query": query,
        "comprehensive_analysis": analysis_result.text,
        "codebase_context_summary": build_context_summary(&codebase_context),
        "model_performance": {
            "model_used": analysis_result.model_used,
            "processing_time_ms": analysis_result.processing_time.as_millis(),
            "context_tokens_used": analysis_result.context_tokens,
            "completion_tokens": analysis_result.completion_tokens,
            "confidence_score": analysis_result.confidence_score,
            "context_window_total": qwen_client.config.context_window
        },
        "generation_guidance": crate::prompts::extract_enhanced_generation_guidance(&analysis_result.text),
        "structured_insights": crate::prompts::extract_enhanced_structured_insights(&analysis_result.text),
        "mcp_metadata": {
            "tool_version": "1.0.0",
            "recommended_for": ["claude", "gpt-4", "custom-agents"],
            "context_quality": analysis_result.confidence_score
        }
    });

    // Cache the response for future use
    let _ = crate::cache::cache_response(
        &query,
        &codebase_context,
        response.clone(),
        analysis_result.confidence_score,
        analysis_result.processing_time,
        analysis_result.context_tokens,
        analysis_result.completion_tokens,
    )
    .await;

    Ok(response)
}

// Helper functions for Qwen integration
#[cfg(feature = "qwen-integration")]
pub fn build_search_context(search_results: &Value, query: &str) -> String {
    let empty_vec = vec![];
    let results = search_results["results"].as_array().unwrap_or(&empty_vec);

    let mut context = format!("SEARCH QUERY: {}\n\nSEARCH RESULTS:\n", query);

    for (i, result) in results.iter().enumerate().take(10) {
        context.push_str(&format!(
            "{}. File: {}\n   Function: {}\n   Summary: {}\n   Score: {}\n\n",
            i + 1,
            result["path"].as_str().unwrap_or("unknown"),
            result["name"].as_str().unwrap_or("unknown"),
            result["summary"].as_str().unwrap_or("no summary"),
            result["score"].as_f64().unwrap_or(0.0)
        ));
    }

    context
}

#[cfg(feature = "qwen-integration")]
pub async fn build_comprehensive_context(
    state: &ServerState,
    query: &str,
    max_tokens: usize,
) -> Result<String, String> {
    let mut context = format!(
        "COMPREHENSIVE CODEBASE ANALYSIS REQUEST\n\nQUERY: {}\n\n",
        query
    );

    // Add basic search results using shared database connection
    let graph = state.graph.lock().await;
    let search_results = bin_search_with_scores_shared(query.to_string(), None, None, 15, &graph)
        .await
        .map_err(|e| e.to_string())?;

    context.push_str("SEMANTIC MATCHES:\n");
    if let Some(results) = search_results["results"].as_array() {
        for (i, result) in results.iter().enumerate().take(10) {
            context.push_str(&format!(
                "{}. {}: {} ({})\n   Summary: {}\n   Score: {:.3}\n\n",
                i + 1,
                result["path"].as_str().unwrap_or("unknown"),
                result["name"].as_str().unwrap_or("unknown"),
                result["node_type"].as_str().unwrap_or("unknown"),
                result["summary"].as_str().unwrap_or("no summary"),
                result["score"].as_f64().unwrap_or(0.0)
            ));
        }
    }

    // Add graph relationship context
    context.push_str("GRAPH RELATIONSHIPS:\n");
    if let Some(results) = search_results["results"].as_array() {
        for result in results.iter().take(5) {
            if let Some(id_str) = result["id"].as_str() {
                if let Ok(node_id) = uuid::Uuid::parse_str(id_str) {
                    let graph = state.graph.lock().await;
                    if let Ok(neighbors) = graph.get_neighbors(node_id).await {
                        context.push_str(&format!(
                            "  {} has {} connected nodes\n",
                            result["name"].as_str().unwrap_or("unknown"),
                            neighbors.len()
                        ));
                    }
                }
            }
        }
    }

    // Truncate context if too long (rough token estimation: ~4 chars per token)
    if context.len() > max_tokens * 4 {
        context.truncate(max_tokens * 4);
        context.push_str("\n\n[Context truncated to fit within token limits]");
    }

    Ok(context)
}

#[cfg(feature = "qwen-integration")]
pub fn build_context_summary(context: &str) -> Value {
    let lines = context.lines().count();
    let chars = context.len();
    let estimated_tokens = chars / 4; // Rough estimate

    json!({
        "context_lines": lines,
        "context_characters": chars,
        "estimated_tokens": estimated_tokens,
        "truncated": context.contains("[Context truncated"),
    })
}

// Performance metrics MCP tool
#[cfg(feature = "qwen-integration")]
async fn performance_metrics(_state: &ServerState, _params: Value) -> Result<Value, String> {
    let mut metrics = crate::performance::get_performance_summary();

    // Add cache performance data
    if let Some(cache_stats) = crate::cache::get_cache_stats() {
        metrics["cache_performance"] = json!(cache_stats);
    }

    // Add cache performance analysis
    if let Some(cache_analysis) = crate::cache::analyze_cache_performance() {
        metrics["cache_analysis"] = json!(cache_analysis);
    }

    Ok(metrics)
}

// Cache statistics MCP tool
#[cfg(feature = "qwen-integration")]
async fn cache_stats(_state: &ServerState, _params: Value) -> Result<Value, String> {
    let cache_stats = crate::cache::get_cache_stats();
    let cache_analysis = crate::cache::analyze_cache_performance();

    Ok(json!({
        "cache_statistics": cache_stats,
        "performance_analysis": cache_analysis,
        "recommendations": generate_cache_recommendations(&cache_stats, &cache_analysis),
        "cache_health": assess_cache_health(&cache_stats)
    }))
}

#[cfg(feature = "qwen-integration")]
pub fn generate_cache_recommendations(
    stats: &Option<crate::cache::CacheStats>,
    analysis: &Option<crate::cache::CachePerformanceReport>,
) -> Vec<String> {
    let mut recommendations = Vec::new();

    if let Some(stats) = stats {
        if stats.hit_rate < 0.3 {
            recommendations
                .push("Low cache hit rate - consider semantic similarity tuning".to_string());
        }

        if stats.memory_usage_mb > 400.0 {
            recommendations
                .push("High memory usage - consider reducing cache size or TTL".to_string());
        }

        if stats.total_requests > 50 && stats.semantic_hit_rate < 0.1 {
            recommendations.push("Low semantic matching - queries might be too varied".to_string());
        }
    }

    if let Some(analysis) = analysis {
        recommendations.extend(analysis.recommendations.clone());
    }

    if recommendations.is_empty() {
        recommendations.push("Cache performance is optimal".to_string());
    }

    recommendations
}

#[cfg(feature = "qwen-integration")]
pub fn assess_cache_health(stats: &Option<crate::cache::CacheStats>) -> String {
    if let Some(stats) = stats {
        if stats.hit_rate > 0.5 && stats.memory_usage_mb < 300.0 {
            "excellent".to_string()
        } else if stats.hit_rate > 0.3 && stats.memory_usage_mb < 400.0 {
            "good".to_string()
        } else if stats.hit_rate > 0.1 {
            "acceptable".to_string()
        } else {
            "needs_optimization".to_string()
        }
    } else {
        "unknown".to_string()
    }
}

// Impact analysis MCP tool - shows what will break before changes are made
#[cfg(feature = "qwen-integration")]
pub async fn impact_analysis(state: &ServerState, params: Value) -> Result<Value, String> {
    let target_function = params
        .get("target_function")
        .and_then(|v| v.as_str())
        .ok_or("missing target_function")?
        .to_string();

    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or("missing file_path")?
        .to_string();

    let change_type = params
        .get("change_type")
        .and_then(|v| v.as_str())
        .unwrap_or("modify");

    // Check if Qwen is available
    let qwen_client = state
        .qwen_client
        .as_ref()
        .ok_or("Qwen2.5-Coder not available. Please install: ollama pull qwen2.5-coder-14b-128k")?;

    // 1. Build dependency context using graph analysis
    let dependency_context = build_dependency_context(state, &target_function, &file_path)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Use Qwen2.5-Coder for intelligent impact analysis
    let impact_prompt = crate::prompts::build_impact_analysis_prompt(
        &target_function,
        &file_path,
        &dependency_context,
        change_type,
    );

    let analysis_result = qwen_client
        .analyze_codebase(&impact_prompt, "")
        .await
        .map_err(|e| e.to_string())?;

    // Record performance metrics
    crate::performance::record_qwen_operation(
        "impact_analysis",
        analysis_result.processing_time,
        analysis_result.context_tokens,
        analysis_result.completion_tokens,
        analysis_result.confidence_score,
    );

    // 3. Structure comprehensive impact response
    Ok(json!({
        "target": {
            "function": target_function,
            "file_path": file_path,
            "change_type": change_type
        },
        "comprehensive_impact_analysis": analysis_result.text,
        "dependency_analysis": parse_dependency_info(&dependency_context),
        "risk_assessment": extract_risk_level(&analysis_result.text),
        "affected_components": extract_affected_components(&analysis_result.text),
        "testing_requirements": extract_testing_requirements(&analysis_result.text),
        "implementation_plan": extract_implementation_plan(&analysis_result.text),
        "model_performance": {
            "model_used": analysis_result.model_used,
            "processing_time_ms": analysis_result.processing_time.as_millis(),
            "context_tokens": analysis_result.context_tokens,
            "completion_tokens": analysis_result.completion_tokens,
            "confidence_score": analysis_result.confidence_score
        },
        "safety_recommendations": extract_safety_recommendations(&analysis_result.text),
        "mcp_metadata": {
            "tool_version": "1.0.0",
            "analysis_type": "impact_assessment",
            "recommended_for": ["claude", "gpt-4", "custom-agents"]
        }
    }))
}

// Build dependency context for impact analysis
#[cfg(feature = "qwen-integration")]
pub async fn build_dependency_context(
    state: &ServerState,
    target_function: &str,
    file_path: &str,
) -> Result<String, String> {
    let mut context = format!(
        "IMPACT ANALYSIS TARGET\n\nFUNCTION: {}\nFILE: {}\n\n",
        target_function, file_path
    );

    // 1. Find functions that might call this target function using shared database
    let graph = state.graph.lock().await;
    let search_query = format!("{} call usage", target_function);
    let usage_results = bin_search_with_scores_shared(search_query, None, None, 20, &graph)
        .await
        .map_err(|e| e.to_string())?;

    context.push_str("FUNCTIONS THAT MAY CALL THIS TARGET:\n");
    if let Some(results) = usage_results["results"].as_array() {
        for (i, result) in results.iter().enumerate().take(10) {
            context.push_str(&format!(
                "{}. {}: {} in {}\n   Summary: {}\n   Score: {:.3}\n",
                i + 1,
                result["name"].as_str().unwrap_or("unknown"),
                result["node_type"].as_str().unwrap_or("unknown"),
                result["path"].as_str().unwrap_or("unknown"),
                result["summary"].as_str().unwrap_or("no summary"),
                result["score"].as_f64().unwrap_or(0.0)
            ));
        }
    }

    // 2. Find functions this target might depend on using same shared database
    let dependency_query = format!("{} dependencies imports", target_function);
    let dep_results = bin_search_with_scores_shared(dependency_query, None, None, 15, &graph)
        .await
        .map_err(|e| e.to_string())?;

    context.push_str("\nFUNCTIONS THIS TARGET MAY DEPEND ON:\n");
    if let Some(results) = dep_results["results"].as_array() {
        for (i, result) in results.iter().enumerate().take(8) {
            context.push_str(&format!(
                "{}. {}: {} in {}\n   Summary: {}\n",
                i + 1,
                result["name"].as_str().unwrap_or("unknown"),
                result["node_type"].as_str().unwrap_or("unknown"),
                result["path"].as_str().unwrap_or("unknown"),
                result["summary"].as_str().unwrap_or("no summary")
            ));
        }
    }

    // 3. Add graph relationship analysis
    context.push_str("\nGRAPH RELATIONSHIPS:\n");
    let graph = state.graph.lock().await;

    // Try to find the target function in the graph and get its connections
    if let Some(results) = usage_results["results"].as_array() {
        for result in results.iter().take(3) {
            if let Some(id_str) = result["id"].as_str() {
                if let Ok(node_id) = uuid::Uuid::parse_str(id_str) {
                    if let Ok(neighbors) = graph.get_neighbors(node_id).await {
                        context.push_str(&format!(
                            "  {} has {} connected components\n",
                            result["name"].as_str().unwrap_or("unknown"),
                            neighbors.len()
                        ));
                    }
                }
            }
        }
    }

    // Truncate if too long (leave room for analysis)
    if context.len() > 60000 * 4 {
        // ~60K tokens worth
        context.truncate(60000 * 4);
        context.push_str("\n\n[Context truncated for analysis efficiency]");
    }

    Ok(context)
}

// Helper functions for impact analysis parsing
#[cfg(feature = "qwen-integration")]
pub fn parse_dependency_info(context: &str) -> Value {
    let callers_count = context.matches("FUNCTIONS THAT MAY CALL").count();
    let dependencies_count = context.matches("FUNCTIONS THIS TARGET MAY DEPEND").count();

    json!({
        "potential_callers": callers_count,
        "potential_dependencies": dependencies_count,
        "has_graph_relationships": context.contains("connected components"),
        "context_truncated": context.contains("[Context truncated")
    })
}

#[cfg(feature = "qwen-integration")]
pub fn extract_risk_level(analysis: &str) -> Value {
    let risk_level = if analysis.contains("HIGH") || analysis.contains("CRITICAL") {
        "HIGH"
    } else if analysis.contains("MEDIUM") || analysis.contains("MODERATE") {
        "MEDIUM"
    } else if analysis.contains("LOW") || analysis.contains("MINIMAL") {
        "LOW"
    } else {
        "UNKNOWN"
    };

    let reasoning = if let Some(start) = analysis.find("RISK_ASSESSMENT:") {
        let risk_section = &analysis[start..];
        if let Some(end) = risk_section.find("2.") {
            risk_section[..end].trim()
        } else {
            "See comprehensive analysis for risk reasoning"
        }
    } else {
        "Risk assessment included in analysis"
    };

    json!({
        "level": risk_level,
        "reasoning": reasoning,
        "confidence": if risk_level == "UNKNOWN" { 0.5 } else { 0.8 }
    })
}

#[cfg(feature = "qwen-integration")]
pub fn extract_affected_components(analysis: &str) -> Value {
    // Extract affected components section
    let components = if let Some(start) = analysis.find("AFFECTED_COMPONENTS:") {
        let components_section = &analysis[start..];
        if let Some(end) = components_section.find("3.") {
            components_section[..end].trim()
        } else {
            "See analysis for affected components"
        }
    } else {
        "Affected components analysis included"
    };

    json!({
        "analysis": components,
        "has_specific_components": analysis.contains("function") || analysis.contains("class") || analysis.contains("module")
    })
}

#[cfg(feature = "qwen-integration")]
pub fn extract_testing_requirements(analysis: &str) -> Value {
    let testing = if let Some(start) = analysis.find("TESTING_STRATEGY:") {
        let testing_section = &analysis[start..];
        if let Some(end) = testing_section.find("5.") {
            testing_section[..end].trim()
        } else {
            "See analysis for testing strategy"
        }
    } else {
        "Testing requirements included in analysis"
    };

    json!({
        "strategy": testing,
        "has_specific_tests": analysis.contains("test") || analysis.contains("spec") || analysis.contains("assert")
    })
}

#[cfg(feature = "qwen-integration")]
pub fn extract_implementation_plan(analysis: &str) -> Value {
    let plan = if let Some(start) = analysis.find("IMPLEMENTATION_PLAN:") {
        let plan_section = &analysis[start..];
        if let Some(end) = plan_section.find("6.") {
            plan_section[..end].trim()
        } else {
            "See analysis for implementation guidance"
        }
    } else {
        "Implementation plan included in analysis"
    };

    json!({
        "plan": plan,
        "has_step_by_step": analysis.contains("step") || analysis.contains("1.") && analysis.contains("2.")
    })
}

#[cfg(feature = "qwen-integration")]
pub fn extract_safety_recommendations(analysis: &str) -> Value {
    let safety = if let Some(start) = analysis.find("ROLLBACK_STRATEGY:") {
        let safety_section = &analysis[start..];
        safety_section.trim()
    } else if analysis.contains("safe") || analysis.contains("rollback") {
        "Safety recommendations included in analysis"
    } else {
        "Standard safety practices apply"
    };

    json!({
        "recommendations": safety,
        "has_rollback_plan": analysis.contains("rollback") || analysis.contains("undo"),
        "safety_level": if analysis.contains("critical") || analysis.contains("dangerous") {
            "high_attention"
        } else {
            "standard_precautions"
        }
    })
}

// Pattern detection MCP tool - works without external models using semantic analysis
pub async fn pattern_detection(state: &ServerState, params: Value) -> Result<Value, String> {
    let scope = params
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("project");

    let focus_area = params
        .get("focus_area")
        .and_then(|v| v.as_str())
        .unwrap_or("all_patterns");

    let max_results = params
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    // 1. Gather code samples for pattern analysis using existing search
    let search_query = match focus_area {
        "naming" => "function class variable naming",
        "error_handling" => "try catch throw error exception",
        "imports" => "import require use from",
        "architecture" => "service component module class",
        "testing" => "test spec assert expect describe",
        _ => "function class method", // General pattern detection
    };

    let graph = state.graph.lock().await;
    let search_results =
        bin_search_with_scores_shared(search_query.to_string(), None, None, max_results, &graph)
            .await
            .map_err(|e| e.to_string())?;

    // 2. Use existing semantic analysis for pattern detection (no external model needed)
    let pattern_detector = crate::pattern_detector::PatternDetector::new(
        crate::pattern_detector::PatternConfig::default(),
    );

    let team_intelligence = pattern_detector
        .detect_patterns_from_search(&search_results)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Structure response for MCP-calling LLMs
    let response = json!({
        "scope": scope,
        "focus_area": focus_area,
        "team_intelligence": crate::pattern_detector::team_intelligence_to_json(&team_intelligence),
        "pattern_summary": {
            "total_patterns_detected": team_intelligence.patterns.len(),
            "high_confidence_patterns": team_intelligence.patterns.iter().filter(|p| p.confidence > 0.8).count(),
            "team_conventions_identified": team_intelligence.conventions.len(),
            "overall_quality_score": team_intelligence.quality_metrics.overall_score,
            "consistency_score": team_intelligence.quality_metrics.consistency_score
        },
        "actionable_insights": generate_pattern_insights(&team_intelligence),
        "generation_guidance": {
            "recommended_patterns": team_intelligence.patterns.iter()
                .filter(|p| p.quality_score > 0.7)
                .map(|p| json!({
                    "pattern": p.name,
                    "description": p.description,
                    "usage_frequency": p.frequency,
                    "recommended": true
                }))
                .collect::<Vec<_>>(),
            "avoid_patterns": team_intelligence.patterns.iter()
                .filter(|p| p.quality_score < 0.5)
                .map(|p| json!({
                    "pattern": p.name,
                    "reason": "Low quality or inconsistent usage",
                    "frequency": p.frequency,
                    "recommended": false
                }))
                .collect::<Vec<_>>()
        },
        "mcp_metadata": {
            "tool_version": "1.0.0",
            "analysis_method": "semantic_analysis_without_external_model",
            "uses_existing_codegraph_infrastructure": true,
            "recommended_for": ["claude", "gpt-4", "custom-agents"]
        }
    });

    Ok(response)
}

// Generate actionable insights from pattern analysis
pub fn generate_pattern_insights(
    intelligence: &crate::pattern_detector::TeamIntelligence,
) -> Value {
    let mut insights = Vec::new();

    // Quality insights
    if intelligence.quality_metrics.overall_score > 0.8 {
        insights.push("High-quality codebase with consistent patterns");
    } else if intelligence.quality_metrics.overall_score < 0.6 {
        insights.push("Codebase could benefit from more consistent patterns");
    }

    // Consistency insights
    if intelligence.quality_metrics.consistency_score > 0.8 {
        insights.push("Excellent consistency in coding conventions");
    } else if intelligence.quality_metrics.consistency_score < 0.6 {
        insights.push("Consider establishing and enforcing coding standards");
    }

    // Pattern-specific insights
    let naming_patterns = intelligence
        .patterns
        .iter()
        .filter(|p| {
            matches!(
                p.pattern_type,
                crate::pattern_detector::PatternType::NamingConvention
            )
        })
        .count();

    if naming_patterns > 0 {
        insights.push("Strong naming conventions detected - good for maintainability");
    } else {
        insights.push("Consider establishing consistent naming conventions");
    }

    let error_patterns = intelligence
        .patterns
        .iter()
        .filter(|p| {
            matches!(
                p.pattern_type,
                crate::pattern_detector::PatternType::ErrorHandling
            )
        })
        .count();

    if error_patterns > 0 {
        insights.push("Good error handling patterns - promotes reliability");
    } else {
        insights.push("Consider standardizing error handling approaches");
    }

    json!({
        "insights": insights,
        "priority_recommendations": generate_priority_recommendations(intelligence),
        "team_strengths": identify_team_strengths(intelligence),
        "improvement_opportunities": identify_improvements(intelligence)
    })
}

fn generate_priority_recommendations(
    intelligence: &crate::pattern_detector::TeamIntelligence,
) -> Vec<String> {
    let mut recommendations = Vec::new();

    if intelligence.quality_metrics.consistency_score < 0.7 {
        recommendations.push("Priority: Establish and document coding standards".to_string());
    }

    if intelligence
        .patterns
        .iter()
        .filter(|p| {
            matches!(
                p.pattern_type,
                crate::pattern_detector::PatternType::ErrorHandling
            )
        })
        .count()
        < 2
    {
        recommendations.push("Priority: Standardize error handling patterns".to_string());
    }

    if intelligence.conventions.len() < 3 {
        recommendations.push("Priority: Define and enforce team conventions".to_string());
    }

    recommendations
}

fn identify_team_strengths(
    intelligence: &crate::pattern_detector::TeamIntelligence,
) -> Vec<String> {
    let mut strengths = Vec::new();

    if intelligence.quality_metrics.overall_score > 0.8 {
        strengths.push("High overall code quality".to_string());
    }

    if intelligence.quality_metrics.consistency_score > 0.8 {
        strengths.push("Excellent consistency in coding style".to_string());
    }

    if intelligence.patterns.len() > 10 {
        strengths.push("Rich set of established patterns".to_string());
    }

    strengths
}

fn identify_improvements(intelligence: &crate::pattern_detector::TeamIntelligence) -> Vec<String> {
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
