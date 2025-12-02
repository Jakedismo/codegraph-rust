use codegraph_core::{CodeNode, Language};
use rayon::prelude::*;
use std::{sync::Arc, time::Instant};
use tokenizers::{Encoding, Tokenizer};
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
        self.max_texts_per_request = max.clamp(1, DEFAULT_MAX_TEXTS_PER_REQUEST);
        self
    }

    pub fn max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens_per_text = max_tokens;
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
    let token_counter = Arc::new(TokenCounter::new(tokenizer.clone(), config.cache_capacity));

    // Batch tokenization to reduce per-node tokenizer overhead
    let batch_size = config.max_texts_per_request;
    // Upper bound estimate: assume ~2 chunks per node as a guard
    let estimate = nodes.len().saturating_mul(2).max(16);
    let mut all_chunks = Vec::with_capacity(estimate);
    let mut all_metas = Vec::with_capacity(estimate);
    let mut stats = ChunkStats::empty();
    stats.total_nodes = nodes.len();

    for (batch_idx, batch) in nodes.chunks(batch_size).enumerate() {
        // Sanitize texts once
        let sanitized: Vec<String> = batch
            .par_iter()
            .with_min_len(16)
            .map(|node| sanitize(node, config.sanitize_mode))
            .collect();

        // Tokenize batch
        let encodings: Vec<Encoding> = tokenizer
            .encode_batch(sanitized.iter().map(|s| s.as_str()).collect::<Vec<_>>(), false)
            .expect("tokenizer batch encode");

        // Estimate capacity to preallocate
        let estimated = encodings
            .iter()
            .map(|e| (e.get_ids().len() / config.max_tokens_per_text + 1).max(1))
            .sum::<usize>();
        let mut batch_chunks = Vec::with_capacity(estimated);
        let mut batch_metas = Vec::with_capacity(estimated);

        for (local_idx, (node, encoding)) in batch.iter().zip(encodings.iter()).enumerate() {
            let global_idx = batch_idx * batch_size + local_idx;
            let (chunks, metas, node_stats) = chunkify_tokens(
                global_idx,
                node,
                &sanitized[local_idx],
                encoding,
                &config,
            );
            stats.sanitize_ms += node_stats.sanitize_ms;
            stats.chunk_ms += node_stats.chunk_ms;
            batch_chunks.extend(chunks);
            batch_metas.extend(metas);
        }

        all_chunks.extend(batch_chunks);
        all_metas.extend(batch_metas);
    }

    stats.total_chunks = all_chunks.len();

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

fn chunkify_tokens(
    node_idx: usize,
    node: &CodeNode,
    sanitized: &str,
    encoding: &Encoding,
    config: &ChunkerConfig,
) -> (Vec<TextChunk>, Vec<ChunkMeta>, NodeStats) {
    let start = Instant::now();

    let tokens = encoding.get_ids();
    let offsets = encoding.get_offsets();
    let max_tokens = config.max_tokens_per_text;

    let mut chunks = Vec::new();
    let mut metas = Vec::new();

    let mut start_idx = 0;
    let mut chunk_idx = 0;
    while start_idx < tokens.len() {
        let end_idx = (start_idx + max_tokens).min(tokens.len());
        let (start_byte, _) = offsets[start_idx];
        let (_, end_byte) = offsets[end_idx - 1];
        let end_byte = end_byte.min(sanitized.len());
        let slice = &sanitized[start_byte..end_byte];

        chunks.push(TextChunk {
            text: slice.to_string(),
            tokens: end_idx - start_idx,
        });
        metas.push(ChunkMeta {
            node_index: node_idx,
            chunk_index: chunk_idx,
            language: node.language.clone(),
            file_path: node.location.file_path.clone(),
            node_name: node.name.to_string(),
        });

        chunk_idx += 1;
        start_idx = end_idx;
    }

    let elapsed = start.elapsed().as_millis();
    (
        chunks,
        metas,
        NodeStats {
            sanitize_ms: 0,
            chunk_ms: elapsed,
        },
    )
}

struct NodeStats {
    sanitize_ms: u128,
    chunk_ms: u128,
}

struct TokenCounter;

impl TokenCounter {
    fn new(_tokenizer: Arc<Tokenizer>, _capacity: usize) -> Self {
        Self
    }

    fn stats(&self) -> (usize, usize) {
        (0, 0)
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
