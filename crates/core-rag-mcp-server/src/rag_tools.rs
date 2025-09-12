use crate::{CoreRagError, CoreRagResult, CoreRagServerConfig};
use chrono::{DateTime, Utc};
use codegraph_core::*;
use codegraph_vector::EmbeddingGenerator;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory index of parsed nodes and their embeddings
#[derive(Default)]
struct RagIndex {
    nodes: HashMap<NodeId, CodeNode>,
    embeddings: HashMap<NodeId, Vec<f32>>,
    by_file: HashMap<String, Vec<NodeId>>, // file path -> node ids
    calls_out: HashMap<NodeId, Vec<NodeId>>, // caller -> callees
    calls_in: HashMap<NodeId, Vec<NodeId>>,  // callee -> callers
}

/// RAG tools providing CodeGraph functionality backed by parser + embeddings
#[derive(Clone)]
pub struct RagTools {
    config: CoreRagServerConfig,
    project_root: PathBuf,
    index: Arc<RwLock<Option<RagIndex>>>,
    embedder: Arc<EmbeddingGenerator>,
}

/// Code search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResult {
    pub id: String,
    pub name: String,
    pub path: String,
    pub node_type: String,
    pub content: String,
    pub score: f32,
    pub language: Option<String>,
}

/// Detailed code information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDetails {
    pub id: String,
    pub name: String,
    pub path: String,
    pub node_type: String,
    pub content: String,
    pub language: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub dependencies: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Relationship analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipAnalysis {
    pub dependencies: Vec<RelatedNode>,
    pub dependents: Vec<RelatedNode>,
    pub related: Vec<RelatedNode>,
}

/// Related node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub relationship_type: String,
    pub score: f32,
}

/// Repository statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    pub total_nodes: u64,
    pub file_count: u64,
    pub function_count: u64,
    pub class_count: u64,
    pub module_count: u64,
    pub test_file_count: u64,
    pub languages: HashMap<String, u64>,
    pub last_updated: DateTime<Utc>,
    pub recent_changes: u64,
}

/// Semantic search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub id: String,
    pub title: String,
    pub path: String,
    pub context: String,
    pub relevance_score: f32,
    pub node_type: String,
}

