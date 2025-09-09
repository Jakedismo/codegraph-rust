# CodeGraph RAG Integration Specifications
## Detailed Implementation Specifications for RAG System Integration

---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

### Document Version: 1.0
### Date: September 2025
### Status: Technical Specification

---

## 1. Core Module Integration Specifications

### 1.1 RAG Module Structure

```rust
// src/rag/mod.rs
pub mod retrieval;
pub mod analysis;
pub mod context;
pub mod openai;
pub mod embedding;

use crate::graph::CodeGraph;
use crate::storage::{VectorStore, GraphStore};
use crate::indexing::IndexManager;

pub struct RAGModule {
    pub retrieval_engine: retrieval::HierarchicalRetriever,
    pub code_analyzer: analysis::MultiModalCodeAnalyzer,
    pub context_manager: context::ContextWindowManager,
    pub openai_client: openai::OpenAILoadBalancer,
    pub embedding_service: embedding::EmbeddingService,
}

impl RAGModule {
    pub async fn new(config: RAGConfig) -> Result<Self, RAGError> {
        let vector_store = VectorStore::new(&config.vector_store_config).await?;
        let graph_store = GraphStore::new(&config.graph_store_config).await?;
        
        Ok(Self {
            retrieval_engine: retrieval::HierarchicalRetriever::new(
                vector_store.clone(),
                graph_store.clone(),
            ).await?,
            code_analyzer: analysis::MultiModalCodeAnalyzer::new(&config.analysis_config).await?,
            context_manager: context::ContextWindowManager::new(&config.context_config),
            openai_client: openai::OpenAILoadBalancer::new(&config.openai_config).await?,
            embedding_service: embedding::EmbeddingService::new(&config.embedding_config).await?,
        })
    }
}
```

### 1.2 Configuration Specifications

```rust
// src/rag/config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGConfig {
    pub vector_store_config: VectorStoreConfig,
    pub graph_store_config: GraphStoreConfig,
    pub analysis_config: AnalysisConfig,
    pub context_config: ContextConfig,
    pub openai_config: OpenAIConfig,
    pub embedding_config: EmbeddingConfig,
    pub performance_config: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    pub engine: VectorEngine, // FAISS, Chroma, Qdrant
    pub dimension: usize,
    pub index_type: IndexType,
    pub similarity_metric: SimilarityMetric,
    pub cache_size_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub tree_sitter_grammars: Vec<Language>,
    pub semantic_models: Vec<SemanticModel>,
    pub embedding_models: Vec<EmbeddingModel>,
    pub fusion_strategy: FusionStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_context_tokens: usize,
    pub compression_threshold: f32,
    pub prioritization_strategy: PrioritizationStrategy,
    pub relevance_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_keys: Vec<String>, // Multiple keys for load balancing
    pub base_urls: Vec<String>, // Support for multiple endpoints
    pub models: Vec<ModelConfig>,
    pub rate_limits: RateLimitConfig,
    pub timeout_config: TimeoutConfig,
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub query_timeout_ms: u64,
    pub max_concurrent_queries: usize,
    pub cache_ttl_seconds: u64,
    pub batch_size: usize,
    pub memory_limit_mb: usize,
}
```

---

## 2. Storage Layer Integration

### 2.1 Vector Store Integration

