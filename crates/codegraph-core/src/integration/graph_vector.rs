use crate::traits::{GraphStore, VectorStore};
use crate::{CodeGraphError, CodeNode, Language, NodeId, NodeType, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use memmap2::Mmap;
use std::collections::HashSet;
use std::fs::File;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Type alias for async embedding function
type EmbeddingFn =
    Arc<dyn Fn(CodeNode) -> Pin<Box<dyn Future<Output = Result<Vec<f32>>> + Send>> + Send + Sync>;

/// Embedding service abstraction used by the integrator.
///
/// This lives in `core` to avoid a dependency cycle on the `codegraph-vector` crate.
/// Callers can provide adapters to OpenAI or Candle-based providers from `codegraph-vector`.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Dimension of produced embeddings
    fn dimension(&self) -> usize;

    /// Generate an embedding for a single node
    async fn embed(&self, node: &CodeNode) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple nodes (default: sequential)
    async fn embed_batch(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(nodes.len());
        for n in nodes {
            out.push(self.embed(n).await?);
        }
        Ok(out)
    }
}

/// Deterministic, lightweight fallback embedder that uses a hash-based projection.
/// Useful for tests and environments without external providers.
pub struct HasherEmbeddingService {
    dim: usize,
}

impl HasherEmbeddingService {
    pub fn new(dimension: usize) -> Self {
        Self { dim: dimension }
    }
}

#[async_trait]
impl EmbeddingService for HasherEmbeddingService {
    fn dimension(&self) -> usize {
        self.dim
    }

    async fn embed(&self, node: &CodeNode) -> Result<Vec<f32>> {
        // Build a deterministic text from the node
        let mut text = String::new();
        if let Some(lang) = &node.language {
            text.push_str(&format!("{:?} ", lang));
        }
        if let Some(nt) = &node.node_type {
            text.push_str(&format!("{:?} ", nt));
        }
        text.push_str(&node.name);
        text.push(' ');
        if let Some(c) = &node.content {
            text.push_str(c.as_str());
        }
        // Truncate for safety
        if text.len() > 4096 {
            text.truncate(4096);
        }

        // Simple RNG based on djb2 hash, normalized to unit vector
        let mut hash: u32 = 5381;
        for b in text.as_bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(*b as u32);
        }
        let mut state = hash;
        let mut v = vec![0.0f32; self.dim];
        for val in v.iter_mut().take(self.dim) {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *val = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
        }
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Ok(v)
    }
}

/// Adapter to build an embedding service from an async function/closure.
pub struct FnEmbeddingService {
    dim: usize,
    func: EmbeddingFn,
}

impl FnEmbeddingService {
    pub fn new<F, Fut>(dimension: usize, f: F) -> Self
    where
        F: Fn(CodeNode) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<f32>>> + Send + 'static,
    {
        let func: EmbeddingFn = Arc::new(move |n: CodeNode| {
            let fut = f(n);
            Box::pin(fut)
        });
        Self {
            dim: dimension,
            func,
        }
    }
}

#[async_trait]
impl EmbeddingService for FnEmbeddingService {
    fn dimension(&self) -> usize {
        self.dim
    }
    async fn embed(&self, node: &CodeNode) -> Result<Vec<f32>> {
        (self.func)((*node).clone()).await
    }
    async fn embed_batch(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        // Simple concurrent batching
        let futures: Vec<_> = nodes.iter().cloned().map(|n| (self.func)(n)).collect();
        let results = futures::future::try_join_all(futures).await?;
        Ok(results)
    }
}

/// Extractor responsible for producing consistent, language-aware text snippets from graph nodes.
pub struct SnippetExtractor {
    // How many lines of context to include around the node location when reading from file
    pub context_lines: usize,
    // Max bytes to read from file to avoid large allocations
    pub max_read_bytes: usize,
}

impl Default for SnippetExtractor {
    fn default() -> Self {
        Self {
            context_lines: 40,
            max_read_bytes: 256 * 1024,
        }
    }
}

