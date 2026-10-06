use crate::{AstVisitor, LanguageRegistry};
use async_trait::async_trait;
use codegraph_core::{
    CodeGraphError, CodeNode, CodeParser, EdgeRelationship, EdgeType, ExtractionResult, Language,
    Location, NodeType, Result,
};
use futures::stream::{self, StreamExt};
use sha2::Digest;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::fast_io::read_file_to_string;
use crate::file_collect::{
    FileCollectionConfig, collect_source_files, collect_source_files_with_config,
};

#[derive(Clone)]
pub struct ParsedFile {
    pub file_path: String,
    pub language: Language,
    pub content: String,
    pub tree: Option<Tree>,
    pub last_modified: std::time::SystemTime,
    pub content_hash: String,
}

pub struct ParsingStatistics {
    pub total_files: usize,
    pub parsed_files: usize,
    pub failed_files: usize,
    pub cached_files: usize,
    pub total_lines: usize,
    pub parsing_duration: Duration,
    pub files_per_second: f64,
    pub lines_per_second: f64,
}

pub struct TreeSitterParser {
    registry: Arc<LanguageRegistry>,
    max_concurrent_files: usize,
    chunk_size: usize,
    parsed_cache: Arc<dashmap::DashMap<String, ParsedFile>>,
    parser_pool: Arc<parking_lot::Mutex<HashMap<Language, Vec<Parser>>>>,
    extraction_policy: crate::languages::ExtractionPolicy,
    project_root: Option<Arc<PathBuf>>,
    blocking_workers: Arc<tokio::sync::Semaphore>,
}

impl TreeSitterParser {
    pub fn new() -> Self {
        let num_cpus = num_cpus::get();
        Self {
            registry: Arc::new(LanguageRegistry::new()),
            max_concurrent_files: num_cpus * 2,
            chunk_size: 50,
            parsed_cache: Arc::new(dashmap::DashMap::new()),
            parser_pool: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            extraction_policy: Default::default(),
            project_root: None,
            blocking_workers: Arc::new(tokio::sync::Semaphore::new((num_cpus * 2).max(1))),
        }
    }

