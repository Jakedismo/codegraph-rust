// ABOUTME: Implements a high-performance async Language Server Protocol client
// ABOUTME: Provides pipelined request handling and concurrent file processing

use anyhow::{Result, anyhow};
use codegraph_core::{CodeNode, EdgeRelationship};
use dashmap::DashMap;
use futures::{StreamExt, stream};
use serde_json::Value as JsonValue;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};
use url::Url;

pub fn encode_lsp_message(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.as_bytes().len(), body).into_bytes()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDocumentSymbol {
    pub name: String,
    pub qualified_name: String,
    pub start_line: u32,
}

pub fn collect_document_symbols(symbols: &JsonValue, joiner: &str) -> Vec<LspDocumentSymbol> {
    fn walk(out: &mut Vec<LspDocumentSymbol>, v: &JsonValue, prefix: Option<&str>, joiner: &str) {
        let Some(name) = v.get("name").and_then(|v| v.as_str()) else {
            return;
        };
        let container = v.get("containerName").and_then(|v| v.as_str());
        let qualified = if let Some(p) = prefix {
            format!("{}{}{}", p, joiner, name)
        } else if let Some(container) = container {
            format!("{}{}{}", container, joiner, name)
        } else {
            name.to_string()
        };

        let start_line = v
            .get("range")
            .or_else(|| v.get("location").and_then(|l| l.get("range")))
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0) as u32;

        out.push(LspDocumentSymbol {
            name: name.to_string(),
            qualified_name: qualified.clone(),
            start_line,
        });

        if let Some(children) = v.get("children").and_then(|c| c.as_array()) {
            for child in children {
                walk(out, child, Some(&qualified), joiner);
            }
        }
    }

    let mut out = Vec::new();
    if let Some(arr) = symbols.as_array() {
        for entry in arr {
            walk(&mut out, entry, None, joiner);
        }
    }
    out
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LspEnrichmentStats {
    pub nodes_enriched: usize,
    pub edges_resolved: usize,
}

/// Async LSP Client handle
/// Manages the lifecycle of the LSP server process and concurrent request dispatch.
#[derive(Clone)]
pub struct LspClient {
    tx: mpsc::Sender<LspRequest>,
    pending_requests: Arc<DashMap<u64, oneshot::Sender<Result<JsonValue>>>>,
    next_id: Arc<AtomicU64>,
    requests: Arc<tokio::sync::Semaphore>,
    documents: Arc<DashMap<String, (String, i64)>>,
    _tasks: Arc<SessionTasks>,
}

struct SessionTasks(Vec<tokio::task::AbortHandle>);
impl Drop for SessionTasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}
#[derive(Default)]
pub struct LspPool {
    clients: tokio::sync::Mutex<std::collections::BTreeMap<String, LspClient>>,
}
impl LspPool {
    async fn client(&self, command: &Path, args: &[&str], root: &str) -> Result<LspClient> {
        let key = codegraph_core::artifact_cache::fingerprint(&(command, args, root))?;
        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get(&key).filter(|client| !client.tx.is_closed()) {
            return Ok(client.clone());
        }
        let client = LspClient::start(command, args, root).await?;
        clients.insert(key, client.clone());
        Ok(client)
    }
    pub async fn clear(&self) {
        self.clients.lock().await.clear();
    }
}

enum LspRequest {
    ServerResponse(JsonValue),
    Request {
        id: u64,
        method: String,
        params: JsonValue,
    },
    Notify {
        method: String,
        params: JsonValue,
    },
}

impl LspClient {
    pub async fn start(command: &Path, args: &[&str], root_uri: &str) -> Result<Self> {
        let start = Instant::now();
        info!(
            "🧠 Starting LSP server (async): {} (rootUri={})",
            command.display(),
            root_uri
        );

        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr"))?;

        let (tx, mut rx) = mpsc::channel::<LspRequest>(100);
        let pending_requests = Arc::new(DashMap::<u64, oneshot::Sender<Result<JsonValue>>>::new());
        let pending_requests_read = pending_requests.clone();

        // Writer task
        let writer = tokio::spawn(async move {
            let _child_guard = child; // Keep child alive as long as this task runs
            while let Some(msg) = rx.recv().await {
                let json = match msg {
                    LspRequest::ServerResponse(json) => json,
                    LspRequest::Request { id, method, params } => {
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": method,
                            "params": params
                        })
                    }
                    LspRequest::Notify { method, params } => {
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": method,
                            "params": params
                        })
                    }
                };

