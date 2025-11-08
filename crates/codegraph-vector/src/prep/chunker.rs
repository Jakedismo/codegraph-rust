use codegraph_core::{CodeNode, Language};
use fxhash::hash64;
use lru::LruCache;
use rayon::prelude::*;
use semchunk_rs::Chunker;
use std::{num::NonZeroUsize, sync::{Arc, Mutex}, time::Instant};
use tokenizers::Tokenizer;
use unicode_normalization::UnicodeNormalization;

const DEFAULT_MAX_TEXTS_PER_REQUEST: usize = 256;

/// Configuration knobs for the fast chunker.
#[derive(Clone)]
pub struct ChunkerConfig {
    pub max_tokens_per_text: usize,
    pub sanitize_mode: SanitizeMode,
    pub cache_capacity: usize,
    pub max_texts_per_request: usize,
}

impl ChunkerConfig {
    pub fn new(max_tokens_per_text: usize) -> Self {
        Self {
            max_tokens_per_text,
            sanitize_mode: SanitizeMode::AsciiFastPath,
            cache_capacity: 2048,
            max_texts_per_request: DEFAULT_MAX_TEXTS_PER_REQUEST,
        }
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
        self.max_texts_per_request = max.min(DEFAULT_MAX_TEXTS_PER_REQUEST).max(1);
        self
    }
}

#[derive(Clone, Copy)]
pub enum SanitizeMode {
    /// Skip Unicode normalization for ASCII-only strings (fast path).
    AsciiFastPath,
    /// Always normalize via NFC and remove emojis/control chars.
    Strict,
}

/// Result of preparing all nodes for embedding.
pub struct ChunkPlan {
    pub chunks: Vec<TextChunk>,
    pub metas: Vec<ChunkMeta>,
    pub stats: ChunkStats,
}

impl ChunkPlan {
    pub fn chunk_to_node(&self) -> Vec<usize> {
        self.metas.iter().map(|m| m.node_index).collect()
    }
}

#[derive(Clone)]
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
    let start_total = Instant::now();
    let token_counter = Arc::new(TokenCounter::new(tokenizer, config.cache_capacity));

    let results: Vec<(Vec<TextChunk>, Vec<ChunkMeta>, NodeStats)> = nodes
        .par_iter()
        .enumerate()
        .map(|(idx, node)| {
            let node_start = Instant::now();
            let sanitized = sanitize(node, config.sanitize_mode);
            let sanitize_ms = node_start.elapsed().as_millis();

            let chunk_start = Instant::now();
            let (chunks, metas) =
                chunkify(idx, node, &sanitized, &config, Arc::clone(&token_counter));
            let chunk_ms = chunk_start.elapsed().as_millis();

            (
                chunks,
                metas,
                NodeStats {
                    sanitize_ms,
                    chunk_ms,
                },
            )
        })
        .collect();

    let mut all_chunks = Vec::new();
    let mut all_metas = Vec::new();
    let mut stats = ChunkStats::empty();
    stats.total_nodes = nodes.len();

    for (chunks, metas, node_stats) in results {
        stats.total_chunks += chunks.len();
        stats.sanitize_ms += node_stats.sanitize_ms;
        stats.chunk_ms += node_stats.chunk_ms;
        all_chunks.extend(chunks);
        all_metas.extend(metas);
    }

    let (hits, misses) = token_counter.stats();
    stats.cache_hits = hits;
    stats.cache_misses = misses;

    tracing::debug!(
        "Chunk plan built in {:?}: {} nodes -> {} chunks (sanitize {}ms, chunk {}ms, cache hit {} / miss {})",
        start_total.elapsed(),
        stats.total_nodes,
        stats.total_chunks,
        stats.sanitize_ms,
        stats.chunk_ms,
        stats.cache_hits,
        stats.cache_misses
    );

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
        .filter(|c| !c.is_control() && *c != '\0')
        .filter(|c| !is_emoji(*c))
        .collect()
}

fn is_emoji(c: char) -> bool {
    let code = c as u32;
    (0x1F600..=0x1F64F).contains(&code)
        || (0x1F300..=0x1F5FF).contains(&code)
        || (0x1F680..=0x1F6FF).contains(&code)
        || (0x2600..=0x26FF).contains(&code)
}

fn chunkify(
    node_idx: usize,
    node: &CodeNode,
    sanitized: &str,
    config: &ChunkerConfig,
    counter: Arc<TokenCounter>,
) -> (Vec<TextChunk>, Vec<ChunkMeta>) {
    let counter_for_chunker = Arc::clone(&counter);
    let chunker =
        Chunker::new(config.max_tokens_per_text, move |s: &str| counter_for_chunker.count(s));
    let mut result_chunks = Vec::new();
    let mut result_meta = Vec::new();

    for (chunk_idx, text) in chunker.chunk_text(sanitized).into_iter().enumerate() {
        let tokens = counter.count(&text);
        result_chunks.push(TextChunk { text, tokens });
        result_meta.push(ChunkMeta {
            node_index: node_idx,
            chunk_index: chunk_idx,
            language: node.language.clone(),
            file_path: node.location.file_path.clone(),
            node_name: node.name.to_string(),
        });
    }

    if result_chunks.is_empty() {
        let tokens = counter.count(sanitized);
        result_chunks.push(TextChunk {
            text: sanitized.to_string(),
            tokens,
        });
        result_meta.push(ChunkMeta {
            node_index: node_idx,
            chunk_index: 0,
            language: node.language.clone(),
            file_path: node.location.file_path.clone(),
            node_name: node.name.to_string(),
        });
    }

    (result_chunks, result_meta)
}

struct NodeStats {
    sanitize_ms: u128,
    chunk_ms: u128,
}

struct TokenCounter {
    tokenizer: Arc<Tokenizer>,
    cache: Mutex<LruCache<u64, usize>>,
    hits: Mutex<usize>,
    misses: Mutex<usize>,
}

impl TokenCounter {
    fn new(tokenizer: Arc<Tokenizer>, capacity: usize) -> Self {
        Self {
            tokenizer,
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(16).unwrap()),
            )),
            hits: Mutex::new(0),
            misses: Mutex::new(0),
        }
    }

    fn count(&self, text: &str) -> usize {
        let key = hash64(text);
        if let Some(tokens) = self.cache.lock().unwrap().get(&key).copied() {
            *self.hits.lock().unwrap() += 1;
            return tokens;
        }
        let tokens = self
            .tokenizer
            .encode(text, false)
            .map(|enc| enc.len())
            .unwrap_or_else(|_| text.len() / 4);
        let mut cache = self.cache.lock().unwrap();
        cache.put(key, tokens);
        *self.misses.lock().unwrap() += 1;
        tokens
    }

    fn stats(&self) -> (usize, usize) {
        (*self.hits.lock().unwrap(), *self.misses.lock().unwrap())
    }
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
