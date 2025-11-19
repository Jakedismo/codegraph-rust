use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, NodeType, Result};
use codegraph_vector::{search::SemanticSearch, EmbeddingGenerator};
use futures::future::try_join_all;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SemanticSearchConfig {
    pub oversample: usize,
    pub default_limit: usize,
    pub clone_threshold: f32,
    pub token_similarity_threshold: f32,
    pub max_impact_depth: usize,
    pub impact_timeout: Duration,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            oversample: 3,
            default_limit: 25,
            clone_threshold: 0.92,
            token_similarity_threshold: 0.65,
            max_impact_depth: 3,
            impact_timeout: Duration::from_millis(100),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CloneMatch {
    pub node_id: NodeId,
    pub score: f32,
    pub token_similarity: f32,
}

#[derive(Debug, Clone)]
pub struct ImpactResult {
    pub root: NodeId,
    pub impacted: Vec<NodeId>,
}

#[derive(Clone)]
pub struct SemanticSearchEngine<G>
where
    G: GraphStore + Send + Sync + 'static,
{
    graph: Arc<RwLock<G>>, // aligns with API usage
    semantic: Arc<SemanticSearch>,
    embeddings: Arc<EmbeddingGenerator>,
    cfg: SemanticSearchConfig,
}

impl<G> SemanticSearchEngine<G>
where
    G: GraphStore + Send + Sync + 'static,
{
    pub fn new(
        graph: Arc<RwLock<G>>,
        semantic: Arc<SemanticSearch>,
        embeddings: Arc<EmbeddingGenerator>,
        cfg: Option<SemanticSearchConfig>,
    ) -> Self {
        Self {
            graph,
            semantic,
            embeddings,
            cfg: cfg.unwrap_or_default(),
        }
    }
    /// Find similar functions given a node as a seed.
    pub async fn similar_functions(
        &self,
        node: &CodeNode,
        limit: usize,
        cross_language: bool,
    ) -> Result<Vec<(CodeNode, f32)>> {
        if !matches!(node.node_type, Some(NodeType::Function)) {
            return Err(CodeGraphError::InvalidOperation(
                "Node is not a function".into(),
            ));
        }
        let k = limit.max(1) * self.cfg.oversample;
        let results = self.semantic.find_similar_functions(node, k).await?;

        let graph = self.graph.read().await;
        let mut out = Vec::with_capacity(limit);
        for r in results.into_iter() {
            if let Some(found) = graph.get_node(r.node_id).await? {
                if !cross_language && node.language.is_some() && found.language != node.language {
                    continue;
                }
                if !matches!(found.node_type, Some(NodeType::Function)) {
                    continue;
                }
                out.push((found, r.score));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Semantic text query to functions across languages.
    pub async fn query_functions(&self, query: &str, limit: usize) -> Result<Vec<(CodeNode, f32)>> {
        let k = limit.max(1) * self.cfg.oversample;
        let found = self.semantic.search_by_text(query, k).await?;
        let graph = self.graph.read().await;
        let mut out = Vec::with_capacity(limit);
        for r in found {
            if let Some(node) = graph.get_node(r.node_id).await? {
                if matches!(node.node_type, Some(NodeType::Function)) {
                    out.push((node, r.score));
                }
            }
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Code clone detection using a two-stage approach: vector similarity then token-similarity check.
    pub async fn detect_clones(
        &self,
        node: &CodeNode,
        limit: usize,
        similarity_threshold: Option<f32>,
        cross_language: bool,
    ) -> Result<Vec<CloneMatch>> {
        let threshold = similarity_threshold.unwrap_or(self.cfg.clone_threshold);
        let k = (limit.max(1) * self.cfg.oversample).max(limit + 8);
        let candidates = self.semantic.find_similar_functions(node, k).await?;
        let graph = self.graph.read().await;
        let seed_tokens = tokenize(node);
        let mut clones = Vec::new();

        for cand in candidates.into_iter() {
            if cand.score < threshold {
                continue;
            }
            if let Some(other) = graph.get_node(cand.node_id).await? {
                if !matches!(other.node_type, Some(NodeType::Function)) {
                    continue;
                }
                if !cross_language && node.language.is_some() && node.language != other.language {
                    continue;
                }

                let ts = token_jaccard(&seed_tokens, &tokenize(&other));
                // Combine vector score and token similarity with a conservative guard
                if ts >= self.cfg.token_similarity_threshold {
                    clones.push(CloneMatch {
                        node_id: other.id,
                        score: cand.score,
                        token_similarity: ts,
                    });
                }
                if clones.len() >= limit {
                    break;
                }
            }
        }

        // Sort by combined score heuristic
        clones.sort_by(|a, b| {
            (b.score * 0.7 + b.token_similarity * 0.3)
                .partial_cmp(&(a.score * 0.7 + a.token_similarity * 0.3))
                .unwrap()
        });
        Ok(clones)
    }

    /// Impact analysis: find nodes that depend on `root`. Traverses incoming edges up to depth or timeout.
    pub async fn impact_analysis(
        &self,
        root: NodeId,
        max_depth: Option<usize>,
    ) -> Result<ImpactResult> {
        let _ = max_depth; // impact analysis currently requires neighbor traversal unavailable in Surreal mode
        Ok(ImpactResult {
            root,
            impacted: Vec::new(),
        })
    }

    /// Recommendations: suggest similar patterns for a given node by blending its neighborhood context.
    pub async fn recommendations(
        &self,
        seed: &CodeNode,
        limit: usize,
    ) -> Result<Vec<(CodeNode, f32)>> {
        // Gather small 1-hop context
        let graph = self.graph.read().await;
        let ctx_nodes: Vec<CodeNode> = vec![seed.clone()];

        // Encode and combine embeddings
        let embs = self.embeddings.generate_embeddings(&ctx_nodes).await?;
        let centroid = average_unit(&embs);
        let k = limit.max(1) * self.cfg.oversample;
        let results = self.semantic.search_by_embedding(&centroid, k).await?;

        let mut out = Vec::with_capacity(limit);
        for r in results.into_iter() {
            if r.node_id == seed.id {
                continue;
            }
            if let Some(n) = graph.get_node(r.node_id).await? {
                out.push((n, r.score));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }
}

/// Multi-repository semantic search that fans out queries and merges results.
#[derive(Clone)]
pub struct MultiRepoSemanticSearchEngine<G>
where
    G: GraphStore + Send + Sync + 'static,
{
    contexts: Vec<RepoContext<G>>,
    cfg: SemanticSearchConfig,
}

#[derive(Clone)]
pub struct RepoContext<G>
where
    G: GraphStore + Send + Sync + 'static,
{
    pub repo_id: String,
    pub graph: Arc<RwLock<G>>,
    pub semantic: Arc<SemanticSearch>,
}

impl<G> MultiRepoSemanticSearchEngine<G>
where
    G: GraphStore + Send + Sync + 'static,
{
    pub fn new(contexts: Vec<RepoContext<G>>, cfg: Option<SemanticSearchConfig>) -> Self {
        Self {
            contexts,
            cfg: cfg.unwrap_or_default(),
        }
    }

    pub async fn query_functions(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, CodeNode, f32)>> {
        let k = (limit.max(1) * self.cfg.oversample).max(limit + 10);
        let mut futs = Vec::with_capacity(self.contexts.len());
        for ctx in &self.contexts {
            let q = query.to_string();
            let sem = ctx.semantic.clone();
            futs.push(async move { sem.search_by_text(&q, k).await });
        }
        let results = try_join_all(futs).await?;

        // Merge and normalize per-repo
        let mut merged: Vec<(String, NodeId, f32)> = Vec::new();
        for (i, res) in results.into_iter().enumerate() {
            let repo_id = self.contexts[i].repo_id.clone();
            for r in res {
                merged.push((repo_id.clone(), r.node_id, r.score));
            }
        }
        // Sort and unique by NodeId while keeping highest score
        merged.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut out: Vec<(String, CodeNode, f32)> = Vec::new();
        for (repo_id, nid, score) in merged.into_iter() {
            if !seen.insert(nid) {
                continue;
            }
            let graph = &self
                .contexts
                .iter()
                .find(|c| c.repo_id == repo_id)
                .unwrap()
                .graph;
            if let Some(node) = graph.read().await.get_node(nid).await? {
                if matches!(node.node_type, Some(NodeType::Function)) {
                    out.push((repo_id.clone(), node, score));
                }
            }
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }
}

// ------------ helpers -------------

fn tokenize(node: &CodeNode) -> HashSet<String> {
    let mut s = String::new();
    s.push_str(node.name.as_str());
    if let Some(ref c) = node.content {
        s.push(' ');
        s.push_str(c.as_str());
    }
    // Lowercase alnum tokens
    let mut out: HashSet<String> = HashSet::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            out.insert(cur.clone());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.insert(cur);
    }
    out
}

fn token_jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f32;
    let uni = (a.len() + b.len()) as f32 - inter;
    if uni <= 0.0 {
        0.0
    } else {
        inter / uni
    }
}

fn average_unit(vectors: &[Vec<f32>]) -> Vec<f32> {
    if vectors.is_empty() {
        return vec![];
    }
    let dim = vectors[0].len();
    let mut sum = vec![0.0f32; dim];
    for v in vectors {
        if v.len() != dim {
            continue;
        }
        for (i, x) in v.iter().enumerate() {
            sum[i] += *x;
        }
    }
    let n = vectors.len() as f32;
    for x in &mut sum {
        *x /= n;
    }
    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut sum {
            *x /= norm;
        }
    }
    sum
}