                let body = serde_json::to_string(&json).unwrap();
                let framed = encode_lsp_message(&body);
                if let Err(e) = stdin.write_all(&framed).await {
                    error!("LSP stdin write failed: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("LSP stdin flush failed: {}", e);
                    break;
                }
            }
        });

        // Reader task
        let response_tx = tx.clone();
        let server_root = root_uri.to_owned();
        let reader = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut content_length_buf = String::new();

            'messages: loop {
                content_length_buf.clear();
                // Read headers
                let mut content_length: Option<usize> = None;

                loop {
                    if reader.read_line(&mut content_length_buf).await.unwrap_or(0) == 0 {
                        break 'messages; // EOF
                    }
                    let line = content_length_buf.trim();
                    if line.is_empty() {
                        break; // End of headers
                    }

                    let lower = line.to_ascii_lowercase();
                    if let Some(rest) = lower.strip_prefix("content-length:") {
                        content_length = rest.trim().parse::<usize>().ok();
                    }
                    content_length_buf.clear();
                }

                let Some(len) = content_length else {
                    continue; // Skip malformed or keep reading
                };

                let mut body_buf = vec![0u8; len];
                if let Err(e) = reader.read_exact(&mut body_buf).await {
                    error!("LSP body read failed: {}", e);
                    break;
                }

                let Ok(body_str) = std::str::from_utf8(&body_buf) else {
                    continue;
                };

                let Ok(json) = serde_json::from_str::<JsonValue>(body_str) else {
                    continue;
                };

                if let (Some(id), Some(method)) =
                    (json.get("id"), json.get("method").and_then(|v| v.as_str()))
                {
                    let result = match method {
                        "workspace/configuration" => serde_json::json!(
                            json["params"]["items"]
                                .as_array()
                                .map_or(vec![], |items| vec![JsonValue::Null; items.len()])
                        ),
                        "workspace/workspaceFolders" => {
                            serde_json::json!([{"uri": server_root, "name": "project"}])
                        }
                        "workspace/applyEdit" => serde_json::json!({"applied": false}),
                        "client/registerCapability"
                        | "client/unregisterCapability"
                        | "window/workDoneProgress/create" => JsonValue::Null,
                        _ => {
                            let _ = response_tx.send(LspRequest::ServerResponse(serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Unsupported client request"}}))).await;
                            continue;
                        }
                    };
                    let _ = response_tx
                        .send(LspRequest::ServerResponse(
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
                        ))
                        .await;
                } else if let Some(id) = json.get("id").and_then(|id| id.as_u64())
                    && let Some((_, tx)) = pending_requests_read.remove(&id)
                {
                    let result = json.get("error").map_or_else(
                        || Ok(json.get("result").cloned().unwrap_or(JsonValue::Null)),
                        |error| Err(anyhow!("LSP error: {error}")),
                    );
                    let _ = tx.send(result);
                }
                // We ignore notifications from server for now
            }
            let pending: Vec<_> = pending_requests_read
                .iter()
                .map(|entry| *entry.key())
                .collect();
            for id in pending {
                if let Some((_, response)) = pending_requests_read.remove(&id) {
                    let _ = response.send(Err(anyhow!("Language server closed its output stream")));
                }
            }
        });

        // Stderr logger
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }
                // debug!("LSP stderr: {}", line.trim());
                line.clear();
            }
        });

        let client = Self {
            tx,
            pending_requests,
            next_id: Arc::new(AtomicU64::new(1)),
            requests: Arc::new(tokio::sync::Semaphore::new(
                std::env::var("CODEGRAPH_LSP_REQUESTS")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(32)
                    .clamp(1, 128),
            )),
            documents: Arc::new(DashMap::new()),
            _tasks: Arc::new(SessionTasks(vec![
                writer.abort_handle(),
                reader.abort_handle(),
                stderr_task.abort_handle(),
            ])),
        };

        // Initialize
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "documentSymbol": {},
                    "definition": {},
                    "references": {}
                },
                "workspace": {}
            }
        });

        let _ = client.request("initialize", init_params).await?;
        client.notify("initialized", serde_json::json!({})).await?;

        info!("🧠 LSP server initialized in {:.1?}", start.elapsed());
        Ok(client)
    }

    pub async fn request(&self, method: &str, params: JsonValue) -> Result<JsonValue> {
        let _permit = self.requests.acquire().await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        self.pending_requests.insert(id, tx);
        struct PendingGuard {
            map: Arc<DashMap<u64, oneshot::Sender<Result<JsonValue>>>>,
            id: u64,
        }
        impl Drop for PendingGuard {
            fn drop(&mut self) {
                self.map.remove(&self.id);
            }
        }
        let _pending_guard = PendingGuard {
            map: self.pending_requests.clone(),
            id,
        };

        self.tx
            .send(LspRequest::Request {
                id,
                method: method.to_string(),
                params,
            })
            .await
            .map_err(|_| anyhow!("LSP server channel closed"))?;

        // 30s timeout for individual requests
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(res) => Ok(res.map_err(|_| anyhow!("LSP response channel closed"))??),
            Err(_) => {
                self.pending_requests.remove(&id);
                Err(anyhow!("LSP request timed out: {}", method))
            }
        }
    }

    pub async fn notify(&self, method: &str, params: JsonValue) -> Result<()> {
        self.tx
            .send(LspRequest::Notify {
                method: method.to_string(),
                params,
            })
            .await
            .map_err(|_| anyhow!("LSP server channel closed"))?;
        Ok(())
    }
}

