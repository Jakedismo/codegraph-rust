// ABOUTME: Parallel, deterministic chunk planning with Unicode-safe token budgets.
use codegraph_core::{CodeNode, Language};
use rayon::prelude::*;
use semchunk_rs::Chunker as SemanticChunker;
use std::{sync::Arc, time::Instant};
use tokenizers::Tokenizer;
use unicode_normalization::UnicodeNormalization;

const DEFAULT_MAX_TEXTS_PER_REQUEST: usize = 256;
const DEFAULT_OVERLAP_TOKENS: usize = 64;
pub type TokenCounter = Arc<dyn Fn(&str) -> usize + Send + Sync>;

/// Configuration knobs for the fast chunker.
#[derive(Clone)]
pub struct ChunkerConfig {
    pub max_tokens_per_text: usize,
    pub sanitize_mode: SanitizeMode,
    pub cache_capacity: usize,
    pub max_texts_per_request: usize,
    pub overlap_tokens: usize,
    pub smart_split: bool,
    pub cache_dir: Option<std::path::PathBuf>,
    pub use_text_splitter: bool,
    pub token_counter: Option<(String, TokenCounter)>,
}

impl ChunkerConfig {
    pub fn new(max_tokens_per_text: usize) -> Self {
        Self {
            max_tokens_per_text,
            sanitize_mode: SanitizeMode::AsciiFastPath,
            cache_capacity: 2048,
            max_texts_per_request: DEFAULT_MAX_TEXTS_PER_REQUEST,
            overlap_tokens: DEFAULT_OVERLAP_TOKENS,
            smart_split: true,
            cache_dir: None,
            token_counter: None,
            use_text_splitter: std::env::var("CODEGRAPH_CHUNK_SPLITTER").as_deref()
                == Ok("text-splitter"),
        }
    }

    pub fn cache_dir(mut self, root: Option<std::path::PathBuf>) -> Self {
        self.cache_dir = root;
        self
    }

    pub fn sanitize_mode(mut self, mode: SanitizeMode) -> Self {
        self.sanitize_mode = mode;
        self
    }

    pub fn cache_capacity(mut self, cap: usize) -> Self {
        self.cache_capacity = cap.max(16);
        self
    }

    pub fn max_texts_per_request(mut self, max: usize) -> Self {
        self.max_texts_per_request = max.clamp(1, DEFAULT_MAX_TEXTS_PER_REQUEST);
        self
    }

    pub fn max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens_per_text = max_tokens;
        self
    }

    pub fn overlap_tokens(mut self, overlap_tokens: usize) -> Self {
        self.overlap_tokens = overlap_tokens;
        self
    }

    pub fn smart_split(mut self, enabled: bool) -> Self {
        self.smart_split = enabled;
        self
    }
}

#[derive(Clone, Copy)]
pub enum SanitizeMode {
    /// Skip Unicode normalization for ASCII-only strings (fast path).
    AsciiFastPath,
    /// Always normalize via NFC and remove non-whitespace controls.
    Strict,
}

/// Result of preparing all nodes for embedding.
pub struct ChunkPlan {
    pub chunks: Vec<TextChunk>,
    pub metas: Vec<ChunkMeta>,
    pub stats: ChunkStats,
}

