// ABOUTME: Implements a minimal Language Server Protocol client for analyzer-backed indexing
// ABOUTME: Provides message framing and request helpers for symbol resolution and enrichment

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship};
use serde_json::Value as JsonValue;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, info};
use url::Url;

#[cfg(unix)]
use nix::fcntl::{fcntl, FcntlArg, OFlag};
#[cfg(unix)]
use std::os::fd::AsFd;

pub fn encode_lsp_message(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.as_bytes().len(), body).into_bytes()
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
    let start_total = Instant::now();
    let root_uri = Url::from_directory_path(project_root)
        .map_err(|_| anyhow::anyhow!("failed to create file URI for {:?}", project_root))?
        .to_string();

    let mut proc = LspProcess::start(server_path, server_args, &root_uri)?;

    let mut nodes_by_file_line_name: std::collections::HashMap<(String, u32, String), usize> =
        std::collections::HashMap::new();
    let mut nodes_by_file_line: std::collections::HashMap<(String, u32), usize> =
        std::collections::HashMap::new();
    let mut node_file_by_id: std::collections::HashMap<codegraph_core::NodeId, String> =
        std::collections::HashMap::new();

    for (idx, node) in nodes.iter().enumerate() {
        let file = node.location.file_path.clone();
        let line0 = node.location.line.saturating_sub(1);
        nodes_by_file_line_name.insert((file.clone(), line0, node.name.to_string()), idx);
        nodes_by_file_line
            .entry((file.clone(), line0))
            .or_insert(idx);
        node_file_by_id.insert(node.id, file);
    }

    let mut stats = LspEnrichmentStats::default();
    let total_files = files.len().max(1);
    let mut processed_files: usize = 0;
    let mut last_progress_log = Instant::now();

    for file_path in files {
        let content = std::fs::read_to_string(file_path)?;
        let file_str = file_path.to_string_lossy().to_string();
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow::anyhow!("failed to create file URI for {}", file_str))?
            .to_string();

        proc.notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": content
                }
            }),
        )?;

        let symbols = proc.request(
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": uri }
            }),
        )?;

        for sym in collect_document_symbols(&symbols, name_joiner) {
            if let Some(&node_idx) =
                nodes_by_file_line_name.get(&(file_str.clone(), sym.start_line, sym.name.clone()))
            {
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

        for edge in edges.iter_mut() {
            let Some(from_file) = node_file_by_id.get(&edge.from) else {
                continue;
            };
            if *from_file != file_str {
                continue;
            }
            let Some(span) = edge.span.as_ref() else {
                continue;
            };

            let pos = byte_offset_to_utf16_position(&content, span.start_byte);
            let def = proc.request(
                "textDocument/definition",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": pos.line, "character": pos.character }
                }),
            )?;

            let Some((target_file, target_line0)) = extract_first_definition_location(&def) else {
                continue;
            };

            if let Some(&target_idx) = nodes_by_file_line.get(&(target_file.clone(), target_line0))
            {
                let target = &nodes[target_idx];
                let target_name = target
                    .metadata
                    .attributes
                    .get("qualified_name")
                    .cloned()
                    .unwrap_or_else(|| target.name.to_string());
                edge.to = target_name;
                edge.metadata
                    .insert("analyzer".to_string(), "lsp_definition".to_string());
                edge.metadata
                    .insert("analyzer_confidence".to_string(), "1.0".to_string());
                stats.edges_resolved += 1;
            }
        }

        processed_files += 1;
        if last_progress_log.elapsed() >= Duration::from_secs(10) {
            info!(
                "🧠 LSP progress: {}/{} files | enriched {} symbols | resolved {} edges | elapsed {:.1?}",
                processed_files,
                total_files,
                stats.nodes_enriched,
                stats.edges_resolved,
                start_total.elapsed()
            );
            last_progress_log = Instant::now();
        }
    }

    Ok(stats)
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

pub struct LspProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
    read_buffer: Vec<u8>,
    next_id: u64,
    stderr_buffer: String,
}

