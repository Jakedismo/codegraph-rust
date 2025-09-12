use anyhow::Result;
use codegraph_core::{CodeNode, NodeType, GraphStore};
use codegraph_graph::CodeGraph;
use codegraph_parser::TreeSitterParser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use regex::Regex;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

pub struct IndexerConfig {
    pub languages: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub recursive: bool,
    pub force_reindex: bool,
    pub watch: bool,
    pub workers: usize,
    pub batch_size: usize,
    pub vector_dimension: usize,
    pub device: Option<String>,
    pub max_seq_len: usize,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            languages: vec![],
            exclude_patterns: vec![],
            include_patterns: vec![],
            recursive: true,
            force_reindex: false,
            watch: false,
            workers: 4,
            batch_size: 100,
            vector_dimension: 1536,
            device: None,
            max_seq_len: 512,
        }
    }
}

pub struct ProjectIndexer {
    config: IndexerConfig,
    progress: MultiProgress,
    parser: TreeSitterParser,
    graph: CodeGraph,
    vector_dim: usize,
    #[cfg(feature = "embeddings")]
    embedder: codegraph_vector::EmbeddingGenerator,
}

impl ProjectIndexer {
    pub async fn new(config: IndexerConfig) -> Result<Self> {
        let progress = MultiProgress::new();
        let parser = TreeSitterParser::new();
        let graph = CodeGraph::new()?;
        #[cfg(feature = "embeddings")]
        let embedder = {
            use codegraph_vector::EmbeddingGenerator;
            // If local requested via env, override local config from CLI flags
            let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER")
                .unwrap_or_default()
                .to_lowercase();
            if provider == "local" {
                #[cfg(feature = "embeddings-local")]
                {
                    use codegraph_vector::embeddings::generator::{
                        AdvancedEmbeddingGenerator, EmbeddingEngineConfig, LocalDeviceTypeCompat,
                        LocalEmbeddingConfigCompat, LocalPoolingCompat,
                    };
                    let mut cfg = EmbeddingEngineConfig::default();
                    cfg.prefer_local_first = true;
                    let device = match config
                        .device
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .as_str()
                    {
                        "metal" => LocalDeviceTypeCompat::Metal,
                        d if d.starts_with("cuda:") => {
                            let id = d.trim_start_matches("cuda:").parse::<usize>().unwrap_or(0);
                            LocalDeviceTypeCompat::Cuda(id)
                        }
                        _ => LocalDeviceTypeCompat::Cpu,
                    };
                    let model_name = std::env::var("CODEGRAPH_LOCAL_MODEL")
                        .unwrap_or_else(|_| "sentence-transformers/all-MiniLM-L6-v2".to_string());
                    cfg.local = Some(LocalEmbeddingConfigCompat {
                        model_name,
                        device,
                        cache_dir: None,
                        max_sequence_length: config.max_seq_len.max(32),
                        pooling_strategy: LocalPoolingCompat::Mean,
                    });
                    // Try to construct advanced engine; fall back to simple generator on error
                    match AdvancedEmbeddingGenerator::new(cfg).await {
                        Ok(engine) => {
                            if !engine.has_provider() {
                                return Err(anyhow::anyhow!(
                                    "Local embedding provider constructed without a backend. Ensure the model is BERT-compatible with safetensors and try --device metal or --device cpu"
                                ));
                            }
                            let mut g = EmbeddingGenerator::default();
                            g.set_advanced_engine(std::sync::Arc::new(engine));
                            tracing::info!(
                                target: "codegraph_mcp::indexer",
                                "Active embeddings: Local (device: {}, max_seq_len: {}, batch_size: {})",
                                config.device.as_deref().unwrap_or("cpu"),
                                config.max_seq_len,
                                config.batch_size
                            );
                            g
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!(
                                "Failed to initialize local embedding provider: {}",
                                e
                            ));
                        }
                    }
                }
                #[cfg(not(feature = "embeddings-local"))]
                {
                    tracing::warn!(
                        target: "codegraph_mcp::indexer",
                        "CODEGRAPH_EMBEDDING_PROVIDER=local requested but the 'embeddings-local' feature is not enabled; using auto provider"
                    );
                    EmbeddingGenerator::with_auto_from_env().await
                }
            } else {
                let g = EmbeddingGenerator::with_auto_from_env().await;
                tracing::info!(
                    target: "codegraph_mcp::indexer",
                    "Active embeddings: {:?} (batch_size: {})",
                    std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").ok(),
                    config.batch_size
                );
                g
            }
        };
        let vector_dim = {
            #[cfg(feature = "embeddings")]
            { embedder.dimension() }
            #[cfg(not(feature = "embeddings"))]
            { config.vector_dimension }
        };