impl SnippetExtractor {
    pub fn extract(&self, node: &CodeNode) -> String {
        // Prefer in-node content if present
        if let Some(c) = &node.content {
            return self.compose_text(node, Some(c.as_str()));
        }
        // Fallback to file extraction based on location
        let path = Path::new(&node.location.file_path);
        match File::open(path).and_then(|f| unsafe { Mmap::map(&f) }) {
            Ok(mmap) => {
                let content = std::str::from_utf8(&mmap).unwrap_or("");
                let snippet = if let Some((start, end)) = self.window_around_location(content, node)
                {
                    content[start..end].to_string()
                } else {
                    // Entire file, truncated
                    content.chars().take(self.max_read_bytes / 2).collect()
                };
                self.compose_text(node, Some(&snippet))
            }
            Err(_) => self.compose_text(node, None),
        }
    }

    fn compose_text(&self, node: &CodeNode, body: Option<&str>) -> String {
        let lang = node
            .language
            .as_ref()
            .map(|l| format!("{:?}", l).to_lowercase())
            .unwrap_or_else(|| "unknown".into());
        let ntype = node
            .node_type
            .as_ref()
            .map(|t| format!("{:?}", t).to_lowercase())
            .unwrap_or_else(|| "unknown".into());
        let mut out = format!("{} {} {}\n", lang, ntype, node.name);
        if let Some(b) = body {
            out.push_str(b);
        }
        if out.len() > self.max_read_bytes {
            out.truncate(self.max_read_bytes);
        }
        out
    }

    fn window_around_location(&self, content: &str, node: &CodeNode) -> Option<(usize, usize)> {
        let line = node.location.line as usize;
        if line == 0 {
            return None;
        }
        let start_line = line.saturating_sub(self.context_lines);
        // If end_line is provided, extend by context; otherwise use current line then extend
        let base_end: usize = node.location.end_line.unwrap_or(line as u32) as usize;
        let end_line = base_end.saturating_add(self.context_lines);

        // Map line numbers to byte offsets
        let mut cur_line = 1usize;
        let mut start_idx = 0usize;
        let mut end_idx = content.len();
        for (idx, ch) in content.char_indices() {
            if cur_line == start_line {
                start_idx = idx;
            }
            if cur_line > end_line {
                end_idx = idx;
                break;
            }
            if ch == '\n' {
                cur_line += 1;
            }
        }
        Some((start_idx.min(content.len()), end_idx.min(content.len())))
    }
}

/// Maintains a vector index synced with the code graph and provides semantic search returning graph nodes.
pub struct GraphVectorIntegrator {
    graph: Arc<dyn GraphStore>,
    vector: Arc<Mutex<Box<dyn VectorStore + Send>>>,
    embedder: Arc<dyn EmbeddingService>,
    extractor: SnippetExtractor,
    // Track node signatures for incremental updates
    signatures: DashMap<NodeId, u64>,
}

impl GraphVectorIntegrator {
    pub fn new(
        graph: Arc<dyn GraphStore>,
        vector: Box<dyn VectorStore + Send>,
        embedder: Arc<dyn EmbeddingService>,
    ) -> Self {
        Self {
            graph,
            vector: Arc::new(Mutex::new(vector)),
            embedder,
            extractor: SnippetExtractor::default(),
            signatures: DashMap::with_capacity(64_000),
        }
    }

    pub fn with_extractor(mut self, extractor: SnippetExtractor) -> Self {
        self.extractor = extractor;
        self
    }

    /// Compute a stable signature of the node's embedding-relevant content for incremental updates.
    fn signature(&self, node: &CodeNode) -> u64 {
        let mut s = std::collections::hash_map::DefaultHasher::new();
        node.id.hash(&mut s);
        node.name.hash(&mut s);
        if let Some(t) = &node.node_type {
            t.hash(&mut s);
        }
        if let Some(l) = &node.language {
            l.hash(&mut s);
        }
        node.location.file_path.hash(&mut s);
        let snippet = self.extractor.extract(node);
        snippet.hash(&mut s);
        s.finish()
    }

    #[allow(dead_code)]
    pub(crate) fn signature_len(&self) -> usize {
        self.signatures.len()
    }

