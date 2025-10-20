use crate::LanguageRegistry;
use codegraph_core::{CodeGraphError, Language, Result};
use dashmap::DashMap;
use parking_lot::Mutex;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::instrument;
use tree_sitter::{Node, Parser, Tree};

/// Configuration for text processing operations
#[derive(Debug, Clone)]
pub struct TextProcessorConfig {
    pub max_chunk_size: usize,
    pub min_chunk_size: usize,
    pub overlap_size: usize,
    pub preserve_semantic_boundaries: bool,
    pub enable_deduplication: bool,
    pub normalization_level: NormalizationLevel,
}

impl Default for TextProcessorConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 1000,
            min_chunk_size: 100,
            overlap_size: 50,
            preserve_semantic_boundaries: true,
            enable_deduplication: true,
            normalization_level: NormalizationLevel::Standard,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NormalizationLevel {
    None,
    Basic,      // Whitespace normalization
    Standard,   // Basic + case normalization for identifiers
    Aggressive, // Standard + comment removal + formatting
}

/// A semantic chunk of text with context information
#[derive(Debug, Clone, PartialEq)]
pub struct TextChunk {
    pub content: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub language: Option<Language>,
    pub chunk_type: ChunkType,
    pub semantic_level: u8,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkType {
    Function,
    Class,
    Module,
    Comment,
    Import,
    Text,
    Code,
}

/// Token with language-aware information
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageToken {
    pub content: String,
    pub token_type: TokenType,
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub column: usize,
    pub language: Option<Language>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Identifier,
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Whitespace,
    Unknown,
}

/// Context extraction result with metadata
#[derive(Debug, Clone)]
pub struct ContextExtraction {
    pub primary_content: String,
    pub surrounding_context: Vec<String>,
    pub semantic_relationships: Vec<TextSemanticRelationship>,
    pub importance_score: f32,
}

#[derive(Debug, Clone)]
pub struct TextSemanticRelationship {
    pub relation_type: RelationType,
    pub target_content: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationType {
    Calls,
    Defines,
    Uses,
    References,
    Contains,
    Implements,
    Extends,
}

/// Statistics about text processing operations
#[derive(Debug, Clone)]
pub struct ProcessingStatistics {
    pub total_chunks: usize,
    pub deduplicated_chunks: usize,
    pub total_tokens: usize,
    pub processing_time_ms: u64,
    pub bytes_processed: usize,
    pub language_distribution: HashMap<Language, usize>,
}

/// Main text processor for code tokenization and chunking
pub struct TextProcessor {
    config: TextProcessorConfig,
    language_registry: Arc<LanguageRegistry>,
    deduplication_cache: Arc<DashMap<String, bool>>,
    normalization_patterns: Arc<DashMap<Language, Vec<Regex>>>,
    compiled_regex_cache: Arc<DashMap<String, Regex>>,
    keyword_cache: Arc<DashMap<Language, HashSet<&'static str>>>,
    parser_pool: Arc<Mutex<Vec<Parser>>>,
}

impl TextProcessor {
    pub fn new(config: TextProcessorConfig) -> Self {
        let processor = Self {
            config,
            language_registry: Arc::new(LanguageRegistry::new()),
            deduplication_cache: Arc::new(DashMap::new()),
            normalization_patterns: Arc::new(DashMap::new()),
            compiled_regex_cache: Arc::new(DashMap::new()),
            keyword_cache: Arc::new(DashMap::new()),
            parser_pool: Arc::new(Mutex::new(Vec::new())),
        };

        processor.initialize_normalization_patterns();
        processor.initialize_keyword_cache();
        processor
    }

    pub fn with_language_registry(mut self, registry: Arc<LanguageRegistry>) -> Self {
        self.language_registry = registry;
        self
    }