```rust
// src/storage/vector_store.rs
use crate::rag::embedding::{CodeEmbedding, EmbeddingMetadata};

#[async_trait]
pub trait VectorStoreBackend: Send + Sync {
    async fn insert(&self, embeddings: Vec<CodeEmbedding>) -> Result<Vec<String>, VectorStoreError>;
    async fn search(&self, query: &[f32], k: usize, filter: Option<Filter>) -> Result<Vec<SearchResult>, VectorStoreError>;
    async fn update(&self, id: &str, embedding: CodeEmbedding) -> Result<(), VectorStoreError>;
    async fn delete(&self, ids: &[String]) -> Result<usize, VectorStoreError>;
    async fn get_stats(&self) -> Result<VectorStoreStats, VectorStoreError>;
}

// FAISS implementation
pub struct FAISSVectorStore {
    index: faiss::Index,
    metadata_store: HashMap<String, EmbeddingMetadata>,
    dimension: usize,
}

impl FAISSVectorStore {
    pub async fn new(config: &VectorStoreConfig) -> Result<Self, VectorStoreError> {
        let index = match config.index_type {
            IndexType::FlatIP => faiss::IndexFlatIP::new(config.dimension)?,
            IndexType::IVFPQ => faiss::IndexIVFPQ::new(config.dimension, 256, 8)?,
            IndexType::HNSW => faiss::IndexHNSWFlat::new(config.dimension, 32)?,
        };
        
        Ok(Self {
            index: Box::new(index),
            metadata_store: HashMap::new(),
            dimension: config.dimension,
        })
    }
}

#[async_trait]
impl VectorStoreBackend for FAISSVectorStore {
    async fn search(&self, query: &[f32], k: usize, filter: Option<Filter>) -> Result<Vec<SearchResult>, VectorStoreError> {
        let (distances, indices) = self.index.search(query, k)?;
        
        let mut results = Vec::new();
        for (i, (distance, idx)) in distances.iter().zip(indices.iter()).enumerate() {
            if let Some(metadata) = self.metadata_store.get(&idx.to_string()) {
                if let Some(ref filter) = filter {
                    if !filter.matches(metadata) {
                        continue;
                    }
                }
                
                results.push(SearchResult {
                    id: idx.to_string(),
                    score: *distance,
                    metadata: metadata.clone(),
                });
            }
        }
        
        Ok(results)
    }
}
```

### 2.2 Graph Store Integration

```rust
// src/storage/graph_store.rs
use crate::graph::{CodeNode, CodeEdge, CodeGraph};

#[async_trait]
pub trait GraphStoreBackend: Send + Sync {
    async fn insert_node(&self, node: CodeNode) -> Result<NodeId, GraphStoreError>;
    async fn insert_edge(&self, edge: CodeEdge) -> Result<EdgeId, GraphStoreError>;
    async fn get_neighbors(&self, node_id: NodeId, direction: Direction) -> Result<Vec<CodeNode>, GraphStoreError>;
    async fn traverse(&self, start: NodeId, query: TraversalQuery) -> Result<Vec<TraversalResult>, GraphStoreError>;
    async fn shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>, GraphStoreError>;
}

// RocksDB-based implementation
pub struct RocksDBGraphStore {
    db: rocksdb::DB,
    node_cf: rocksdb::ColumnFamily,
    edge_cf: rocksdb::ColumnFamily,
    index_cf: rocksdb::ColumnFamily,
}

impl RocksDBGraphStore {
    pub async fn new(path: &str) -> Result<Self, GraphStoreError> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let cfs = vec!["nodes", "edges", "indices"];
        let db = rocksdb::DB::open_cf(&opts, path, &cfs)?;
        
        Ok(Self {
            node_cf: db.cf_handle("nodes").unwrap(),
            edge_cf: db.cf_handle("edges").unwrap(),
            index_cf: db.cf_handle("indices").unwrap(),
            db,
        })
    }
}

#[async_trait]
impl GraphStoreBackend for RocksDBGraphStore {
    async fn traverse(&self, start: NodeId, query: TraversalQuery) -> Result<Vec<TraversalResult>, GraphStoreError> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();
        
        queue.push_back((start, 0));
        
        while let Some((node_id, depth)) = queue.pop_front() {
            if depth > query.max_depth || visited.contains(&node_id) {
                continue;
            }
            
            visited.insert(node_id.clone());
            
            let node = self.get_node(&node_id).await?;
            if query.filter.matches(&node) {
                results.push(TraversalResult {
                    node,
                    depth,
                    path_length: depth,
                });
            }
            
            let neighbors = self.get_neighbors(node_id, query.direction).await?;
            for neighbor in neighbors {
                if !visited.contains(&neighbor.id) {
                    queue.push_back((neighbor.id, depth + 1));
                }
            }
        }
        
        Ok(results)
    }
}
```

---

## 3. API Layer Integration

### 3.1 GraphQL Schema Definitions

