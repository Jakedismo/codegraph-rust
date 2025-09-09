use codegraph_graph::CodeGraph;
use codegraph_parser::TreeSitterParser;
use codegraph_vector::{EmbeddingGenerator, FaissVectorStore, SemanticSearch};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub graph: Arc<RwLock<CodeGraph>>,
    pub parser: Arc<TreeSitterParser>,
    pub vector_store: Arc<FaissVectorStore>,
    pub embedding_generator: Arc<EmbeddingGenerator>,
    pub semantic_search: Arc<SemanticSearch>,
}

impl AppState {
    pub async fn new() -> codegraph_core::Result<Self> {
        let graph = Arc::new(RwLock::new(CodeGraph::new()));
        let parser = Arc::new(TreeSitterParser::new());
        let vector_store = Arc::new(FaissVectorStore::new(384)?);
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let semantic_search = Arc::new(SemanticSearch::new(
            vector_store.clone(),
            embedding_generator.clone(),
        ));

        Ok(Self {
            graph,
            parser,
            vector_store,
            embedding_generator,
            semantic_search,
        })
    }
}