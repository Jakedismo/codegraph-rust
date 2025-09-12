use serde_json::{json, Value};
use codegraph_core::GraphStore;
use std::sync::Arc;

#[derive(Clone)]
struct ServerState {
    graph: Arc<tokio::sync::Mutex<codegraph_graph::CodeGraph>>,
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
