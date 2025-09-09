use crate::core::{
    CodeGraphResult, CodeNode, CodeEdge, ParsedCode, GraphQuery, SearchQuery, 
    SearchResult, BatchSearchQuery, IndexStats, TransactionContext, NodeId, 
    EdgeId, IndexId, Vector, Language, SourceRange
};
use async_trait::async_trait;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[async_trait]
pub trait GraphStore: Send + Sync + Clone + 'static {
    type Transaction: GraphTransaction + Send + Sync;
    type NodeStream: futures::Stream<Item = CodeGraphResult<CodeNode<'static>>> + Send;
    type EdgeStream: futures::Stream<Item = CodeGraphResult<CodeEdge>> + Send;

    async fn begin_transaction(&self) -> CodeGraphResult<Self::Transaction>;
    
    async fn commit_transaction(&self, tx: Self::Transaction) -> CodeGraphResult<()>;
    
    async fn rollback_transaction(&self, tx: Self::Transaction) -> CodeGraphResult<()>;

    async fn insert_node(&self, node: &CodeNode<'_>) -> CodeGraphResult<NodeId>;
    
    async fn insert_nodes(&self, nodes: &[CodeNode<'_>]) -> CodeGraphResult<Vec<NodeId>>;
    
    async fn insert_edge(&self, edge: &CodeEdge) -> CodeGraphResult<EdgeId>;
    
    async fn insert_edges(&self, edges: &[CodeEdge]) -> CodeGraphResult<Vec<EdgeId>>;
    
    async fn get_node(&self, id: NodeId) -> CodeGraphResult<Option<CodeNode<'static>>>;
    
    async fn get_nodes(&self, ids: &[NodeId]) -> CodeGraphResult<Vec<Option<CodeNode<'static>>>>;
    
    async fn get_edge(&self, id: EdgeId) -> CodeGraphResult<Option<CodeEdge>>;
    
    async fn get_edges(&self, ids: &[EdgeId]) -> CodeGraphResult<Vec<Option<CodeEdge>>>;

    async fn update_node(&self, id: NodeId, node: &CodeNode<'_>) -> CodeGraphResult<bool>;
    
    async fn update_edge(&self, id: EdgeId, edge: &CodeEdge) -> CodeGraphResult<bool>;

    async fn delete_node(&self, id: NodeId) -> CodeGraphResult<bool>;
    
    async fn delete_edge(&self, id: EdgeId) -> CodeGraphResult<bool>;

    async fn query_nodes(&self, query: &GraphQuery) -> CodeGraphResult<Self::NodeStream>;
    
    async fn query_edges(&self, query: &GraphQuery) -> CodeGraphResult<Self::EdgeStream>;

    async fn get_neighbors(&self, node_id: NodeId, depth: u32) -> CodeGraphResult<Vec<CodeNode<'static>>>;
    
    async fn get_shortest_path(&self, from: NodeId, to: NodeId) -> CodeGraphResult<Vec<CodeEdge>>;

    async fn bulk_upsert_nodes(&self, nodes: &[CodeNode<'_>]) -> CodeGraphResult<Vec<NodeId>>;
    
    async fn bulk_upsert_edges(&self, edges: &[CodeEdge]) -> CodeGraphResult<Vec<EdgeId>>;

    async fn count_nodes(&self, query: &GraphQuery) -> CodeGraphResult<usize>;
    
    async fn count_edges(&self, query: &GraphQuery) -> CodeGraphResult<usize>;

    async fn health_check(&self) -> CodeGraphResult<HashMap<String, String>>;
    
    async fn optimize(&self) -> CodeGraphResult<()>;
}

#[async_trait]
pub trait GraphTransaction: Send + Sync {
    async fn insert_node(&mut self, node: &CodeNode<'_>) -> CodeGraphResult<NodeId>;
    
    async fn insert_edge(&mut self, edge: &CodeEdge) -> CodeGraphResult<EdgeId>;
    
    async fn get_node(&self, id: NodeId) -> CodeGraphResult<Option<CodeNode<'static>>>;
    
    async fn get_edge(&self, id: EdgeId) -> CodeGraphResult<Option<CodeEdge>>;
    
    async fn update_node(&mut self, id: NodeId, node: &CodeNode<'_>) -> CodeGraphResult<bool>;
    
    async fn update_edge(&mut self, id: EdgeId, edge: &CodeEdge) -> CodeGraphResult<bool>;
    
    async fn delete_node(&mut self, id: NodeId) -> CodeGraphResult<bool>;
    
    async fn delete_edge(&mut self, id: EdgeId) -> CodeGraphResult<bool>;
}

#[async_trait]
pub trait Parser<'a>: Send + Sync + Clone {
    type Output: IntoIterator<Item = ParsedCode<'a>> + Send;
    
    fn supports_language(&self, language: Language) -> bool;
    
    async fn parse(&self, source: &'a str, file_path: &'a str, language: Language) 
                  -> CodeGraphResult<ParsedCode<'a>>;
    
    async fn parse_batch(&self, inputs: &'a [(Cow<'a, str>, Cow<'a, str>, Language)]) 
                        -> CodeGraphResult<Self::Output>;
    
    async fn parse_file(&self, file_path: &'a str) -> CodeGraphResult<ParsedCode<'a>>;
    
    async fn parse_directory(&self, dir_path: &'a str, recursive: bool) 
                            -> CodeGraphResult<Self::Output>;

    fn extract_dependencies(&self, parsed: &ParsedCode<'a>) -> CodeGraphResult<Vec<String>>;
    
    fn extract_symbols(&self, parsed: &ParsedCode<'a>) -> CodeGraphResult<HashMap<String, SourceRange>>;
    
    async fn incremental_parse(&self, old_parsed: &ParsedCode<'a>, 
                              new_source: &'a str, changes: &[SourceRange]) 
                             -> CodeGraphResult<ParsedCode<'a>>;

    fn validate_syntax(&self, source: &'a str, language: Language) -> CodeGraphResult<Vec<String>>;
    
    async fn get_ast_json(&self, source: &'a str, language: Language) -> CodeGraphResult<String>;
}

pub trait ZeroCopyParser<'a>: Send + Sync {
    type Tree: Send + Sync;
    type Node: Send + Sync;
    type Cursor: Send + Sync;

    fn parse_zero_copy(&self, source: &'a [u8], language: Language) -> CodeGraphResult<Self::Tree>;
    
    fn tree_cursor(&self, tree: &Self::Tree) -> Self::Cursor;
    
    fn cursor_node(&self, cursor: &Self::Cursor) -> Option<Self::Node>;
    
    fn node_text<'b>(&self, node: &Self::Node, source: &'b [u8]) -> &'b [u8] 
    where 'a: 'b;
    
    fn node_range(&self, node: &Self::Node) -> SourceRange;
    
    fn node_kind_str(&self, node: &Self::Node) -> &'static str;
    
    fn cursor_goto_first_child(&mut self, cursor: &mut Self::Cursor) -> bool;
    
    fn cursor_goto_next_sibling(&mut self, cursor: &mut Self::Cursor) -> bool;
    
    fn cursor_goto_parent(&mut self, cursor: &mut Self::Cursor) -> bool;
    
    fn node_children(&self, node: &Self::Node) -> Vec<Self::Node>;
    
    fn walk_tree<F>(&self, tree: &Self::Tree, source: &'a [u8], visitor: F) -> CodeGraphResult<()>
    where F: Fn(&Self::Node, &'a [u8], usize) -> CodeGraphResult<bool>;
}

#[async_trait]
pub trait VectorIndex: Send + Sync + Clone + 'static {
    type IndexHandle: Send + Sync;
    
    async fn create_index(&self, name: &str, dimensions: usize, 
                         config: IndexConfig) -> CodeGraphResult<IndexId>;
    
    async fn delete_index(&self, name: &str) -> CodeGraphResult<bool>;
    
    async fn get_index(&self, name: &str) -> CodeGraphResult<Option<Self::IndexHandle>>;
    
    async fn list_indices(&self) -> CodeGraphResult<Vec<IndexId>>;

    async fn insert_vector(&self, index: &str, id: NodeId, vector: &Vector) 
                          -> CodeGraphResult<()>;
    
    async fn insert_vectors(&self, index: &str, vectors: &[(NodeId, Vector)]) 
                           -> CodeGraphResult<()>;
    
    async fn update_vector(&self, index: &str, id: NodeId, vector: &Vector) 
                          -> CodeGraphResult<bool>;
    
    async fn delete_vector(&self, index: &str, id: NodeId) -> CodeGraphResult<bool>;

    async fn search(&self, query: &SearchQuery) -> CodeGraphResult<Vec<SearchResult>>;
    
    async fn batch_search(&self, query: &BatchSearchQuery) -> CodeGraphResult<Vec<Vec<SearchResult>>>;

    async fn similarity(&self, vector1: &Vector, vector2: &Vector) -> CodeGraphResult<f32>;
    
    async fn cosine_similarity(&self, vector1: &Vector, vector2: &Vector) -> CodeGraphResult<f32>;
    
    async fn euclidean_distance(&self, vector1: &Vector, vector2: &Vector) -> CodeGraphResult<f32>;

    async fn get_vector(&self, index: &str, id: NodeId) -> CodeGraphResult<Option<Vector>>;
    
    async fn get_vectors(&self, index: &str, ids: &[NodeId]) -> CodeGraphResult<Vec<Option<Vector>>>;

    async fn build_index(&self, index: &str) -> CodeGraphResult<()>;
    
    async fn rebuild_index(&self, index: &str) -> CodeGraphResult<()>;

    async fn get_stats(&self, index: &str) -> CodeGraphResult<IndexStats>;
    
    async fn optimize_index(&self, index: &str) -> CodeGraphResult<()>;

    async fn approximate_search(&self, query: &SearchQuery, 
                               approximation_factor: f32) -> CodeGraphResult<Vec<SearchResult>>;
    
    async fn range_search(&self, index: &str, query_vector: &Vector, 
                         radius: f32) -> CodeGraphResult<Vec<SearchResult>>;
}

#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub index_type: IndexType,
    pub metric: DistanceMetric,
    pub parameters: HashMap<String, IndexParameter>,
    pub memory_limit_mb: Option<usize>,
    pub build_threads: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum IndexType {
    FlatL2,
    FlatIP,
    IVFFlat { nlist: usize },
    IVFPQ { nlist: usize, m: usize, nbits: usize },
    HNSW { m: usize, ef_construction: usize },
    LSH { nbits: usize },
}

#[derive(Debug, Clone)]
pub enum DistanceMetric {
    L2,
    InnerProduct,
    Cosine,
    Manhattan,
    Jaccard,
}

#[derive(Debug, Clone)]
pub enum IndexParameter {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

pub trait AsyncIterator<T> {
    type Error;
    
    fn poll_next(
        self: Pin<&mut Self>, 
        cx: &mut std::task::Context<'_>
    ) -> std::task::Poll<Option<Result<T, Self::Error>>>;
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

pub trait LifetimeManager: Send + Sync {
    type Resource: Send + Sync;
    
    fn acquire(&self) -> BoxFuture<'_, CodeGraphResult<Arc<Self::Resource>>>;
    
    fn release(&self, resource: Arc<Self::Resource>) -> BoxFuture<'_, CodeGraphResult<()>>;
    
    fn health_check(&self) -> BoxFuture<'_, CodeGraphResult<bool>>;
    
    fn resource_count(&self) -> usize;
    
    fn cleanup(&self) -> BoxFuture<'_, CodeGraphResult<usize>>;
}

#[async_trait]
pub trait ConnectionPool<T>: Send + Sync + Clone
where
    T: Send + Sync + 'static,
{
    async fn get(&self) -> CodeGraphResult<Arc<T>>;
    
    async fn return_to_pool(&self, item: Arc<T>) -> CodeGraphResult<()>;
    
    async fn size(&self) -> usize;
    
    async fn active_connections(&self) -> usize;
    
    async fn health_check(&self) -> CodeGraphResult<()>;
    
    async fn drain(&self) -> CodeGraphResult<()>;
}

pub struct ConcurrentProcessor<T, R> {
    workers: usize,
    queue_size: usize,
    processor: Arc<dyn Fn(T) -> BoxFuture<'static, CodeGraphResult<R>> + Send + Sync>,
}

impl<T, R> ConcurrentProcessor<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    pub fn new<F>(workers: usize, queue_size: usize, processor: F) -> Self
    where
        F: Fn(T) -> BoxFuture<'static, CodeGraphResult<R>> + Send + Sync + 'static,
    {
        Self {
            workers,
            queue_size,
            processor: Arc::new(processor),
        }
    }
    
    pub async fn process_batch(&self, items: Vec<T>) -> CodeGraphResult<Vec<R>> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(self.queue_size);
        let (result_tx, mut result_rx) = tokio::sync::mpsc::channel(items.len());
        
        for _ in 0..self.workers {
            let processor = Arc::clone(&self.processor);
            let tx = tx.clone();
            let result_tx = result_tx.clone();
            
            tokio::spawn(async move {
                while let Ok(item) = rx.recv().await {
                    match processor(item).await {
                        Ok(result) => {
                            if result_tx.send(Ok(result)).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            if result_tx.send(Err(e)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
        
        for item in items {
            tx.send(item).await.map_err(|_| {
                crate::core::CodeGraphError::config_error("Failed to send item to processor")
            })?;
        }
        drop(tx);
        
        let mut results = Vec::new();
        while let Some(result) = result_rx.recv().await {
            results.push(result?);
        }
        
        Ok(results)
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            index_type: IndexType::FlatL2,
            metric: DistanceMetric::L2,
            parameters: HashMap::new(),
            memory_limit_mb: None,
            build_threads: Some(num_cpus::get()),
        }
    }
}

pub struct AsyncBatch<T> {
    items: Vec<T>,
    batch_size: usize,
}

impl<T> AsyncBatch<T> {
    pub fn new(items: Vec<T>, batch_size: usize) -> Self {
        Self { items, batch_size }
    }
    
    pub async fn process<F, R, Fut>(&self, processor: F) -> CodeGraphResult<Vec<R>>
    where
        F: Fn(&[T]) -> Fut + Send + Sync,
        Fut: Future<Output = CodeGraphResult<Vec<R>>> + Send,
        R: Send,
    {
        let mut results = Vec::new();
        
        for chunk in self.items.chunks(self.batch_size) {
            let batch_results = processor(chunk).await?;
            results.extend(batch_results);
        }
        
        Ok(results)
    }
    
    pub async fn process_parallel<F, R, Fut>(&self, processor: F, concurrency: usize) -> CodeGraphResult<Vec<R>>
    where
        F: Fn(&[T]) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = CodeGraphResult<Vec<R>>> + Send + 'static,
        R: Send + 'static,
        T: Send + Sync,
    {
        use futures::stream::{self, StreamExt};
        
        let chunks: Vec<_> = self.items.chunks(self.batch_size).collect();
        
        let results: Vec<Vec<R>> = stream::iter(chunks)
            .map(|chunk| {
                let processor = processor.clone();
                async move { processor(chunk).await }
            })
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<CodeGraphResult<Vec<_>>>()?;
        
        Ok(results.into_iter().flatten().collect())
    }
}