```graphql
# schema.graphql
type Query {
  codeIntelligence(
    query: String!
    language: String
    contextLimit: Int = 10
    includeDefinitions: Boolean = true
    includeReferences: Boolean = true
  ): CodeIntelligenceResponse!
  
  semanticSearch(
    query: String!
    limit: Int = 20
    threshold: Float = 0.7
    filters: SearchFilters
  ): [SemanticMatch!]!
  
  codeContext(
    location: CodeLocation!
    radius: Int = 5
    includeCallGraph: Boolean = false
  ): CodeContextResponse!
  
  explainCode(
    code: String!
    language: String!
    focusArea: String
  ): CodeExplanation!
}

type CodeIntelligenceResponse {
  answer: String!
  contexts: [CodeContext!]!
  confidence: Float!
  sources: [CodeSource!]!
  suggestions: [CodeSuggestion!]
  metrics: QueryMetrics!
}

type CodeContext {
  content: String!
  location: CodeLocation!
  relevanceScore: Float!
  contextType: ContextType!
  syntax: SyntaxInfo
  semantics: SemanticInfo
}

type CodeLocation {
  file: String!
  line: Int!
  column: Int!
  endLine: Int
  endColumn: Int
}

enum ContextType {
  FUNCTION_DEFINITION
  CLASS_DEFINITION
  VARIABLE_USAGE
  IMPORT_STATEMENT
  DOCUMENTATION
  TEST_CASE
}

input SearchFilters {
  languages: [String!]
  filePatterns: [String!]
  contextTypes: [ContextType!]
  minRelevanceScore: Float
  maxAge: Int
}
```

### 3.2 GraphQL Resolver Implementation

```rust
// src/api/graphql/resolvers.rs
use async_graphql::{Context, Object, Result, ID};
use crate::rag::RAGModule;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn code_intelligence(
        &self,
        ctx: &Context<'_>,
        query: String,
        language: Option<String>,
        context_limit: Option<usize>,
        include_definitions: Option<bool>,
        include_references: Option<bool>,
    ) -> Result<CodeIntelligenceResponse> {
        let rag_module = ctx.data::<Arc<RAGModule>>()?;
        
        let code_query = CodeQuery {
            text: query,
            language,
            context_limit: context_limit.unwrap_or(10),
            include_definitions: include_definitions.unwrap_or(true),
            include_references: include_references.unwrap_or(true),
        };
        
        let response = rag_module.process_query(code_query).await
            .map_err(|e| async_graphql::Error::new(format!("RAG processing failed: {}", e)))?;
        
        Ok(response.into())
    }
    
    async fn semantic_search(
        &self,
        ctx: &Context<'_>,
        query: String,
        limit: Option<usize>,
        threshold: Option<f64>,
        filters: Option<SearchFilters>,
    ) -> Result<Vec<SemanticMatch>> {
        let rag_module = ctx.data::<Arc<RAGModule>>()?;
        
        let search_request = SemanticSearchRequest {
            query,
            limit: limit.unwrap_or(20),
            threshold: threshold.unwrap_or(0.7),
            filters: filters.unwrap_or_default(),
        };
        
        let matches = rag_module.semantic_search(search_request).await
            .map_err(|e| async_graphql::Error::new(format!("Semantic search failed: {}", e)))?;
        
        Ok(matches)
    }
}

// GraphQL to internal type conversions
impl From<RAGResponse> for CodeIntelligenceResponse {
    fn from(response: RAGResponse) -> Self {
        CodeIntelligenceResponse {
            answer: response.generated_response,
            contexts: response.contexts.into_iter().map(Into::into).collect(),
            confidence: response.confidence_score,
            sources: response.sources.into_iter().map(Into::into).collect(),
            suggestions: response.suggestions.into_iter().map(Into::into).collect(),
            metrics: response.metrics.into(),
        }
    }
}
```

### 3.3 MCP Server Integration

