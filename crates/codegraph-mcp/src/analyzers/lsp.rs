// ABOUTME: Implements a high-performance async Language Server Protocol client
// ABOUTME: Provides pipelined request handling and concurrent file processing

use anyhow::{anyhow, Result};
use codegraph_core::{CodeNode, EdgeRelationship};
use dashmap::DashMap;
use futures::{stream, StreamExt};
use serde_json::Value as JsonValue;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
#[derive(Clone)]
pub struct LspClient {
    tx: mpsc::Sender<LspRequest>,
    pending_requests: Arc<DashMap<u64, oneshot::Sender<Result<JsonValue>>>>,
    next_id: Arc<AtomicU64>,
}

enum LspRequest {
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

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("missing stderr"))?;

        let (tx, mut rx) = mpsc::channel::<LspRequest>(100);
        let pending_requests = Arc::new(DashMap::<u64, oneshot::Sender<Result<JsonValue>>>::new());
        let pending_requests_read = pending_requests.clone();

        // Writer task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let json = match msg {
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
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut content_length_buf = String::new();

            loop {
                content_length_buf.clear();
                // Read headers
                let mut content_length: Option<usize> = None;
                
                loop {
                    if reader.read_line(&mut content_length_buf).await.unwrap_or(0) == 0 {
                        return; // EOF
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

                // Handle response
                if let Some(id) = json.get("id").and_then(|id| id.as_u64()) {
                    if let Some((_, tx)) = pending_requests_read.remove(&id) {
                        if let Some(error) = json.get("error") {
                            let _ = tx.send(Err(anyhow!("LSP error: {}", error)));
                        } else {
                            let result = json.get("result").cloned().unwrap_or(JsonValue::Null);
                            let _ = tx.send(Ok(result));
                        }
                    }
                }
                // We ignore notifications from server for now
            }
        });

        // Stderr logger
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 { break; }
                // debug!("LSP stderr: {}", line.trim());
                line.clear();
            }
        });

        let client = Self {
            tx,
            pending_requests,
            next_id: Arc::new(AtomicU64::new(1)),
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
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        
        self.pending_requests.insert(id, tx);
        
        self.tx.send(LspRequest::Request {
            id,
            method: method.to_string(),
            params,
        }).await.map_err(|_| anyhow!("LSP server channel closed"))?;

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
        self.tx.send(LspRequest::Notify {
            method: method.to_string(),
            params,
        }).await.map_err(|_| anyhow!("LSP server channel closed"))?;
        Ok(())
    }
}

pub fn enrich_nodes_and_edges_with_lsp(
    server_path: &Path,
    server_args: &[&str],
    language_id: &str,
    name_joiner: &str,
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
        enrich_async(server_path, server_args, language_id, name_joiner, project_root, files, nodes, edges).await
    })
}