    /// Prepare nodes by ensuring `content` holds the extracted snippet text.
    fn prepare_nodes(&self, nodes: &[CodeNode]) -> Vec<CodeNode> {
        nodes
            .iter()
            .map(|n| {
                let mut c = n.clone();
                c.content = Some(self.extractor.extract(n).into());
                c
            })
            .collect()
    }

    /// Index embeddings for the provided nodes, skipping unchanged ones.
    /// Returns the number of nodes embedded and stored.
    pub async fn index_nodes(&self, nodes: &[CodeNode]) -> Result<usize> {
        if nodes.is_empty() {
            return Ok(0);
        }

        let prepared = self.prepare_nodes(nodes);

        // Filter to changed nodes only
        let mut changed = Vec::with_capacity(prepared.len());
        for n in prepared.into_iter() {
            let sig = self.signature(&n);
            match self.signatures.get(&n.id) {
                Some(prev) if *prev == sig => {} // unchanged
                _ => {
                    changed.push((n, sig));
                }
            }
        }

        if changed.is_empty() {
            return Ok(0);
        }

        let nodes_only: Vec<CodeNode> = changed.iter().map(|(n, _)| n.clone()).collect();
        let embeddings = self.embedder.embed_batch(&nodes_only).await?;
        if embeddings.len() != nodes_only.len() {
            return Err(CodeGraphError::Vector(
                "embedding batch size mismatch".into(),
            ));
        }

        // Attach embeddings to nodes
        let mut to_store: Vec<CodeNode> = Vec::with_capacity(nodes_only.len());
        for (n, emb) in nodes_only.into_iter().zip(embeddings.into_iter()) {
            let mut c = n.clone();
            c.embedding = Some(emb);
            to_store.push(c);
        }

        // Store in vector index
        {
            let mut vs = self.vector.lock().await;
            vs.store_embeddings(&to_store).await?;
        }

        // Update signatures map
        for (n, sig) in changed.into_iter() {
            self.signatures.insert(n.id, sig);
        }
        Ok(to_store.len())
    }

    /// Process graph updates: index new/modified nodes, drop signatures for deleted nodes.
    pub async fn sync_changes(
        &self,
        created_or_modified: &[CodeNode],
        deleted: &[NodeId],
    ) -> Result<(usize, usize)> {
        let added = self.index_nodes(created_or_modified).await?;
        for id in deleted {
            self.signatures.remove(id);
        }
        Ok((added, deleted.len()))
    }

