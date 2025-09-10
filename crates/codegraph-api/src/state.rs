use codegraph_graph::CodeGraph;
use codegraph_parser::TreeSitterParser;
use codegraph_vector::{EmbeddingGenerator, FaissVectorStore, SemanticSearch};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::connection_pool::{ConnectionPoolConfig, HttpClientPool, load_base_urls_from_env};
use crate::http2_optimizer::{Http2Optimizer, Http2OptimizerConfig};
use crate::service_registry::ServiceRegistry;
use codegraph_core::{ConfigManager, Settings};

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<Settings>>,
    pub config: Arc<ConfigManager>,
    pub graph: Arc<RwLock<CodeGraph>>,
    pub parser: Arc<TreeSitterParser>,
    pub vector_store: Arc<FaissVectorStore>,
    pub embedding_generator: Arc<EmbeddingGenerator>,
    pub semantic_search: Arc<SemanticSearch>,
    pub ws_metrics: Arc<WebSocketMetrics>,
    pub http_client_pool: Arc<HttpClientPool>,
    pub http2_optimizer: Arc<Http2Optimizer>,
    pub service_registry: Arc<ServiceRegistry>,
}

impl AppState {
    pub async fn new(config: Arc<ConfigManager>) -> codegraph_core::Result<Self> {
        let graph = Arc::new(RwLock::new(CodeGraph::new()));
        let parser = Arc::new(TreeSitterParser::new());
        let vector_store = Arc::new(FaissVectorStore::new(384)?);
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let semantic_search = Arc::new(SemanticSearch::new(
            vector_store.clone(),
            embedding_generator.clone(),
        ));

        // Network connection pool with keep-alive and load balancing
        let pool_cfg = ConnectionPoolConfig::from_env();
        let base_urls = load_base_urls_from_env();
        let http_client_pool = Arc::new(HttpClientPool::new(pool_cfg, base_urls).expect("Failed to init HttpClientPool"));

        // HTTP/2 optimization
        let http2_config = Http2OptimizerConfig::default();
        let http2_optimizer = Arc::new(Http2Optimizer::new(http2_config));
        {
            // Periodically close idle connections to keep pool healthy
            let pool = http_client_pool.clone();
            tokio::spawn(async move {
                let interval = std::time::Duration::from_secs(300);
                loop {
                    tokio::time::sleep(interval).await;
                    pool.close_idle();
                }
            });
        }

        Ok(Self {
            settings: config.settings().clone(),
            config,
            graph,
            parser,
            vector_store,
            embedding_generator,
            semantic_search,
            ws_metrics: Arc::new(WebSocketMetrics::default()),
            http_client_pool,
            http2_optimizer,
            service_registry: Arc::new(ServiceRegistry::new()),
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
