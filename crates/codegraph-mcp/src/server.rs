use serde_json::{json, Value};
use codegraph_core::GraphStore;
use std::sync::Arc;

#[cfg(feature = "qwen-integration")]
use crate::qwen::{QwenClient, QwenConfig};

#[derive(Clone)]
struct ServerState {
    graph: Arc<tokio::sync::Mutex<codegraph_graph::CodeGraph>>,
    #[cfg(feature = "qwen-integration")]
    qwen_client: Option<QwenClient>,
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
        #[cfg(feature = "qwen-integration")]
        "codegraph.semantic_intelligence" => semantic_intelligence(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.enhanced_search" => enhanced_search(state, params).await,
        #[cfg(feature = "qwen-integration")]
        "codegraph.performance_metrics" => performance_metrics(state, params).await,
        _ => Err(format!("Unknown method: {}", method)),
    }
}

// Handlers
async fn vector_search(_state: &ServerState, params: Value) -> Result<Value, String> {
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

    let res = bin_search_with_scores(query, paths, langs, limit)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res)
}

async fn graph_neighbors(state: &ServerState, params: Value) -> Result<Value, String> {
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

async fn graph_traverse(state: &ServerState, params: Value) -> Result<Value, String> {
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
    let dry = params.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
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

// Vector search helper; FAISS path if enabled, else fallback
pub async fn bin_search_with_scores(
    query: String,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
    limit: usize,
) -> anyhow::Result<Value> {
    #[cfg(feature = "faiss")]
    {
        use faiss::index::io::read_index;
        use faiss::index::Index as _;
        use std::path::Path;

        // Build embedding via engine
        let emb = {
            #[cfg(feature = "embeddings")]
            {
                let gen = codegraph_vector::EmbeddingGenerator::with_auto_from_env().await;
                let e = gen.generate_text_embedding(&query).await?;
                crate::indexer::normalize(&e)
            }
            #[cfg(not(feature = "embeddings"))]
            {
                let dimension = 1536;
                let e = crate::indexer::simple_text_embedding(&query, dimension);
                crate::indexer::normalize(&e)
            }
        };

        let mut scored: Vec<(codegraph_core::NodeId, f32)> = Vec::new();
        let mut search_index = |
            index_path: &Path,
            ids_path: &Path,
            topk: usize,
        | -> anyhow::Result<()> {
            if !index_path.exists() || !ids_path.exists() {
                return Ok(());
            }
            let mut index = read_index(index_path.to_string_lossy())?;
            let mapping_raw = std::fs::read_to_string(ids_path)?;
            let mapping: Vec<codegraph_core::NodeId> = serde_json::from_str(&mapping_raw)?;
            let res = index.search(&emb, topk)?;
            for (i, label) in res.labels.into_iter().enumerate() {
                if let Some(idx_val) = label.get() {
                    let idx = idx_val as usize;
                    if idx < mapping.len() {
                        let score = res.distances[i];
                        scored.push((mapping[idx], score));
                    }
                }
            }
            Ok(())
        };
        let mut shard_count = 0usize;
        if let Some(prefs) = &paths {
            for p in prefs {
                let seg = p.trim_start_matches("./").split('/').next().unwrap_or("");
                if seg.is_empty() {
                    continue;
                }
                let idx = Path::new(".codegraph/shards/path").join(format!("{}.index", seg));
                let ids = Path::new(".codegraph/shards/path").join(format!("{}_ids.json", seg));
                let _ = search_index(&idx, &ids, limit * 5)?;
                shard_count += 1;
            }
        }
        if let Some(l) = &langs {
            for lang in l {
                let norm = lang.to_lowercase();
                let idx = Path::new(".codegraph/shards/lang").join(format!("{}.index", norm));
                let ids = Path::new(".codegraph/shards/lang").join(format!("{}_ids.json", norm));
                let _ = search_index(&idx, &ids, limit * 5)?;
                shard_count += 1;
            }
        }
        if shard_count == 0 {
            let idx = Path::new(".codegraph/faiss.index");
            let ids = Path::new(".codegraph/faiss_ids.json");
            let _ = search_index(idx, ids, limit * 5)?;
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.dedup_by_key(|(id, _)| *id);
        let top: Vec<(codegraph_core::NodeId, f32)> = scored.into_iter().take(limit).collect();

    let graph = {
        use std::time::Duration;
        let mut attempts = 0;
        loop {
            match codegraph_graph::CodeGraph::new_read_only() {
                Ok(g) => break g,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("LOCK") && attempts < 10 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        attempts += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    };
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
        return Ok(json!({"results": out}));
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
                    Err(e) => json!({"jsonrpc":"2.0","id": id, "error": {"code": -32000, "message": e}}),
                }
            }
            Err(e) => json!({"jsonrpc":"2.0","id": null, "error": {"code": -32700, "message": format!("parse error: {}", e)}}),
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
async fn init_qwen_client() -> Option<QwenClient> {
    let config = QwenConfig::default();
    let client = QwenClient::new(config);

    match client.check_availability().await {
        Ok(true) => {
            tracing::info!("✅ Qwen2.5-Coder-14B-128K available for CodeGraph intelligence");
            Some(client)
        }
        Ok(false) => {
            tracing::warn!("⚠️ Qwen2.5-Coder model not found. Install with: ollama pull qwen2.5-coder-14b-128k");
            None
        }
        Err(e) => {
            tracing::error!("❌ Failed to connect to Qwen2.5-Coder: {}", e);
            None
        }
    }
}

// Enhanced semantic search with Qwen2.5-Coder intelligence
#[cfg(feature = "qwen-integration")]
async fn enhanced_search(state: &ServerState, params: Value) -> Result<Value, String> {
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

    // 1. Perform standard vector search using existing functionality
    let search_results = bin_search_with_scores(query.clone(), None, None, max_results * 2)
        .await
        .map_err(|e| e.to_string())?;

    // 2. If Qwen analysis is requested and available, enhance results
    if include_analysis {
        if let Some(qwen_client) = &state.qwen_client {
            // Build context from search results for Qwen analysis
            let search_context = build_search_context(&search_results, &query);

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
                    // Combine search results with Qwen intelligence
                    return Ok(json!({
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
                    }));
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
async fn semantic_intelligence(state: &ServerState, params: Value) -> Result<Value, String> {
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
    let qwen_client = state.qwen_client.as_ref()
        .ok_or("Qwen2.5-Coder not available. Please install: ollama pull qwen2.5-coder-14b-128k")?;

    // 1. Gather comprehensive codebase context
    let codebase_context = build_comprehensive_context(state, &query, max_context_tokens).await
        .map_err(|e| e.to_string())?;

    // 2. Use Qwen2.5-Coder for comprehensive analysis
    let analysis_result = qwen_client.analyze_codebase(&query, &codebase_context).await
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
    Ok(json!({
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
    }))
}

// Helper functions for Qwen integration
#[cfg(feature = "qwen-integration")]
fn build_search_context(search_results: &Value, query: &str) -> String {
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
async fn build_comprehensive_context(
    state: &ServerState,
    query: &str,
    max_tokens: usize,
) -> Result<String, String> {
    let mut context = format!("COMPREHENSIVE CODEBASE ANALYSIS REQUEST\n\nQUERY: {}\n\n", query);

    // Add basic search results
    let search_results = bin_search_with_scores(query.to_string(), None, None, 15)
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
                        context.push_str(&format!("  {} has {} connected nodes\n",
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
fn build_context_summary(context: &str) -> Value {
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
    // Return current performance metrics for monitoring
    Ok(crate::performance::get_performance_summary())
}
