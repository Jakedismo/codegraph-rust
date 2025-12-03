use codegraph_core::{CodeNode, Language};
use semchunk_rs::Chunker as SemanticChunker;
use std::{sync::Arc, time::Instant};
use tokenizers::Tokenizer;
use unicode_normalization::UnicodeNormalization;

const DEFAULT_MAX_TEXTS_PER_REQUEST: usize = 256;
const DEFAULT_OVERLAP_TOKENS: usize = 64;

/// Configuration knobs for the fast chunker.
#[derive(Clone)]
pub struct ChunkerConfig {
    pub max_tokens_per_text: usize,
    pub sanitize_mode: SanitizeMode,
    pub cache_capacity: usize,
    pub max_texts_per_request: usize,
    pub overlap_tokens: usize,
    pub smart_split: bool,
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
    let _ = config.max_texts_per_request;
    let _ = config.cache_capacity;

    // Upper bound estimate: assume ~2 chunks per node as a guard
    let estimate = nodes.len().saturating_mul(2).max(16);
    let mut all_chunks = Vec::with_capacity(estimate);
    let mut all_metas = Vec::with_capacity(estimate);
    let mut stats = ChunkStats::empty();
    stats.total_nodes = nodes.len();

    for (node_idx, node) in nodes.iter().enumerate() {
        let sanitized = sanitize(node, config.sanitize_mode);
        let segments: Vec<String> = if config.smart_split {
            smart_split(&sanitized)
        } else {
            vec![sanitized.clone()]
        };
        let chunker = SemanticChunker::new(
            config.max_tokens_per_text,
            Box::new({
                let tok = tokenizer.clone();
                move |s: &str| count_tokens(&tok, s)
            }),
        );

        let mut raw_chunks = Vec::new();
        for segment in segments {
            raw_chunks.extend(chunker.chunk(&segment));
        }
        let mut overlap_tail: Option<String> = None;
        let mut chunk_idx = 0;

        for chunk_text in raw_chunks {
            let mut text = chunk_text;

            if let Some(tail) = &overlap_tail {
                if config.overlap_tokens > 0 {
                    // Prepend overlap tail if within budget
                    let candidate = format!("{}{}", tail, text);
                    if count_tokens(&tokenizer, &candidate) <= config.max_tokens_per_text {
                        text = candidate;
                    }
                }
            }

            let tokens = count_tokens(&tokenizer, &text);
            all_chunks.push(TextChunk {
                text: text.clone(),
                tokens,
            });
            all_metas.push(ChunkMeta {
                node_index: node_idx,
                chunk_index: chunk_idx,
                language: node.language.clone(),
                file_path: node.location.file_path.clone(),
                node_name: node.name.to_string(),
            });

            chunk_idx += 1;

            // Capture tail for next chunk (approximate overlap using chars, fallback to tokens)
            if config.overlap_tokens > 0 {
                let approx_chars = config.overlap_tokens * 4;
                let tail_str = if text.len() > approx_chars {
                    text[text.len().saturating_sub(approx_chars)..].to_string()
                } else {
                    text.clone()
                };
                overlap_tail = Some(tail_str);
            }
        }

        stats.chunk_ms += 0; // semchunk internally does the work; keep zeroed to avoid misleading metrics
    }

    stats.total_chunks = all_chunks.len();

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

fn count_tokens(tokenizer: &Tokenizer, text: &str) -> usize {
    tokenizer
        .encode(text, false)
        .map(|e| e.get_ids().len())
        .unwrap_or_else(|_| (text.len() + 3) / 4)
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
