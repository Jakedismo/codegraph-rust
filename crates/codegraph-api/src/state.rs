use codegraph_graph::CodeGraph;
use codegraph_parser::TreeSitterParser;
use codegraph_vector::{EmbeddingGenerator, FaissVectorStore, SemanticSearch};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
pub struct AppState {
    pub graph: Arc<RwLock<CodeGraph>>,
    pub parser: Arc<TreeSitterParser>,
    pub vector_store: Arc<FaissVectorStore>,
    pub embedding_generator: Arc<EmbeddingGenerator>,
    pub semantic_search: Arc<SemanticSearch>,
    pub ws_metrics: Arc<WebSocketMetrics>,
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
            ws_metrics: Arc::new(WebSocketMetrics::default()),
        })
    }
}

#[derive(Default)]
pub struct WebSocketMetrics {
    pub active_subscriptions: AtomicUsize,
    pub peak_subscriptions: AtomicUsize,
    pub total_subscriptions: AtomicUsize,
}

impl WebSocketMetrics {
    pub fn on_subscribe(&self) {
        let now = self.active_subscriptions.fetch_add(1, Ordering::Relaxed) + 1;
        self.total_subscriptions.fetch_add(1, Ordering::Relaxed);
        // track peak
        let mut peak = self.peak_subscriptions.load(Ordering::Relaxed);
        while now > peak && self
            .peak_subscriptions
            .compare_exchange(peak, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            peak = self.peak_subscriptions.load(Ordering::Relaxed);
        }
    }

    pub fn on_unsubscribe(&self) {
        self.active_subscriptions.fetch_sub(1, Ordering::Relaxed);
    }
}
