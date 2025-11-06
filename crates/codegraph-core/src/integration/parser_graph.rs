use crate::traits::{CodeParser, GraphStore};
use crate::{CodeNode, EdgeType, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Pluggable edge writer so core does not depend on graph crate edge APIs.
#[async_trait]
pub trait EdgeSink: Send + Sync {
    async fn add_edge(
        &self,
        from: crate::NodeId,
        to: crate::NodeId,
        edge_type: EdgeType,
        metadata: HashMap<String, String>,
    ) -> Result<()>;

    async fn add_edges_batch(
        &self,
        edges: Vec<(
            crate::NodeId,
            crate::NodeId,
            EdgeType,
            HashMap<String, String>,
        )>,
    ) -> Result<()> {
        for (from, to, t, meta) in edges {
            self.add_edge(from, to, t, meta).await?;
        }
        Ok(())
    }
}

/// High-level integrator wiring parser output into graph storage with incremental updates
/// and basic dependency resolution (calls/imports).
pub struct ParserGraphIntegrator<P, G, E>
where
    P: CodeParser + Send + Sync + 'static,
    G: GraphStore + Send + 'static,
    E: EdgeSink + 'static,
{
    parser: Arc<P>,
    graph: Arc<tokio::sync::Mutex<G>>,
    edges: Arc<E>,

    // Caches for incremental updates and linking
    file_mtime: Arc<RwLock<HashMap<PathBuf, std::time::SystemTime>>>,
    // map of symbol -> NodeId (both qualified and unqualified)
    symbol_index: Arc<RwLock<HashMap<crate::SharedStr, crate::NodeId>>>,
    // map of file -> NodeIds in that file
    file_nodes: Arc<RwLock<HashMap<PathBuf, Vec<crate::NodeId>>>>,
}

impl<P, G, E> ParserGraphIntegrator<P, G, E>
where
    P: CodeParser + Send + Sync + 'static,
    G: GraphStore + Send + 'static,
    E: EdgeSink + 'static,
{
    pub fn new(parser: Arc<P>, graph: Arc<tokio::sync::Mutex<G>>, edges: Arc<E>) -> Self {
        Self {
            parser,
            graph,
            edges,
            file_mtime: Arc::new(RwLock::new(HashMap::new())),
            symbol_index: Arc::new(RwLock::new(HashMap::new())),
            file_nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a single file: parse, add nodes, create edges within batch, update caches.
    pub async fn process_file(&self, file_path: &str) -> Result<ProcessSummary> {
        let path = Path::new(file_path).to_path_buf();

        // incremental gate: skip unchanged files
        if let Ok(meta) = fs::metadata(&path).await {
            if let Ok(modified) = meta.modified() {
                if let Some(prev) = self.file_mtime.read().get(&path).cloned() {
                    if prev == modified {
                        debug!("Skipping unchanged file: {}", file_path);
                        return Ok(ProcessSummary::skipped(file_path));
                    }
                }
                self.file_mtime.write().insert(path.clone(), modified);
            }
        }

        // parse
        let nodes = self.parser.parse_file(file_path).await?;
        let mut added = Vec::with_capacity(nodes.len());
        let mut names_in_file: Vec<(crate::SharedStr, crate::NodeId, EdgeContext)> = Vec::new();

        // write nodes and build symbol index
        {
            let mut graph = self.graph.lock().await;
            for node in nodes.into_iter() {
                let id = node.id;
                let name = node.name.clone();
                let fq = qualified_name(file_path, &name);

                graph.add_node(node.clone()).await?;
                added.push(id);

                self.symbol_index.write().insert(name.clone(), id);
                self.symbol_index
                    .write()
                    .insert(crate::SharedStr::from(fq), id);
                names_in_file.push((name, id, edge_context_for(&node)));
            }
        }

        self.file_nodes.write().insert(path.clone(), added.clone());

        // derive edges within file (calls/imports)
        let edges = self.derive_edges_for_file(&path, &names_in_file).await?;
        if !edges.is_empty() {
            self.edges.add_edges_batch(edges).await?;
        }

        Ok(ProcessSummary::processed(file_path, added.len()))
    }

    /// Process a directory recursively with bounded concurrency (batch-ready for 100+ files).
    pub async fn process_directory(&self, dir: &str, max_concurrent: usize) -> Result<DirSummary> {
        let start = std::time::Instant::now();

        let files = collect_source_files(dir).await?;
        let total = files.len();
        let semaphore = Arc::new(Semaphore::new(max_concurrent.max(1)));

        let mut processed = 0usize;
        let mut skipped = 0usize;

        // Two-phase approach for better linking: first ingest nodes, then edges.
        // Phase 1: parse + add nodes for all files (incremental aware)
        for chunk in files.chunks(64) {
            for file in chunk {
                // Bound concurrency using semaphore, but avoid spawning to keep non-Send futures acceptable
                let _permit = semaphore.clone().acquire_owned().await.unwrap();
                match self.process_file(file.to_string_lossy().as_ref()).await {
                    Ok(sum) => match sum.status {
                        ProcessStatus::Processed => processed += 1,
                        ProcessStatus::Skipped => skipped += 1,
                    },
                    Err(e) => warn!("process_file error: {}", e),
                }
            }
        }

        // Phase 2: cross-file edges (imports/calls) using global symbol index
        let cross_edges = self.derive_cross_file_edges().await?;
        if !cross_edges.is_empty() {
            self.edges.add_edges_batch(cross_edges).await?;
        }

        let elapsed = start.elapsed();
        info!(
            "Processed {} files ({} skipped) in {:.2}s",
            processed,
            skipped,
            elapsed.as_secs_f64()
        );
        Ok(DirSummary {
            total,
            processed,
            skipped,
            duration: elapsed,
        })
    }

    /// OPTIMIZED: Process directory with smart filtering for performance
    pub async fn process_directory_with_config(
        &self,
        dir: &str,
        max_concurrent: usize,
        exclude_generated: bool,
    ) -> Result<DirSummary> {
        let start = std::time::Instant::now();

        // Use basic file collection but filter out massive generated files for performance
        let mut all_files = collect_source_files(dir).await?;

        if exclude_generated {
            all_files.retain(|path| {
                let path_str = path.to_string_lossy();

                // Universal build artifact exclusions (all languages)
                let excluded =
                    // Rust artifacts
                    path_str.contains("/target/") ||
                    path_str.contains("bindings.rs") ||

                    // Node.js/TypeScript artifacts
                    path_str.contains("/node_modules/") ||
                    path_str.contains("/dist/") ||
                    path_str.contains("/.next/") ||
                    path_str.contains("/out/") ||

                    // Python artifacts
                    path_str.contains("/__pycache__/") ||
                    path_str.contains("/.venv/") ||
                    path_str.contains("/venv/") ||
                    path_str.contains("/build/") ||

                    // Java artifacts
                    path_str.contains("/target/classes/") ||
                    path_str.contains("/target/generated-sources/") ||
                    path_str.contains("/.gradle/") ||

                    // Go artifacts
                    path_str.contains("/vendor/") ||

                    // C++ artifacts
                    path_str.contains("/cmake-build-") ||
                    path_str.contains("/CMakeFiles/") ||

                    // Swift artifacts
                    path_str.contains("/.build/") ||
                    path_str.contains("/DerivedData/") ||

                    // C# artifacts
                    path_str.contains("/bin/") ||
                    path_str.contains("/obj/") ||

                    // PHP artifacts
                    path_str.contains("/vendor/") ||

                    // Ruby artifacts
                    path_str.contains("/vendor/bundle/") ||

                    // Universal generated file patterns
                    path_str.contains("_generated") ||
                    path_str.contains(".generated") ||
                    path_str.contains("generated_") ||
                    path_str.ends_with(".pb.go") ||
                    path_str.ends_with(".pb.rs") ||
                    path_str.ends_with("_pb2.py") ||
                    path_str.contains("/protobuf/") ||

                    // IDE and tooling artifacts
                    path_str.contains("/.idea/") ||
                    path_str.contains("/.vscode/") ||
                    path_str.contains("/.git/") ||
                    path_str.contains("/coverage/");

                !excluded
            });
        }

        let files = all_files;
        let total = files.len();
        let semaphore = Arc::new(Semaphore::new(max_concurrent.max(1)));

        let mut processed = 0usize;
        let mut skipped = 0usize;

        info!(
            "Processing {} files for edge derivation (all languages supported)",
            total
        );

        // Two-phase approach for better linking: first ingest nodes, then edges.
        // Phase 1: parse + add nodes for all files (incremental aware) with progress tracking
        for chunk in files.chunks(64) {
            for file in chunk {
                // Bound concurrency using semaphore, but avoid spawning to keep non-Send futures acceptable
                let _permit = semaphore.clone().acquire_owned().await.unwrap();
                match self.process_file(file.to_string_lossy().as_ref()).await {
                    Ok(sum) => match sum.status {
                        ProcessStatus::Processed => {
                            processed += 1;
                            // Simple progress logging every 10 files
                            if processed % 10 == 0 {
                                info!(
                                    "Edge analysis progress: {}/{} files processed",
                                    processed, total
                                );
                            }
                        }
                        ProcessStatus::Skipped => skipped += 1,
                    },
                    Err(e) => warn!("process_file error: {}", e),
                }
            }
        }

        // Phase 2: cross-file edges (imports/calls) using global symbol index
        let cross_edges = self.derive_cross_file_edges().await?;
        if !cross_edges.is_empty() {
            self.edges.add_edges_batch(cross_edges).await?;
        }

        let elapsed = start.elapsed();
        info!(
            "Edge processing completed: {} files ({} skipped) in {:.2}s",
            processed,
            skipped,
            elapsed.as_secs_f64()
        );
        Ok(DirSummary {
            total,
            processed,
            skipped,
            duration: elapsed,
        })
    }

    /// Reprocess only changed files in a directory (incremental update).
    pub async fn incremental_update_dir(
        &self,
        dir: &str,
        max_concurrent: usize,
    ) -> Result<DirSummary> {
        self.process_directory(dir, max_concurrent).await
    }

    #[allow(dead_code)]
    fn clone_arc(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            graph: self.graph.clone(),
            edges: self.edges.clone(),
            file_mtime: self.file_mtime.clone(),
            symbol_index: self.symbol_index.clone(),
            file_nodes: self.file_nodes.clone(),
        }
    }

    async fn derive_edges_for_file(
        &self,
        file: &Path,
        names_in_file: &[(crate::SharedStr, crate::NodeId, EdgeContext)],
    ) -> Result<
        Vec<(
            crate::NodeId,
            crate::NodeId,
            EdgeType,
            HashMap<String, String>,
        )>,
    > {
        let mut edges = Vec::new();

        // Build quick lookup of local candidates and global symbols
        let global = self.symbol_index.read();
        let mut global_map: HashMap<&str, crate::NodeId> = HashMap::new();
        for (k, v) in global.iter() {
            global_map.insert(k.as_str(), *v);
        }

        // For each function/import in this file, scan for references
        for (name, from_id, ctx) in names_in_file {
            match ctx {
                EdgeContext::Import(content) => {
                    for target in parse_import_targets(content.as_str()) {
                        if let Some(&to_id) = global_map.get(target.as_str()) {
                            let mut meta = HashMap::new();
                            meta.insert("kind".into(), "import".into());
                            meta.insert("file".into(), file.to_string_lossy().to_string());
                            edges.push((*from_id, to_id, EdgeType::Imports, meta));
                        }
                    }
                }
                EdgeContext::Content(content) => {
                    // Local calls: search for other names in same file
                    for (other_name, to_id, _ctx2) in names_in_file.iter() {
                        if other_name == name {
                            continue;
                        }
                        if content
                            .as_str()
                            .contains(&format!("{}(", other_name.as_str()))
                        {
                            let mut meta = HashMap::new();
                            meta.insert("kind".into(), "call".into());
                            meta.insert("file".into(), file.to_string_lossy().to_string());
                            edges.push((*from_id, *to_id, EdgeType::Calls, meta));
                        } else if content.as_str().contains(other_name.as_str()) {
                            // Reference without obvious call signature
                            let mut meta = HashMap::new();
                            meta.insert("kind".into(), "reference".into());
                            meta.insert("file".into(), file.to_string_lossy().to_string());
                            edges.push((*from_id, *to_id, EdgeType::References, meta));
                        }
                    }
                    // Implements (rough heuristic for Rust/Java): "impl <Trait> for <Type>" or "implements <Iface>"
                    let lc = content.to_lowercase();
                    if lc.contains(" implements ") {
                        for (other_name, to_id, _ctx2) in names_in_file.iter() {
                            if lc.contains(&other_name.to_lowercase()) {
                                let mut meta = HashMap::new();
                                meta.insert("kind".into(), "implements".into());
                                meta.insert("file".into(), file.to_string_lossy().to_string());
                                edges.push((*from_id, *to_id, EdgeType::Implements, meta));
                            }
                        }
                    }
                }
                EdgeContext::Other => {}
            }
        }

        Ok(edges)
    }

    async fn derive_cross_file_edges(
        &self,
    ) -> Result<
        Vec<(
            crate::NodeId,
            crate::NodeId,
            EdgeType,
            HashMap<String, String>,
        )>,
    > {
        // Detect cross-file function call relationships by scanning function bodies against a symbol table.
        let mut edges = Vec::new();
        let mut seen: HashSet<(crate::NodeId, crate::NodeId, EdgeType)> = HashSet::new();

        // snapshot of symbol index and file map
        let symbols: Vec<(crate::SharedStr, crate::NodeId)> = self
            .symbol_index
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let files: Vec<(PathBuf, Vec<crate::NodeId>)> = self
            .file_nodes
            .read()
            .iter()
            .map(|(p, v)| (p.clone(), v.clone()))
            .collect();

        // Prepare simple names set (unique) to reduce redundant checks
        let mut simple_names: Vec<String> = Vec::new();
        {
            let mut seen_names = HashSet::new();
            for (k, _) in &symbols {
                let key = if let Some(pos) = k.rfind("::") {
                    &k[pos + 2..]
                } else {
                    k.as_str()
                };
                if !key.is_empty() && seen_names.insert(key) {
                    simple_names.push(key.to_string());
                }
            }
            // Keep it bounded to avoid quadratic blowup
            if simple_names.len() > 5000 {
                simple_names.truncate(5000);
            }
        }

        // For each node in each file, fetch content and scan for calls
        for (_file, node_ids) in files {
            // pull nodes in small batches to leverage underlying coalescers
            for node_id in node_ids {
                // Fetch node and inspect
                if let Some(node) = self.graph.lock().await.get_node(node_id).await? {
                    if !matches!(node.node_type, Some(crate::NodeType::Function)) {
                        continue;
                    }
                    let content = match &node.content {
                        Some(c) => c.as_str(),
                        None => continue,
                    };

                    // Heuristic: match `name(` for potential call
                    for name in &simple_names {
                        // Skip self calls by name equality
                        if node.name.as_str() == name {
                            continue;
                        }
                        if let Some((&target_id, _)) = symbols
                            .iter()
                            .find(|(k, _)| k.ends_with(name) || k.as_str() == name)
                            .map(|(k, v)| (v, k))
                        {
                            if content.contains(&format!("{}(", name)) {
                                let key = (node.id, target_id, EdgeType::Calls);
                                if seen.insert(key.clone()) {
                                    let mut meta = HashMap::new();
                                    meta.insert("kind".into(), "call".into());
                                    edges.push((node.id, target_id, EdgeType::Calls, meta));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(edges)
    }
}

/// Process result summaries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Processed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct ProcessSummary {
    pub file: String,
    pub status: ProcessStatus,
    pub nodes_added: usize,
}

impl ProcessSummary {
    fn processed(file: &str, nodes_added: usize) -> Self {
        Self {
            file: file.to_string(),
            status: ProcessStatus::Processed,
            nodes_added,
        }
    }
    fn skipped(file: &str) -> Self {
        Self {
            file: file.to_string(),
            status: ProcessStatus::Skipped,
            nodes_added: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirSummary {
    pub total: usize,
    pub processed: usize,
    pub skipped: usize,
    pub duration: std::time::Duration,
}

/// Edge extraction hints for a node
#[derive(Debug, Clone)]
enum EdgeContext {
    Import(crate::SharedStr),
    Content(crate::SharedStr),
    Other,
}

fn edge_context_for(node: &CodeNode) -> EdgeContext {
    match node.node_type {
        Some(crate::NodeType::Import) => node
            .content
            .clone()
            .map(EdgeContext::Import)
            .unwrap_or(EdgeContext::Other),
        Some(crate::NodeType::Function)
        | Some(crate::NodeType::Class)
        | Some(crate::NodeType::Struct) => node
            .content
            .clone()
            .map(EdgeContext::Content)
            .unwrap_or(EdgeContext::Other),
        _ => EdgeContext::Other,
    }
}

fn qualified_name(file: &str, name: &str) -> String {
    format!("{}::{}", file, name)
}

fn parse_import_targets(s: &str) -> Vec<String> {
    // naive import/use splitter covering Rust/JS/TS minimal patterns
    let mut out = Vec::new();
    let text = s.trim();
    if text.starts_with("use ") {
        // Rust: use a::b::c as d; or use a::b::{c,d};
        let body = text.trim_start_matches("use ").trim().trim_end_matches(';');
        if let Some(brace) = body.find('{') {
            if let Some(end) = body.rfind('}') {
                let inner = &body[brace + 1..end];
                for part in inner.split(',') {
                    let n = part.trim();
                    if !n.is_empty() {
                        out.push(n.split_whitespace().next().unwrap_or("").to_string());
                    }
                }
            }
        } else {
            out.push(body.split_whitespace().next().unwrap_or("").to_string());
        }
    } else if text.starts_with("import ") {
        // JS/TS: import { A, B as C } from 'x'
        if let Some(brace_start) = text.find('{') {
            if let Some(brace_end) = text.find('}') {
                let inner = &text[brace_start + 1..brace_end];
                for part in inner.split(',') {
                    let n = part.trim();
                    if n.is_empty() {
                        continue;
                    }
                    let name = n.split_whitespace().next().unwrap_or("");
                    if !name.is_empty() {
                        out.push(name.to_string());
                    }
                }
            }
        } else {
            // import Default from 'x' | import * as ns from 'x' (skip wildcard)
            let tokens: Vec<&str> = text[6..].split_whitespace().collect();
            if !tokens.is_empty() && tokens[0] != "*" {
                out.push(tokens[0].trim_matches(',').to_string());
            }
        }
    }
    out
}

async fn collect_source_files(dir: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![PathBuf::from(dir)];
    while let Some(d) = dirs.pop() {
        let mut rd = match fs::read_dir(&d).await {
            Ok(x) => x,
            Err(e) => {
                warn!("read_dir failed for {}: {}", d.display(), e);
                continue;
            }
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            if let Ok(ft) = entry.file_type().await {
                if ft.is_dir() {
                    dirs.push(path);
                } else if ft.is_file() && is_supported_source(&path) {
                    files.push(path);
                }
            }
        }
    }
    Ok(files)
}

fn is_supported_source(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some("rs" | "ts" | "js" | "py" | "go" | "java" | "cpp" | "cc" | "cxx") => true,
        _ => false,
    }
}