```rust
// src/api/mcp/server.rs
use mcp_rs::*;
use serde_json::Value;

pub struct CodeGraphMCPServer {
    rag_module: Arc<RAGModule>,
    tools: Vec<Box<dyn McpTool>>,
}

impl CodeGraphMCPServer {
    pub fn new(rag_module: Arc<RAGModule>) -> Self {
        let tools: Vec<Box<dyn McpTool>> = vec![
            Box::new(CodeIntelligenceTool::new(rag_module.clone())),
            Box::new(SemanticSearchTool::new(rag_module.clone())),
            Box::new(CodeExplanationTool::new(rag_module.clone())),
            Box::new(ContextRetrievalTool::new(rag_module.clone())),
        ];
        
        Self { rag_module, tools }
    }
}

#[async_trait]
impl McpServer for CodeGraphMCPServer {
    async fn handle_initialize(&self, params: InitializeParams) -> Result<InitializeResult, McpError> {
        Ok(InitializeResult {
            protocol_version: "1.0".to_string(),
            server_info: ServerInfo {
                name: "CodeGraph RAG Server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ServerCapabilities {
                tools: Some(self.get_tool_definitions()),
                resources: Some(self.get_resource_definitions()),
                ..Default::default()
            },
        })
    }
    
    async fn handle_list_tools(&self) -> Result<Vec<ToolDefinition>, McpError> {
        Ok(self.tools.iter().map(|tool| tool.definition()).collect())
    }
    
    async fn handle_call_tool(&self, params: CallToolParams) -> Result<CallToolResult, McpError> {
        for tool in &self.tools {
            if tool.name() == params.name {
                return tool.execute(params.arguments).await;
            }
        }
        
        Err(McpError::ToolNotFound(params.name))
    }
}

// Tool implementations
pub struct CodeIntelligenceTool {
    rag_module: Arc<RAGModule>,
}

#[async_trait]
impl McpTool for CodeIntelligenceTool {
    fn name(&self) -> &str { "code_intelligence" }
    
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "code_intelligence".to_string(),
            description: "Provide code intelligence using RAG".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Code-related question or query"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language context"
                    },
                    "context_limit": {
                        "type": "integer",
                        "description": "Maximum number of context items to retrieve"
                    }
                },
                "required": ["query"]
            }),
        }
    }
    
    async fn execute(&self, arguments: Value) -> Result<CallToolResult, McpError> {
        let query: String = arguments.get("query")
            .and_then(|v| v.as_str())
            .ok_or(McpError::InvalidArguments("query required".to_string()))?
            .to_string();
            
        let language = arguments.get("language").and_then(|v| v.as_str()).map(String::from);
        let context_limit = arguments.get("context_limit").and_then(|v| v.as_u64()).map(|v| v as usize);
        
        let code_query = CodeQuery {
            text: query,
            language,
            context_limit: context_limit.unwrap_or(10),
            include_definitions: true,
            include_references: true,
        };
        
        let response = self.rag_module.process_query(code_query).await
            .map_err(|e| McpError::ExecutionError(e.to_string()))?;
        
        Ok(CallToolResult {
            content: vec![ToolContent::Text {
                text: serde_json::to_string_pretty(&response).unwrap(),
            }],
            is_error: false,
        })
    }
}
```

---

## 4. Performance Optimization Integration

### 4.1 Caching Layer

```rust
// src/cache/mod.rs
use moka::future::Cache;
use redis::AsyncCommands;

pub struct CacheLayer {
    l1_cache: Cache<String, CachedResponse>, // In-memory cache
    l2_cache: Option<RedisCache>, // Distributed cache
    ttl_seconds: u64,
}

impl CacheLayer {
    pub async fn new(config: &CacheConfig) -> Result<Self, CacheError> {
        let l1_cache = Cache::builder()
            .max_capacity(config.l1_max_entries)
            .time_to_live(Duration::from_secs(config.ttl_seconds))
            .build();
        
        let l2_cache = if let Some(redis_config) = &config.redis_config {
            Some(RedisCache::new(redis_config).await?)
        } else {
            None
        };
        
        Ok(Self {
            l1_cache,
            l2_cache,
            ttl_seconds: config.ttl_seconds,
        })
    }
    
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        // Try L1 cache first
        if let Some(response) = self.l1_cache.get(key).await {
            return Some(response);
        }
        
        // Try L2 cache
        if let Some(ref l2_cache) = self.l2_cache {
            if let Ok(Some(response)) = l2_cache.get(key).await {
                // Populate L1 cache
                self.l1_cache.insert(key.to_string(), response.clone()).await;
                return Some(response);
            }
        }
        
        None
    }
    
    pub async fn set(&self, key: String, response: CachedResponse) {
        // Set in L1 cache
        self.l1_cache.insert(key.clone(), response.clone()).await;
        
        // Set in L2 cache
        if let Some(ref l2_cache) = self.l2_cache {
            let _ = l2_cache.set(&key, &response, self.ttl_seconds).await;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub response: RAGResponse,
    pub timestamp: SystemTime,
    pub cache_key: String,
}

impl CachedResponse {
    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        self.timestamp.elapsed().unwrap_or(Duration::MAX).as_secs() > ttl_seconds
    }
}
```

### 4.2 Metrics and Monitoring