impl LspProcess {
    pub fn start(command: &Path, args: &[&str], root_uri: &str) -> Result<Self> {
        let start = Instant::now();
        info!(
            "🧠 Starting LSP server: {} (rootUri={})",
            command.display(),
            root_uri
        );
        debug!("🧠 LSP args: {:?}", args);

        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing stdout"))?;
        let stderr = child.stderr.take();

        #[cfg(unix)]
        set_stdout_nonblocking(&stdout)?;

        let mut proc = Self {
            child,
            stdin,
            stdout,
            stderr,
            read_buffer: Vec::with_capacity(16 * 1024),
            next_id: 1,
            stderr_buffer: String::new(),
        };

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

        let _ = proc.request_with_timeout("initialize", init_params, lsp_request_timeout())?;
        proc.notify("initialized", serde_json::json!({}))?;

        info!("🧠 LSP server initialized in {:.1?}", start.elapsed());
        Ok(proc)
    }

    pub fn notify(&mut self, method: &str, params: JsonValue) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.write_message(&msg)?;
        Ok(())
    }

    pub fn request(&mut self, method: &str, params: JsonValue) -> Result<JsonValue> {
        self.request_with_timeout(method, params, lsp_request_timeout())
    }

    pub fn request_with_timeout(
        &mut self,
        method: &str,
        params: JsonValue,
        timeout: Duration,
    ) -> Result<JsonValue> {
        let id = self.next_id;
        self.next_id += 1;

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        if let Err(e) = self.write_message(&msg) {
            if let Some(status) = self.child.try_wait().ok().flatten() {
                let stderr = self.read_stderr_snapshot().unwrap_or_default();
                if stderr.is_empty() {
                    return Err(anyhow::anyhow!(
                        "LSP request write failed; process exited: {}",
                        status
                    ))
                    .map_err(|err| err.context(e));
                }
                return Err(anyhow::anyhow!(
                    "LSP request write failed; process exited: {} stderr={}",
                    status,
                    stderr.trim()
                ))
                .map_err(|err| err.context(e));
            }
            return Err(e);
        }

        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                let status = self.child.try_wait().ok().flatten();
                return Err(anyhow::anyhow!(
                    "LSP request timed out after {:.1?}: method={} id={} status={:?}",
                    timeout,
                    method,
                    id,
                    status
                ));
            }

            let next = self.read_message_until(deadline)?;
            let Some(v) = next else {
                if let Some(status) = self.child.try_wait().ok().flatten() {
                    let stderr = self.read_stderr_snapshot().unwrap_or_default();
                    if stderr.is_empty() {
                        return Err(anyhow::anyhow!("LSP process exited: {}", status));
                    }
                    return Err(anyhow::anyhow!(
                        "LSP process exited: {} stderr={}",
                        status,
                        stderr.trim()
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };

            if v.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(err) = v.get("error") {
                    return Err(anyhow::anyhow!("LSP request failed: {}", err));
                }
                return Ok(v.get("result").cloned().unwrap_or_else(|| JsonValue::Null));
            }
        }
    }

    fn write_message(&mut self, msg: &JsonValue) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let framed = encode_lsp_message(&body);
        self.stdin.write_all(&framed)?;
        self.stdin.flush()?;
        Ok(())
    }

    fn read_stderr_snapshot(&mut self) -> Result<String> {
        if self.stderr.is_none() {
            return Ok(self.stderr_buffer.clone());
        }

        let mut buf = Vec::new();
        if let Some(mut stderr) = self.stderr.take() {
            let _ = stderr.read_to_end(&mut buf);
        }

        if !buf.is_empty() {
            let text = String::from_utf8_lossy(&buf);
            if !self.stderr_buffer.is_empty() {
                self.stderr_buffer.push('\n');
            }
            self.stderr_buffer.push_str(text.trim_end());
        }

        Ok(self.stderr_buffer.clone())
    }

    fn read_message_until(&mut self, deadline: Instant) -> Result<Option<JsonValue>> {
        loop {
            if let Some((body, consumed)) = decode_one_lsp_message(&self.read_buffer)? {
                self.read_buffer.drain(..consumed);
                let v: JsonValue = serde_json::from_str(&body)?;
                return Ok(Some(v));
            }

            if Instant::now() >= deadline {
                return Ok(None);
            }

            let mut buf = [0u8; 8192];
            let n = match self.stdout.read(&mut buf) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) => return Err(e.into()),
            };
            if n == 0 {
                return Ok(None);
            }
            self.read_buffer.extend_from_slice(&buf[..n]);
        }
    }
}