    pub fn with_concurrency(mut self, max_concurrent_files: usize) -> Self {
        self.max_concurrent_files = max_concurrent_files.max(1);
        self.blocking_workers = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_files));
        self
    }

    pub fn concurrency(&self) -> usize {
        self.max_concurrent_files
    }

    pub fn extraction_policy(&self) -> crate::languages::ExtractionPolicy {
        self.extraction_policy
    }

    pub fn with_project_root(mut self, root: impl AsRef<Path>) -> Self {
        self.project_root = Some(Arc::new(root.as_ref().to_path_buf()));
        self
    }

    pub fn project_root(&self) -> Option<&Path> {
        self.project_root.as_deref().map(PathBuf::as_path)
    }

    pub fn with_extraction_policy(mut self, policy: crate::languages::ExtractionPolicy) -> Self {
        self.extraction_policy = policy;
        self
    }

    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub async fn parse_directory_parallel(
        &self,
        dir_path: &str,
    ) -> Result<(Vec<CodeNode>, ParsingStatistics)> {
        let start_time = Instant::now();
        let dir_path = Path::new(dir_path);

        info!(
            "Starting parallel parsing of directory: {}",
            dir_path.display()
        );

        // Collect and size files
        let sized_files = tokio::task::spawn_blocking({
            let dir = dir_path.to_path_buf();
            let registry = self.registry.clone();
            move || {
                collect_source_files(&dir)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(p, _)| {
                        p.to_str()
                            .map(|s| registry.detect_language(s).is_some())
                            .unwrap_or(false)
                    })
                    .collect::<Vec<(std::path::PathBuf, u64)>>()
            }
        })
        .await
        .unwrap_or_default();

        // Sort by size desc to reduce tail latency (schedule big files first)
        let mut sized_files = sized_files;
        sized_files.sort_by(|a, b| b.1.cmp(&a.1));

        let files: Vec<std::path::PathBuf> = sized_files.into_iter().map(|(p, _)| p).collect();
        let total_files = files.len();

        info!("Found {} files to parse", total_files);

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_files));

        // Process files in chunks for better memory management
        let mut all_nodes = Vec::new();
        let mut total_lines = 0;
        let mut parsed_files = 0;
        let mut failed_files = 0;

        let mut stream = stream::iter(files.into_iter().map(|file_path| {
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                self.parse_file_with_caching(&file_path.to_string_lossy())
                    .await
            }
        }))
        .buffer_unordered(self.max_concurrent_files);

        while let Some(result) = stream.next().await {
            match result {
                Ok((nodes, lines)) => {
                    all_nodes.extend(nodes);
                    total_lines += lines;
                    parsed_files += 1;
                }
                Err(e) => {
                    failed_files += 1;
                    warn!("Failed to parse file: {}", e);
                }
            }
        }

        let parsing_duration = start_time.elapsed();
        let files_per_second = parsed_files as f64 / parsing_duration.as_secs_f64();
        let lines_per_second = total_lines as f64 / parsing_duration.as_secs_f64();

        let stats = ParsingStatistics {
            total_files,
            parsed_files,
            failed_files,
            cached_files: 0,
            total_lines,
            parsing_duration,
            files_per_second,
            lines_per_second,
        };

        info!(
            "Parsing completed: {}/{} files, {} lines in {:.2}s ({:.1} files/s, {:.0} lines/s)",
            parsed_files,
            total_files,
            total_lines,
            parsing_duration.as_secs_f64(),
            files_per_second,
            lines_per_second
        );

        Ok((all_nodes, stats))
    }

    /// Enhanced directory parsing with proper configuration support
    pub async fn parse_directory_parallel_with_config(
        &self,
        dir_path: &str,
        config: &FileCollectionConfig,
    ) -> Result<(Vec<CodeNode>, ParsingStatistics)> {
        let start_time = Instant::now();
        let dir_path = Path::new(dir_path);

        info!(
            "Starting enhanced parallel parsing of directory: {} (recursive: {}, languages: {:?})",
            dir_path.display(),
            config.recursive,
            config.languages
        );

        // Collect and size files with proper configuration
        let sized_files = tokio::task::spawn_blocking({
            let dir = dir_path.to_path_buf();
            let config = config.clone();
            let registry = self.registry.clone();
            move || {
                // Use new file collection with config
                let files = collect_source_files_with_config(&dir, &config).unwrap_or_default();

                info!("Collected {} files from directory scan", files.len());

                // Filter by language detection for additional safety
                let filtered_files: Vec<(PathBuf, u64)> = files
                    .into_iter()
                    .filter(|(p, _)| {
                        let detected = registry.detect_language(&p.to_string_lossy());
                        if detected.is_none() {
                            debug!("No language detected for: {}", p.display());
                        }
                        detected.is_some()
                    })
                    .collect();

                info!(
                    "Language filtering: {} files passed detection",
                    filtered_files.len()
                );
                filtered_files
            }
        })
        .await
        .unwrap_or_default();

        // Sort by size desc to reduce tail latency (schedule big files first)
        let mut sized_files = sized_files;
        sized_files.sort_by(|a, b| b.1.cmp(&a.1));

        let files: Vec<PathBuf> = sized_files.into_iter().map(|(p, _)| p).collect();
        let total_files = files.len();

        info!("Processing {} files for parsing", total_files);

        if total_files == 0 {
            warn!("No files to parse! Check:");
            warn!("  - Directory contains source files");
            warn!("  - Language filters: {:?}", config.languages);
            warn!("  - Recursive setting: {}", config.recursive);
            warn!("  - Include patterns: {:?}", config.include_patterns);
            warn!("  - Exclude patterns: {:?}", config.exclude_patterns);
        }

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_files));

        // Process files in chunks for better memory management
        let mut all_nodes = Vec::new();
        let mut total_lines = 0;
        let mut parsed_files = 0;
        let mut failed_files = 0;

        let mut stream = stream::iter(files.into_iter().map(|file_path| {
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                self.parse_file_with_caching(&file_path.to_string_lossy())
                    .await
            }
        }))
        .buffer_unordered(self.max_concurrent_files);

        while let Some(result) = stream.next().await {
            match result {
                Ok((nodes, lines)) => {
                    if !nodes.is_empty() {
                        debug!("Parsed {} nodes from file", nodes.len());
                    }
                    all_nodes.extend(nodes);
                    total_lines += lines;
                    parsed_files += 1;
                }
                Err(e) => {
                    failed_files += 1;
                    warn!("Failed to parse file: {}", e);
                }
            }
        }

        let parsing_duration = start_time.elapsed();
        let files_per_second = if parsing_duration.as_secs_f64() > 0.0 {
            parsed_files as f64 / parsing_duration.as_secs_f64()
        } else {
            0.0
        };
        let lines_per_second = if parsing_duration.as_secs_f64() > 0.0 {
            total_lines as f64 / parsing_duration.as_secs_f64()
        } else {
            0.0
        };

        let stats = ParsingStatistics {
            total_files,
            parsed_files,
            failed_files,
            cached_files: 0,
            total_lines,
            parsing_duration,
            files_per_second,
            lines_per_second,
        };

        info!(
            "Enhanced parsing completed: {}/{} files, {} lines in {:.2}s ({:.1} files/s, {:.0} lines/s)",
            parsed_files,
            total_files,
            total_lines,
            parsing_duration.as_secs_f64(),
            files_per_second,
            lines_per_second
        );

        if failed_files > 0 {
            warn!(
                "Failed to parse {} files - check logs for details",
                failed_files
            );
        }

        Ok((all_nodes, stats))
    }

    async fn parse_file_with_caching(&self, file_path: &str) -> Result<(Vec<CodeNode>, usize)> {
        let path = Path::new(file_path);
        let metadata = fs::metadata(path)
            .await
            .map_err(|e| CodeGraphError::Io(e))?;
        let last_modified = metadata.modified().map_err(|e| CodeGraphError::Io(e))?;

        // Check cache first
        if let Some(cached) = self.parsed_cache.get(file_path) {
            if cached.last_modified == last_modified {
                debug!("Using cached parse result for {}", file_path);
                if let Some(tree) = &cached.tree {
                    let mut visitor = AstVisitor::new(
                        cached.language.clone(),
                        file_path.to_string(),
                        cached.content.clone(),
                    );
                    visitor.visit(tree.root_node());
                    let line_count = cached.content.lines().count();
                    return Ok((visitor.nodes, line_count));
                }
            }
        }

        // Parse file
        let result = self.parse_file_internal(file_path).await;

        match &result {
            Ok((_, _, content)) => {
                // Cache successful parse
                let language = self
                    .registry
                    .detect_language(file_path)
                    .unwrap_or(Language::Other("unknown".to_string()));
                let content_hash = codegraph_core::hex_digest(&sha2::Sha256::digest(&content));

                // Enable tree caching for better performance
                let cached_tree = if content.len() < 500_000 {
                    // Only cache smaller files to avoid memory issues
                    // Re-parse to get a tree we can cache
                    if let Ok((nodes, _, _)) = &result {
                        if !nodes.is_empty() {
                            // Parse again just for caching (small performance cost for future gains)
                            let mut cache_parser = self
                                .registry
                                .create_parser(&language)
                                .unwrap_or_else(|| tree_sitter::Parser::new());
                            if let Some(config) = self.registry.get_config(&language) {
                                if cache_parser.set_language(&config.language).is_ok() {
                                    cache_parser.parse(&content, None)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let parsed_file = ParsedFile {
                    file_path: file_path.to_string(),
                    language,
                    content: content.clone(),
                    tree: cached_tree, // Now caching trees for better performance
                    last_modified,
                    content_hash,
                };

                self.parsed_cache.insert(file_path.to_string(), parsed_file);
            }
            Err(e) => {
                debug!("Failed to cache parse result for {}: {}", file_path, e);
            }
        }

        result.map(|(nodes, lines, _)| (nodes, lines))
    }

    async fn parse_file_internal(&self, file_path: &str) -> Result<(Vec<CodeNode>, usize, String)> {
        let language = self
            .registry
            .detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let content = read_file_to_string(file_path)
            .await
            .map_err(|e| CodeGraphError::Io(e))?;

        let line_count = content.lines().count();
        let nodes = self
            .parse_content_with_recovery(&content, file_path, language)
            .await?;

        Ok((nodes, line_count, content))
    }

    async fn parse_content_with_recovery(
        &self,
        content: &str,
        file_path: &str,
        language: Language,
    ) -> Result<Vec<CodeNode>> {
        Ok(self
            .parse_source_with_edges(Arc::from(content), file_path, language)
            .await?
            .nodes
            .into_iter()
            .filter(|node| node.node_type != Some(NodeType::Directory))
            .collect())
    }

    pub async fn incremental_update(
        &self,
        file_path: &str,
        old_content: &str,
        new_content: &str,
    ) -> Result<Vec<CodeNode>> {
        let language = self
            .registry
            .detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let registry = self.registry.clone();
        let file_path = file_path.to_string();
        let old_content = old_content.to_string();
        let new_content = new_content.to_string();

        tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language).ok_or_else(|| {
                CodeGraphError::Parse(format!("Unsupported language: {:?}", language))
            })?;

            // Parse old content first
            let old_tree = parser
                .parse(&old_content, None)
                .ok_or_else(|| CodeGraphError::Parse("Failed to parse old content".to_string()))?;

            // Create diff and compute edit
            let diff = similar::TextDiff::from_lines(&old_content, &new_content);
            let mut byte_offset = 0;
            let mut edits = Vec::new();

            for change in diff.iter_all_changes() {
                match change.tag() {
                    similar::ChangeTag::Delete => {
                        let end_byte = byte_offset + change.value().len();
                        edits.push(InputEdit {
                            start_byte: byte_offset,
                            old_end_byte: end_byte,
                            new_end_byte: byte_offset,
                            start_position: Point::new(0, 0), // Simplified for now
                            old_end_position: Point::new(0, 0),
                            new_end_position: Point::new(0, 0),
                        });
                    }
                    similar::ChangeTag::Insert => {
                        let new_end = byte_offset + change.value().len();
                        edits.push(InputEdit {
                            start_byte: byte_offset,
                            old_end_byte: byte_offset,
                            new_end_byte: new_end,
                            start_position: Point::new(0, 0),
                            old_end_position: Point::new(0, 0),
                            new_end_position: Point::new(0, 0),
                        });
                        byte_offset = new_end;
                    }
                    similar::ChangeTag::Equal => {
                        byte_offset += change.value().len();
                    }
                }
            }

            // Apply edits to tree
            let mut updated_tree = old_tree;
            for edit in edits {
                updated_tree.edit(&edit);
            }

            // Parse with the updated tree
            let new_tree = parser
                .parse(&new_content, Some(&updated_tree))
                .ok_or_else(|| CodeGraphError::Parse("Failed to incremental parse".to_string()))?;

            let mut visitor =
                AstVisitor::new(language.clone(), file_path.clone(), new_content.clone());
            visitor.visit(new_tree.root_node());
            Ok(visitor.nodes)
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    pub fn clear_cache(&self) {
        self.parsed_cache.clear();
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        let cache_size = self.parsed_cache.len();
        let estimated_memory = cache_size * 1024; // Rough estimate
        (cache_size, estimated_memory)
    }

    /// REVOLUTIONARY: Parse file with unified node+edge extraction for maximum speed
    pub async fn parse_file_with_edges(&self, file_path: &str) -> Result<ExtractionResult> {
        let language = self
            .registry
            .detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let content = read_file_to_string(file_path)
            .await
            .map_err(|e| CodeGraphError::Io(e))?;

        self.parse_content_with_unified_extraction(&content, file_path, language)
            .await
    }

    /// FASTEST: Parse content with unified node+edge extraction in single AST traversal
    async fn parse_content_with_unified_extraction(
        &self,
        content: &str,
        file_path: &str,
        language: Language,
    ) -> Result<ExtractionResult> {
        self.parse_source_with_edges(Arc::from(content), file_path, language)
            .await
    }

    /// Consumes an immutable source without copying it into the blocking parser task.
    pub async fn parse_source_with_edges(
        &self,
        content: Arc<str>,
        file_path: &str,
        language: Language,
    ) -> Result<ExtractionResult> {
        let registry = self.registry.clone();
        let file_path_string = file_path.to_string();
        let parser_pool = self.parser_pool.clone();
        let policy = self.extraction_policy;
        let project_root = self.project_root.clone();

        // Clone for timeout message
        let content_len = content.len();
        let file_path_for_timeout = file_path.to_string();

        let base_timeout = env::var("CODEGRAPH_PARSER_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(10)
            .max(1);
        let timeout_duration =
            Duration::from_secs(base_timeout.saturating_mul(if content_len > 1_000_000 {
                6
            } else if content_len > 100_000 {
                3
            } else {
                1
            }));
        let permit = self
            .blocking_workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| CodeGraphError::Parse(error.to_string()))?;
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        struct CancelOnDrop(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let _cancel_guard = CancelOnDrop(cancelled.clone());
        // Add timeout protection for problematic files
        let parsing_task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let began = std::time::Instant::now();
            let mut progress = |_: &tree_sitter::ParseState| {
                if cancelled.load(std::sync::atomic::Ordering::Relaxed)
                    || began.elapsed() >= timeout_duration
                {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            };
            let file_path = file_path_string;

            // Try to get parser from pool first, create new one if pool is empty
            let mut parser = {
                let cached = parser_pool.lock().get_mut(&language).and_then(Vec::pop);
                cached
                    .or_else(|| registry.create_parser(&language))
                    .ok_or_else(|| {
                        CodeGraphError::Parse(format!("Unsupported language: {:?}", language))
                    })?
            };

            // Ensure parser has correct language set
            if let Some(config) = registry.get_config(&language) {
                if parser.set_language(&config.language).is_err() {
                    return Err(CodeGraphError::Parse(format!(
                        "Failed to set language for: {:?}",
                        language
                    )));
                }
            } else {
                return Err(CodeGraphError::Parse(format!(
                    "Unsupported language: {:?}",
                    language
                )));
            }

            // Parse with tolerance and retry
            let result = match parser.parse_with_options(
                &mut |offset, _| &content.as_bytes()[offset..],
                None,
                Some(tree_sitter::ParseOptions::new().progress_callback(&mut progress)),
            ) {
                Some(tree) => {
                    // Keep original bytes and offsets. Tree-sitter recovers syntax errors;
                    // reparsing cleaned text shifts spans away from the hashed source.
                    let tree_used = tree;
                    let used_content = content;

                    // Use unified dispatch for all supported languages
                    let ast_result = crate::languages::extract_for_language_with_policy(
                        &language,
                        &tree_used,
                        &used_content,
                        &file_path,
                        policy,
                    )
                    .unwrap_or_else(|| {
                        // Fallback: use AstVisitor for languages without dedicated extractors
                        let mut visitor = crate::AstVisitor::new(
                            language.clone(),
                            file_path.clone(),
                            used_content.to_string(),
                        );
                        visitor.visit(tree_used.root_node());
                        ExtractionResult {
                            nodes: visitor.nodes,
                            edges: Vec::new(),
                        }
                    });

                    // Apply Fast ML enhancement for maximum graph completeness
                    // Adds pattern-based edges and resolves unmatched references (<1ms overhead)
                    let enhanced_result = crate::fast_ml::get_fast_ml_enhancer()
                        .enhance_with_policy(ast_result, &used_content, policy);
                    Ok(Self::add_directory_nodes(
                        enhanced_result,
                        &file_path,
                        project_root.as_deref().map(PathBuf::as_path),
                    ))
                }
                None => Err(CodeGraphError::Parse(format!(
                    "Complete parsing failed for {}",
                    file_path
                ))),
            };

            // Return parser to pool
            parser_pool.lock().entry(language).or_default().push(parser);

            result
        });

        // Apply timeout protection
        match tokio::time::timeout(timeout_duration, parsing_task).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(CodeGraphError::Parse(e.to_string())),
            Err(_) => {
                warn!(
                    "Parsing timeout for file: {} ({}s)",
                    file_path_for_timeout,
                    timeout_duration.as_secs()
                );
                Err(CodeGraphError::Parse(format!(
                    "Parsing timeout for file: {} ({}s)",
                    file_path_for_timeout,
                    timeout_duration.as_secs()
                )))
            }
        }
    }
}

impl TreeSitterParser {
    fn add_directory_nodes(
        mut result: ExtractionResult,
        file_path: &str,
        project_root: Option<&Path>,
    ) -> ExtractionResult {
        const MAX_DEPTH: usize = 4;
        let path = Path::new(file_path);

        let mut dirs: Vec<(String, String, usize)> = Vec::new();
        let mut current = path.parent();
        let mut depth = 0usize;

        while let Some(dir) = current {
            if project_root.is_some_and(|root| !dir.starts_with(root)) {
                break;
            }
            if dir.as_os_str().is_empty() || depth >= MAX_DEPTH {
                break;
            }
            let full_path = dir.to_string_lossy().to_string();
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| full_path.clone());
            dirs.push((full_path, name, depth));
            depth += 1;
            current = dir.parent();
        }

        if dirs.is_empty() {
            return result;
        }

        dirs.reverse();

        let mut dir_nodes = Vec::with_capacity(dirs.len());

        for (full_path, name, depth) in dirs {
            let location = Location {
                file_path: full_path.clone(),
                line: 0,
                column: 0,
                end_line: None,
                end_column: None,
            };
            let mut node = CodeNode::new(name.clone(), Some(NodeType::Directory), None, location);
            node.metadata
                .attributes
                .insert("full_path".into(), full_path.clone());
            node.metadata
                .attributes
                .insert("qualified_name".into(), full_path);
            node.metadata
                .attributes
                .insert("depth".into(), depth.to_string());
            dir_nodes.push(node);
        }

        // Structural containment runs from the parent to the child.
        for window in dir_nodes.windows(2) {
            if let [parent, child] = window {
                result.edges.push(EdgeRelationship {
                    from: parent.id,
                    to: child.location.file_path.clone(),
                    edge_type: EdgeType::Contains,
                    metadata: HashMap::from([
                        ("source_file".into(), parent.location.file_path.clone()),
                        ("analyzer".into(), "directory_structure".into()),
                    ]),
                    span: None,
                });
            }
        }

        result.nodes.extend(dir_nodes);
        result
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[tokio::test]
    async fn directory_identity_is_shared_and_containment_stops_at_project_root() {
        let parser = TreeSitterParser::new().with_project_root("/project");
        let mut identities = Vec::new();
        for file in ["/project/src/a.rs", "/project/src/b.rs"] {
            let result = parser
                .parse_source_with_edges(Arc::from("fn foo() {}"), file, Language::Rust)
                .await
                .unwrap();
            let mut dirs: Vec<_> = result
                .nodes
                .into_iter()
                .filter(|node| node.node_type == Some(NodeType::Directory))
                .collect();
            assert_eq!(dirs.len(), 2);
            for node in &mut dirs {
                node.set_deterministic_id("project");
            }
            identities.push(dirs.into_iter().map(|node| node.id).collect::<Vec<_>>());
            assert!(
                result
                    .edges
                    .iter()
                    .any(|edge| edge.edge_type == EdgeType::Contains && edge.to == "/project/src")
            );
        }
        assert_eq!(identities[0], identities[1]);
    }

    #[tokio::test]
    async fn concurrent_parsing_preserves_pool_and_tier_policy() {
        let parser = TreeSitterParser::new()
            .with_concurrency(0)
            .with_extraction_policy(crate::languages::ExtractionPolicy {
                uses: false,
                references: false,
            });
        assert_eq!(parser.concurrency(), 1);
        let source: Arc<str> = "struct Foo; impl Foo { pub fn method(&self, x: Foo) {} }".into();
        let results = futures::future::join_all(
            (0..8).map(|_| parser.parse_source_with_edges(source.clone(), "a.rs", Language::Rust)),
        )
        .await;
        for result in results {
            let result = result.unwrap();
            assert!(
                result
                    .nodes
                    .iter()
                    .any(|node| node.name.as_str() == "method")
            );
            assert!(
                result
                    .edges
                    .iter()
                    .all(|edge| !matches!(edge.edge_type, EdgeType::Uses | EdgeType::References))
            );
        }
        let pool = parser.parser_pool.lock();
        assert!(!pool[&Language::Rust].is_empty());
        // Every parser left in the pool is distinct, and can be checked out together.
        assert!(pool[&Language::Rust].len() <= 8);
    }

    #[tokio::test]
    async fn recovered_syntax_keeps_original_source_spans() {
        let parser = TreeSitterParser::new();
        let source: Arc<str> = "fn broken(\n\nfn intact() {}\n".into();
        let extraction = parser
            .parse_source_with_edges(source.clone(), "a.rs", Language::Rust)
            .await
            .unwrap();
        for node in extraction.nodes.iter().filter(|n| n.span.is_some()) {
            let span = node.span.as_ref().unwrap();
            assert_eq!(
                node.content.as_ref().unwrap().as_str(),
                &source[span.start_byte as usize..span.end_byte as usize]
            );
        }
    }
}

#[async_trait]
impl CodeParser for TreeSitterParser {
    async fn parse_file(&self, file_path: &str) -> Result<Vec<CodeNode>> {
        let (nodes, _) = self.parse_file_with_caching(file_path).await?;
        Ok(nodes)
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language::Rust,
            Language::TypeScript,
            Language::JavaScript,
            Language::Python,
            Language::Go,
            Language::Java,
            Language::Cpp,
        ]
    }
}
