use crate::{AstVisitor, LanguageRegistry};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, CodeParser, ExtractionResult, Language, Result};
use futures::stream::{self, StreamExt};
use sha2::Digest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::env;
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::fast_io::read_file_to_string;
use crate::file_collect::{
    collect_source_files, collect_source_files_with_config, FileCollectionConfig,
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
                let content_hash = format!("{:x}", sha2::Sha256::digest(&content));

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
        let registry = self.registry.clone();
        let content_string = content.to_string();
        let file_path_string = file_path.to_string();
        let parser_pool = self.parser_pool.clone();

        // Clone for timeout message
        let content_len = content.len();
        let file_path_for_timeout = file_path.to_string();

        // Add timeout protection for problematic files
        let parsing_task = tokio::task::spawn_blocking(move || {
            let content = content_string;
            let file_path = file_path_string;
            // Try to get parser from pool first, create new one if pool is empty
            let mut parser = {
                let mut pool = parser_pool.lock();
                let mut found_parser = None;

                for parser_set in pool.iter_mut() {
                    if let Some(p) = parser_set.remove(&language) {
                        found_parser = Some(p);
                        break;
                    }
                }

                found_parser.unwrap_or_else(|| {
                    registry.create_parser(&language).unwrap_or_else(|| {
                        // Fallback: create a default parser
                        tree_sitter::Parser::new()
                    })
                })
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

            // First attempt: try to parse normally
            let result = match parser.parse(&content, None) {
                Some(tree) => {
                    let mut tree_used = tree;
                    let mut used_content = content.clone();
                    if tree_used.root_node().has_error() {
                        // Tolerant cleaner: strip common noisy directives/macros and retry once
                        let cleaned = Self::tolerant_clean(&content);
                        if cleaned != content {
                            if let Some(tree2) = parser.parse(&cleaned, None) {
                                if !tree2.root_node().has_error() {
                                    tracing::debug!(
                                        target: "codegraph_parser::parser",
                                        "Re-parsed successfully with tolerant cleaner: {}",
                                        file_path
                                    );
                                    tree_used = tree2;
                                    used_content = cleaned;
                                } else {
                                    warn!("Parse tree has errors for file: {}", file_path);
                                }
                            }
                        } else {
                            warn!("Parse tree has errors for file: {}", file_path);
                        }
                    }

                    if matches!(language, Language::Rust) {
                        // REVOLUTIONARY: Use unified Rust extractor for nodes + edges in single pass
                        use crate::languages::rust::RustExtractor;
                        let result = RustExtractor::extract_with_edges(
                            &tree_used,
                            &used_content,
                            &file_path,
                        );
                        Ok(result.nodes) // Return only nodes for backward compatibility
                    } else if matches!(language, Language::Python) {
                        // Use Python extractor (docstrings, type hints, call graph metadata)
                        let extraction =
                            crate::languages::python::extract_python(&file_path, &used_content);
                        Ok(extraction.nodes)
                    } else if matches!(language, Language::JavaScript) {
                        // Use JavaScript-specific extraction (currently stub)
                        let nodes = crate::languages::javascript::extract_js_ts_nodes(
                            language.clone(),
                            &file_path,
                            &used_content,
                            tree_used.root_node(),
                        );
                        Ok(nodes)
                    } else if matches!(language, Language::TypeScript) {
                        // Use generic AstVisitor for TypeScript (bypassing stub)
                        let mut visitor = AstVisitor::new(
                            language.clone(),
                            file_path.clone(),
                            used_content.clone(),
                        );
                        visitor.visit(tree_used.root_node());
                        Ok(visitor.nodes)
                    } else if matches!(language, Language::Swift) {
                        // Use advanced Swift extractor for iOS/macOS development
                        use crate::languages::swift::SwiftExtractor;
                        Ok(SwiftExtractor::extract(
                            &tree_used,
                            &used_content,
                            &file_path,
                        ))
                    } else if matches!(language, Language::CSharp) {
                        // Use advanced C# extractor for .NET development
                        use crate::languages::csharp::CSharpExtractor;
                        Ok(CSharpExtractor::extract(
                            &tree_used,
                            &used_content,
                            &file_path,
                        ))
                    } else if matches!(language, Language::Ruby) {
                        // Use advanced Ruby extractor for Rails development
                        use crate::languages::ruby::RubyExtractor;
                        Ok(RubyExtractor::extract(
                            &tree_used,
                            &used_content,
                            &file_path,
                        ))
                    } else if matches!(language, Language::Php) {
                        // Use advanced PHP extractor for Laravel/web development
                        use crate::languages::php::PhpExtractor;
                        Ok(PhpExtractor::extract(&tree_used, &used_content, &file_path))
                    } else {
                        let mut visitor = AstVisitor::new(
                            language.clone(),
                            file_path.clone(),
                            used_content.clone(),
                        );
                        visitor.visit(tree_used.root_node());
                        Ok(visitor.nodes)
                    }
                }
                None => {
                    // Fallback: try to parse line by line for basic recovery
                    warn!(
                        "Complete parsing failed for {}, attempting line-by-line recovery",
                        file_path
                    );

                    let mut recovered_nodes = Vec::new();
                    let lines: Vec<&str> = content.lines().collect();

                    for (line_num, line) in lines.iter().enumerate() {
                        if let Some(tree) = parser.parse(line, None) {
                            if !tree.root_node().has_error() {
                                if matches!(language, Language::Rust) {
                                    use crate::languages::rust::RustExtractor;
                                    let nodes = RustExtractor::extract(
                                        &tree,
                                        line,
                                        &format!("{}:{}", file_path, line_num + 1),
                                    );
                                    recovered_nodes.extend(nodes);
                                } else if matches!(language, Language::Python) {
                                    let extraction = crate::languages::python::extract_python(
                                        &format!("{}:{}", file_path, line_num + 1),
                                        line,
                                    );
                                    recovered_nodes.extend(extraction.nodes);
                                } else if matches!(language, Language::Swift) {
                                    use crate::languages::swift::SwiftExtractor;
                                    let nodes = SwiftExtractor::extract(
                                        &tree,
                                        line,
                                        &format!("{}:{}", file_path, line_num + 1),
                                    );
                                    recovered_nodes.extend(nodes);
                                } else if matches!(language, Language::CSharp) {
                                    use crate::languages::csharp::CSharpExtractor;
                                    let nodes = CSharpExtractor::extract(
                                        &tree,
                                        line,
                                        &format!("{}:{}", file_path, line_num + 1),
                                    );
                                    recovered_nodes.extend(nodes);
                                } else if matches!(language, Language::Ruby) {
                                    use crate::languages::ruby::RubyExtractor;
                                    let nodes = RubyExtractor::extract(
                                        &tree,
                                        line,
                                        &format!("{}:{}", file_path, line_num + 1),
                                    );
                                    recovered_nodes.extend(nodes);
                                } else if matches!(language, Language::Php) {
                                    use crate::languages::php::PhpExtractor;
                                    let nodes = PhpExtractor::extract(
                                        &tree,
                                        line,
                                        &format!("{}:{}", file_path, line_num + 1),
                                    );
                                    recovered_nodes.extend(nodes);
                                } else {
                                    let mut visitor = AstVisitor::new(
                                        language.clone(),
                                        format!("{}:{}", file_path, line_num + 1),
                                        line.to_string(),
                                    );
                                    visitor.visit(tree.root_node());
                                    recovered_nodes.extend(visitor.nodes);
                                }
                            }
                        }
                    }

                    if recovered_nodes.is_empty() {
                        Err(CodeGraphError::Parse(format!(
                            "Failed to parse file: {}",
                            file_path
                        )))
                    } else {
                        info!(
                            "Recovered {} nodes from partially parsed file: {}",
                            recovered_nodes.len(),
                            file_path
                        );
                        Ok(recovered_nodes)
                    }
                }
            };

            // Return parser to pool for reuse
            {
                let mut pool = parser_pool.lock();
                if let Some(parser_set) = pool.first_mut() {
                    parser_set.insert(language, parser);
                } else {
                    let mut new_set = std::collections::HashMap::new();
                    new_set.insert(language, parser);
                    pool.push(new_set);
                }
            }

            result
        });

        // Apply timeout protection - configurable via CODEGRAPH_PARSER_TIMEOUT_SECS
        let base_timeout: u64 = env::var("CODEGRAPH_PARSER_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);
        let timeout_duration = if content_len > 1_000_000 {
            Duration::from_secs(base_timeout.max(300)) // Large files get more time
        } else if content_len > 100_000 {
            Duration::from_secs((base_timeout * 3).max(200)) // Medium files get moderate time
        } else {
            Duration::from_secs(base_timeout.max(150)) // Small files should parse quickly
        };

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

    // Enhanced tolerant cleaner to strip common constructs that confuse grammars
    fn tolerant_clean(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut in_block_comment = false;
        let mut in_multiline_macro = false;

        for line in src.lines() {
            let trimmed = line.trim_start();

            // Handle block comments
            if trimmed.contains("/*") && !in_block_comment {
                in_block_comment = true;
            }
            if in_block_comment {
                if trimmed.contains("*/") {
                    in_block_comment = false;
                }
                continue;
            }

            // Handle multi-line macros
            if trimmed.starts_with("macro_rules!") || in_multiline_macro {
                in_multiline_macro = true;
                if trimmed.ends_with("}") && !trimmed.ends_with("\\}") {
                    in_multiline_macro = false;
                }
                continue;
            }

            // Skip problematic lines that commonly cause parse errors
            if trimmed.starts_with("#pragma")
                || trimmed.starts_with("//@generated")
                || trimmed.starts_with("//!") // Doc comments that can be complex
                || trimmed.starts_with("#[cfg(")
                || trimmed.starts_with("#[derive(")
                || trimmed.starts_with("#[allow(")
                || trimmed.starts_with("#[warn(")
                || trimmed.starts_with("#[deny(")
                || trimmed.starts_with("#[forbid(")
                || trimmed.starts_with("extern crate")
                || trimmed.starts_with("use std::mem::transmute") // Unsafe constructs
                || trimmed.contains("unsafe {") // Skip unsafe blocks that often have complex syntax
                || trimmed.starts_with("pub use") && trimmed.contains("::*") // Complex re-exports
                || trimmed.contains("__asm__") // Assembly code
                || trimmed.contains("asm!")
            // Rust inline assembly
            {
                // Replace with empty line to maintain line numbers for debugging
                out.push('\n');
                continue;
            }

            // Clean up complex generic syntax that can confuse parsers
            let cleaned_line = if trimmed.contains('<') && trimmed.contains('>') {
                // Simplify complex generic bounds that often cause issues
                line.replace("where T: Clone + Send + Sync + 'static", "")
                    .replace("impl<T>", "impl")
                    .replace("for<'a>", "")
            } else {
                line.to_string()
            };

            out.push_str(&cleaned_line);
            out.push('\n');
        }
        out
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
        let registry = self.registry.clone();
        let content_string = content.to_string();
        let file_path_string = file_path.to_string();
        let parser_pool = self.parser_pool.clone();

        // Clone for timeout message
        let content_len = content.len();
        let file_path_for_timeout = file_path.to_string();

        // Add timeout protection for problematic files
        let parsing_task = tokio::task::spawn_blocking(move || {
            let content = content_string;
            let file_path = file_path_string;

            // Try to get parser from pool first, create new one if pool is empty
            let mut parser = {
                let mut pool = parser_pool.lock();
                let mut found_parser = None;

                for parser_set in pool.iter_mut() {
                    if let Some(p) = parser_set.remove(&language) {
                        found_parser = Some(p);
                        break;
                    }
                }

                found_parser.unwrap_or_else(|| {
                    registry
                        .create_parser(&language)
                        .unwrap_or_else(|| tree_sitter::Parser::new())
                })
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
            let result = match parser.parse(&content, None) {
                Some(tree) => {
                    let mut tree_used = tree;
                    let mut used_content = content.clone();
                    if tree_used.root_node().has_error() {
                        let cleaned = Self::tolerant_clean(&content);
                        if cleaned != content {
                            if let Some(tree2) = parser.parse(&cleaned, None) {
                                if !tree2.root_node().has_error() {
                                    tree_used = tree2;
                                    used_content = cleaned;
                                }
                            }
                        }
                    }

                    // REVOLUTIONARY: Use unified extractors for MAXIMUM SPEED
                    let ast_result = if matches!(language, Language::Rust) {
                        use crate::languages::rust::RustExtractor;
                        RustExtractor::extract_with_edges(&tree_used, &used_content, &file_path)
                    } else if matches!(language, Language::TypeScript) {
                        use crate::languages::javascript::TypeScriptExtractor;
                        TypeScriptExtractor::extract_with_edges(
                            &tree_used,
                            &used_content,
                            &file_path,
                            language.clone(),
                        )
                    } else if matches!(language, Language::JavaScript) {
                        use crate::languages::javascript::TypeScriptExtractor;
                        TypeScriptExtractor::extract_with_edges(
                            &tree_used,
                            &used_content,
                            &file_path,
                            language.clone(),
                        )
                    } else if matches!(language, Language::Python) {
                        use crate::languages::python::PythonExtractor;
                        PythonExtractor::extract_with_edges(&tree_used, &used_content, &file_path)
                    } else {
                        // Fallback: use AstVisitor for other languages (no edges yet)
                        let mut visitor = crate::AstVisitor::new(
                            language.clone(),
                            file_path.clone(),
                            used_content.clone(),
                        );
                        visitor.visit(tree_used.root_node());
                        ExtractionResult {
                            nodes: visitor.nodes,
                            edges: Vec::new(), // No edges for unsupported languages yet
                        }
                    };

                    // Apply Fast ML enhancement for maximum graph completeness
                    // Adds pattern-based edges and resolves unmatched references (<1ms overhead)
                    let enhanced_result =
                        crate::fast_ml::enhance_extraction(ast_result, &used_content);
                    Ok(enhanced_result)
                }
                None => {
                    // Fallback: return empty result
                    warn!("Complete parsing failed for {}", file_path);
                    Ok(ExtractionResult {
                        nodes: Vec::new(),
                        edges: Vec::new(),
                    })
                }
            };

            // Return parser to pool
            {
                let mut pool = parser_pool.lock();
                if let Some(parser_set) = pool.first_mut() {
                    parser_set.insert(language, parser);
                } else {
                    let mut new_set = std::collections::HashMap::new();
                    new_set.insert(language, parser);
                    pool.push(new_set);
                }
            }

            result
        });

        // Apply timeout protection
        let timeout_duration = if content_len > 1_000_000 {
            Duration::from_secs(60)
        } else if content_len > 100_000 {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(10)
        };

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