pub fn enrich_nodes_and_edges_with_lsp(
    server_path: &Path,
    server_args: &[&str],
    language_id: &str,
    name_joiner: &str,
    resolve_definitions: bool,
    project_root: &Path,
    files: &[PathBuf],
    nodes: &mut [CodeNode],
    edges: &mut [EdgeRelationship],
) -> Result<LspEnrichmentStats> {
    // Bridge to async world using a runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        enrich_async(
            &LspPool::default(),
            None,
            server_path,
            server_args,
            language_id,
            name_joiner,
            resolve_definitions,
            project_root,
            files,
            nodes,
            edges,
        )
        .await
    })
}

// Keep the shared batch interface explicit across language-specific adapters.
#[allow(clippy::too_many_arguments)]
pub async fn enrich_async(
    pool: &LspPool,
    sources: Option<&codegraph_parser::SourceSnapshots>,
    server_path: &Path,
    server_args: &[&str],
    language_id: &str,
    name_joiner: &str,
    resolve_definitions: bool,
    project_root: &Path,
    files: &[PathBuf],
    nodes: &mut [CodeNode],
    edges: &mut [EdgeRelationship],
) -> Result<LspEnrichmentStats> {
    let project_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let root_uri = Url::from_directory_path(&project_root)
        .map_err(|_| anyhow::anyhow!("failed to create file URI"))?
        .to_string();

    let client = pool.client(server_path, server_args, &root_uri).await?;

    if let Some(sources) = sources {
        let current: std::collections::HashSet<_> = sources
            .iter()
            .map(|source| absolute_file_path(&project_root, &source.path))
            .collect();
        let deleted: Vec<_> = client
            .documents
            .iter()
            .filter_map(|entry| {
                let path = Url::parse(entry.key()).ok()?.to_file_path().ok()?;
                (!current.contains(&path)).then(|| entry.key().clone())
            })
            .collect();
        for uri in deleted {
            client
                .notify(
                    "textDocument/didClose",
                    serde_json::json!({"textDocument": {"uri": uri}}),
                )
                .await?;
            client.documents.remove(&uri);
        }
    }
    // Build lookup maps (same as before)
    let mut nodes_by_file_line_name: std::collections::HashMap<(String, u32, String), Vec<usize>> =
        std::collections::HashMap::new();
    let mut nodes_by_file_line: std::collections::HashMap<(String, u32), Vec<usize>> =
        std::collections::HashMap::new();
    let mut files_with_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, node) in nodes.iter().enumerate() {
        let file = node.location.file_path.clone();
        let line0 = node.location.line.saturating_sub(1);
        for key in normalized_file_keys(&project_root, Path::new(&file)) {
            nodes_by_file_line_name
                .entry((key.clone(), line0, node.name.to_string()))
                .or_default()
                .push(idx);
            nodes_by_file_line
                .entry((key.clone(), line0))
                .or_default()
                .push(idx);
            files_with_nodes.insert(key);
        }
    }

    let mut utf16_columns: Vec<_> = nodes
        .iter()
        .map(|node| {
            (
                node.location.column,
                node.location.end_column.unwrap_or(u32::MAX),
            )
        })
        .collect();
    if let Some(sources) = sources {
        let mut by_file = std::collections::BTreeMap::<&str, Vec<usize>>::new();
        for (index, node) in nodes.iter().enumerate() {
            by_file
                .entry(&node.location.file_path)
                .or_default()
                .push(index);
        }
        for snapshot in sources.iter() {
            let Some(indices) = by_file.get(snapshot.path.to_string_lossy().as_ref()) else {
                continue;
            };
            let text = snapshot.contents()?;
            let lines: Vec<_> = text.split('\n').collect();
            for &index in indices {
                let node = &nodes[index];
                let column = |line: u32, bytes: u32| -> u32 {
                    lines
                        .get(line.saturating_sub(1) as usize)
                        .and_then(|text| text.get(..bytes as usize))
                        .map(|prefix| prefix.encode_utf16().count() as u32)
                        .unwrap_or(bytes)
                };
                utf16_columns[index] = (
                    column(node.location.line, node.location.column),
                    node.location
                        .end_column
                        .map(|bytes| {
                            column(node.location.end_line.unwrap_or(node.location.line), bytes)
                        })
                        .unwrap_or(u32::MAX),
                );
            }
        }
    }

    let def_edges_by_file = if resolve_definitions {
        definition_edge_indices_by_file(&project_root, nodes, edges)
    } else {
        std::collections::HashMap::new()
    };
    let def_edges_by_file = Arc::new(def_edges_by_file); // Share across tasks

    // Filter files
    let mut files_to_process: Vec<PathBuf> = Vec::new();
    for file_path in files {
        let file_keys = normalized_file_keys(&project_root, file_path);
        let has_nodes = file_keys.iter().any(|key| files_with_nodes.contains(key));
        let has_edges = file_keys
            .iter()
            .any(|key| def_edges_by_file.contains_key(key));
        if !has_nodes && !has_edges {
            continue;
        }
        files_to_process.push(file_path.clone());
    }

    let total_files = files_to_process.len();
    info!(
        "🧠 LSP Analysis: Processing {} files concurrently",
        total_files
    );

    // Pre-collect edge spans to avoid borrowing `edges` inside the async block
    let mut file_edge_spans: std::collections::HashMap<String, Vec<(usize, u32)>> =
        std::collections::HashMap::new();

    if resolve_definitions {
        for (file, indices) in def_edges_by_file.iter() {
            let mut spans = Vec::new();
            for &idx in indices {
                if let Some(span) = edges[idx].span.as_ref() {
                    spans.push((idx, span.start_byte));
                }
            }
            file_edge_spans.insert(file.clone(), spans);
        }
    }
    let file_edge_spans = Arc::new(file_edge_spans);

    let stream = stream::iter(files_to_process)
        .map(|file_path| {
            let client = client.clone();
            let project_root = project_root.clone();
            let language_id = match file_path.extension().and_then(|ext| ext.to_str()) {
                Some("js" | "mjs" | "cjs") => "javascript", Some("jsx") => "javascriptreact", Some("tsx") => "typescriptreact", _ => language_id,
            }.to_string();
            let file_edge_spans = file_edge_spans.clone();

            async move {
                let abs_path = absolute_file_path(&project_root, &file_path);
                let content = match sources.and_then(|sources| sources.get(&abs_path).or_else(|| sources.get(&file_path))) {
                    Some(source) => source.contents_async().await?,
                    None => std::sync::Arc::<str>::from(tokio::fs::read_to_string(&abs_path).await?),
                };
                let file_keys = normalized_file_keys(&project_root, &file_path);

                let uri = Url::from_file_path(&abs_path).map_err(|_| anyhow!("Invalid source URI: {}", abs_path.display()))?;
                let uri_str = uri.to_string();

                let pos_index = LspPositionIndex::new(&content);

                let source_hash = codegraph_core::artifact_cache::fingerprint(&content.as_ref())?;
                let previous = client.documents.get(&uri_str).map(|entry| entry.clone());
                if let Some((hash, version)) = previous {
                    if hash != source_hash {
                        client.notify("textDocument/didChange", serde_json::json!({"textDocument": {"uri": uri_str, "version": version + 1}, "contentChanges": [{"text": content.as_ref()}]})).await?;
                        client.documents.insert(uri_str.clone(), (source_hash, version + 1));
                    }
                } else {
                    client.notify("textDocument/didOpen", serde_json::json!({"textDocument": {"uri": uri_str, "languageId": language_id, "version": 1, "text": content.as_ref()}})).await?;
                    client.documents.insert(uri_str.clone(), (source_hash, 1));
                }

                // Symbols
                let symbols = client
                    .request(
                        "textDocument/documentSymbol",
                        serde_json::json!({ "textDocument": { "uri": uri_str } }),
                    )
                    .await?;

                // Definitions
                let mut def_results = Vec::new();
                if resolve_definitions && !file_keys.is_empty() {
                    let mut positions = std::collections::BTreeMap::<(u32,u32), Vec<usize>>::new();
                    let mut seen_edges = std::collections::HashSet::new();
                    for key in &file_keys { if let Some(spans) = file_edge_spans.get(key) {
                        for &(edge_idx, byte_offset) in spans { if seen_edges.insert(edge_idx) {
                            let pos = pos_index.position_for_byte_offset(byte_offset);
                            positions.entry((pos.line, pos.character)).or_default().push(edge_idx);
                        } }
                    } }
                    let mut definitions = stream::iter(positions.into_iter().map(|((line, character), indices)| {
                        let client = client.clone(); let uri = uri_str.clone();
                        async move { let definition = client.request("textDocument/definition", serde_json::json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}})).await?;
                            Ok::<_,anyhow::Error>((indices, definition)) }
                    })).buffer_unordered(32);
                    while let Some(result) = definitions.next().await { let (indices, definition) = result?; for index in indices { def_results.push((index, definition.clone())); } }
                }

                Ok::<_, anyhow::Error>(Some((file_keys, symbols, def_results)))
            }
        })
        .buffer_unordered(16); // Concurrency limit: 16 files at once

    let mut stats = LspEnrichmentStats::default();
    let mut results = stream;
    let mut processed = 0;

    // Process results as they come in and mutate state
    while let Some(res) = results.next().await {
        if let Some((file_keys, symbols, def_results)) = res? {
            // 1. Process Symbols
            for sym in collect_document_symbols(&symbols, name_joiner) {
                let mut node_idx: Option<usize> = None;
                for key in &file_keys {
                    let key_tuple = (key.clone(), sym.start_line, sym.name.clone());
                    if let Some(indices) = nodes_by_file_line_name.get(&key_tuple) {
                        // A name and line alone cannot distinguish same-line definitions.
                        node_idx = unique_symbol_index(indices);
                        break;
                    }
                }
                if let Some(node_idx) = node_idx {
                    let node = &mut nodes[node_idx];
                    node.metadata
                        .attributes
                        .insert("qualified_name".to_string(), sym.qualified_name.clone());
                    node.metadata
                        .attributes
                        .insert("analyzer".to_string(), "lsp_symbols".to_string());
                    node.metadata
                        .attributes
                        .insert("analyzer_confidence".to_string(), "1.0".to_string());
                    stats.nodes_enriched += 1;
                }
            }

            // 2. Process Definitions
            for (edge_idx, def) in def_results {
                let Some((target_file, target_line0, target_column)) =
                    extract_first_definition_location(&def)
                else {
                    continue;
                };

                let candidates = nodes_by_file_line
                    .get(&(target_file.clone(), target_line0))
                    .or_else(|| {
                        let relative = relative_file_key(&project_root, Path::new(&target_file))?;
                        nodes_by_file_line.get(&(relative, target_line0))
                    });
                let target_idx = candidates.and_then(|indices| {
                    let mut ranked: Vec<_> = indices
                        .iter()
                        .copied()
                        .filter(|index| {
                            let node = &nodes[*index];
                            let (start, end) = utf16_columns[*index];
                            start <= target_column
                                && (node.location.end_line != Some(node.location.line)
                                    || target_column <= end)
                        })
                        .collect();
                    ranked.sort_by_key(|index| {
                        nodes[*index].span.as_ref().map_or(u32::MAX, |span| {
                            span.end_byte.saturating_sub(span.start_byte)
                        })
                    });
                    match ranked.as_slice() {
                        [index] => Some(*index),
                        [first, second, ..]
                            if nodes[*first]
                                .span
                                .as_ref()
                                .map(|span| span.end_byte - span.start_byte)
                                != nodes[*second]
                                    .span
                                    .as_ref()
                                    .map(|span| span.end_byte - span.start_byte) =>
                        {
                            Some(*first)
                        }
                        _ => None,
                    }
                });

                if let Some(target_idx) = target_idx {
                    let target = &nodes[target_idx];
                    let target_name = target
                        .metadata
                        .attributes
                        .get("qualified_name")
                        .cloned()
                        .unwrap_or_else(|| target.name.to_string());

                    let edge = &mut edges[edge_idx];
                    edge.to = target_name;
                    edge.metadata
                        .insert("target_node_id".into(), target.id.to_string());
                    edge.metadata
                        .insert("analyzer".to_string(), "lsp_definition".to_string());
                    edge.metadata
                        .insert("analyzer_confidence".to_string(), "1.0".to_string());
                    stats.edges_resolved += 1;
                }
            }
            processed += 1;
            if processed % 10 == 0 {
                info!(
                    "🧠 LSP progress: {}/{} files processed",
                    processed, total_files
                );
            }
        }
    }

    Ok(stats)
}