        Ok(Self {
            config,
            progress,
            parser,
            graph,
            vector_dim,
            #[cfg(feature = "embeddings")]
            embedder,
        })
    }

    pub async fn index_project(&mut self, path: impl AsRef<Path>) -> Result<IndexStats> {
        let path = path.as_ref();
        info!("Starting project indexing: {:?}", path);

        // Check if already indexed
        if !self.config.force_reindex && self.is_indexed(path).await? {
            warn!("Project already indexed. Use --force to reindex.");
            return Ok(IndexStats::default());
        }

        // Parse project into CodeNodes
        let parse_pb = self.create_progress_bar(0, "Parsing project");
        let (mut nodes, _pstats) = self
            .parser
            .parse_directory_parallel(&path.to_string_lossy())
            .await?;
        parse_pb.finish_with_message("Parsing complete");

        // Generate embeddings and attach (batched)
        let total = nodes.len() as u64;
        let embed_pb = self.create_progress_bar(total, "Generating embeddings");
        let batch = self.config.batch_size.max(1);
        let mut processed = 0u64;
        for chunk in nodes.chunks_mut(batch) {
            #[cfg(feature = "embeddings")]
            {
                let embs = self.embedder.generate_embeddings(&chunk).await?;
                for (n, e) in chunk.iter_mut().zip(embs.into_iter()) {
                    n.embedding = Some(e);
                }
            }
            #[cfg(not(feature = "embeddings"))]
            {
                for n in chunk.iter_mut() {
                    let text = prepare_node_text(n);
                    let emb = simple_text_embedding(&text, self.vector_dim);
                    n.embedding = Some(normalize(&emb));
                }
            }
            processed += chunk.len() as u64;
            embed_pb.set_position(processed.min(total));
        }
        embed_pb.finish_with_message("Embeddings complete");

        #[cfg(feature = "faiss")]
        {
            use faiss::index::flat::FlatIndex;
            use faiss::index::io::write_index;
            use faiss::index::Index;
            use std::collections::HashMap;

            // Helper to write single FAISS index + id map
            let mut write_shard = |vectors: &[f32], ids: &[codegraph_core::NodeId], path: &Path| -> Result<()> {
                if vectors.is_empty() { return Ok(()); }
                let mut idx = FlatIndex::new_ip(self.vector_dim as u32).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                idx.add(vectors).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
                write_index(&idx, path.to_string_lossy()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok(())
            };

            // Global index
            let mut global_vecs: Vec<f32> = Vec::new();
            let mut global_ids: Vec<codegraph_core::NodeId> = Vec::new();

            // Path shard (first segment)
            let mut path_shards: HashMap<String, (Vec<f32>, Vec<codegraph_core::NodeId>)> = HashMap::new();
            // Language shard
            let mut lang_shards: HashMap<String, (Vec<f32>, Vec<codegraph_core::NodeId>)> = HashMap::new();

            for n in &nodes {
                if let Some(e) = &n.embedding {
                    global_vecs.extend_from_slice(e);
                    global_ids.push(n.id);

                    // path shard
                    let seg = n
                        .location
                        .file_path
                        .trim_start_matches("./")
                        .split('/')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !seg.is_empty() {
                        let entry = path_shards.entry(seg).or_insert_with(|| (Vec::new(), Vec::new()));
                        entry.0.extend_from_slice(e);
                        entry.1.push(n.id);
                    }

                    // language shard
                    if let Some(lang) = &n.language {
                        let lname = format!("{:?}", lang).to_lowercase();
                        let entry = lang_shards.entry(lname).or_insert_with(|| (Vec::new(), Vec::new()));
                        entry.0.extend_from_slice(e);
                        entry.1.push(n.id);
                    }
                }
            }

            let out_dir = Path::new(".codegraph");
            tokio::fs::create_dir_all(out_dir).await?;
            // Global
            write_shard(&global_vecs, &global_ids, &out_dir.join("faiss.index"))?;
            tokio::fs::write(out_dir.join("faiss_ids.json"), serde_json::to_vec(&global_ids)?).await?;

            // Path shards
            let path_dir = out_dir.join("shards/path");
            for (seg, (vecs, ids)) in path_shards {
                let idx_path = path_dir.join(format!("{}.index", seg));
                write_shard(&vecs, &ids, &idx_path)?;
                tokio::fs::write(path_dir.join(format!("{}_ids.json", seg)), serde_json::to_vec(&ids)?).await?;
            }

            // Language shards
            let lang_dir = out_dir.join("shards/lang");
            for (lang, (vecs, ids)) in lang_shards {
                let idx_path = lang_dir.join(format!("{}.index", lang));
                write_shard(&vecs, &ids, &idx_path)?;
                tokio::fs::write(lang_dir.join(format!("{}_ids.json", lang)), serde_json::to_vec(&ids)?).await?;
            }
        }

        // Save embeddings for FAISS-backed search if enabled
        #[cfg(feature = "faiss")]
        {
            let out_path = Path::new(".codegraph").join("embeddings.json");
            save_embeddings_to_file(out_path, &nodes).await?;
        }

        // Store nodes into graph and compute stats
        let main_pb = self.create_progress_bar(nodes.len() as u64, "Storing nodes");
        let mut stats = IndexStats::default();
        let mut seen_files = std::collections::HashSet::new();
        for n in nodes.into_iter() {
            match n.node_type {
                Some(NodeType::Function) => stats.functions += 1,
                Some(NodeType::Class) => stats.classes += 1,
                _ => {}
            }
            if let Some(ref c) = n.content { stats.lines += c.lines().count(); }
            if let Some(ref path) = n.location.file_path.as_str().into() {
                if seen_files.insert(n.location.file_path.clone()) {
                    stats.files += 1;
                }
            }
            if n.embedding.is_some() {
                stats.embeddings += 1;
            }
            self.graph.add_node(n).await?;
            main_pb.inc(1);
        }
        main_pb.finish_with_message("Indexing complete");

        // Save index metadata
        self.save_index_metadata(path, &stats).await?;

        info!("Indexing complete: {:?}", stats);
        Ok(stats)
    }

    async fn index_file(
        path: PathBuf,
        parse_pb: ProgressBar,
        embed_pb: ProgressBar,
    ) -> Result<FileStats> {
        debug!("Indexing file: {:?}", path);
        let mut stats = FileStats::default();

        // Read file content
        let content = fs::read_to_string(&path).await?;
        stats.lines = content.lines().count();

        // Very rough heuristics for functions/classes counts per common languages
        let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
        let (fn_regex, class_regex) = match ext {
            "rs" => (Some(Regex::new(r"\bfn\s+\w+").unwrap()), None),
            "py" => (
                Some(Regex::new(r"\bdef\s+\w+\s*\(").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+\s*:").unwrap()),
            ),
            "ts" | "js" => (
                Some(Regex::new(r"\bfunction\s+\w+|\b\w+\s*=\s*\(.*\)\s*=>").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+").unwrap()),
            ),
            "go" => (Some(Regex::new(r"\bfunc\s+\w+\s*\(").unwrap()), None),
            "java" => (
                Some(Regex::new(r"\b\w+\s+\w+\s*\(.*\)\s*\{").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+").unwrap()),
            ),
            "cpp" | "cc" | "cxx" | "hpp" | "h" | "c" => (
                Some(Regex::new(r"\b\w+\s+\w+\s*\(.*\)\s*\{").unwrap()),
                None,
            ),
            _ => (None, None),
        };

        parse_pb.set_message(format!("Parsing {}", path.display()));
        parse_pb.inc(1);

        if let Some(re) = fn_regex {
            stats.functions = re.find_iter(&content).count();
        }
        if let Some(re) = class_regex {
            stats.classes = re.find_iter(&content).count();
        }

        // Pretend to generate embeddings by counting tokens roughly
        embed_pb.set_message(format!("Embedding {}", path.display()));
        stats.embeddings = content.split_whitespace().count() / 100; // 1 per ~100 tokens
        embed_pb.inc(1);

        Ok(stats)
    }

    async fn collect_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let walker = if self.config.recursive {
            WalkDir::new(path)
        } else {
            WalkDir::new(path).max_depth(1)
        };

        for entry in walker {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if self.should_index(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    fn should_index(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
        let path_str = path.to_string_lossy();

        // Exclude patterns (simple substring match)
        for pat in &self.config.exclude_patterns {
            if path_str.contains(pat) || file_name.contains(pat) {
                return false;
            }
        }

        // Include patterns (if provided, must match at least one)
        if !self.config.include_patterns.is_empty()
            && !self
                .config
                .include_patterns
                .iter()
                .any(|p| path_str.contains(p) || file_name.contains(p))
        {
            return false;
        }

        // Language filtering by extension
        if !self.config.languages.is_empty() {
            let ext = path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("")
                .to_lowercase();
            let lang_matches = |langs: &Vec<String>, e: &str| -> bool {
                let lang = e;
                langs.iter().any(|l| match l.as_str() {
                    "rust" | "rs" => matches!(lang, "rs"),
                    "python" | "py" => matches!(lang, "py"),
                    "js" | "javascript" | "jsx" => matches!(lang, "js" | "jsx"),
                    "ts" | "typescript" | "tsx" => matches!(lang, "ts" | "tsx"),
                    "go" => matches!(lang, "go"),
                    "java" => matches!(lang, "java"),
                    "cpp" | "c++" | "cc" | "cxx" | "hpp" | "h" | "c" => {
                        matches!(lang, "cpp" | "cc" | "cxx" | "hpp" | "h" | "c")
                    }
                    _ => false,
                })
            };
            if !lang_matches(&self.config.languages, &ext) {
                return false;
            }
        }

        // Default excludes
        for dir in [
            ".git",
            "node_modules",
            "target",
            ".codegraph",
            "dist",
            "build",
        ] {
            if path_str.contains(dir) {
                return false;
            }
        }

        true
    }

    async fn is_indexed(&self, path: &Path) -> Result<bool> {
        let metadata_path = Path::new(".codegraph").join("index.json");
        if !metadata_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(metadata_path).await?;
        let metadata: IndexMetadata = serde_json::from_str(&content)?;
        Ok(metadata.project_path == path)
    }

    async fn save_index_metadata(&self, path: &Path, stats: &IndexStats) -> Result<()> {
        let metadata = IndexMetadata {
            project_path: path.to_path_buf(),
            indexed_at: chrono::Utc::now(),
            stats: stats.clone(),
            config: IndexConfigMetadata {
                languages: self.config.languages.clone(),
                recursive: self.config.recursive,
                workers: self.config.workers,
            },
        };

        let metadata_path = Path::new(".codegraph").join("index.json");
        fs::create_dir_all(".codegraph").await?;
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(metadata_path, json).await?;
        Ok(())
    }

    fn create_progress_bar(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.progress.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message(message.to_string());
        pb
    }

    pub async fn watch_for_changes(&self, path: impl AsRef<Path>) -> Result<()> {
        use notify::event::{EventKind, ModifyKind};
        use notify::{Event, RecursiveMode, Watcher};

        let path = path.as_ref().to_path_buf();
        let (tx, mut rx) = mpsc::channel(100);

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, _>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            })?;

        watcher.watch(&path, RecursiveMode::Recursive)?;
        info!("Watching for changes in: {:?}", path);

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(_) => {
                    for path in event.paths {
                        if self.should_index(&path) {
                            info!("File changed: {:?}, reindexing...", path);
                            // No-op stub for now
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub fn prepare_node_text(node: &CodeNode) -> String {
    let lang = node
        .language
        .as_ref()
        .map(|l| format!("{:?}", l))
        .unwrap_or_else(|| "unknown".to_string());
    let kind = node
        .node_type
        .as_ref()
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "unknown".to_string());
    let mut text = format!("{} {} {}", lang, kind, node.name);
    if let Some(c) = &node.content {
        text.push(' ');
        text.push_str(c);
    }
    if text.len() > 2048 {
        text.truncate(2048);
    }
    text
}

pub fn simple_text_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let mut embedding = vec![0.0f32; dimension];
    let mut hash = 5381u32;
    for b in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u32);
    }
    let mut state = hash;
    for i in 0..dimension {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        embedding[i] = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
    }
    embedding
}

pub fn normalize(v: &[f32]) -> Vec<f32> {
    let mut out = v.to_vec();
    let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut out {
            *x /= norm;
        }
    }
    out
}

#[cfg(feature = "faiss")]
async fn save_embeddings_to_file(out: PathBuf, nodes: &[CodeNode]) -> Result<()> {
    use serde_json as json;
    let mut items: Vec<(codegraph_core::NodeId, Vec<f32>)> = Vec::new();
    for n in nodes {
        if let Some(e) = &n.embedding {
            items.push((n.id, e.clone()));
        }
    }
    if let Some(dir) = out.parent() { tokio::fs::create_dir_all(dir).await?; }
    let data = json::to_string(&items)?;
    tokio::fs::write(out, data).await?;
    Ok(())
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexStats {
    pub files: usize,
    pub lines: usize,
    pub functions: usize,
    pub classes: usize,
    pub embeddings: usize,
    pub errors: usize,
}

impl IndexStats {
    fn merge(&mut self, other: FileStats) {
        self.files += 1;
        self.lines += other.lines;
        self.functions += other.functions;
        self.classes += other.classes;
        self.embeddings += other.embeddings;
    }
}

#[derive(Debug, Default, Clone)]
struct FileStats {
    lines: usize,
    functions: usize,
    classes: usize,
    embeddings: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IndexMetadata {
    project_path: PathBuf,
    indexed_at: chrono::DateTime<chrono::Utc>,
    stats: IndexStats,
    config: IndexConfigMetadata,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IndexConfigMetadata {
    languages: Vec<String>,
    recursive: bool,
    workers: usize,
}