```rust
// src/metrics/mod.rs
use prometheus::{Counter, Histogram, Gauge, Registry};
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct RAGMetrics {
    // Query metrics
    pub query_total: Counter,
    pub query_duration: Histogram,
    pub query_errors: Counter,
    
    // Retrieval metrics
    pub retrieval_duration: Histogram,
    pub retrieval_accuracy: Gauge,
    pub context_items_retrieved: Histogram,
    
    // OpenAI metrics
    pub openai_requests: Counter,
    pub openai_errors: Counter,
    pub openai_latency: Histogram,
    pub openai_tokens_used: Counter,
    
    // Cache metrics
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub cache_hit_rate: Gauge,
    
    // Resource metrics
    pub memory_usage: Gauge,
    pub cpu_usage: Gauge,
    pub active_connections: Gauge,
}

impl RAGMetrics {
    pub fn new(registry: &Registry) -> Result<Self, Box<dyn std::error::Error>> {
        let query_total = Counter::new("rag_queries_total", "Total number of RAG queries")?;
        let query_duration = Histogram::new("rag_query_duration_seconds", "RAG query duration")?;
        let query_errors = Counter::new("rag_query_errors_total", "Total number of RAG query errors")?;
        
        let retrieval_duration = Histogram::new("rag_retrieval_duration_seconds", "Retrieval duration")?;
        let retrieval_accuracy = Gauge::new("rag_retrieval_accuracy", "Retrieval accuracy score")?;
        let context_items_retrieved = Histogram::new("rag_context_items_retrieved", "Number of context items retrieved")?;
        
        let openai_requests = Counter::new("openai_requests_total", "Total OpenAI API requests")?;
        let openai_errors = Counter::new("openai_errors_total", "Total OpenAI API errors")?;
        let openai_latency = Histogram::new("openai_latency_seconds", "OpenAI API latency")?;
        let openai_tokens_used = Counter::new("openai_tokens_used_total", "Total OpenAI tokens used")?;
        
        let cache_hits = Counter::new("cache_hits_total", "Total cache hits")?;
        let cache_misses = Counter::new("cache_misses_total", "Total cache misses")?;
        let cache_hit_rate = Gauge::new("cache_hit_rate", "Cache hit rate")?;
        
        let memory_usage = Gauge::new("memory_usage_bytes", "Memory usage in bytes")?;
        let cpu_usage = Gauge::new("cpu_usage_percent", "CPU usage percentage")?;
        let active_connections = Gauge::new("active_connections", "Number of active connections")?;
        
        // Register all metrics
        registry.register(Box::new(query_total.clone()))?;
        registry.register(Box::new(query_duration.clone()))?;
        registry.register(Box::new(query_errors.clone()))?;
        registry.register(Box::new(retrieval_duration.clone()))?;
        registry.register(Box::new(retrieval_accuracy.clone()))?;
        registry.register(Box::new(context_items_retrieved.clone()))?;
        registry.register(Box::new(openai_requests.clone()))?;
        registry.register(Box::new(openai_errors.clone()))?;
        registry.register(Box::new(openai_latency.clone()))?;
        registry.register(Box::new(openai_tokens_used.clone()))?;
        registry.register(Box::new(cache_hits.clone()))?;
        registry.register(Box::new(cache_misses.clone()))?;
        registry.register(Box::new(cache_hit_rate.clone()))?;
        registry.register(Box::new(memory_usage.clone()))?;
        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(active_connections.clone()))?;
        
        Ok(Self {
            query_total,
            query_duration,
            query_errors,
            retrieval_duration,
            retrieval_accuracy,
            context_items_retrieved,
            openai_requests,
            openai_errors,
            openai_latency,
            openai_tokens_used,
            cache_hits,
            cache_misses,
            cache_hit_rate,
            memory_usage,
            cpu_usage,
            active_connections,
        })
    }
    
    pub fn record_query(&self, duration: Duration, context_count: usize, success: bool) {
        self.query_total.inc();
        self.query_duration.observe(duration.as_secs_f64());
        self.context_items_retrieved.observe(context_count as f64);
        
        if !success {
            self.query_errors.inc();
        }
    }
    
    pub fn record_cache_hit(&self, hit: bool) {
        if hit {
            self.cache_hits.inc();
        } else {
            self.cache_misses.inc();
        }
        
        let total_hits = self.cache_hits.get();
        let total_misses = self.cache_misses.get();
        let hit_rate = total_hits / (total_hits + total_misses);
        self.cache_hit_rate.set(hit_rate);
    }
}
```

---

## 5. Testing and Validation Framework

### 5.1 Integration Test Structure