    /// Semantic search by free-text query; returns graph nodes (if present in graph).
    pub async fn semantic_search_text(&self, query: &str, limit: usize) -> Result<Vec<CodeNode>> {
        // Build a synthetic node for query embedding
        let qnode = CodeNode {
            id: NodeId::nil(),
            name: "__query__".into(),
            node_type: Some(NodeType::Other("query".into())),
            language: Some(Language::Other("text".into())),
            location: crate::Location {
                file_path: "__query__".into(),
                line: 0,
                column: 0,
                end_line: None,
                end_column: None,
            },
            content: Some(query.into()),
            metadata: crate::Metadata {
                attributes: Default::default(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            embedding: None,
            complexity: None,
        };
        let qvec = self.embedder.embed(&qnode).await?;
        self.semantic_search_embedding(&qvec, limit).await
    }

    /// Semantic search by embedding; returns resolved nodes from the graph.
    pub async fn semantic_search_embedding(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> Result<Vec<CodeNode>> {
        if query_vec.len() != self.embedder.dimension() {
            return Err(CodeGraphError::Vector(format!(
                "Query vector dim {} != embedder dim {}",
                query_vec.len(),
                self.embedder.dimension()
            )));
        }
        let ids = {
            let vs = self.vector.lock().await;
            vs.search_similar(query_vec, limit.saturating_mul(3).max(limit + 8))
                .await?
        };
        // Resolve nodes from graph, dedupe, and truncate
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            if let Some(n) = self.graph.get_node(id).await? {
                out.push(n);
            }
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Language, Location, Metadata, NodeType};
    use std::collections::HashMap;
    use tokio_test::block_on;

    struct InMemoryGraph {
        nodes: DashMap<NodeId, CodeNode>,
    }

    #[async_trait]
    impl GraphStore for InMemoryGraph {
        async fn add_node(&mut self, node: CodeNode) -> Result<()> {
            self.nodes.insert(node.id, node);
            Ok(())
        }
        async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
            Ok(self.nodes.get(&id).map(|e| e.clone()))
        }
        async fn update_node(&mut self, node: CodeNode) -> Result<()> {
            self.nodes.insert(node.id, node);
            Ok(())
        }
        async fn remove_node(&mut self, id: NodeId) -> Result<()> {
            self.nodes.remove(&id);
            Ok(())
        }
        async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
            Ok(self
                .nodes
                .iter()
                .filter(|e| e.name.as_str() == name)
                .map(|e| e.clone())
                .collect())
        }
    }

    struct InMemoryVectorStore {
        dim: usize,
        // NodeId -> embedding
        embs: DashMap<NodeId, Vec<f32>>,
    }

    #[async_trait]
    impl VectorStore for InMemoryVectorStore {
        async fn store_embeddings(&mut self, nodes: &[CodeNode]) -> Result<()> {
            for n in nodes {
                if let Some(e) = &n.embedding {
                    self.embs.insert(n.id, e.clone());
                }
            }
            Ok(())
        }
        async fn search_similar(
            &self,
            query_embedding: &[f32],
            limit: usize,
        ) -> Result<Vec<NodeId>> {
            let mut sims: Vec<(NodeId, f32)> = self
                .embs
                .iter()
                .map(|kv| {
                    let s = cosine(&kv.value(), query_embedding);
                    (*kv.key(), s)
                })
                .collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            Ok(sims.into_iter().take(limit).map(|(id, _)| id).collect())
        }
        async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
            Ok(self.embs.get(&node_id).map(|e| e.clone()))
        }
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }

    fn make_node(name: &str, lang: Language, t: NodeType, content: &str) -> CodeNode {
        let now = chrono::Utc::now();
        CodeNode {
            id: NodeId::new_v4(),
            name: name.into(),
            node_type: Some(t),
            language: Some(lang),
            location: Location {
                file_path: "mem".into(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
            content: Some(content.into()),
            metadata: Metadata {
                attributes: HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            embedding: None,
            complexity: None,
        }
    }

    #[test]
    fn index_and_search_memory() {
        block_on(async {
            let graph = Arc::new(InMemoryGraph {
                nodes: DashMap::new(),
            });
            let vstore = InMemoryVectorStore {
                dim: 384,
                embs: DashMap::new(),
            };
            let embedder = Arc::new(HasherEmbeddingService::new(384));
            let integrator = GraphVectorIntegrator::new(graph.clone(), Box::new(vstore), embedder);

            // Build sample nodes
            let a = make_node(
                "sum",
                Language::Rust,
                NodeType::Function,
                "fn sum(a: i32, b: i32) -> i32 { a + b }",
            );
            let b = make_node(
                "add",
                Language::Rust,
                NodeType::Function,
                "fn add(x: i32, y: i32) -> i32 { x + y }",
            );
            let c = make_node(
                "read_file",
                Language::Rust,
                NodeType::Function,
                "fn read_file(p: &str) -> String { std::fs::read_to_string(p).unwrap() }",
            );
            graph.nodes.insert(a.id, a.clone());
            graph.nodes.insert(b.id, b.clone());
            graph.nodes.insert(c.id, c.clone());

            // Index nodes
            let n = integrator
                .index_nodes(&[a.clone(), b.clone(), c.clone()])
                .await
                .unwrap();
            assert_eq!(n, 3);

            // Search by text
            let results = integrator
                .semantic_search_text("sum two numbers", 2)
                .await
                .unwrap();
            assert!(!results.is_empty());

            // Incremental: re-index with unchanged nodes should skip
            let n2 = integrator
                .index_nodes(&[a.clone(), b.clone(), c.clone()])
                .await
                .unwrap();
            assert_eq!(n2, 0);
        });
    }
}
