use crate::connection_pool::{load_base_urls_from_env, ConnectionPoolConfig, HttpClientPool};
use crate::performance::{PerformanceOptimizer, PerformanceOptimizerConfig};
use crate::service_registry::ServiceRegistry;
use async_trait::async_trait;
use codegraph_core::{CodeNode, ConfigManager, GraphStore, NodeId};
use codegraph_graph::{SurrealDbConfig as GraphSurrealConfig, SurrealDbStorage};
use codegraph_parser::TreeSitterParser;
use codegraph_vector::{EmbeddingGenerator, SemanticSearch, SurrealVectorStore};
use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::RwLock;

// Simple in-memory graph implementation for now
pub struct InMemoryGraph {
    nodes: HashMap<NodeId, CodeNode>,
}

impl InMemoryGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub async fn get_stats(&self) -> codegraph_core::Result<GraphStats> {
        let node_count = self.nodes.len();
        let edge_count = 0; // Not tracking edges in this simple implementation
        Ok(GraphStats::new(node_count, edge_count, 0))
    }

    pub async fn test_connection(&self) -> codegraph_core::Result<bool> {
        // Always return true for in-memory graph
        Ok(true)
    }

    pub async fn get_neighbors(&self, _node_id: NodeId) -> codegraph_core::Result<Vec<NodeId>> {
        // Simple stub - no edges tracked
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub total_size_bytes: usize,
    pub total_nodes: usize, // Alias for node_count
    pub total_edges: usize, // Alias for edge_count
}

impl GraphStats {
    pub fn new(node_count: usize, edge_count: usize, total_size_bytes: usize) -> Self {
        Self {
            node_count,
            edge_count,
            total_size_bytes,
            total_nodes: node_count,
            total_edges: edge_count,
        }
    }
}

#[async_trait]
impl GraphStore for InMemoryGraph {
    async fn add_node(&mut self, node: CodeNode) -> codegraph_core::Result<()> {
        self.nodes.insert(node.id, node);
        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> codegraph_core::Result<Option<CodeNode>> {
        Ok(self.nodes.get(&id).cloned())
    }

    async fn update_node(&mut self, node: CodeNode) -> codegraph_core::Result<()> {
        self.nodes.insert(node.id, node);
        Ok(())
    }

    async fn remove_node(&mut self, id: NodeId) -> codegraph_core::Result<()> {
        self.nodes.remove(&id);
        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> codegraph_core::Result<Vec<CodeNode>> {
        Ok(self
            .nodes
            .values()
            .filter(|n| n.name.as_str() == name)
            .cloned()
            .collect())
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: codegraph_core::CodeGraphConfig,
    pub config: Arc<ConfigManager>,
    pub graph: Arc<RwLock<InMemoryGraph>>,
    pub parser: Arc<TreeSitterParser>,
    pub vector_store: Arc<SurrealVectorStore>,
    pub embedding_generator: Arc<EmbeddingGenerator>,
    pub semantic_search: Arc<SemanticSearch>,
    pub ws_metrics: Arc<WebSocketMetrics>,
    pub http_client_pool: Arc<HttpClientPool>,
    // pub http2_optimizer: Arc<Http2Optimizer>,
    pub service_registry: Arc<ServiceRegistry>,
    pub performance: Arc<PerformanceOptimizer>,
}

impl AppState {
    pub async fn new(config: Arc<ConfigManager>) -> codegraph_core::Result<Self> {
        let graph = Arc::new(RwLock::new(InMemoryGraph::new()));

        // Try to initialize with real storage, fallback to stub if it fails
        let storage_path = std::env::var("CODEGRAPH_STORAGE_PATH")
            .unwrap_or_else(|_| "./codegraph_data".to_string());

        let parser = Arc::new(TreeSitterParser::new());
        let core_surreal = codegraph_core::SurrealDbConfig::default();
        let surreal_config = adapt_surreal_config(&core_surreal);
        let surreal_storage = Arc::new(TokioMutex::new(
            SurrealDbStorage::new(surreal_config).await?,
        ));
        let vector_store = Arc::new(SurrealVectorStore::with_surreal_storage(
            surreal_storage.clone(),
            200,
        ));
        // Use advanced embeddings when CODEGRAPH_EMBEDDING_PROVIDER=local, otherwise fallback
        let embedding_generator = Arc::new(EmbeddingGenerator::with_auto_from_env().await);
        let semantic_search = Arc::new(SemanticSearch::new(
            vector_store.clone(),
            embedding_generator.clone(),
        ));

        // Network connection pool with keep-alive and load balancing
        let pool_cfg = ConnectionPoolConfig::from_env();
        let base_urls = load_base_urls_from_env();
        let http_client_pool = Arc::new(
            HttpClientPool::new(pool_cfg, base_urls).expect("Failed to init HttpClientPool"),
        );

        // HTTP/2 optimization disabled in this build

        // API-level performance optimizer (LRU caching + complexity guardrails)
        let perf = Arc::new(PerformanceOptimizer::new(
            PerformanceOptimizerConfig::default(),
        ));
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
            settings: config.config().clone(),
            config,
            graph,
            parser,
            vector_store,
            embedding_generator,
            semantic_search,
            ws_metrics: Arc::new(WebSocketMetrics::default()),
            http_client_pool,
            // http2_optimizer,
            service_registry: Arc::new(ServiceRegistry::new()),
            performance: perf,
        })
    }
}

fn adapt_surreal_config(source: &codegraph_core::SurrealDbConfig) -> GraphSurrealConfig {
    GraphSurrealConfig {
        connection: source.connection.clone(),
        namespace: source.namespace.clone(),
        database: source.database.clone(),
        username: source.username.clone(),
        password: source
            .password
            .as_ref()
            .map(|secret| secret.expose_secret().to_string()),
        strict_mode: source.strict_mode,
        auto_migrate: source.auto_migrate,
        cache_enabled: true,
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
        while now > peak
            && self
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