fn absolute_file_path(project_root: &Path, file_path: &Path) -> PathBuf {
    let combined = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        project_root.join(file_path)
    };
    std::fs::canonicalize(&combined).unwrap_or_else(|_| normalize_path(&combined))
}

fn relative_file_key(project_root: &Path, file_path: &Path) -> Option<String> {
    let abs = absolute_file_path(project_root, file_path);
    abs.strip_prefix(project_root)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn unique_symbol_index(indices: &[usize]) -> Option<usize> {
    match indices {
        [index] => Some(*index),
        _ => None,
    }
}

fn extract_first_definition_location(def: &JsonValue) -> Option<(String, u32, u32)> {
    let loc = if let Some(arr) = def.as_array() {
        let first = arr.first()?;
        if arr.iter().any(|location| location != first) {
            return None;
        }
        first
    } else {
        def
    };

    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())?;
    let range = loc
        .get("targetSelectionRange")
        .or_else(|| loc.get("range"))
        .or_else(|| loc.get("targetRange"))?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;

    let url = Url::parse(uri).ok()?;
    let path = url.to_file_path().ok()?;
    let column = start.get("character")?.as_u64()? as u32;
    Some((path.to_string_lossy().to_string(), line, column))
}

pub fn byte_offset_to_utf16_position(text: &str, byte_offset: u32) -> LspPosition {
    let target = (byte_offset as usize).min(text.len());

    let mut line: u32 = 0;
    let mut character: u32 = 0;

    for (idx, ch) in text.char_indices() {
        if idx >= target {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
            continue;
        }

        character += ch.encode_utf16(&mut [0u16; 2]).len() as u32;
    }

    LspPosition { line, character }
}