impl Drop for LspProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
fn set_stdout_nonblocking(stdout: &ChildStdout) -> Result<()> {
    let flags = OFlag::from_bits_truncate(fcntl(stdout.as_fd(), FcntlArg::F_GETFL)?);
    fcntl(stdout.as_fd(), FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK))?;
    Ok(())
}

fn lsp_request_timeout() -> Duration {
    let secs = std::env::var("CODEGRAPH_LSP_REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600);
    Duration::from_secs(secs.max(5))
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
        let pos_a = byte_offset_to_utf16_position(text, 0);
        assert_eq!(
            pos_a,
            LspPosition {
                line: 0,
                character: 0
            }
        );

        let pos_b = byte_offset_to_utf16_position(text, 1);
        assert_eq!(
            pos_b,
            LspPosition {
                line: 0,
                character: 1
            }
        );

        let emoji_start = "a".len() as u32;
        let after_emoji = ("a🙂".len()) as u32;
        let pos_after_emoji = byte_offset_to_utf16_position(text, after_emoji);
        assert_eq!(
            pos_after_emoji,
            LspPosition {
                line: 0,
                character: 3
            }
        );

        let pos_second_line = byte_offset_to_utf16_position(text, ("a🙂b\n".len()) as u32);
        assert_eq!(
            pos_second_line,
            LspPosition {
                line: 1,
                character: 0
            }
        );
        let _ = emoji_start;
    }

    #[test]
    fn collects_hierarchical_document_symbols_with_qualified_names() {
        let symbols = serde_json::json!([
            {
                "name": "mod_a",
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 10, "character": 0 } },
                "children": [
                    {
                        "name": "foo",
                        "range": { "start": { "line": 2, "character": 0 }, "end": { "line": 3, "character": 0 } }
                    }
                ]
            }
        ]);

        let flat = collect_document_symbols(&symbols, "::");
        assert!(flat.iter().any(|s| s.qualified_name == "mod_a"));
        assert!(flat
            .iter()
            .any(|s| s.qualified_name == "mod_a::foo" && s.start_line == 2));
    }

    #[cfg(unix)]
    #[test]
    fn lsp_request_times_out_when_server_is_silent() {
        let mut child = Command::new("sh")
            .args(["-c", "cat >/dev/null"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn fake LSP server");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take();

        set_stdout_nonblocking(&stdout).expect("set nonblocking");

        let mut proc = LspProcess {
            child,
            stdin,
            stdout,
            stderr,
            read_buffer: Vec::new(),
            next_id: 1,
            stderr_buffer: String::new(),
        };

        let err = proc
            .request_with_timeout(
                "initialize",
                serde_json::json!({}),
                Duration::from_millis(50),
            )
            .expect_err("initialize should time out");
        let msg = format!("{err:#}");
        assert!(msg.contains("timed out"), "unexpected error: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn lsp_exit_includes_stderr_in_error() {
        let mut child = Command::new("sh")
            .args(["-c", "echo \"boom\" 1>&2; exit 1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn fake LSP server");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take();

        set_stdout_nonblocking(&stdout).expect("set nonblocking");

        let mut proc = LspProcess {
            child,
            stdin,
            stdout,
            stderr,
            read_buffer: Vec::new(),
            next_id: 1,
            stderr_buffer: String::new(),
        };

        let err = proc
            .request_with_timeout(
                "initialize",
                serde_json::json!({}),
                Duration::from_millis(200),
            )
            .expect_err("initialize should fail");
        let msg = format!("{err:#}");
        assert!(msg.contains("boom"), "stderr should be included: {msg}");
    }
}