async fn enrich_async(
    server_path: &Path,
    server_args: &[&str],
    language_id: &str,
    name_joiner: &str,
    project_root: &Path,
    files: &[PathBuf],
    nodes: &mut [CodeNode],
    edges: &mut [EdgeRelationship],
) -> Result<LspEnrichmentStats> {
    let project_root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let root_uri = Url::from_directory_path(&project_root)
        .map_err(|_| anyhow::anyhow!("failed to create file URI"))?
        .to_string();

    let client = LspClient::start(server_path, server_args, &root_uri).await?;

    // Build lookup maps (same as before)
    let mut nodes_by_file_line_name: std::collections::HashMap<(String, u32, String), usize> =
        std::collections::HashMap::new();
    let mut nodes_by_file_line: std::collections::HashMap<(String, u32), usize> =
        std::collections::HashMap::new();
    let mut node_file_by_id: std::collections::HashMap<codegraph_core::NodeId, String> =
        std::collections::HashMap::new();
    let mut files_with_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, node) in nodes.iter().enumerate() {
        let file = node.location.file_path.clone();
        if let Some(abs) = absolute_file_key(&project_root, Path::new(&file)) {
            if abs != file {
                let line0 = node.location.line.saturating_sub(1);
                nodes_by_file_line_name.insert((abs.clone(), line0, node.name.to_string()), idx);
                nodes_by_file_line.entry((abs.clone(), line0)).or_insert(idx);
                files_with_nodes.insert(abs);
            }
        }
        let line0 = node.location.line.saturating_sub(1);
        nodes_by_file_line_name.insert((file.clone(), line0, node.name.to_string()), idx);
        nodes_by_file_line.entry((file.clone(), line0)).or_insert(idx);
        node_file_by_id.insert(node.id, file);
        if let Some(file) = node_file_by_id.get(&node.id) {
            files_with_nodes.insert(file.clone());
        }
    }

    let def_edges_by_file = definition_edge_indices_by_file(&project_root, nodes, edges);
    let def_edges_by_file = Arc::new(def_edges_by_file); // Share across tasks

    // Filter files
    let mut files_to_process: Vec<PathBuf> = Vec::new();
    for file_path in files {
        let abs_path = absolute_file_path(&project_root, file_path);
        let file_str = file_path.to_string_lossy().to_string();
        let abs_file_str = abs_path.to_string_lossy().to_string();
        if !files_with_nodes.contains(&file_str)
            && !files_with_nodes.contains(&abs_file_str)
            && !def_edges_by_file.contains_key(&file_str)
            && !def_edges_by_file.contains_key(&abs_file_str)
        {
            continue;
        }
        files_to_process.push(file_path.clone());
    }

    let total_files = files_to_process.len();
    info!("🧠 LSP Analysis: Processing {} files concurrently", total_files);

    // Pre-collect edge spans to avoid borrowing `edges` inside the async block
    let mut file_edge_spans: std::collections::HashMap<String, Vec<(usize, u32)>> = std::collections::HashMap::new();
    
    for (file, indices) in def_edges_by_file.iter() {
        let mut spans = Vec::new();
        for &idx in indices {
            if let Some(span) = edges[idx].span.as_ref() {
                spans.push((idx, span.start_byte));
            }
        }
        file_edge_spans.insert(file.clone(), spans);
    }
    let file_edge_spans = Arc::new(file_edge_spans);

    let stream = stream::iter(files_to_process)
        .map(|file_path| {
            let client = client.clone();
            let project_root = project_root.clone();
            let language_id = language_id.to_string();
            let file_edge_spans = file_edge_spans.clone();
            
            async move {
                let abs_path = absolute_file_path(&project_root, &file_path);
                let Ok(content) = tokio::fs::read_to_string(&abs_path).await else { return Ok(None) };
                let file_str = file_path.to_string_lossy().to_string();
                let abs_file_str = abs_path.to_string_lossy().to_string();
                
                let Ok(uri) = Url::from_file_path(&abs_path) else { return Ok(None) };
                let uri_str = uri.to_string();
                
                let pos_index = LspPositionIndex::new(&content);

                // Open
                client.notify(
                    "textDocument/didOpen",
                    serde_json::json!({
                        "textDocument": {
                            "uri": uri_str,
                            "languageId": language_id,
                            "version": 1,
                            "text": content
                        }
                    })
                ).await?;

                // Symbols
                let symbols = client.request(
                    "textDocument/documentSymbol",
                    serde_json::json!({ "textDocument": { "uri": uri_str } }),
                ).await?;

                // Definitions
                let mut def_results = Vec::new();
                if let Some(spans) = file_edge_spans.get(&abs_file_str).or_else(|| file_edge_spans.get(&file_str)) {
                    for &(edge_idx, byte_offset) in spans {
                        let pos = pos_index.position_for_byte_offset(byte_offset);
                        // Fire definition request
                        let def_response = client.request(
                            "textDocument/definition",
                            serde_json::json!({
                                "textDocument": { "uri": uri_str },
                                "position": { "line": pos.line, "character": pos.character }
                            })
                        ).await;
                        
                        if let Ok(def) = def_response {
                            def_results.push((edge_idx, def));
                        }
                    }
                }

                // Close (fire and forget)
                let _ = client.notify(
                    "textDocument/didClose",
                    serde_json::json!({ "textDocument": { "uri": uri_str } }),
                ).await;

                Ok::<_, anyhow::Error>(Some((file_str, abs_file_str, symbols, def_results)))
            }
        })
        .buffer_unordered(16); // Concurrency limit: 16 files at once

    let mut stats = LspEnrichmentStats::default();
    let mut results = stream;
    let mut processed = 0;
    
    // Process results as they come in and mutate state
    while let Some(res) = results.next().await {
        if let Ok(Some((file_str, abs_file_str, symbols, def_results))) = res {
            // 1. Process Symbols
            for sym in collect_document_symbols(&symbols, name_joiner) {
                let rel_key = (file_str.clone(), sym.start_line, sym.name.clone());
                let abs_key = (abs_file_str.clone(), sym.start_line, sym.name.clone());
                let node_idx = nodes_by_file_line_name
                    .get(&rel_key)
                    .or_else(|| nodes_by_file_line_name.get(&abs_key))
                    .copied();
                if let Some(node_idx) = node_idx {
                    let node = &mut nodes[node_idx];
                    node.metadata.attributes.insert("qualified_name".to_string(), sym.qualified_name.clone());
                    node.metadata.attributes.insert("analyzer".to_string(), "lsp_symbols".to_string());
                    node.metadata.attributes.insert("analyzer_confidence".to_string(), "1.0".to_string());
                    stats.nodes_enriched += 1;
                }
            }

            // 2. Process Definitions
            for (edge_idx, def) in def_results {
                let Some((target_file, target_line0)) = extract_first_definition_location(&def) else { continue; };
                
                let target_idx = nodes_by_file_line
                    .get(&(target_file.clone(), target_line0))
                    .copied()
                    .or_else(|| {
                        let rel_target = Path::new(&target_file);
                        let rel_key = relative_file_key(&project_root, rel_target)?;
                        nodes_by_file_line.get(&(rel_key, target_line0)).copied()
                    });

                if let Some(target_idx) = target_idx {
                    let target = &nodes[target_idx];
                    let target_name = target.metadata.attributes.get("qualified_name")
                        .cloned()
                        .unwrap_or_else(|| target.name.to_string());
                    
                    let edge = &mut edges[edge_idx];
                    edge.to = target_name;
                    edge.metadata.insert("analyzer".to_string(), "lsp_definition".to_string());
                    edge.metadata.insert("analyzer_confidence".to_string(), "1.0".to_string());
                    stats.edges_resolved += 1;
                }
            }
            processed += 1;
            if processed % 10 == 0 {
                 info!("🧠 LSP progress: {}/{} files processed", processed, total_files);
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
    normalize_path(&combined)
}

fn absolute_file_key(project_root: &Path, file_path: &Path) -> Option<String> {
    Some(
        absolute_file_path(project_root, file_path)
            .to_string_lossy()
            .to_string(),
    )
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
            Component::CurDir => {{}}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn extract_first_definition_location(def: &JsonValue) -> Option<(String, u32)> {
    let loc = if let Some(arr) = def.as_array() {
        arr.first()? 
    } else {
        def
    };

    let uri = loc
        .get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())?;
    let range = loc.get("range").or_else(|| loc.get("targetRange"))?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;

    let url = Url::parse(uri).ok()?;
    let path = url.to_file_path().ok()?;
    Some((path.to_string_lossy().to_string(), line))
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

        out.entry(file_key.clone()).or_default().push(idx);
        if let Some(abs) = absolute_file_key(project_root, Path::new(file_key)) {
            if abs != *file_key {
                out.entry(abs).or_default().push(idx);
            }
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
    use std::time::Duration;

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