```rust
// tests/integration/rag_integration_test.rs
use std::sync::Arc;
use tokio::test;
use codegraph_rag::{RAGModule, RAGConfig, CodeQuery};

#[tokio::test]
async fn test_end_to_end_code_intelligence() {
    let config = RAGConfig::test_config();
    let rag_module = Arc::new(RAGModule::new(config).await.unwrap());
    
    let query = CodeQuery {
        text: "How do I implement a binary search tree in Rust?".to_string(),
        language: Some("rust".to_string()),
        context_limit: 5,
        include_definitions: true,
        include_references: true,
    };
    
    let response = rag_module.process_query(query).await.unwrap();
    
    assert!(!response.generated_response.is_empty());
    assert!(!response.contexts.is_empty());
    assert!(response.confidence_score > 0.5);
    assert!(response.metrics.latency.as_millis() < 100); // Sub-100ms target
}

#[tokio::test]
async fn test_semantic_search_accuracy() {
    let config = RAGConfig::test_config();
    let rag_module = Arc::new(RAGModule::new(config).await.unwrap());
    
    // Index test code snippets
    let test_snippets = vec![
        CodeSnippet {
            content: "fn binary_search<T: Ord>(arr: &[T], target: &T) -> Option<usize> { ... }".to_string(),
            language: "rust".to_string(),
            file_path: "search.rs".to_string(),
        },
        // More test snippets...
    ];
    
    for snippet in test_snippets {
        rag_module.index_code_snippet(snippet).await.unwrap();
    }
    
    let search_request = SemanticSearchRequest {
        query: "binary search implementation".to_string(),
        limit: 10,
        threshold: 0.7,
        filters: SearchFilters::default(),
    };
    
    let results = rag_module.semantic_search(search_request).await.unwrap();
    
    assert!(!results.is_empty());
    assert!(results[0].relevance_score > 0.8);
    assert!(results[0].content.contains("binary_search"));
}

#[tokio::test]
async fn test_context_optimization() {
    let config = RAGConfig::test_config();
    let rag_module = Arc::new(RAGModule::new(config).await.unwrap());
    
    // Create a large context that exceeds token limit
    let large_contexts = generate_large_context_set(1000); // 1000 context items
    
    let optimized = rag_module.context_manager
        .optimize_context(large_contexts, &test_query())
        .await
        .unwrap();
    
    assert!(optimized.total_tokens <= rag_module.context_manager.max_context_size);
    assert!(optimized.compression_ratio > 0.0);
    assert!(!optimized.contexts.is_empty());
}

#[tokio::test]
async fn test_openai_integration_resilience() {
    let mut config = RAGConfig::test_config();
    config.openai_config.base_urls = vec![
        "https://api.openai.com/v1".to_string(),
        "https://backup-api.openai.com/v1".to_string(), // Mock backup
    ];
    
    let rag_module = Arc::new(RAGModule::new(config).await.unwrap());
    
    // Simulate primary API failure
    rag_module.openai_client.mark_unhealthy(&rag_module.openai_client.primary_client).await;
    
    let query = CodeQuery::test_query();
    let response = rag_module.process_query(query).await.unwrap();
    
    assert!(!response.generated_response.is_empty());
    // Should have used fallback client
    assert_eq!(response.metrics.api_client_used, "backup");
}
```

### 5.2 Performance Benchmarks

