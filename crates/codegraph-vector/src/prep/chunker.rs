// ABOUTME: Parallel, deterministic chunk planning with Unicode-safe token budgets.
use codegraph_core::{CodeGraphError, CodeNode, Language, Result};
use rayon::prelude::*;
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
    pub skip_chunking: bool,
    pub cache_dir: Option<std::path::PathBuf>,
    pub use_text_splitter: bool,
    pub token_counter: Option<(String, TokenCounter)>,
    pub overlap_counter: Option<(String, TokenCounter)>,
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
            skip_chunking: std::env::var("CODEGRAPH_EMBEDDING_SKIP_CHUNKING")
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            cache_dir: None,
            token_counter: None,
            overlap_counter: None,
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
        self.max_texts_per_request = max.max(1);
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

    pub fn skip_chunking(mut self, enabled: bool) -> Self {
        self.skip_chunking = enabled;
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
) -> Result<ChunkPlan> {
    build_chunk_plan_with_sources(nodes, &std::collections::HashMap::new(), tokenizer, config)
}

/// Span-aware variant: uses file_sources to slice exact spans when available.
pub fn build_chunk_plan_with_sources(
    nodes: &[CodeNode],
    file_sources: &std::collections::HashMap<String, String>,
    tokenizer: Arc<Tokenizer>,
    config: ChunkerConfig,
) -> Result<ChunkPlan> {
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
) -> Result<ChunkPlan> {
    if config.max_tokens_per_text == 0 {
        return Err(CodeGraphError::Vector(
            "Chunk token limit must be positive".into(),
        ));
    }
    let mut tokenizer = tokenizer.as_ref().clone();
    tokenizer
        .with_truncation(None)
        .map_err(|error| CodeGraphError::Vector(error.to_string()))?;
    tokenizer.with_padding(None);
    let tokenizer = Arc::new(tokenizer);
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
    let overlap_count = config
        .overlap_counter
        .as_ref()
        .map(|counter| counter.1.clone())
        .unwrap_or_else(|| {
            let tokenizer = tokenizer.clone();
            Arc::new(move |text| {
                tokenizer
                    .encode(text, false)
                    .map_or(usize::MAX, |tokens| tokens.len())
            })
        });
    let artifacts = config
        .cache_dir
        .clone()
        .map(|path| path.into_os_string())
        .map(|root| codegraph_core::artifact_cache::ArtifactCache::new(root, "chunks-v3"));
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
            let overlap_count = &overlap_count;
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
                    "chunks-v3",
                    &tokenizer_key,
                    &sanitized,
                    config.max_tokens_per_text,
                    config.overlap_tokens,
                    config.smart_split,
                    config.skip_chunking,
                    config.use_text_splitter,
                    &node.language,
                    config.overlap_counter.as_ref().map(|counter| &counter.0),
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
                if cached.is_none() {
                    let chunks = prepare_chunks(
                        &sanitized,
                        node.language.as_ref(),
                        config,
                        count,
                        overlap_count,
                    )
                    .map_err(|error| {
                        CodeGraphError::Vector(format!(
                            "{}:{} node '{}': {error}",
                            node.location.file_path, node.location.line, node.name
                        ))
                    })?;
                    if let Some(cache) = &artifacts
                        && let Err(error) = cache.put(&key, &chunks)
                    {
                        tracing::debug!("Chunk cache write failed: {error}");
                    }
                    *cached = Some(chunks);
                }
                let chunks = cached.as_ref().expect("prepared chunks").clone();
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
                Ok((
                    node_idx,
                    chunks,
                    metas,
                    sanitize_ms,
                    chunk_start.elapsed().as_millis(),
                    hit,
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
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
    Ok(ChunkPlan {
        chunks: all_chunks,
        metas: all_metas,
        stats,
    })
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
        .encode(text, true)
        .map(|e| e.get_ids().len())
        .unwrap_or(usize::MAX)
}

fn prepare_chunks(
    text: &str,
    language: Option<&Language>,
    config: &ChunkerConfig,
    count: &TokenCounter,
    overlap_count: &TokenCounter,
) -> Result<Vec<TextChunk>> {
    let limit = config.max_tokens_per_text;
    // Fit first: do not even parse syntax for units that fit the complete input budget.
    let tokens = count(text);
    if tokens <= limit {
        return Ok(vec![TextChunk {
            text: text.to_owned(),
            tokens,
        }]);
    }
    if config.skip_chunking {
        return Err(CodeGraphError::Vector(format!(
            "Chunking is disabled but this node exceeds the {limit}-token input limit (including prefixes/special tokens). Enable chunking or select a larger serving context; no text was truncated."
        )));
    }
    let mut parts = Vec::new();
    if config.use_text_splitter {
        struct Sizer(TokenCounter);
        impl text_splitter::ChunkSizer for Sizer {
            fn size(&self, text: &str) -> usize {
                self.0(text)
            }
        }
        let splitter = text_splitter::TextSplitter::new(
            text_splitter::ChunkConfig::new(limit)
                .with_sizer(Sizer(count.clone()))
                .with_trim(false),
        );
        for text in splitter.chunks(text) {
            split_with_counter(text, count.as_ref(), limit, &mut parts)?;
        }
    } else if config.smart_split {
        let boundaries = language
            .map(|language| codegraph_parser::chunk_boundaries::syntax_boundaries(text, language))
            .unwrap_or_default();
        split_at_syntax(
            text,
            0,
            text.len(),
            &boundaries,
            count.as_ref(),
            limit,
            &mut parts,
        )?;
    } else {
        split_with_counter(text, count.as_ref(), limit, &mut parts)?;
    }
    // Merge adjacent leaves while they fit: syntax boundaries are candidates,
    // never a reason to emit many tiny embeddings. Concatenation preserves whitespace.
    let mut merged: Vec<String> = Vec::new();
    for part in parts {
        if let Some(previous) = merged.last_mut() {
            let candidate = format!("{previous}{part}");
            if count(&candidate) <= limit {
                *previous = candidate;
                continue;
            }
        }
        merged.push(part);
    }
    let mut chunks: Vec<TextChunk> = Vec::with_capacity(merged.len());
    let mut previous_base = String::new();
    for base in merged {
        let tail = if config.overlap_tokens > 0 && !previous_base.is_empty() {
            token_tail(
                &previous_base,
                config.overlap_tokens,
                overlap_count.as_ref(),
                |tail| count(&format!("{tail}{base}")) <= limit,
            )
        } else {
            ""
        };
        let text = format!("{tail}{base}");
        let tokens = count(&text);
        if tokens > limit {
            return Err(CodeGraphError::Vector(
                "Chunk tokenization exceeds the input budget".into(),
            ));
        }
        chunks.push(TextChunk { text, tokens });
        previous_base = base;
    }
    Ok(chunks)
}

fn split_at_syntax(
    text: &str,
    start: usize,
    end: usize,
    boundaries: &[codegraph_parser::chunk_boundaries::SyntaxBoundary],
    count: &dyn Fn(&str) -> usize,
    limit: usize,
    output: &mut Vec<String>,
) -> Result<()> {
    let slice = &text[start..end];
    if count(slice) <= limit {
        output.push(slice.to_owned());
        return Ok(());
    }
    let midpoint = start + (end - start) / 2;
    // Coarse blocks first; avoid repeatedly peeling off small headers or braces.
    let cut = boundaries
        .iter()
        .filter(|b| b.offset > start + (end - start) / 4 && b.offset < end - (end - start) / 4)
        .min_by_key(|b| (b.depth, b.offset.abs_diff(midpoint)))
        .map(|b| b.offset);
    if let Some(cut) = cut {
        split_at_syntax(text, start, cut, boundaries, count, limit, output)?;
        split_at_syntax(text, cut, end, boundaries, count, limit, output)?;
    } else {
        // Unsupported syntax/oversized leaves: preserve lines where possible, then
        // use UTF-8 cuts. Braces in strings and comments are never syntax markers.
        if let Some(cut) = slice
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(index, _)| index + 1)
            .filter(|index| *index > slice.len() / 4 && *index < slice.len() - slice.len() / 4)
            .min_by_key(|index| index.abs_diff(slice.len() / 2))
        {
            split_at_syntax(text, start, start + cut, boundaries, count, limit, output)?;
            split_at_syntax(text, start + cut, end, boundaries, count, limit, output)?;
        } else {
            split_with_counter(slice, count, limit, output)?;
        }
    }
    Ok(())
}

fn split_with_counter(
    text: &str,
    count: &dyn Fn(&str) -> usize,
    limit: usize,
    output: &mut Vec<String>,
) -> Result<()> {
    if count(text) <= limit {
        output.push(text.to_owned());
        return Ok(());
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
        return Err(CodeGraphError::Vector("A single Unicode character or input prefix exceeds the token budget; increase CODEGRAPH_CHUNK_MAX_TOKENS or correct the provider tokenizer".into()));
    }
    split_with_counter(&text[..midpoint], count, limit, output)?;
    split_with_counter(&text[midpoint..], count, limit, output)
}

// Find a token-counted UTF-8 suffix. Recheck both suffix and complete request:
// BPE boundaries can change when overlap is concatenated with the next chunk.
fn token_tail<'a>(
    text: &'a str,
    limit: usize,
    count: &dyn Fn(&str) -> usize,
    fits: impl Fn(&str) -> bool,
) -> &'a str {
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut low = 0;
    let mut high = offsets.len() - 1;
    while low < high {
        let mid = low + (high - low) / 2;
        let tail = &text[offsets[mid]..];
        if count(tail) <= limit && fits(tail) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    let tail = &text[offsets[low]..];
    if count(tail) <= limit && fits(tail) {
        tail
    } else {
        ""
    }
}

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
    fn tokenizer() -> Arc<Tokenizer> {
        Arc::new(
            Tokenizer::from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tokenizers/qwen2.5-coder.json"
            ))
            .unwrap(),
        )
    }
    fn node(text: &str, language: Language) -> CodeNode {
        CodeNode::new(
            "unit",
            Some(codegraph_core::NodeType::Function),
            Some(language),
            codegraph_core::Location {
                file_path: "fixture.rs".into(),
                line: 1,
                column: 0,
                end_line: None,
                end_column: None,
            },
        )
        .with_content(text.to_owned())
    }
    fn byte_budget(limit: usize) -> ChunkerConfig {
        let mut config = ChunkerConfig::new(limit)
            .overlap_tokens(0)
            .skip_chunking(false);
        config.token_counter = Some(("byte-budget".into(), Arc::new(str::len)));
        config
    }
    #[test]
    fn fitting_ast_unit_stays_intact_including_blank_lines_and_braces() {
        let text = "fn café() {\n\tlet a = \"}; 🦀\";\n\n\tif ready {\n\t\trun();\n\t}\n}\n";
        let plan = build_chunk_plan(
            &[node(text, Language::Rust)],
            tokenizer(),
            byte_budget(4096),
        )
        .unwrap();
        assert_eq!(plan.chunks.len(), 1);
        assert_eq!(plan.chunks[0].text, text);
    }
    #[test]
    fn oversized_units_cut_at_ast_boundaries_and_preserve_every_source_byte() {
        for (language, text) in [
            (
                Language::Rust,
                format!(
                    "fn café() {{\n{}\n}}\n",
                    (0..60)
                        .map(|i| format!("\tlet value_{i} = \"}}; café 🦀\";\n"))
                        .collect::<String>()
                ),
            ),
            (
                Language::Python,
                format!(
                    "def café():\n{}\n",
                    (0..60)
                        .map(|i| format!("    value_{i} = '}}; café 🦀'\n"))
                        .collect::<String>()
                ),
            ),
        ] {
            let cuts = codegraph_parser::chunk_boundaries::syntax_boundaries(&text, &language);
            let plan =
                build_chunk_plan(&[node(&text, language)], tokenizer(), byte_budget(96)).unwrap();
            assert!(plan.chunks.len() > 1);
            assert!(plan.chunks.iter().all(|chunk| chunk.tokens <= 96));
            assert_eq!(
                plan.chunks
                    .iter()
                    .map(|chunk| chunk.text.as_str())
                    .collect::<String>(),
                text
            );
            let mut offset = 0;
            for chunk in plan.chunks.iter().take(plan.chunks.len() - 1) {
                offset += chunk.text.len();
                assert!(
                    cuts.iter().any(|cut| cut.offset == offset),
                    "non-AST cut at {offset}"
                );
            }
            assert!(
                plan.chunks
                    .iter()
                    .any(|chunk| chunk.text.contains("}; café 🦀"))
            );
        }
    }
    #[test]
    fn skip_chunking_rejects_oversized_units_even_with_a_warm_split_cache() {
        let root = tempfile::tempdir().unwrap();
        let text = "fn f() {\n let a = 42;\n}\n".repeat(30);
        let nodes = [node(&text, Language::Rust)];
        let config = byte_budget(64).cache_dir(Some(root.path().into()));
        let plan = build_chunk_plan(&nodes, tokenizer(), config.clone()).unwrap();
        assert!(plan.chunks.len() > 1);
        let error = match build_chunk_plan(&nodes, tokenizer(), config.skip_chunking(true)) {
            Ok(_) => panic!("oversized skipped unit must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("Chunking is disabled"));
        assert_eq!(
            build_chunk_plan(&nodes, tokenizer(), byte_budget(4096).skip_chunking(true))
                .unwrap()
                .chunks
                .len(),
            1
        );
    }
    #[test]
    fn token_counted_overlap_preserves_unicode_and_rechecks_combined_budget() {
        let text = "a café 🦀 計算";
        let count = |text: &str| text.chars().count();
        assert_eq!(token_tail(text, 3, &count, |_| true), " 計算");
        assert_eq!(token_tail(text, 3, &count, |tail| tail.len() <= 6), "計算");
        assert_eq!(token_tail(text, 0, &count, |_| true), "");
        let mut config = byte_budget(90);
        config.overlap_tokens = 12;
        config.overlap_counter = Some(("characters".into(), Arc::new(|text| text.chars().count())));
        let source = "fn café() {\n let name = \"🚀 計算\";\n}\n".repeat(30);
        let plan = build_chunk_plan(&[node(&source, Language::Rust)], tokenizer(), config).unwrap();
        assert!(plan.chunks.iter().all(|chunk| chunk.tokens <= 90));
        // Consume only the new suffix of each chunk to reconstruct the source.
        let mut consumed = 0;
        let mut overlaps = 0;
        for chunk in plan.chunks {
            let remaining = &source[consumed..];
            let cut = chunk
                .text
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(chunk.text.len()))
                .find(|index| {
                    !chunk.text[*index..].is_empty()
                        && remaining.starts_with(&chunk.text[*index..])
                        && source[..consumed].ends_with(&chunk.text[..*index])
                })
                .unwrap();
            overlaps += usize::from(cut > 0);
            consumed += chunk.text.len() - cut;
        }
        assert_eq!(consumed, source.len());
        assert!(overlaps > 0);
    }
    #[test]
    fn truncating_tokenizers_and_indivisible_characters_cannot_bypass_the_budget() {
        let mut tokenizer = tokenizer().as_ref().clone();
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 8,
                ..Default::default()
            }))
            .unwrap();
        let text = "let a = \"🚀 café\"; ".repeat(60);
        let plan = build_chunk_plan(
            &[node(&text, Language::Rust)],
            Arc::new(tokenizer),
            ChunkerConfig::new(32)
                .overlap_tokens(0)
                .skip_chunking(false),
        )
        .unwrap();
        assert!(plan.chunks.len() > 1);
        assert_eq!(
            plan.chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            text
        );
        assert!(
            build_chunk_plan(
                &[node("🚀", Language::Rust)],
                self::tokenizer(),
                byte_budget(1)
            )
            .is_err()
        );
        assert!(build_chunk_plan(&[], self::tokenizer(), byte_budget(0)).is_err());
    }
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
        let plan = build_chunk_plan(&[node], tokenizer, config).unwrap();
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
        )
        .unwrap();
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