impl ChunkPlan {
    pub fn empty() -> Self {
        Self {
            chunks: Vec::new(),
            metas: Vec::new(),
            stats: ChunkStats::empty(),
        }
    }
    pub fn chunk_to_node(&self) -> Vec<usize> {
        self.metas.iter().map(|m| m.node_index).collect()
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TextChunk {
    pub text: String,
    pub tokens: usize,
}

#[derive(Clone)]
pub struct ChunkMeta {
    pub node_index: usize,
    pub chunk_index: usize,
    pub language: Option<Language>,
    pub file_path: String,
    pub node_name: String,
}

pub struct ChunkStats {
    pub total_nodes: usize,
    pub total_chunks: usize,
    pub sanitize_ms: u128,
    pub chunk_ms: u128,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl ChunkStats {
    fn empty() -> Self {
        Self {
            total_nodes: 0,
            total_chunks: 0,
            sanitize_ms: 0,
            chunk_ms: 0,
            cache_hits: 0,
            cache_misses: 0,
        }
    }
}

/// Main entry point: build a chunk plan for a slice of nodes.
pub fn build_chunk_plan(
    nodes: &[CodeNode],
    tokenizer: Arc<Tokenizer>,
    config: ChunkerConfig,
) -> ChunkPlan {
    build_chunk_plan_with_sources(nodes, &std::collections::HashMap::new(), tokenizer, config)
}

/// Span-aware variant: uses file_sources to slice exact spans when available.
pub fn build_chunk_plan_with_sources(
    nodes: &[CodeNode],
    file_sources: &std::collections::HashMap<String, String>,
    tokenizer: Arc<Tokenizer>,
    config: ChunkerConfig,
) -> ChunkPlan {
    build_chunk_plan_with_source_lookup(
        nodes,
        |path| {
            file_sources
                .get(path)
                .map(|source| Arc::<str>::from(source.as_str()))
        },
        tokenizer,
        config,
    )
}

pub fn build_chunk_plan_with_source_lookup(
    nodes: &[CodeNode],
    source_lookup: impl Fn(&str) -> Option<Arc<str>> + Sync,
    tokenizer: Arc<Tokenizer>,
    config: ChunkerConfig,
) -> ChunkPlan {
    let tokenizer_key = config
        .token_counter
        .as_ref()
        .map(|counter| counter.0.clone())
        .unwrap_or_else(|| {
            codegraph_core::artifact_cache::fingerprint(
                &tokenizer.to_string(false).unwrap_or_default(),
            )
            .unwrap()
        });
    let count: Arc<dyn Fn(&str) -> usize + Send + Sync> = config
        .token_counter
        .as_ref()
        .map(|counter| counter.1.clone())
        .unwrap_or_else(|| {
            let tok = tokenizer.clone();
            Arc::new(move |text| count_tokens(&tok, text))
        });
    let artifacts = config
        .cache_dir
        .clone()
        .map(|path| path.into_os_string())
        .map(|root| codegraph_core::artifact_cache::ArtifactCache::new(root, "chunks-v2"));
    type Entry = Arc<parking_lot::Mutex<Option<Vec<TextChunk>>>>;
    let cache = parking_lot::Mutex::new(lru::LruCache::<String, Entry>::new(
        std::num::NonZeroUsize::new(config.cache_capacity.max(1)).unwrap(),
    ));
    let mut groups = std::collections::BTreeMap::<&str, Vec<(usize, &CodeNode)>>::new();
    for (index, node) in nodes.iter().enumerate() {
        groups
            .entry(&node.location.file_path)
            .or_default()
            .push((index, node));
    }
    let mut plans: Vec<_> = groups
        .par_iter()
        .flat_map(|(path, group)| {
            let source = source_lookup(path);
            let cache = &cache;
            let artifacts = &artifacts;
            let config = &config;
            let count = &count;
            let tokenizer_key = &tokenizer_key;
            group.par_iter().map(move |&(node_idx, node)| {
                let sanitize_start = Instant::now();
                let base_text =
                    if let (Some(span), Some(source)) = (node.span.as_ref(), source.as_ref()) {
                        let start = span.start_byte as usize;
                        let end = span.end_byte as usize;
                        if start < end
                            && end <= source.len()
                            && source.is_char_boundary(start)
                            && source.is_char_boundary(end)
                        {
                            source[start..end].to_string()
                        } else {
                            sanitize(node, config.sanitize_mode)
                        }
                    } else if let Some(content) = node.content.as_ref() {
                        content.to_string()
                    } else {
                        sanitize(node, config.sanitize_mode)
                    };

                let sanitized = match config.sanitize_mode {
                    SanitizeMode::AsciiFastPath if base_text.is_ascii() => base_text,
                    _ => super_sanitize(&base_text),
                };
                let sanitize_ms = sanitize_start.elapsed().as_millis();
                let key = codegraph_core::artifact_cache::fingerprint(&(
                    "chunks-v2",
                    &tokenizer_key,
                    &sanitized,
                    config.max_tokens_per_text,
                    config.overlap_tokens,
                    config.smart_split,
                    config.use_text_splitter,
                ))
                .unwrap();
                let entry = {
                    let mut cache = cache.lock();
                    if let Some(entry) = cache.get(&key) {
                        entry.clone()
                    } else {
                        let entry = Arc::new(parking_lot::Mutex::new(None));
                        cache.put(key.clone(), entry.clone());
                        entry
                    }
                };
                let mut cached = entry.lock();
                if cached.is_none() {
                    *cached = artifacts.as_ref().and_then(|cache| cache.get(&key));
                }
                let hit = cached.is_some();
                let chunk_start = Instant::now();
                let chunks = cached
                    .get_or_insert_with(|| {
                        let mut all_chunks = Vec::new();
                        let segments: Vec<String> = if config.smart_split {
                            smart_split(&sanitized)
                        } else {
                            vec![sanitized.clone()]
                        };
                        let chunker = SemanticChunker::new(
                            config.max_tokens_per_text,
                            Box::new({
                                let count = count.clone();
                                move |s: &str| count(s)
                            }),
                        );

                        let mut raw_chunks = Vec::new();
                        for segment in segments {
                            if config.use_text_splitter {
                                struct Sizer(Arc<dyn Fn(&str) -> usize + Send + Sync>);
                                impl text_splitter::ChunkSizer for Sizer {
                                    fn size(&self, chunk: &str) -> usize {
                                        self.0(chunk)
                                    }
                                }
                                let splitter = text_splitter::TextSplitter::new(
                                    text_splitter::ChunkConfig::new(
                                        config.max_tokens_per_text.max(1),
                                    )
                                    .with_sizer(Sizer(count.clone()))
                                    .with_trim(false),
                                );
                                for text in splitter.chunks(&segment) {
                                    split_with_counter(
                                        text.to_owned(),
                                        count.as_ref(),
                                        config.max_tokens_per_text,
                                        &mut raw_chunks,
                                    );
                                }
                                continue;
                            }
                            // semchunk 0.1.1's character fallback indexes by byte count and panics
                            // on multibyte text. Keep structural boundaries and split UTF-8 safely.
                            if !segment.is_ascii() {
                                split_with_counter(
                                    segment,
                                    count.as_ref(),
                                    config.max_tokens_per_text,
                                    &mut raw_chunks,
                                );
                                continue;
                            }
                            for text in chunker.chunk(&segment) {
                                split_with_counter(
                                    text,
                                    count.as_ref(),
                                    config.max_tokens_per_text,
                                    &mut raw_chunks,
                                );
                            }
                        }
                        let mut overlap_tail: Option<String> = None;

                        for chunk_text in raw_chunks {
                            let mut text = chunk_text;

                            if let Some(tail) = &overlap_tail
                                && config.overlap_tokens > 0
                            {
                                // Prepend overlap tail if within budget
                                let candidate = format!("{}{}", tail, text);
                                if count(&candidate) <= config.max_tokens_per_text {
                                    text = candidate;
                                }
                            }

                            let tokens = count(&text);
                            all_chunks.push(TextChunk {
                                text: text.clone(),
                                tokens,
                            });
                            // Capture tail for next chunk (approximate overlap using chars, UTF-8 safe)
                            if config.overlap_tokens > 0 {
                                let approx_chars = config.overlap_tokens * 4;
                                overlap_tail = Some(take_tail_utf8(&text, approx_chars));
                            }
                        }

                        if let Some(cache) = &artifacts
                            && let Err(error) = cache.put(&key, &all_chunks)
                        {
                            tracing::debug!("Chunk cache write failed: {error}");
                        }
                        all_chunks
                    })
                    .clone();
                let metas = chunks
                    .iter()
                    .enumerate()
                    .map(|(chunk_index, _)| ChunkMeta {
                        node_index: node_idx,
                        chunk_index,
                        language: node.language.clone(),
                        file_path: node.location.file_path.clone(),
                        node_name: node.name.to_string(),
                    })
                    .collect::<Vec<_>>();
                (
                    node_idx,
                    chunks,
                    metas,
                    sanitize_ms,
                    chunk_start.elapsed().as_millis(),
                    hit,
                )
            })
        })
        .collect();
    plans.sort_by_key(|plan| plan.0);
    let mut stats = ChunkStats::empty();
    stats.total_nodes = nodes.len();
    let mut all_chunks = Vec::new();
    let mut all_metas = Vec::new();
    for (_, chunks, metas, sanitize_ms, chunk_ms, hit) in plans {
        all_chunks.extend(chunks);
        all_metas.extend(metas);
        stats.sanitize_ms += sanitize_ms;
        stats.chunk_ms += chunk_ms;
        stats.cache_hits += usize::from(hit);
        stats.cache_misses += usize::from(!hit);
    }
    stats.total_chunks = all_chunks.len();
    ChunkPlan {
        chunks: all_chunks,
        metas: all_metas,
        stats,
    }
}

fn sanitize(node: &CodeNode, mode: SanitizeMode) -> String {
    let source: &str = node
        .content
        .as_ref()
        .map(|s| s.as_ref())
        .unwrap_or_else(|| node.name.as_ref());

    match mode {
        SanitizeMode::AsciiFastPath if source.is_ascii() => source.to_string(),
        _ => super_sanitize(source),
    }
}

fn super_sanitize(text: &str) -> String {
    let normalized: String = text.nfc().collect();
    normalized
        .chars()
        .filter(|c| !c.is_control() || matches!(*c, '\n' | '\r' | '\t'))
        .collect()
}

fn count_tokens(tokenizer: &Tokenizer, text: &str) -> usize {
    tokenizer
        .encode(text, false)
        .map(|e| e.get_ids().len())
        .unwrap_or(text.len())
}

// Semantic merging can slightly exceed the budget when token boundaries change.
// Recount completed chunks and split at UTF-8 boundaries without dropping text.
fn split_with_counter(
    text: String,
    count: &dyn Fn(&str) -> usize,
    limit: usize,
    output: &mut Vec<String>,
) {
    if count(&text) <= limit {
        output.push(text);
        return;
    }
    let mut midpoint = text.len() / 2;
    while midpoint > 0 && !text.is_char_boundary(midpoint) {
        midpoint -= 1;
    }
    if midpoint == 0 {
        midpoint = text
            .char_indices()
            .nth(1)
            .map_or(text.len(), |(index, _)| index);
    }
    if midpoint == text.len() {
        // One character cannot be divided further; validated provider budgets
        // are larger than the tokenizer's maximum tokens for a single character.
        output.push(text);
        return;
    }
    split_with_counter(text[..midpoint].to_owned(), count, limit, output);
    split_with_counter(text[midpoint..].to_owned(), count, limit, output);
}

/// Lightweight structural split: keep blank-line and brace boundaries to align with AST structure.
fn smart_split(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        let is_boundary = trimmed.is_empty() || trimmed == "}" || trimmed.ends_with("};");

        if is_boundary && !current.is_empty() {
            segments.push(current.clone());
            current.clear();
        }
        if !trimmed.is_empty() {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    if segments.is_empty() {
        segments.push(text.to_string());
    }

    segments
}

fn take_tail_utf8(text: &str, approx_chars: usize) -> String {
    if approx_chars == 0 {
        return String::new();
    }
    let mut count = 0;
    let mut start_idx = 0;
    for (idx, _) in text.char_indices().rev() {
        count += 1;
        start_idx = idx;
        if count >= approx_chars {
            break;
        }
    }
    text[start_idx..].to_string()
}

/// Combine per-chunk embeddings back into per-node vectors by averaging.
pub fn aggregate_chunk_embeddings(
    node_count: usize,
    chunk_to_node: &[usize],
    chunk_embeddings: Vec<Vec<f32>>,
    dimension: usize,
) -> Vec<Vec<f32>> {
    let mut node_embeddings = vec![vec![0.0f32; dimension]; node_count];
    let mut node_chunk_counts = vec![0usize; node_count];

    for (chunk_idx, chunk_embedding) in chunk_embeddings.into_iter().enumerate() {
        if chunk_idx >= chunk_to_node.len() {
            break;
        }
        let node_idx = chunk_to_node[chunk_idx];
        if node_idx >= node_count {
            continue;
        }

        let target = &mut node_embeddings[node_idx];
        let len = target.len().min(chunk_embedding.len());
        for i in 0..len {
            target[i] += chunk_embedding[i];
        }
        node_chunk_counts[node_idx] += 1;
    }

    for (embedding, count) in node_embeddings
        .iter_mut()
        .zip(node_chunk_counts.into_iter())
    {
        if count > 0 {
            let inv = 1.0f32 / count as f32;
            for val in embedding.iter_mut() {
                *val *= inv;
            }
        }
    }

    node_embeddings
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    #[test]
    fn normalization_preserves_structural_whitespace_and_unicode() {
        assert_eq!(
            super_sanitize("fn café() {\n\tlet x = \"🚀\";\n}\0"),
            "fn café() {\n\tlet x = \"🚀\";\n}"
        );
    }
    #[test]
    fn optional_splitter_preserves_whitespace_and_exact_counter_budget() {
        use codegraph_core::{Location, NodeType};
        let tokenizer = Arc::new(
            Tokenizer::from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tokenizers/qwen2.5-coder.json"
            ))
            .unwrap(),
        );
        let text = "fn café() {\n\tlet x = \"🚀\";\n}\n".repeat(20);
        let node = CodeNode::new(
            "sample",
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: "a.rs".into(),
                line: 1,
                column: 0,
                end_line: None,
                end_column: None,
            },
        )
        .with_content(text.clone());
        let mut config = ChunkerConfig::new(47).overlap_tokens(0).smart_split(false);
        config.use_text_splitter = true;
        config.token_counter = Some(("utf8-byte-budget".into(), Arc::new(str::len)));
        let plan = build_chunk_plan(&[node], tokenizer, config);
        assert!(plan.chunks.iter().all(|chunk| chunk.text.len() <= 47));
        assert_eq!(
            plan.chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            text
        );
    }
    #[test]
    fn parallel_plans_keep_source_order_and_provider_token_budget() {
        use codegraph_core::{Location, NodeType};
        let tokenizer = Arc::new(
            Tokenizer::from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tokenizers/qwen2.5-coder.json"
            ))
            .unwrap(),
        );
        let nodes: Vec<_> = (0..16)
            .map(|index| {
                CodeNode::new(
                    format!("n{index}"),
                    Some(NodeType::Function),
                    Some(Language::Rust),
                    Location {
                        file_path: "a.rs".into(),
                        line: 1,
                        column: 0,
                        end_line: None,
                        end_column: None,
                    },
                )
                .with_content("fn café() {\n let 🚀 = café();\n}\n".repeat(12))
            })
            .collect();
        let plan = build_chunk_plan(
            &nodes,
            tokenizer.clone(),
            ChunkerConfig::new(32).overlap_tokens(0),
        );
        assert!(
            plan.metas
                .windows(2)
                .all(|pair| pair[0].node_index <= pair[1].node_index)
        );
        assert!(
            plan.chunks
                .iter()
                .all(|chunk| count_tokens(&tokenizer, &chunk.text) <= 32)
        );
        assert!(plan.stats.cache_hits > 0);
        assert!(plan.chunks.iter().any(|chunk| chunk.text.contains('🚀')));
    }
}