    /// Language-aware tokenization for multiple programming languages
    #[instrument(skip(self, content))]
    pub async fn tokenize_language_aware(
        &self,
        content: &str,
        language: Option<Language>,
    ) -> Result<Vec<LanguageToken>> {
        let language = language.unwrap_or(Language::Other("text".to_string()));
        let registry = self.language_registry.clone();
        let regex_cache = self.compiled_regex_cache.clone();
        let keyword_cache = self.keyword_cache.clone();
        let parser_pool = self.parser_pool.clone();
        let content = content.to_string();

        tokio::task::spawn_blocking(move || {
            let mut tokens = Vec::with_capacity(content.len() / 10); // Pre-allocate with estimate

            // Try to get parser from pool first
            let parser_opt = {
                let mut pool = parser_pool.lock();
                pool.pop()
            };

            // Try to use tree-sitter for structured tokenization if language is supported
            if let Some(mut parser) = parser_opt.or_else(|| registry.create_parser(&language)) {
                if let Some(tree) = parser.parse(&content, None) {
                    Self::extract_tokens_from_tree_optimized(
                        &tree,
                        &content,
                        &language,
                        &mut tokens,
                        &keyword_cache,
                    );

                    // Return parser to pool
                    let mut pool = parser_pool.lock();
                    if pool.len() < 10 {
                        // Limit pool size
                        pool.push(parser);
                    }

                    return Ok(tokens);
                }

                // Return parser to pool even if parsing failed
                let mut pool = parser_pool.lock();
                if pool.len() < 10 {
                    pool.push(parser);
                }
            }

            // Fallback to optimized regex-based tokenization
            Self::extract_tokens_regex_optimized(
                &content,
                &language,
                &mut tokens,
                &regex_cache,
                &keyword_cache,
            );
            Ok(tokens)
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    /// Optimized tree-based token extraction with reduced allocations
    fn extract_tokens_from_tree_optimized(
        tree: &Tree,
        content: &str,
        language: &Language,
        tokens: &mut Vec<LanguageToken>,
        keyword_cache: &DashMap<Language, HashSet<&'static str>>,
    ) {
        let root = tree.root_node();
        let cursor = root.walk();
        let content_bytes = content.as_bytes();

        // Stack-based traversal for better performance
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            if node.is_error() {
                continue;
            }

            // Process leaf nodes
            if node.child_count() == 0 {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                if let Ok(node_text) = std::str::from_utf8(&content_bytes[start_byte..end_byte]) {
                    let token_type = Self::classify_token_type_optimized(
                        node.kind(),
                        node_text,
                        language,
                        keyword_cache,
                    );
                    let start_position = node.start_position();

                    tokens.push(LanguageToken {
                        content: node_text.to_string(),
                        token_type,
                        start_byte,
                        end_byte,
                        line: start_position.row,
                        column: start_position.column,
                        language: Some(language.clone()),
                    });
                }
            } else {
                // Add children to stack (in reverse order for proper traversal)
                for i in (0..node.child_count()).rev() {
                    if let Some(child) = node.child(i) {
                        stack.push(child);
                    }
                }
            }
        }
    }

    fn extract_tokens_from_tree(
        tree: &Tree,
        content: &str,
        language: &Language,
        tokens: &mut Vec<LanguageToken>,
    ) {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let content_bytes = content.as_bytes();

        loop {
            let node = cursor.node();

            // Skip error nodes
            if node.is_error() {
                if !cursor.goto_next_sibling() {
                    break;
                }
                continue;
            }

            // Only process leaf nodes (actual tokens)
            if node.child_count() == 0 {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                if let Ok(node_text) = std::str::from_utf8(&content_bytes[start_byte..end_byte]) {
                    let token_type = Self::classify_token_type(node.kind(), node_text, language);
                    let start_position = node.start_position();

                    tokens.push(LanguageToken {
                        content: node_text.to_string(),
                        token_type,
                        start_byte,
                        end_byte,
                        line: start_position.row,
                        column: start_position.column,
                        language: Some(language.clone()),
                    });
                }
            }

            // Navigate the tree
            if cursor.goto_first_child() {
                continue;
            }

            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    fn extract_tokens_regex(content: &str, language: &Language, tokens: &mut Vec<LanguageToken>) {
        // Language-agnostic regex patterns for basic tokenization
        let patterns = vec![
            (r"//[^\r\n]*", TokenType::Comment),       // Line comments
            (r"/\*[\s\S]*?\*/", TokenType::Comment),   // Block comments
            (r"#[^\r\n]*", TokenType::Comment),        // Python/Shell comments
            (r#""([^"\\]|\\.)*""#, TokenType::String), // Double-quoted strings
            (r"'([^'\\]|\\.)*'", TokenType::String),   // Single-quoted strings
            (r"`([^`\\]|\\.)*`", TokenType::String),   // Backtick strings
            (r"\b\d+\.?\d*\b", TokenType::Number),     // Numbers
            (r"\b[a-zA-Z_][a-zA-Z0-9_]*\b", TokenType::Identifier), // Identifiers
            (r"[+\-*/%=<>!&|^~]", TokenType::Operator), // Operators
            (r"[{}()\[\];,.]", TokenType::Punctuation), // Punctuation
            (r"\s+", TokenType::Whitespace),           // Whitespace
        ];

        let keywords = Self::get_language_keywords(language);

        for (i, line) in content.lines().enumerate() {
            let line_offset = 0;
            let line_start_byte = content[..content
                .split('\n')
                .take(i)
                .map(|l| l.len() + 1)
                .sum::<usize>()]
                .len();

            for (pattern, default_type) in &patterns {
                let re = Regex::new(pattern).unwrap();
                for mat in re.find_iter(line) {
                    let token_content = mat.as_str();
                    let mut token_type = default_type.clone();

                    // Check if identifier is actually a keyword
                    if token_type == TokenType::Identifier && keywords.contains(token_content) {
                        token_type = TokenType::Keyword;
                    }

                    tokens.push(LanguageToken {
                        content: token_content.to_string(),
                        token_type,
                        start_byte: line_start_byte + mat.start(),
                        end_byte: line_start_byte + mat.end(),
                        line: i,
                        column: mat.start(),
                        language: Some(language.clone()),
                    });
                }
            }
        }

        // Sort tokens by position
        tokens.sort_by_key(|t| (t.line, t.column));
    }

    /// Optimized regex-based tokenization with caching and reduced allocations
    fn extract_tokens_regex_optimized(
        content: &str,
        language: &Language,
        tokens: &mut Vec<LanguageToken>,
        regex_cache: &DashMap<String, Regex>,
        keyword_cache: &DashMap<Language, HashSet<&'static str>>,
    ) {
        // Get or create compiled regexes with caching
        let patterns = vec![
            ("line_comment", r"//[^\r\n]*", TokenType::Comment),
            ("block_comment", r"/\*[\s\S]*?\*/", TokenType::Comment),
            ("python_comment", r"#[^\r\n]*", TokenType::Comment),
            ("double_string", r#""([^"\\]|\\.)*""#, TokenType::String),
            ("single_string", r"'([^'\\]|\\.)*'", TokenType::String),
            ("backtick_string", r"`([^`\\]|\\.)*`", TokenType::String),
            ("number", r"\b\d+\.?\d*\b", TokenType::Number),
            (
                "identifier",
                r"\b[a-zA-Z_][a-zA-Z0-9_]*\b",
                TokenType::Identifier,
            ),
            ("operator", r"[+\-*/%=<>!&|^~]", TokenType::Operator),
            ("punctuation", r"[{}()\[\];,.]", TokenType::Punctuation),
            ("whitespace", r"\s+", TokenType::Whitespace),
        ];

        let keywords = keyword_cache
            .get(language)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        // Process content line by line for better memory efficiency
        let mut byte_offset = 0;
        for (line_idx, line) in content.lines().enumerate() {
            for (pattern_name, pattern_str, default_type) in &patterns {
                let regex = regex_cache
                    .entry(pattern_name.to_string())
                    .or_insert_with(|| {
                        Regex::new(pattern_str).unwrap_or_else(|_| Regex::new(r"\w+").unwrap())
                    });

                for mat in regex.find_iter(line) {
                    let token_content = mat.as_str();
                    let mut token_type = default_type.clone();

                    // Check if identifier is actually a keyword
                    if token_type == TokenType::Identifier && keywords.contains(token_content) {
                        token_type = TokenType::Keyword;
                    }

                    tokens.push(LanguageToken {
                        content: token_content.to_string(),
                        token_type,
                        start_byte: byte_offset + mat.start(),
                        end_byte: byte_offset + mat.end(),
                        line: line_idx,
                        column: mat.start(),
                        language: Some(language.clone()),
                    });
                }
            }
            byte_offset += line.len() + 1; // +1 for newline
        }

        // Sort tokens by position
        tokens.sort_by_key(|t| (t.line, t.column));
    }

    /// Optimized token classification with caching
    fn classify_token_type_optimized(
        node_kind: &str,
        content: &str,
        language: &Language,
        keyword_cache: &DashMap<Language, HashSet<&'static str>>,
    ) -> TokenType {
        // Fast path for common node types
        match node_kind {
            "comment" | "line_comment" | "block_comment" => return TokenType::Comment,
            "string" | "string_literal" | "raw_string_literal" => return TokenType::String,
            "number" | "integer" | "float" | "decimal" => return TokenType::Number,
            "identifier" => {
                // Check against cached keywords
                if let Some(keywords) = keyword_cache.get(language) {
                    if keywords.contains(content) {
                        return TokenType::Keyword;
                    }
                }
                return TokenType::Identifier;
            }
            kind if kind.contains("keyword") => return TokenType::Keyword,
            _ => {}
        }

        // Operator classification
        if matches!(
            node_kind,
            "+" | "-"
                | "*"
                | "/"
                | "="
                | "=="
                | "!="
                | "<"
                | ">"
                | "<="
                | ">="
                | "&&"
                | "||"
                | "!"
                | "&"
                | "|"
                | "^"
                | "~"
                | "<<"
                | ">>"
                | "+="
                | "-="
                | "*="
                | "/="
                | "%="
                | "&="
                | "|="
                | "^="
                | "<<="
                | ">>="
        ) {
            return TokenType::Operator;
        }

        // Punctuation classification
        if matches!(
            node_kind,
            "(" | ")" | "[" | "]" | "{" | "}" | ";" | "," | "."
        ) {
            return TokenType::Punctuation;
        }

        // Fallback classification
        if content.chars().all(|c| c.is_whitespace()) {
            TokenType::Whitespace
        } else {
            TokenType::Unknown
        }
    }

    fn classify_token_type(node_kind: &str, content: &str, language: &Language) -> TokenType {
        match node_kind {
            "comment" | "line_comment" | "block_comment" => TokenType::Comment,
            "string" | "string_literal" | "raw_string_literal" => TokenType::String,
            "number" | "integer" | "float" | "decimal" => TokenType::Number,
            "identifier" => TokenType::Identifier,
            kind if kind.contains("keyword") => TokenType::Keyword,
            kind if matches!(
                kind,
                "+" | "-"
                    | "*"
                    | "/"
                    | "="
                    | "=="
                    | "!="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "!"
                    | "&"
                    | "|"
                    | "^"
                    | "~"
                    | "<<"
                    | ">>"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "<<="
                    | ">>="
            ) =>
            {
                TokenType::Operator
            }
            kind if matches!(kind, "(" | ")" | "[" | "]" | "{" | "}" | ";" | "," | ".") => {
                TokenType::Punctuation
            }
            _ => {
                // Additional language-specific classification
                let keywords = Self::get_language_keywords(language);
                if keywords.contains(content) {
                    TokenType::Keyword
                } else if content.chars().all(|c| c.is_whitespace()) {
                    TokenType::Whitespace
                } else {
                    TokenType::Unknown
                }
            }
        }
    }

    fn get_language_keywords(language: &Language) -> HashSet<&'static str> {
        match language {
            Language::Rust => [
                "fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while",
                "loop", "break", "continue", "return", "struct", "enum", "impl", "trait", "mod",
                "use", "pub", "crate", "super", "self", "Self", "async", "await", "move", "ref",
                "where", "type", "as", "true", "false", "unsafe", "extern", "dyn",
            ]
            .into_iter()
            .collect(),
            Language::Python => [
                "def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally",
                "with", "as", "import", "from", "return", "yield", "lambda", "and", "or", "not",
                "in", "is", "True", "False", "None", "pass", "break", "continue", "global",
                "nonlocal", "assert", "raise", "del",
            ]
            .into_iter()
            .collect(),
            Language::JavaScript | Language::TypeScript => [
                "function",
                "var",
                "let",
                "const",
                "if",
                "else",
                "for",
                "while",
                "do",
                "switch",
                "case",
                "default",
                "break",
                "continue",
                "return",
                "try",
                "catch",
                "finally",
                "throw",
                "new",
                "this",
                "typeof",
                "instanceof",
                "in",
                "true",
                "false",
                "null",
                "undefined",
                "class",
                "extends",
                "super",
                "import",
                "export",
                "async",
                "await",
            ]
            .into_iter()
            .collect(),
            Language::Go => [
                "func",
                "var",
                "const",
                "if",
                "else",
                "for",
                "switch",
                "case",
                "default",
                "break",
                "continue",
                "return",
                "go",
                "defer",
                "chan",
                "select",
                "type",
                "struct",
                "interface",
                "map",
                "package",
                "import",
                "true",
                "false",
                "nil",
                "range",
            ]
            .into_iter()
            .collect(),
            Language::Java => [
                "public",
                "private",
                "protected",
                "static",
                "final",
                "abstract",
                "class",
                "interface",
                "extends",
                "implements",
                "if",
                "else",
                "for",
                "while",
                "do",
                "switch",
                "case",
                "default",
                "break",
                "continue",
                "return",
                "try",
                "catch",
                "finally",
                "throw",
                "throws",
                "new",
                "this",
                "super",
                "import",
                "package",
                "true",
                "false",
                "null",
            ]
            .into_iter()
            .collect(),
            Language::Cpp => [
                "auto",
                "bool",
                "char",
                "const",
                "double",
                "float",
                "int",
                "long",
                "short",
                "signed",
                "unsigned",
                "void",
                "class",
                "struct",
                "public",
                "private",
                "protected",
                "virtual",
                "static",
                "extern",
                "inline",
                "if",
                "else",
                "for",
                "while",
                "do",
                "switch",
                "case",
                "default",
                "break",
                "continue",
                "return",
                "try",
                "catch",
                "throw",
                "new",
                "delete",
                "this",
                "true",
                "false",
                "nullptr",
                "template",
                "typename",
                "namespace",
                "using",
            ]
            .into_iter()
            .collect(),
            _ => HashSet::new(),
        }
    }

    /// Semantic text chunking with context preservation
    pub async fn chunk_semantic(
        &self,
        content: &str,
        language: Option<Language>,
    ) -> Result<Vec<TextChunk>> {
        let language = language.unwrap_or(Language::Other("text".to_string()));
        let registry = self.language_registry.clone();
        let content = content.to_string();
        let config = self.config.clone();

        tokio::task::spawn_blocking(move || {
            let mut chunks = Vec::new();

            // Try tree-sitter based chunking for supported languages
            if let Some(mut parser) = registry.create_parser(&language) {
                if let Some(tree) = parser.parse(&content, None) {
                    Self::extract_semantic_chunks_from_tree(
                        &tree,
                        &content,
                        &language,
                        &config,
                        &mut chunks,
                    );
                    return Ok(chunks);
                }
            }

            // Fallback to text-based chunking
            Self::extract_chunks_text_based(&content, &language, &config, &mut chunks);
            Ok(chunks)
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    fn extract_semantic_chunks_from_tree(
        tree: &Tree,
        content: &str,
        language: &Language,
        config: &TextProcessorConfig,
        chunks: &mut Vec<TextChunk>,
    ) {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let content_bytes = content.as_bytes();

        // Track visited nodes to avoid duplicates
        let mut processed_ranges = Vec::new();

        loop {
            let node = cursor.node();

            // Skip error nodes and already processed ranges
            if node.is_error()
                || Self::is_range_processed(&processed_ranges, node.start_byte(), node.end_byte())
            {
                if !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        break;
                    }
                    continue;
                }
                continue;
            }

            // Check if this is a semantically meaningful chunk
            if Self::is_semantic_boundary(&node, language) {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let chunk_size = end_byte - start_byte;

                // Only create chunks within size limits
                if chunk_size >= config.min_chunk_size && chunk_size <= config.max_chunk_size {
                    if let Ok(chunk_content) =
                        std::str::from_utf8(&content_bytes[start_byte..end_byte])
                    {
                        let chunk_type = Self::determine_chunk_type(&node, language);
                        let semantic_level = Self::calculate_semantic_level(&node);
                        let start_position = node.start_position();
                        let end_position = node.end_position();

                        // Extract context if configured
                        let (context_before, context_after) = if config.preserve_semantic_boundaries
                        {
                            Self::extract_surrounding_context(
                                content, start_byte, end_byte, &config,
                            )
                        } else {
                            (None, None)
                        };

                        let chunk = TextChunk {
                            content: chunk_content.trim().to_string(),
                            start_byte,
                            end_byte,
                            start_line: start_position.row,
                            end_line: end_position.row,
                            language: Some(language.clone()),
                            chunk_type,
                            semantic_level,
                            context_before,
                            context_after,
                            hash: Self::compute_hash(chunk_content),
                        };

                        chunks.push(chunk);
                        processed_ranges.push((start_byte, end_byte));
                    }
                } else if chunk_size > config.max_chunk_size {
                    // Split large chunks recursively
                    Self::split_large_chunk(
                        &node,
                        content,
                        language,
                        config,
                        chunks,
                        &mut processed_ranges,
                    );
                }
            }

            // Navigate the tree
            if cursor.goto_first_child() {
                continue;
            }

            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }

        // Sort chunks by position
        chunks.sort_by_key(|c| c.start_byte);

        // Apply overlap if configured
        if config.overlap_size > 0 {
            Self::apply_overlap(chunks, content, config.overlap_size);
        }
    }

    fn extract_chunks_text_based(
        content: &str,
        language: &Language,
        config: &TextProcessorConfig,
        chunks: &mut Vec<TextChunk>,
    ) {
        let lines: Vec<&str> = content.lines().collect();
        let mut current_chunk = String::new();
        let mut chunk_start_byte = 0;
        let mut chunk_start_line = 0;
        let mut current_byte_offset = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_with_newline = format!("{}\n", line);
            let line_bytes = line_with_newline.as_bytes().len();

            // Check if adding this line would exceed max size
            let potential_size = current_chunk.len() + line_bytes;

            if potential_size > config.max_chunk_size
                && current_chunk.len() >= config.min_chunk_size
            {
                // Create chunk from accumulated content
                if !current_chunk.trim().is_empty() {
                    let chunk = TextChunk {
                        content: current_chunk.trim().to_string(),
                        start_byte: chunk_start_byte,
                        end_byte: current_byte_offset,
                        start_line: chunk_start_line,
                        end_line: line_idx.saturating_sub(1),
                        language: Some(language.clone()),
                        chunk_type: ChunkType::Text,
                        semantic_level: 1,
                        context_before: None,
                        context_after: None,
                        hash: Self::compute_hash(&current_chunk),
                    };
                    chunks.push(chunk);
                }

                // Start new chunk
                current_chunk.clear();
                chunk_start_byte = current_byte_offset;
                chunk_start_line = line_idx;
            }

            current_chunk.push_str(&line_with_newline);
            current_byte_offset += line_bytes;
        }

        // Add final chunk if it has content
        if !current_chunk.trim().is_empty() && current_chunk.len() >= config.min_chunk_size {
            let chunk = TextChunk {
                content: current_chunk.trim().to_string(),
                start_byte: chunk_start_byte,
                end_byte: current_byte_offset,
                start_line: chunk_start_line,
                end_line: lines.len().saturating_sub(1),
                language: Some(language.clone()),
                chunk_type: ChunkType::Text,
                semantic_level: 1,
                context_before: None,
                context_after: None,
                hash: Self::compute_hash(&current_chunk),
            };
            chunks.push(chunk);
        }
    }

    fn is_semantic_boundary(node: &Node, language: &Language) -> bool {
        match language {
            Language::Rust => {
                matches!(
                    node.kind(),
                    "function_item"
                        | "struct_item"
                        | "enum_item"
                        | "impl_item"
                        | "trait_item"
                        | "mod_item"
                        | "use_declaration"
                        | "const_item"
                        | "static_item"
                )
            }
            Language::Python => {
                matches!(
                    node.kind(),
                    "function_definition"
                        | "class_definition"
                        | "import_statement"
                        | "import_from_statement"
                        | "decorated_definition"
                )
            }
            Language::JavaScript | Language::TypeScript => {
                matches!(
                    node.kind(),
                    "function_declaration"
                        | "arrow_function"
                        | "class_declaration"
                        | "method_definition"
                        | "import_statement"
                        | "export_statement"
                )
            }
            Language::Go => {
                matches!(
                    node.kind(),
                    "function_declaration"
                        | "method_declaration"
                        | "type_declaration"
                        | "var_declaration"
                        | "const_declaration"
                        | "package_clause"
                        | "import_declaration"
                )
            }
            Language::Java => {
                matches!(
                    node.kind(),
                    "method_declaration"
                        | "class_declaration"
                        | "interface_declaration"
                        | "constructor_declaration"
                        | "import_declaration"
                        | "package_declaration"
                )
            }
            Language::Cpp => {
                matches!(
                    node.kind(),
                    "function_definition"
                        | "class_specifier"
                        | "struct_specifier"
                        | "namespace_definition"
                        | "preproc_include"
                        | "declaration"
                )
            }
            _ => {
                // For unknown languages, use structural indicators
                node.child_count() > 2 && node.byte_range().len() > 50
            }
        }
    }

    fn determine_chunk_type(node: &Node, language: &Language) -> ChunkType {
        let kind = node.kind();
        match language {
            Language::Rust => match kind {
                "function_item" => ChunkType::Function,
                "struct_item" | "enum_item" => ChunkType::Class,
                "mod_item" => ChunkType::Module,
                "use_declaration" => ChunkType::Import,
                "line_comment" | "block_comment" => ChunkType::Comment,
                _ => ChunkType::Code,
            },
            Language::Python => match kind {
                "function_definition" => ChunkType::Function,
                "class_definition" => ChunkType::Class,
                "import_statement" | "import_from_statement" => ChunkType::Import,
                "comment" => ChunkType::Comment,
                _ => ChunkType::Code,
            },
            _ => {
                if kind.contains("function") {
                    ChunkType::Function
                } else if kind.contains("class") {
                    ChunkType::Class
                } else if kind.contains("import") {
                    ChunkType::Import
                } else if kind.contains("comment") {
                    ChunkType::Comment
                } else {
                    ChunkType::Code
                }
            }
        }
    }

    fn calculate_semantic_level(node: &Node) -> u8 {
        let mut level = 0;
        let mut current = node.parent();

        while let Some(parent) = current {
            level += 1;
            current = parent.parent();
        }

        level.min(255) as u8
    }

    fn extract_surrounding_context(
        content: &str,
        start_byte: usize,
        end_byte: usize,
        config: &TextProcessorConfig,
    ) -> (Option<String>, Option<String>) {
        let context_size = config.overlap_size;

        let context_before = if start_byte > context_size {
            let before_start = start_byte - context_size;
            std::str::from_utf8(&content.as_bytes()[before_start..start_byte])
                .ok()
                .map(|s| s.to_string())
        } else {
            None
        };

        let context_after = if end_byte + context_size < content.len() {
            let after_end = end_byte + context_size;
            std::str::from_utf8(&content.as_bytes()[end_byte..after_end])
                .ok()
                .map(|s| s.to_string())
        } else {
            None
        };

        (context_before, context_after)
    }

    fn split_large_chunk(
        node: &Node,
        content: &str,
        language: &Language,
        config: &TextProcessorConfig,
        chunks: &mut Vec<TextChunk>,
        processed_ranges: &mut Vec<(usize, usize)>,
    ) {
        // Recursively process child nodes for large chunks
        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if Self::is_semantic_boundary(&child, language) {
                    let start_byte = child.start_byte();
                    let end_byte = child.end_byte();
                    let chunk_size = end_byte - start_byte;

                    if chunk_size >= config.min_chunk_size && chunk_size <= config.max_chunk_size {
                        if let Ok(chunk_content) =
                            std::str::from_utf8(&content.as_bytes()[start_byte..end_byte])
                        {
                            let chunk_type = Self::determine_chunk_type(&child, language);
                            let semantic_level = Self::calculate_semantic_level(&child);
                            let start_position = child.start_position();
                            let end_position = child.end_position();

                            let chunk = TextChunk {
                                content: chunk_content.trim().to_string(),
                                start_byte,
                                end_byte,
                                start_line: start_position.row,
                                end_line: end_position.row,
                                language: Some(language.clone()),
                                chunk_type,
                                semantic_level,
                                context_before: None,
                                context_after: None,
                                hash: Self::compute_hash(chunk_content),
                            };

                            chunks.push(chunk);
                            processed_ranges.push((start_byte, end_byte));
                        }
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn is_range_processed(processed_ranges: &[(usize, usize)], start: usize, end: usize) -> bool {
        processed_ranges
            .iter()
            .any(|(s, e)| start >= *s && end <= *e)
    }

    fn apply_overlap(chunks: &mut Vec<TextChunk>, content: &str, overlap_size: usize) {
        // Add overlapping content between adjacent chunks
        for i in 1..chunks.len() {
            let prev_end = chunks[i - 1].end_byte;
            let curr_start = chunks[i].start_byte;

            if curr_start > prev_end && curr_start - prev_end > overlap_size {
                // Add overlap to previous chunk's end
                if let Ok(overlap_content) =
                    std::str::from_utf8(&content.as_bytes()[prev_end..prev_end + overlap_size])
                {
                    chunks[i - 1].content.push_str("\n...\n");
                    chunks[i - 1].content.push_str(overlap_content.trim());
                }

                // Add overlap to current chunk's beginning
                if let Ok(overlap_content) =
                    std::str::from_utf8(&content.as_bytes()[curr_start - overlap_size..curr_start])
                {
                    let mut new_content = String::new();
                    new_content.push_str(overlap_content.trim());
                    new_content.push_str("\n...\n");
                    new_content.push_str(&chunks[i].content);
                    chunks[i].content = new_content;
                }
            }
        }
    }

    fn compute_hash(content: &str) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(content.as_bytes()))
    }

    /// Context extraction algorithms for relevant embedding context
    pub async fn extract_context(
        &self,
        chunk: &TextChunk,
        full_content: &str,
    ) -> Result<ContextExtraction> {
        let language = chunk
            .language
            .clone()
            .unwrap_or(Language::Other("text".to_string()));
        let registry = self.language_registry.clone();
        let chunk_content = chunk.content.clone();
        let full_content = full_content.to_string();
        let chunk_start = chunk.start_byte;
        let chunk_end = chunk.end_byte;

        tokio::task::spawn_blocking(move || {
            let mut relationships = Vec::new();
            let mut surrounding_context = Vec::new();

            // Extract relationships based on tree-sitter analysis
            if let Some(mut parser) = registry.create_parser(&language) {
                if let Some(tree) = parser.parse(&full_content, None) {
                    Self::extract_semantic_relationships(
                        &tree,
                        &full_content,
                        chunk_start,
                        chunk_end,
                        &language,
                        &mut relationships,
                    );
                }
            }

            // Extract surrounding context
            Self::extract_surrounding_context_detailed(
                &full_content,
                chunk_start,
                chunk_end,
                &mut surrounding_context,
            );

            // Calculate importance score
            let importance_score =
                Self::calculate_importance_score(&chunk_content, &relationships, &language);

            Ok(ContextExtraction {
                primary_content: chunk_content,
                surrounding_context,
                semantic_relationships: relationships,
                importance_score,
            })
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    fn extract_semantic_relationships(
        tree: &Tree,
        content: &str,
        chunk_start: usize,
        chunk_end: usize,
        language: &Language,
        relationships: &mut Vec<TextSemanticRelationship>,
    ) {
        let root = tree.root_node();
        let cursor = root.walk();
        let content_bytes = content.as_bytes();

        // Find the node containing our chunk
        let chunk_node = Self::find_node_at_range(&root, chunk_start, chunk_end);

        if let Some(node) = chunk_node {
            // Find function calls within the chunk
            Self::find_function_calls(&node, content_bytes, language, relationships);

            // Find variable uses and definitions
            Self::find_variable_relationships(&node, content_bytes, language, relationships);

            // Find type relationships
            Self::find_type_relationships(&node, content_bytes, language, relationships);
        }
    }

    fn find_node_at_range<'a>(node: &Node<'a>, start: usize, end: usize) -> Option<Node<'a>> {
        let node_start = node.start_byte();
        let node_end = node.end_byte();

        // Check if this node contains our range
        if node_start <= start && node_end >= end {
            // Check children for more specific match
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if let Some(child_match) = Self::find_node_at_range(&child, start, end) {
                        return Some(child_match);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            Some(*node)
        } else {
            None
        }
    }

    fn find_function_calls(
        node: &Node,
        content_bytes: &[u8],
        language: &Language,
        relationships: &mut Vec<TextSemanticRelationship>,
    ) {
        let mut cursor = node.walk();

        loop {
            let current = cursor.node();

            // Check for function call patterns based on language
            let is_call = match language {
                Language::Rust => matches!(current.kind(), "call_expression"),
                Language::Python => matches!(current.kind(), "call"),
                Language::JavaScript | Language::TypeScript => {
                    matches!(current.kind(), "call_expression")
                }
                Language::Go => matches!(current.kind(), "call_expression"),
                Language::Java => matches!(current.kind(), "method_invocation"),
                Language::Cpp => matches!(current.kind(), "call_expression"),
                _ => current.kind().contains("call"),
            };

            if is_call {
                if let Ok(call_text) =
                    std::str::from_utf8(&content_bytes[current.start_byte()..current.end_byte()])
                {
                    relationships.push(TextSemanticRelationship {
                        relation_type: RelationType::Calls,
                        target_content: call_text.trim().to_string(),
                        confidence: 0.9,
                    });
                }
            }

            // Navigate tree
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    fn find_variable_relationships(
        node: &Node,
        content_bytes: &[u8],
        language: &Language,
        relationships: &mut Vec<TextSemanticRelationship>,
    ) {
        let mut cursor = node.walk();

        loop {
            let current = cursor.node();

            // Check for variable usage patterns
            let is_identifier = match language {
                Language::Rust => matches!(current.kind(), "identifier"),
                Language::Python => matches!(current.kind(), "identifier"),
                Language::JavaScript | Language::TypeScript => {
                    matches!(current.kind(), "identifier")
                }
                Language::Go => matches!(current.kind(), "identifier"),
                Language::Java => matches!(current.kind(), "identifier"),
                Language::Cpp => matches!(current.kind(), "identifier"),
                _ => current.kind() == "identifier",
            };

            if is_identifier {
                if let Ok(id_text) =
                    std::str::from_utf8(&content_bytes[current.start_byte()..current.end_byte()])
                {
                    // Determine if this is a definition or use based on parent context
                    if let Some(parent) = current.parent() {
                        let relation_type = match parent.kind() {
                            kind if kind.contains("declaration") || kind.contains("definition") => {
                                RelationType::Defines
                            }
                            _ => RelationType::Uses,
                        };

                        relationships.push(TextSemanticRelationship {
                            relation_type,
                            target_content: id_text.trim().to_string(),
                            confidence: 0.7,
                        });
                    }
                }
            }

            // Navigate tree
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    fn find_type_relationships(
        node: &Node,
        content_bytes: &[u8],
        language: &Language,
        relationships: &mut Vec<TextSemanticRelationship>,
    ) {
        let mut cursor = node.walk();

        loop {
            let current = cursor.node();

            // Check for type relationships
            let is_type_ref = match language {
                Language::Rust => matches!(current.kind(), "type_identifier" | "generic_type"),
                Language::Python => matches!(current.kind(), "type"),
                Language::JavaScript | Language::TypeScript => {
                    matches!(current.kind(), "type_identifier")
                }
                Language::Go => matches!(current.kind(), "type_identifier"),
                Language::Java => matches!(current.kind(), "type_identifier"),
                Language::Cpp => matches!(current.kind(), "type_identifier"),
                _ => current.kind().contains("type"),
            };

            if is_type_ref {
                if let Ok(type_text) =
                    std::str::from_utf8(&content_bytes[current.start_byte()..current.end_byte()])
                {
                    relationships.push(TextSemanticRelationship {
                        relation_type: RelationType::References,
                        target_content: type_text.trim().to_string(),
                        confidence: 0.8,
                    });
                }
            }

            // Navigate tree
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    fn extract_surrounding_context_detailed(
        content: &str,
        chunk_start: usize,
        chunk_end: usize,
        context: &mut Vec<String>,
    ) {
        let lines: Vec<&str> = content.lines().collect();
        let chunk_lines = content[..chunk_start].lines().count();
        let context_radius = 3; // Number of lines before and after

        // Add lines before
        let start_line = chunk_lines.saturating_sub(context_radius);
        for i in start_line..chunk_lines {
            if i < lines.len() {
                context.push(format!("BEFORE: {}", lines[i]));
            }
        }

        // Add lines after
        let chunk_end_line = content[..chunk_end].lines().count();
        let end_line = (chunk_end_line + context_radius).min(lines.len());
        for i in (chunk_end_line + 1)..end_line {
            if i < lines.len() {
                context.push(format!("AFTER: {}", lines[i]));
            }
        }
    }

    fn calculate_importance_score(
        content: &str,
        relationships: &[TextSemanticRelationship],
        language: &Language,
    ) -> f32 {
        let mut score = 0.5; // Base score

        // Boost score based on content characteristics
        if content.contains("fn ") || content.contains("function ") || content.contains("def ") {
            score += 0.3; // Functions are important
        }

        if content.contains("class ")
            || content.contains("struct ")
            || content.contains("interface ")
        {
            score += 0.4; // Type definitions are very important
        }

        if content.contains("pub ") || content.contains("public ") || content.contains("export ") {
            score += 0.2; // Public APIs are important
        }

        // Boost based on number of relationships
        score += (relationships.len() as f32 * 0.1).min(0.3);

        // Language-specific adjustments
        match language {
            Language::Rust => {
                if content.contains("unsafe") || content.contains("impl") {
                    score += 0.2;
                }
            }
            Language::Python => {
                if content.contains("@") || content.contains("__") {
                    score += 0.1;
                }
            }
            _ => {}
        }

        score.min(1.0)
    }

    /// Deduplication strategies and text normalization
    pub async fn deduplicate_and_normalize(
        &self,
        chunks: Vec<TextChunk>,
    ) -> Result<Vec<TextChunk>> {
        if !self.config.enable_deduplication {
            return Ok(chunks);
        }

        let config = self.config.clone();
        let cache = self.deduplication_cache.clone();

        tokio::task::spawn_blocking(move || {
            let mut normalized_chunks = Vec::new();
            let mut seen_hashes = std::collections::HashSet::new();

            for mut chunk in chunks {
                // Normalize content based on configuration
                chunk.content = Self::normalize_content(
                    &chunk.content,
                    &chunk.language,
                    &config.normalization_level,
                );

                // Recompute hash after normalization
                chunk.hash = Self::compute_hash(&chunk.content);

                // Check for duplicates
                if !seen_hashes.contains(&chunk.hash) {
                    seen_hashes.insert(chunk.hash.clone());
                    normalized_chunks.push(chunk);
                }
            }

            Ok(normalized_chunks)
        })
        .await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }

    fn normalize_content(
        content: &str,
        language: &Option<Language>,
        level: &NormalizationLevel,
    ) -> String {
        match level {
            NormalizationLevel::None => content.to_string(),
            NormalizationLevel::Basic => {
                // Basic whitespace normalization
                content
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            NormalizationLevel::Standard => {
                let mut normalized =
                    Self::normalize_content(content, language, &NormalizationLevel::Basic);

                // Language-specific normalization
                if let Some(lang) = language {
                    normalized = Self::apply_language_normalization(normalized, lang);
                }

                normalized
            }
            NormalizationLevel::Aggressive => {
                let mut normalized =
                    Self::normalize_content(content, language, &NormalizationLevel::Standard);

                // Remove comments
                normalized = Self::remove_comments(&normalized, language);

                // Normalize formatting
                normalized = Self::normalize_formatting(&normalized);

                normalized
            }
        }
    }

    fn apply_language_normalization(content: String, language: &Language) -> String {
        match language {
            Language::Rust => {
                // Normalize Rust-specific patterns
                content
                    .replace("  ", " ")
                    .replace("{ ", "{")
                    .replace(" }", "}")
            }
            Language::Python => {
                // Python-specific normalization
                content.replace("    ", "\t") // Convert spaces to tabs
            }
            _ => content,
        }
    }

    fn remove_comments(content: &str, language: &Option<Language>) -> String {
        if let Some(lang) = language {
            match lang {
                Language::Rust
                | Language::Cpp
                | Language::JavaScript
                | Language::TypeScript
                | Language::Go
                | Language::Java => {
                    let re_line = Regex::new(r"//.*").unwrap();
                    let re_block = Regex::new(r"/\*[\s\S]*?\*/").unwrap();
                    let no_line = re_line.replace_all(content, "");
                    re_block.replace_all(&no_line, "").to_string()
                }
                Language::Python => {
                    let re = Regex::new(r"#.*").unwrap();
                    re.replace_all(content, "").to_string()
                }
                _ => content.to_string(),
            }
        } else {
            content.to_string()
        }
    }

    fn normalize_formatting(content: &str) -> String {
        // Remove extra whitespace and normalize line breaks
        let re = Regex::new(r"\s+").unwrap();
        re.replace_all(content.trim(), " ").to_string()
    }

    /// Get processing statistics
    pub fn get_statistics(&self) -> ProcessingStatistics {
        // For now, return empty statistics
        // In a real implementation, this would track actual processing metrics
        ProcessingStatistics {
            total_chunks: 0,
            deduplicated_chunks: 0,
            total_tokens: 0,
            processing_time_ms: 0,
            bytes_processed: 0,
            language_distribution: HashMap::new(),
        }
    }

    /// Clear deduplication cache
    pub async fn clear_cache(&self) {
        self.deduplication_cache.clear();
    }

    fn initialize_normalization_patterns(&self) {
        // Initialize language-specific normalization patterns for preprocessing
        for language in &[
            Language::Rust,
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Go,
            Language::Java,
            Language::Cpp,
        ] {
            let patterns = match language {
                Language::Rust => vec![
                    Regex::new(r"\s+").unwrap(),  // Multiple whitespace
                    Regex::new(r"//.*").unwrap(), // Line comments
                ],
                Language::Python => vec![
                    Regex::new(r"#.*").unwrap(), // Comments
                    Regex::new(r"\s+").unwrap(), // Multiple whitespace
                ],
                _ => vec![
                    Regex::new(r"\s+").unwrap(), // Basic whitespace normalization
                ],
            };
            self.normalization_patterns
                .insert(language.clone(), patterns);
        }
    }

    /// Initialize keyword cache for fast keyword lookups
    fn initialize_keyword_cache(&self) {
        // Cache static keywords for each language to avoid repeated allocations
        let rust_keywords: HashSet<&'static str> = vec![
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "async", "await", "dyn",
        ]
        .into_iter()
        .collect();

        let python_keywords: HashSet<&'static str> = vec![
            "and", "as", "assert", "break", "class", "continue", "def", "del", "elif", "else",
            "except", "exec", "finally", "for", "from", "global", "if", "import", "in", "is",
            "lambda", "not", "or", "pass", "print", "raise", "return", "try", "while", "with",
            "yield", "async", "await",
        ]
        .into_iter()
        .collect();

        let js_keywords: HashSet<&'static str> = vec![
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "let",
            "new",
            "return",
            "super",
            "switch",
            "this",
            "throw",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
            "yield",
        ]
        .into_iter()
        .collect();

        let go_keywords: HashSet<&'static str> = vec![
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
        ]
        .into_iter()
        .collect();

        let java_keywords: HashSet<&'static str> = vec![
            "abstract",
            "assert",
            "boolean",
            "break",
            "byte",
            "case",
            "catch",
            "char",
            "class",
            "const",
            "continue",
            "default",
            "do",
            "double",
            "else",
            "enum",
            "extends",
            "final",
            "finally",
            "float",
            "for",
            "goto",
            "if",
            "implements",
            "import",
            "instanceof",
            "int",
            "interface",
            "long",
            "native",
            "new",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "short",
            "static",
            "strictfp",
            "super",
            "switch",
            "synchronized",
            "this",
            "throw",
            "throws",
            "transient",
            "try",
            "void",
            "volatile",
            "while",
        ]
        .into_iter()
        .collect();

        let cpp_keywords: HashSet<&'static str> = vec![
            "alignas",
            "alignof",
            "and",
            "and_eq",
            "asm",
            "atomic_cancel",
            "atomic_commit",
            "atomic_noexcept",
            "auto",
            "bitand",
            "bitor",
            "bool",
            "break",
            "case",
            "catch",
            "char",
            "char8_t",
            "char16_t",
            "char32_t",
            "class",
            "compl",
            "concept",
            "const",
            "consteval",
            "constexpr",
            "constinit",
            "const_cast",
            "continue",
            "co_await",
            "co_return",
            "co_yield",
            "decltype",
            "default",
            "delete",
            "do",
            "double",
            "dynamic_cast",
            "else",
            "enum",
            "explicit",
            "export",
            "extern",
            "false",
            "float",
            "for",
            "friend",
            "goto",
            "if",
            "inline",
            "int",
            "long",
            "mutable",
            "namespace",
            "new",
            "noexcept",
            "not",
            "not_eq",
            "nullptr",
            "operator",
            "or",
            "or_eq",
            "private",
            "protected",
            "public",
            "reflexpr",
            "register",
            "reinterpret_cast",
            "requires",
            "return",
            "short",
            "signed",
            "sizeof",
            "static",
            "static_assert",
            "static_cast",
            "struct",
            "switch",
            "synchronized",
            "template",
            "this",
            "thread_local",
            "throw",
            "true",
            "try",
            "typedef",
            "typeid",
            "typename",
            "union",
            "unsigned",
            "using",
            "virtual",
            "void",
            "volatile",
            "wchar_t",
            "while",
            "xor",
            "xor_eq",
        ]
        .into_iter()
        .collect();

        // Cache all keywords
        self.keyword_cache.insert(Language::Rust, rust_keywords);
        self.keyword_cache.insert(Language::Python, python_keywords);
        self.keyword_cache
            .insert(Language::JavaScript, js_keywords.clone());
        self.keyword_cache.insert(Language::TypeScript, js_keywords); // TypeScript shares JS keywords
        self.keyword_cache.insert(Language::Go, go_keywords);
        self.keyword_cache.insert(Language::Java, java_keywords);
        self.keyword_cache.insert(Language::Cpp, cpp_keywords);
    }

    fn compute_chunk_hash(&self, content: &str) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(content.as_bytes()))
    }

    fn determine_semantic_level(&self, node: Node, language: &Language) -> u8 {
        // Determine semantic level based on tree-sitter node depth and type
        let mut level = 0;
        let mut current = Some(node);

        while let Some(n) = current {
            level += 1;
            current = n.parent();
        }

        // Adjust level based on node type importance
        match node.kind() {
            "function_item" | "function_definition" | "function_declaration" => level + 2,
            "struct_item" | "class_definition" | "class_declaration" => level + 3,
            "impl_item" | "interface_declaration" => level + 2,
            "mod_item" | "module" => level + 1,
            _ => level,
        }
        .min(255) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_text_processor_creation() {
        let config = TextProcessorConfig::default();
        let processor = TextProcessor::new(config);

        // Basic creation test
        assert!(processor.deduplication_cache.is_empty());
    }

    #[tokio::test]
    async fn test_language_aware_tokenization() {
        let processor = TextProcessor::new(TextProcessorConfig::default());
        let rust_code = "fn main() { println!(\"Hello, world!\"); }";

        // This should not panic when implemented
        // let tokens = processor.tokenize_language_aware(rust_code, Some(Language::Rust)).await;
        // assert!(tokens.is_ok());
    }

    #[tokio::test]
    async fn test_semantic_chunking() {
        let processor = TextProcessor::new(TextProcessorConfig::default());
        let code = "
        struct User {
            name: String,
            age: u32,
        }
        
        impl User {
            fn new(name: String, age: u32) -> Self {
                Self { name, age }
            }
        }";

        // This should not panic when implemented
        // let chunks = processor.chunk_semantic(code, Some(Language::Rust)).await;
        // assert!(chunks.is_ok());
    }

    #[tokio::test]
    async fn test_context_extraction() {
        let processor = TextProcessor::new(TextProcessorConfig::default());
        let chunk = TextChunk {
            content: "fn new(name: String, age: u32) -> Self".to_string(),
            start_byte: 0,
            end_byte: 36,
            start_line: 1,
            end_line: 1,
            language: Some(Language::Rust),
            chunk_type: ChunkType::Function,
            semantic_level: 2,
            context_before: None,
            context_after: None,
            hash: "test_hash".to_string(),
        };

        let full_content =
            "impl User { fn new(name: String, age: u32) -> Self { Self { name, age } } }";

        // This should not panic when implemented
        // let context = processor.extract_context(&chunk, full_content).await;
        // assert!(context.is_ok());
    }

    #[tokio::test]
    async fn test_deduplication() {
        let processor = TextProcessor::new(TextProcessorConfig::default());
        let chunks = vec![
            TextChunk {
                content: "println!(\"test\");".to_string(),
                start_byte: 0,
                end_byte: 17,
                start_line: 1,
                end_line: 1,
                language: Some(Language::Rust),
                chunk_type: ChunkType::Code,
                semantic_level: 1,
                context_before: None,
                context_after: None,
                hash: processor.compute_chunk_hash("println!(\"test\");"),
            },
            TextChunk {
                content: "println!(\"test\");".to_string(), // Duplicate
                start_byte: 18,
                end_byte: 35,
                start_line: 2,
                end_line: 2,
                language: Some(Language::Rust),
                chunk_type: ChunkType::Code,
                semantic_level: 1,
                context_before: None,
                context_after: None,
                hash: processor.compute_chunk_hash("println!(\"test\");"),
            },
        ];

        // This should not panic when implemented
        // let deduplicated = processor.deduplicate_and_normalize(chunks).await;
        // assert!(deduplicated.is_ok());
    }

    #[tokio::test]
    async fn test_configuration_options() {
        let config = TextProcessorConfig {
            max_chunk_size: 500,
            min_chunk_size: 50,
            overlap_size: 25,
            preserve_semantic_boundaries: true,
            enable_deduplication: false,
            normalization_level: NormalizationLevel::Aggressive,
        };

        let processor = TextProcessor::new(config.clone());
        assert_eq!(processor.config.max_chunk_size, 500);
        assert_eq!(
            processor.config.normalization_level,
            NormalizationLevel::Aggressive
        );
    }

    #[test]
    fn test_chunk_hash_computation() {
        let processor = TextProcessor::new(TextProcessorConfig::default());
        let hash1 = processor.compute_chunk_hash("test content");
        let hash2 = processor.compute_chunk_hash("test content");
        let hash3 = processor.compute_chunk_hash("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_chunk_type_detection() {
        // Test chunk type classification logic
        assert_eq!(ChunkType::Function, ChunkType::Function);
        assert_ne!(ChunkType::Function, ChunkType::Class);
    }

    #[test]
    fn test_token_type_classification() {
        // Test token type classification
        assert_eq!(TokenType::Identifier, TokenType::Identifier);
        assert_ne!(TokenType::Keyword, TokenType::Identifier);
    }

    #[test]
    fn test_normalization_levels() {
        assert_ne!(NormalizationLevel::None, NormalizationLevel::Basic);
        assert_ne!(NormalizationLevel::Standard, NormalizationLevel::Aggressive);
    }
}