```rust
// benches/rag_performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codegraph_rag::*;

fn benchmark_query_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rag_module = rt.block_on(async {
        Arc::new(RAGModule::new(RAGConfig::benchmark_config()).await.unwrap())
    });
    
    let queries = vec![
        "How to implement quicksort?",
        "What is the difference between HashMap and BTreeMap?",
        "How to handle errors in Rust?",
        "Explain async/await in Rust",
        "How to write unit tests?",
    ];
    
    let mut group = c.benchmark_group("query_latency");
    
    for query in queries {
        group.bench_with_input(
            BenchmarkId::new("process_query", query),
            &query,
            |b, &query| {
                b.to_async(&rt).iter(|| async {
                    let code_query = CodeQuery {
                        text: query.to_string(),
                        language: Some("rust".to_string()),
                        context_limit: 10,
                        include_definitions: true,
                        include_references: true,
                    };
                    
                    black_box(rag_module.process_query(code_query).await.unwrap())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_semantic_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rag_module = rt.block_on(async {
        let module = Arc::new(RAGModule::new(RAGConfig::benchmark_config()).await.unwrap());
        
        // Index 10,000 code snippets for realistic benchmark
        for i in 0..10000 {
            let snippet = generate_test_snippet(i);
            module.index_code_snippet(snippet).await.unwrap();
        }
        
        module
    });
    
    c.bench_function("semantic_search_10k", |b| {
        b.to_async(&rt).iter(|| async {
            let request = SemanticSearchRequest {
                query: "binary search implementation".to_string(),
                limit: 20,
                threshold: 0.7,
                filters: SearchFilters::default(),
            };
            
            black_box(rag_module.semantic_search(request).await.unwrap())
        })
    });
}

fn benchmark_context_optimization(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let rag_module = rt.block_on(async {
        Arc::new(RAGModule::new(RAGConfig::benchmark_config()).await.unwrap())
    });
    
    let context_sizes = vec![10, 50, 100, 500, 1000];
    let mut group = c.benchmark_group("context_optimization");
    
    for size in context_sizes {
        group.bench_with_input(
            BenchmarkId::new("optimize_context", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let contexts = generate_contexts(size);
                    let query = CodeQuery::test_query();
                    
                    black_box(
                        rag_module.context_manager
                            .optimize_context(contexts, &query)
                            .await
                            .unwrap()
                    )
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_query_latency,
    benchmark_semantic_search,
    benchmark_context_optimization
);
criterion_main!(benches);
```

---

## 6. Deployment Configuration

### 6.1 Docker Integration

```dockerfile
# Dockerfile
FROM rust:1.75-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY tests ./tests

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Build release binary
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/codegraph-rag ./
COPY config ./config
COPY models ./models

# Create non-root user
RUN useradd -r -u 1000 codegraph
RUN chown -R codegraph:codegraph /app
USER codegraph

EXPOSE 8080 9090
CMD ["./codegraph-rag", "--config", "config/production.toml"]
```

### 6.2 Kubernetes Deployment

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codegraph-rag
  labels:
    app: codegraph-rag
spec:
  replicas: 3
  selector:
    matchLabels:
      app: codegraph-rag
  template:
    metadata:
      labels:
        app: codegraph-rag
    spec:
      containers:
      - name: codegraph-rag
        image: codegraph-rag:latest
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        - name: OPENAI_API_KEY
          valueFrom:
            secretKeyRef:
              name: openai-secret
              key: api-key
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: config
          mountPath: /app/config
        - name: models
          mountPath: /app/models
      volumes:
      - name: config
        configMap:
          name: codegraph-config
      - name: models
        persistentVolumeClaim:
          claimName: models-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: codegraph-rag-service
spec:
  selector:
    app: codegraph-rag
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  type: LoadBalancer
```

---

## 7. Configuration Management

### 7.1 Environment-Specific Configurations

```toml
# config/production.toml
[server]
host = "0.0.0.0"
port = 8080
metrics_port = 9090

[rag]
max_concurrent_queries = 1000
query_timeout_ms = 30000

[vector_store]
engine = "FAISS"
dimension = 1536
index_type = "HNSW"
similarity_metric = "Cosine"
cache_size_mb = 512

[graph_store]
backend = "RocksDB"
path = "/app/data/graph"
max_open_files = 1000

[openai]
api_keys = ["${OPENAI_API_KEY}"]
base_urls = ["https://api.openai.com/v1"]
model = "gpt-4"
max_tokens = 4096
temperature = 0.1
timeout_seconds = 30
max_retries = 3

[cache]
l1_max_entries = 10000
ttl_seconds = 3600
redis_url = "${REDIS_URL}"

[logging]
level = "info"
format = "json"
file = "/app/logs/codegraph-rag.log"

[metrics]
enabled = true
endpoint = "/metrics"
namespace = "codegraph_rag"
```

```toml
# config/development.toml
[server]
host = "127.0.0.1"
port = 3000
metrics_port = 3001

[rag]
max_concurrent_queries = 10
query_timeout_ms = 60000

[vector_store]
engine = "InMemory"
dimension = 768
cache_size_mb = 64

[openai]
model = "gpt-3.5-turbo"
max_tokens = 2048
temperature = 0.3

[cache]
l1_max_entries = 1000
ttl_seconds = 300
# No Redis in development

[logging]
level = "debug"
format = "pretty"
```

This comprehensive integration specification provides the detailed implementation guidance needed to successfully integrate the RAG system into the CodeGraph architecture, ensuring high performance, reliability, and maintainability.