#[derive(Debug, Clone)]
pub struct LspPositionIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LspPositionIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = Vec::new();
        line_starts.push(0);
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                let next = idx.saturating_add(1);
                if next <= text.len() {
                    line_starts.push(next);
                }
            }
        }
        Self { text, line_starts }
    }

    pub fn position_for_byte_offset(&self, byte_offset: u32) -> LspPosition {
        let target = (byte_offset as usize).min(self.text.len());
        let line_idx = match self.line_starts.binary_search(&target) {
            Ok(i) => i,
            Err(insert) => insert.saturating_sub(1),
        };
        let line_start = *self.line_starts.get(line_idx).unwrap_or(&0);

        let mut character: u32 = 0;
        for (idx, ch) in self.text[line_start..].char_indices() {
            let abs = line_start.saturating_add(idx);
            if abs >= target {
                break;
            }
            character += ch.encode_utf16(&mut [0u16; 2]).len() as u32;
        }

        LspPosition {
            line: line_idx as u32,
            character,
        }
    }
}

fn normalized_file_keys(project_root: &Path, file_path: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    let normalized = absolute_file_path(project_root, file_path);
    let normalized_str = normalized.to_string_lossy().to_string();
    keys.push(normalized_str.clone());

    if normalized.is_absolute() {
        if let Some(rel) = normalized.strip_prefix(project_root).ok() {
            keys.push(rel.to_string_lossy().to_string());
        }
    } else {
        let abs = absolute_file_path(project_root, &normalized);
        keys.push(abs.to_string_lossy().to_string());
    }

    let mut seen = std::collections::HashSet::new();
    keys.retain(|key| seen.insert(key.clone()));
    keys
}