impl RagTools {
    /// Create new RAG tools instance (index is built lazily on first use)
    pub fn new(config: CoreRagServerConfig) -> CoreRagResult<Self> {
        config.validate()?;

        // Determine project root: CORE_RAG_PROJECT_DIR or database_path if it is a dir, else CWD
        let project_root = std::env::var("CORE_RAG_PROJECT_DIR")
            .map(PathBuf::from)
            .ok()
            .or_else(|| {
                if config.database_path.is_dir() {
                    Some(config.database_path.clone())
                } else {
                    None
                }
            })
            .unwrap_or(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let embedder = Arc::new(EmbeddingGenerator::default());

        Ok(Self {
            config,
            project_root,
            index: Arc::new(RwLock::new(None)),
            embedder,
        })
    }

    async fn ensure_index_built(&self) -> CoreRagResult<()> {
        {
            let guard = self.index.read().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        // Build index: parse project and embed nodes
        let parser = codegraph_parser::parser::TreeSitterParser::new();
        let (mut nodes, _stats) = parser
            .parse_directory_parallel(self.project_root.to_string_lossy().as_ref())
            .await
            .map_err(|e| CoreRagError::parser(format!("Parse failed: {}", e)))?;

        // Generate embeddings and populate index
        let mut index = RagIndex::default();
        for node in nodes.drain(..) {
            let id = node.id;
            let path = node.location.file_path.clone();
            let embedding = self
                .embedder
                .generate_embedding(&node)
                .await
                .map_err(|e| CoreRagError::vector_search(format!("Embedding failed: {}", e)))?;
            index.embeddings.insert(id, embedding);
            index.by_file.entry(path).or_default().push(id);
            index.nodes.insert(id, node);
        }

        // Hybrid call-graph: prefer parser pipeline edges (for supported languages),
        // then fall back to regex name matching for the rest.
        use codegraph_parser::file_collect::collect_source_files;
        use codegraph_parser::pipeline::ConversionPipeline;
        use codegraph_parser::fast_io::read_file_to_string;
        use regex::Regex;

        let files: Vec<(std::path::PathBuf, u64)> = tokio::task::spawn_blocking({
            let dir = self.project_root.clone();
            move || collect_source_files(&dir).unwrap_or_default()
        })
        .await
        .unwrap_or_default();

        let mut pipeline = ConversionPipeline::new()
            .map_err(|e| CoreRagError::parser(format!("Pipeline init failed: {}", e)))?;

        // Helper to resolve an index id from a pipeline node with tighter matching
        let mut resolve_index_id = |pn: &CodeNode| -> Option<NodeId> {
            let file = &pn.location.file_path;
            let line_p = pn.location.line as i64;
            let col_p = pn.location.column as i64;
            let ids = index.by_file.get(file)?;

            // Rank candidates by: type match, name (case-insensitive), minimal line/column distance
            #[derive(Copy, Clone, Debug)]
            struct Score(NodeId, i64);
            let mut scored: Vec<Score> = Vec::new();
            for id in ids {
                if let Some(n) = index.nodes.get(id) {
                    // Name check required
                    if !n.name.eq_ignore_ascii_case(&pn.name) {
                        continue;
                    }
                    // Type alignment bonus (0 penalty if same type; 50 otherwise)
                    let type_penalty = match (n.node_type.as_ref(), pn.node_type.as_ref()) {
                        (Some(t1), Some(t2)) if t1 == t2 => 0,
                        _ => 50,
                    };
                    let dl = (n.location.line as i64 - line_p).abs();
                    let dc = (n.location.column as i64 - col_p).abs();
                    let dist = dl * 10 + dc + type_penalty; // weight lines more than columns
                    scored.push(Score(*id, dist));
                }
            }
            scored.sort_by_key(|s| s.1);
            scored.first().map(|s| s.0)
        };

        // AST-based edges where possible
        for (path, _size) in &files {
            let path_str = path.to_string_lossy().to_string();
            let Ok(source) = read_file_to_string(&path_str).await else { continue };
            if let Ok(result) = pipeline.process_file(path.as_path(), source) {
                // Map pipeline node ids to canonical index ids
                let mut pip_nodes: HashMap<NodeId, CodeNode> = HashMap::new();
                for n in &result.nodes {
                    pip_nodes.insert(n.id, n.clone());
                }

                for e in result.edges {
                    if !matches!(e.edge_type, EdgeType::Calls) { continue; }
                    let Some(from_p) = pip_nodes.get(&e.from) else { continue; };
                    let Some(to_p) = pip_nodes.get(&e.to) else { continue; };
                    if let (Some(from_id), Some(to_id)) = (resolve_index_id(from_p), resolve_index_id(to_p)) {
                        if from_id != to_id {
                            index.calls_out.entry(from_id).or_default().push(to_id);
                            index.calls_in.entry(to_id).or_default().push(from_id);
                        }
                    }
                }
            }
        }

        // Regex fallback for any remaining relationships
        let ident_call = Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\(").unwrap();
        let mut name_to_ids: HashMap<String, Vec<NodeId>> = HashMap::new();
        for (id, node) in &index.nodes {
            if matches!(node.node_type, Some(NodeType::Function)) {
                name_to_ids
                    .entry(node.name.to_string().to_lowercase())
                    .or_default()
                    .push(*id);
            }
        }
        for (id, node) in &index.nodes {
            let Some(content) = node.content.as_deref() else { continue; };
            let mut seen: HashSet<NodeId> = HashSet::new();
            for cap in ident_call.captures_iter(content) {
                let fname = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
                if let Some(targets) = name_to_ids.get(&fname) {
                    for &tid in targets {
                        if tid == *id { continue; }
                        if seen.insert(tid) {
                            index.calls_out.entry(*id).or_default().push(tid);
                            index.calls_in.entry(tid).or_default().push(*id);
                        }
                    }
                }
            }
        }
        let mut guard = self.index.write().await;
        *guard = Some(index);
        Ok(())
    }

    /// Search for code using vector similarity over the indexed project
    pub async fn search_code(
        &self,
        query: &str,
        limit: u32,
        threshold: f32,
    ) -> CoreRagResult<Vec<CodeSearchResult>> {
        self.ensure_index_built().await?;
        let guard = self.index.read().await;
        let idx = guard.as_ref().unwrap();

        let q = self
            .embedder
            .generate_text_embedding(query)
            .await
            .map_err(|e| CoreRagError::vector_search(format!("Embedding failed: {}", e)))?;

        let mut scored: Vec<(NodeId, f32)> = idx
            .embeddings
            .iter()
            .map(|(id, emb)| (*id, cosine(&q, emb)))
            .filter(|(_, s)| *s >= threshold)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        let mut out = Vec::with_capacity(scored.len());
        for (id, score) in scored {
            if let Some(node) = idx.nodes.get(&id) {
                out.push(CodeSearchResult {
                    id: id.to_string(),
                    name: node.name.to_string(),
                    path: node.location.file_path.clone(),
                    node_type: node
                        .node_type
                        .as_ref()
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_else(|| "unknown".to_string()),
                    content: node.content.as_deref().unwrap_or("").to_string(),
                    score,
                    language: node.language.as_ref().map(|l| format!("{:?}", l).to_lowercase()),
                });
            }
        }
        Ok(out)
    }

    /// Get detailed information about a code node
    pub async fn get_code_details(&self, node_id: &str) -> CoreRagResult<Option<CodeDetails>> {
        self.ensure_index_built().await?;
        let guard = self.index.read().await;
        let idx = guard.as_ref().unwrap();
        let id = match uuid::Uuid::parse_str(node_id) {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };
        if let Some(node) = idx.nodes.get(&id) {
            let (start_line, end_line) = (node.location.line, node.location.end_line.unwrap_or(node.location.line));
            let deps = idx
                .calls_out
                .get(&id)
                .into_iter()
                .flat_map(|v| v.iter())
                .filter_map(|nid| idx.nodes.get(nid))
                .map(|n| n.name.to_string())
                .take(8)
                .collect::<Vec<_>>();
            let details = CodeDetails {
                id: id.to_string(),
                name: node.name.to_string(),
                path: node.location.file_path.clone(),
                node_type: node
                    .node_type
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "unknown".to_string()),
                content: node.content.as_deref().unwrap_or("").to_string(),
                language: node.language.as_ref().map(|l| format!("{:?}", l).to_lowercase()),
                start_line,
                end_line,
                dependencies: deps,
                metadata: node
                    .metadata
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            };
            Ok(Some(details))
        } else {
            Ok(None)
        }
    }

    /// Analyze code relationships and dependencies (heuristic based)
    pub async fn analyze_relationships(
        &self,
        node_id: &str,
        _depth: u32,
    ) -> CoreRagResult<RelationshipAnalysis> {
        self.ensure_index_built().await?;
        let guard = self.index.read().await;
        let idx = guard.as_ref().unwrap();
        let id = match uuid::Uuid::parse_str(node_id) {
            Ok(id) => id,
            Err(_) => {
                return Ok(RelationshipAnalysis {
                    dependencies: vec![],
                    dependents: vec![],
                    related: vec![],
                })
            }
        };
        let Some(node) = idx.nodes.get(&id) else {
            return Ok(RelationshipAnalysis { dependencies: vec![], dependents: vec![], related: vec![] });
        };

        // Dependencies: direct call graph edges (callees)
        let deps = idx
            .calls_out
            .get(&id)
            .into_iter()
            .flat_map(|v| v.iter())
            .filter_map(|nid| idx.nodes.get(nid))
            .map(|n| RelatedNode {
                id: n.id.to_string(),
                name: n.name.to_string(),
                path: n.location.file_path.clone(),
                relationship_type: "calls".to_string(),
                score: 1.0,
            })
            .collect::<Vec<_>>();

        // Dependents: direct callers
        let dependents = idx
            .calls_in
            .get(&id)
            .into_iter()
            .flat_map(|v| v.iter())
            .filter_map(|nid| idx.nodes.get(nid))
            .map(|n| RelatedNode {
                id: n.id.to_string(),
                name: n.name.to_string(),
                path: n.location.file_path.clone(),
                relationship_type: "called_by".to_string(),
                score: 1.0,
            })
            .collect::<Vec<_>>();

        // Related: top similar nodes across project
        let q = idx.embeddings.get(&id).cloned().unwrap_or_default();
        let mut related: Vec<(NodeId, f32)> = idx
            .embeddings
            .iter()
            .filter_map(|(nid, emb)| if nid != &id { Some((*nid, cosine(&q, emb))) } else { None })
            .collect();
        related.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        related.truncate(8);
        let related_nodes = related
            .into_iter()
            .filter_map(|(nid, score)| idx.nodes.get(&nid).map(|n| (nid, n, score)))
            .map(|(nid, n, score)| RelatedNode {
                id: nid.to_string(),
                name: n.name.to_string(),
                path: n.location.file_path.clone(),
                relationship_type: "semantic_similarity".to_string(),
                score,
            })
            .collect();

        Ok(RelationshipAnalysis {
            dependencies: deps,
            dependents,
            related: related_nodes,
        })
    }

    /// Get repository statistics computed from the parsed project
    pub async fn get_repo_stats(&self) -> CoreRagResult<RepoStats> {
        self.ensure_index_built().await?;
        let guard = self.index.read().await;
        let idx = guard.as_ref().unwrap();

        let total_nodes = idx.nodes.len() as u64;
        let mut languages: HashMap<String, u64> = HashMap::new();
        let mut function_count = 0u64;
        let mut class_count = 0u64;
        let mut module_count = 0u64;
        let mut test_file_count = 0u64;

        let mut unique_files: HashSet<String> = HashSet::new();
        for node in idx.nodes.values() {
            if let Some(lang) = node.language.as_ref() {
                *languages.entry(format!("{:?}", lang).to_lowercase()).or_insert(0) += 1;
            }
            match node.node_type {
                Some(NodeType::Function) => function_count += 1,
                Some(NodeType::Class) => class_count += 1,
                Some(NodeType::Module) => module_count += 1,
                _ => {}
            }
            if node.location.file_path.contains("test") || node.location.file_path.contains("_test") {
                test_file_count += 1;
            }
            unique_files.insert(node.location.file_path.clone());
        }

        Ok(RepoStats {
            total_nodes,
            file_count: unique_files.len() as u64,
            function_count,
            class_count,
            module_count,
            test_file_count,
            languages,
            last_updated: Utc::now(),
            recent_changes: 0,
        })
    }

    /// Perform semantic search using natural language
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: u32,
    ) -> CoreRagResult<Vec<SemanticSearchResult>> {
        self.ensure_index_built().await?;
        let guard = self.index.read().await;
        let idx = guard.as_ref().unwrap();

        let q = self
            .embedder
            .generate_text_embedding(query)
            .await
            .map_err(|e| CoreRagError::vector_search(format!("Embedding failed: {}", e)))?;

        let mut scored: Vec<(NodeId, f32)> = idx
            .embeddings
            .iter()
            .map(|(id, emb)| (*id, cosine(&q, emb)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit as usize);

        let results = scored
            .into_iter()
            .filter_map(|(id, score)| idx.nodes.get(&id).map(|n| (id, n, score)))
            .map(|(id, node, score)| SemanticSearchResult {
                id: id.to_string(),
                title: node.name.to_string(),
                path: node.location.file_path.clone(),
                context: node.content.as_deref().unwrap_or("").to_string(),
                relevance_score: score,
                node_type: node
                    .node_type
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "unknown".to_string()),
            })
            .collect();

        Ok(results)
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na.sqrt() * nb.sqrt()) }
}

fn similar_in_file(node: &CodeNode, idx: &RagIndex, take: usize) -> Vec<String> {
    similar_in_file_details(node, idx, take)
        .into_iter()
        .map(|(_, name, _)| name)
        .collect()
}

fn similar_in_file_details(
    node: &CodeNode,
    idx: &RagIndex,
    take: usize,
) -> Vec<(NodeId, String, String)> {
    let mut out = Vec::new();
    if let Some(ids) = idx.by_file.get(&node.location.file_path) {
        for other_id in ids {
            if other_id == &node.id { continue; }
            if let Some(other) = idx.nodes.get(other_id) {
                out.push((other.id, other.name.to_string(), other.location.file_path.clone()));
                if out.len() >= take { break; }
            }
        }
    }
    out
}
