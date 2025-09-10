use crate::{AstVisitor, LanguageRegistry};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, CodeParser, Language, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::Semaphore;
use tree_sitter::{InputEdit, Parser, Point, Tree};
use tracing::{debug, error, info, warn};
use sha2::Digest;
use futures::stream::{self, StreamExt};

use crate::fast_io::read_file_to_string;
use crate::file_collect::collect_source_files;

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
    parser_pool: Arc<parking_lot::Mutex<Vec<HashMap<Language, Parser>>>>,
}

impl TreeSitterParser {
    pub fn new() -> Self {
        let num_cpus = num_cpus::get();
        Self {
            registry: Arc::new(LanguageRegistry::new()),
            max_concurrent_files: num_cpus * 2,
            chunk_size: 50,
            parsed_cache: Arc::new(dashmap::DashMap::new()),
            parser_pool: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    pub fn with_concurrency(mut self, max_concurrent_files: usize) -> Self {
        self.max_concurrent_files = max_concurrent_files;
        self
    }

    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    fn get_parser_from_pool(&self, language: &Language) -> Option<Parser> {
        let mut pool = self.parser_pool.lock();
        
        for parser_set in pool.iter_mut() {
            if let Some(parser) = parser_set.remove(language) {
                return Some(parser);
            }
        }
        
        // Create new parser if pool is empty
        self.registry.create_parser(language)
    }
    
    fn return_parser_to_pool(&self, language: Language, parser: Parser) {
        let mut pool = self.parser_pool.lock();
        
        if let Some(parser_set) = pool.first_mut() {
            parser_set.insert(language, parser);
        } else {
            let mut new_set = HashMap::new();
            new_set.insert(language, parser);
            pool.push(new_set);
        }
    }

    async fn collect_files_recursive(&self, dir_path: &Path) -> Result<Vec<std::path::PathBuf>> {
        // Use ignore's fast walker (in blocking context) to honor gitignore and be faster
        let dir = dir_path.to_path_buf();
        let registry = self.registry.clone();
        let files: Vec<std::path::PathBuf> = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            match collect_source_files(&dir) {
                Ok(paths) => {
                    for (p, _size) in paths {
                        if let Some(s) = p.to_str() {
                            if registry.detect_language(s).is_some() {
                                out.push(p);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
            out
        })
        .await
        .unwrap_or_default();

        Ok(files)
    }

    pub async fn parse_directory_parallel(&self, dir_path: &str) -> Result<(Vec<CodeNode>, ParsingStatistics)> {
        let start_time = Instant::now();
        let dir_path = Path::new(dir_path);
        
        info!("Starting parallel parsing of directory: {}", dir_path.display());
        
        // Collect and size files
        let sized_files = tokio::task::spawn_blocking({
            let dir = dir_path.to_path_buf();
            let registry = self.registry.clone();
            move || {
                collect_source_files(&dir)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(p, _)| p.to_str().map(|s| registry.detect_language(s).is_some()).unwrap_or(false))
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
                self.parse_file_with_caching(&file_path.to_string_lossy()).await
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
            total_lines,
            parsing_duration,
            files_per_second,
            lines_per_second,
        };
        
        info!(
            "Parsing completed: {}/{} files, {} lines in {:.2}s ({:.1} files/s, {:.0} lines/s)",
            parsed_files, total_files, total_lines,
            parsing_duration.as_secs_f64(),
            files_per_second, lines_per_second
        );
        
        Ok((all_nodes, stats))
    }

    async fn parse_file_with_caching(&self, file_path: &str) -> Result<(Vec<CodeNode>, usize)> {
        let path = Path::new(file_path);
        let metadata = fs::metadata(path).await.map_err(|e| CodeGraphError::Io(e))?;
        let last_modified = metadata.modified().map_err(|e| CodeGraphError::Io(e))?;
        
        // Check cache first
        if let Some(cached) = self.parsed_cache.get(file_path) {
            if cached.last_modified == last_modified {
                debug!("Using cached parse result for {}", file_path);
                if let Some(tree) = &cached.tree {
                    let mut visitor = AstVisitor::new(cached.language.clone(), file_path.to_string(), cached.content.clone());
                    visitor.visit(tree.root_node());
                    let line_count = cached.content.lines().count();
                    return Ok((visitor.nodes, line_count));
                }
            }
        }
        
        // Parse file
        let result = self.parse_file_internal(file_path).await;
        
        match &result {
            Ok((nodes, _, content)) => {
                // Cache successful parse
                let language = self.registry.detect_language(file_path).unwrap_or(Language::Other("unknown".to_string()));
                let content_hash = format!("{:x}", sha2::Sha256::digest(&content));
                
                let parsed_file = ParsedFile {
                    file_path: file_path.to_string(),
                    language,
                    content,
                    tree: None, // We'd need to clone the tree here for full caching
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
        let language = self.registry.detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let content = read_file_to_string(file_path).await
            .map_err(|e| CodeGraphError::Io(e))?;
        
        let line_count = content.lines().count();
        let nodes = self.parse_content_with_recovery(&content, file_path, language).await?;
        
        Ok((nodes, line_count, content))
    }

    async fn parse_content_with_recovery(&self, content: &str, file_path: &str, language: Language) -> Result<Vec<CodeNode>> {
        let registry = self.registry.clone();
        let content = content.to_string();
        let file_path = file_path.to_string();

        tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language)
                .ok_or_else(|| CodeGraphError::Parse(format!("Unsupported language: {:?}", language)))?;

            // First attempt: try to parse normally
            match parser.parse(&content, None) {
                Some(tree) => {
                    if tree.root_node().has_error() {
                        warn!("Parse tree has errors for file: {}", file_path);
                        // Continue with partial parsing - we can still extract useful information
                    }
                    
                    let mut visitor = AstVisitor::new(language.clone(), file_path.clone(), content.clone());
                    visitor.visit(tree.root_node());
                    Ok(visitor.nodes)
                }
                None => {
                    // Fallback: try to parse line by line for basic recovery
                    warn!("Complete parsing failed for {}, attempting line-by-line recovery", file_path);
                    
                    let mut recovered_nodes = Vec::new();
                    let lines: Vec<&str> = content.lines().collect();
                    
                    for (line_num, line) in lines.iter().enumerate() {
                        if let Some(tree) = parser.parse(line, None) {
                            if !tree.root_node().has_error() {
                                let mut visitor = AstVisitor::new(
                                    language.clone(),
                                    format!("{}:{}", file_path, line_num + 1),
                                    line.to_string()
                                );
                                visitor.visit(tree.root_node());
                                recovered_nodes.extend(visitor.nodes);
                            }
                        }
                    }
                    
                    if recovered_nodes.is_empty() {
                        Err(CodeGraphError::Parse(format!("Failed to parse file: {}", file_path)))
                    } else {
                        info!("Recovered {} nodes from partially parsed file: {}", recovered_nodes.len(), file_path);
                        Ok(recovered_nodes)
                    }
                }
            }
        }).await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    pub async fn incremental_update(&self, file_path: &str, old_content: &str, new_content: &str) -> Result<Vec<CodeNode>> {
        let language = self.registry.detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let registry = self.registry.clone();
        let file_path = file_path.to_string();
        let old_content = old_content.to_string();
        let new_content = new_content.to_string();

        tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language)
                .ok_or_else(|| CodeGraphError::Parse(format!("Unsupported language: {:?}", language)))?;

            // Parse old content first
            let old_tree = parser.parse(&old_content, None)
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
            let new_tree = parser.parse(&new_content, Some(&updated_tree))
                .ok_or_else(|| CodeGraphError::Parse("Failed to incremental parse".to_string()))?;

            let mut visitor = AstVisitor::new(language, file_path, new_content);
            visitor.visit(new_tree.root_node());

            Ok(visitor.nodes)
        }).await
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