fn definition_edge_indices_by_file(
    project_root: &Path,
    nodes: &[CodeNode],
    edges: &[EdgeRelationship],
) -> std::collections::HashMap<String, Vec<usize>> {
    let mut file_by_id: std::collections::HashMap<codegraph_core::NodeId, String> =
        std::collections::HashMap::with_capacity(nodes.len());

    for node in nodes {
        file_by_id.insert(node.id, node.location.file_path.clone());
    }

    let mut out: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (idx, edge) in edges.iter().enumerate() {
        if edge.span.is_none() {
            continue;
        }
        let Some(file_key) = file_by_id.get(&edge.from) else {
            continue;
        };

        for key in normalized_file_keys(project_root, Path::new(file_key)) {
            out.entry(key).or_default().push(idx);
        }
    }

    out
}

pub fn decode_one_lsp_message(buffer: &[u8]) -> Result<Option<(String, usize)>> {
    let buf_str = match std::str::from_utf8(buffer) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let Some(header_end) = buf_str.find("\r\n\r\n") else {
        return Ok(None);
    };

    let headers = &buf_str[..header_end];
    let mut content_length: Option<usize> = None;
    for line in headers.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }

    let Some(content_length) = content_length else {
        return Ok(None);
    };

    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Ok(None);
    }

    let body = std::str::from_utf8(&buffer[body_start..body_end])?.to_string();
    Ok(Some((body, body_end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_symbol_and_definition_results_remain_unresolved() {
        assert_eq!(unique_symbol_index(&[1]), Some(1));
        assert_eq!(unique_symbol_index(&[1, 2]), None);
        let first = serde_json::json!({"uri":"file:///tmp/a.rs", "range":{"start":{"line":0,"character":3}}});
        let second = serde_json::json!({"uri":"file:///tmp/b.rs", "range":{"start":{"line":0,"character":3}}});
        assert!(extract_first_definition_location(&serde_json::json!([first, second])).is_none());
        assert!(extract_first_definition_location(&serde_json::json!([first, first])).is_some());
    }

    #[tokio::test]
    async fn closed_server_fails_initialization_without_waiting_for_timeout() {
        let started = Instant::now();
        let result = LspClient::start(Path::new("/usr/bin/true"), &[], "file:///tmp").await;
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[tokio::test]
    async fn warm_server_answers_client_requests_and_deduplicates_definition_positions() {
        use codegraph_core::{EdgeType, Language, Location, NodeType, Span};
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("server.py");
        std::fs::write(&script, r#"
import sys,json
def send(value):
    body=json.dumps(value).encode()
    sys.stdout.buffer.write(('Content-Length: %d\r\n\r\n'%len(body)).encode()+body)
    sys.stdout.buffer.flush()
counts={'definitions':0,'opens':0,'changes':0}
parallel=[]
while True:
    line=sys.stdin.buffer.readline()
    if not line: break
    length=int(line.decode().split(':')[1]); sys.stdin.buffer.readline()
    msg=json.loads(sys.stdin.buffer.read(length)); method=msg.get('method')
    if method=='initialize':
        initialize=msg['id']; root=msg['params']['rootUri']
        send({'jsonrpc':'2.0','id':'configuration','method':'workspace/configuration','params':{'items':[{}]}})
    elif msg.get('id')=='configuration':
        assert msg['result']==[None]
        send({'jsonrpc':'2.0','id':initialize,'result':{'capabilities':{}}})
    elif method=='textDocument/didOpen': counts['opens']+=1
    elif method=='textDocument/didChange': counts['changes']+=1
    elif method=='textDocument/documentSymbol': send({'jsonrpc':'2.0','id':msg['id'],'result':[]})
    elif method=='textDocument/definition':
        counts['definitions']+=1
        send({'jsonrpc':'2.0','id':msg['id'],'result':{'uri':root.rstrip('/')+'/target.rs','range':{'start':{'line':0,'character':3},'end':{'line':0,'character':9}}}})
    elif method=='test/counts': send({'jsonrpc':'2.0','id':msg['id'],'result':counts})
    elif method=='test/parallel':
        parallel.append(msg['id'])
        if len(parallel)==2:
            for id in parallel: send({'jsonrpc':'2.0','id':id,'result':True})
            parallel=[]
"#).unwrap();
        let caller = dir.path().join("caller.rs");
        let target = dir.path().join("target.rs");
        std::fs::write(&caller, "fn caller() { target(); }").unwrap();
        std::fs::write(&target, "fn target() {}").unwrap();
        let files = vec![caller.clone(), target.clone()];
        let make_node = |name: &str, path: &Path| {
            CodeNode::new(
                name.to_owned(),
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: path.to_string_lossy().into_owned(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(30),
                },
            )
            .with_deterministic_id("project")
        };
        let mut nodes = vec![make_node("caller", &caller), make_node("target", &target)];
        let mut edges = vec![
            EdgeRelationship {
                from: nodes[0].id,
                to: "target".into(),
                edge_type: EdgeType::Calls,
                metadata: Default::default(),
                span: Some(Span {
                    start_byte: 14,
                    end_byte: 20
                })
            };
            2
        ];
        let sources = codegraph_parser::SourceSnapshots::capture(
            &files
                .iter()
                .map(|path| (path.clone(), std::fs::metadata(path).unwrap().len()))
                .collect::<Vec<_>>(),
            1024 * 1024,
            2,
        )
        .await
        .unwrap();
        let pool = LspPool::default();
        let args = ["-u", script.to_str().unwrap()];
        for _ in 0..2 {
            enrich_async(
                &pool,
                Some(&sources),
                Path::new("python3"),
                &args,
                "rust",
                "::",
                true,
                dir.path(),
                &files,
                &mut nodes,
                &mut edges,
            )
            .await
            .unwrap();
        }
        assert!(
            edges
                .iter()
                .all(|edge| edge.metadata["target_node_id"] == nodes[1].id.to_string())
        );
        let uri = Url::from_directory_path(std::fs::canonicalize(dir.path()).unwrap())
            .unwrap()
            .to_string();
        let client = pool
            .client(Path::new("python3"), &args, &uri)
            .await
            .unwrap();
        let counts = client
            .request("test/counts", JsonValue::Null)
            .await
            .unwrap();
        assert_eq!(counts["definitions"], 2);
        assert_eq!(counts["opens"], 2);
        let (a, b) = tokio::join!(
            client.request("test/parallel", JsonValue::Null),
            client.request("test/parallel", JsonValue::Null)
        );
        assert_eq!(a.unwrap(), true);
        assert_eq!(b.unwrap(), true);
        pool.clear().await;
    }

    #[test]
    fn lsp_message_round_trips_through_framing() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let encoded = encode_lsp_message(body);
        let decoded = decode_one_lsp_message(&encoded)
            .expect("decode should succeed")
            .expect("message should be complete");
        assert_eq!(decoded.0, body);
        assert_eq!(decoded.1, encoded.len());
    }

    #[test]
    fn byte_offsets_map_to_utf16_positions() {
        let text = "a🙂b\nc";
        let index = LspPositionIndex::new(text);

        for offset in 0..=(text.len() as u32) {
            let expected = byte_offset_to_utf16_position(text, offset);
            let observed = index.position_for_byte_offset(offset);
            assert_eq!(observed, expected, "mismatch at byte offset {offset}");
        }
    }
}
