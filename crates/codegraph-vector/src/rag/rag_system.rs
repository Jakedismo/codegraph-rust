use codegraph_core::{CodeNode, NodeId, Result};
use crate::EmbeddingGenerator;
#[cfg(feature = "faiss")]
use crate::{SemanticSearch, FaissVectorStore};
use crate::rag::{
    QueryProcessor, ProcessedQuery, ContextRetriever, RetrievalConfig, 
    ResultRanker, RankingConfig, ResponseGenerator, GenerationConfig,
    GeneratedResponse, RankedResult
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGConfig {
    pub retrieval: RetrievalConfig,
    pub ranking: RankingConfig,
    pub generation: GenerationConfig,
    pub performance_target_ms: u64,
    pub cache_size: usize,
    pub enable_metrics: bool,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self {
            retrieval: RetrievalConfig::default(),
            ranking: RankingConfig::default(),
            generation: GenerationConfig::default(),
            performance_target_ms: 200,
            cache_size: 1000,
            enable_metrics: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_id: NodeId,
    pub original_query: String,
    pub processed_query: ProcessedQuery,
    pub retrieved_results: Vec<RankedResult>,
    pub response: String,
    pub confidence_score: f32,
    pub context_used: Vec<String>,
    pub processing_time_ms: u64,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub query_processing_ms: u64,
    pub context_retrieval_ms: u64,
    pub result_ranking_ms: u64,
    pub response_generation_ms: u64,
    pub total_processing_ms: u64,
    pub cache_hits: u32,
    pub results_retrieved: usize,
    pub results_ranked: usize,
}

pub struct RAGSystem {
    config: RAGConfig,
    query_processor: QueryProcessor,
    context_retriever: Arc<RwLock<ContextRetriever>>,
    result_ranker: Arc<RwLock<ResultRanker>>,
    response_generator: ResponseGenerator,
    #[cfg(feature = "faiss")]
    semantic_search: Option<Arc<SemanticSearch>>,
    embedding_generator: Arc<EmbeddingGenerator>,
    query_cache: Arc<RwLock<HashMap<String, QueryResult>>>,
    metrics: Arc<RwLock<SystemMetrics>>,
}

#[derive(Debug, Default)]
struct SystemMetrics {
    total_queries: u64,
    average_response_time_ms: f64,
    cache_hit_rate: f64,
    successful_queries: u64,
    failed_queries: u64,
}

impl RAGSystem {
    pub async fn new(config: RAGConfig) -> Result<Self> {
        info!("Initializing RAG system with config: {:?}", config);

        let query_processor = QueryProcessor::new();
        let context_retriever = Arc::new(RwLock::new(ContextRetriever::with_config(config.retrieval.clone())));
        let result_ranker = Arc::new(RwLock::new(ResultRanker::with_config(config.ranking.clone())));
        let response_generator = ResponseGenerator::with_config(config.generation.clone());
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        
        Ok(Self {
            config,
            query_processor,
            context_retriever,
            result_ranker,
            response_generator,
            #[cfg(feature = "faiss")]
            semantic_search: None,
            embedding_generator,
            query_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(SystemMetrics::default())),
        })
    }

    pub async fn initialize_vector_store(&mut self) -> Result<()> {
        #[cfg(feature = "faiss")]
        {
            let vector_store = Arc::new(FaissVectorStore::new(384)?);
            let semantic_search = Arc::new(SemanticSearch::new(
                vector_store,
                self.embedding_generator.clone(),
            ));
            
            {
                let mut retriever = self.context_retriever.write().await;
                retriever.set_semantic_search(semantic_search.clone());
            }
            
            self.semantic_search = Some(semantic_search);
            info!("Vector store initialized with FAISS backend");
        }
        
        #[cfg(not(feature = "faiss"))]
        {
            warn!("FAISS feature not enabled, semantic search will be limited");
        }
        
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn process_query(&self, query: &str) -> Result<QueryResult> {
        let query_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();
        
        debug!("Processing query: {} (ID: {})", query, query_id);

        // Check cache first
        if let Some(cached_result) = self.check_cache(query).await? {
            self.update_cache_metrics().await;
            return Ok(cached_result);
        }

        let mut performance_metrics = PerformanceMetrics {
            query_processing_ms: 0,
            context_retrieval_ms: 0,
            result_ranking_ms: 0,
            response_generation_ms: 0,
            total_processing_ms: 0,
            cache_hits: 0,
            results_retrieved: 0,
            results_ranked: 0,
        };

        // Step 1: Process the query
        let query_start = std::time::Instant::now();
        let processed_query = self.query_processor.analyze_query(query).await?;
        performance_metrics.query_processing_ms = query_start.elapsed().as_millis() as u64;

        // Step 2: Retrieve context
        let retrieval_start = std::time::Instant::now();
        let retrieval_results = {
            let retriever = self.context_retriever.read().await;
            retriever.retrieve_context(
                query,
                &processed_query.semantic_embedding,
                &processed_query.keywords,
            ).await?
        };
        performance_metrics.context_retrieval_ms = retrieval_start.elapsed().as_millis() as u64;
        performance_metrics.results_retrieved = retrieval_results.len();

        // Step 3: Rank results
        let ranking_start = std::time::Instant::now();
        let ranked_results = {
            let mut ranker = self.result_ranker.write().await;
            ranker.rank_results(
                retrieval_results,
                query,
                &processed_query.semantic_embedding,
            ).await?
        };
        performance_metrics.result_ranking_ms = ranking_start.elapsed().as_millis() as u64;
        performance_metrics.results_ranked = ranked_results.len();

        // Step 4: Generate response
        let generation_start = std::time::Instant::now();
        let generated_response = self.response_generator.generate_response(query, &ranked_results).await?;
        performance_metrics.response_generation_ms = generation_start.elapsed().as_millis() as u64;

        let total_time = start_time.elapsed();
        performance_metrics.total_processing_ms = total_time.as_millis() as u64;

        // Create result
        let result = QueryResult {
            query_id,
            original_query: query.to_string(),
            processed_query,
            retrieved_results: ranked_results,
            response: generated_response.answer,
            confidence_score: generated_response.confidence,
            context_used: generated_response.sources.iter()
                .map(|s| s.snippet.clone())
                .collect(),
            processing_time_ms: performance_metrics.total_processing_ms,
            performance_metrics,
        };

        // Cache the result
        self.cache_result(query, result.clone()).await?;

        // Update metrics
        self.update_query_metrics(&result).await;

        // Check performance target
        if result.processing_time_ms > self.config.performance_target_ms {
            warn!("Query processing exceeded target time: {}ms > {}ms", 
                result.processing_time_ms, self.config.performance_target_ms);
        }

        info!("Query processed successfully in {}ms (ID: {})", 
            result.processing_time_ms, query_id);

        Ok(result)
    }

    pub async fn add_context(&mut self, node: CodeNode) -> Result<()> {
        debug!("Adding context node: {}", node.name.as_str());
        
        {
            let mut retriever = self.context_retriever.write().await;
            retriever.add_node_to_cache(node.clone());
        }

        // Add to semantic search if available
        #[cfg(feature = "faiss")]
        if let Some(ref semantic_search) = self.semantic_search {
            if node.embedding.is_none() {
                // Generate embedding if not present
                let embedding = self.embedding_generator.generate_embedding(&node).await?;
                let mut updated_node = node;
                updated_node.embedding = Some(embedding);
                
                // TODO: Add to vector store
                debug!("Generated embedding for node: {}", updated_node.name.as_str());
            }
        }

        Ok(())
    }

    pub async fn retrieve_context(&self, query: &str, limit: usize) -> Result<Vec<crate::rag::RetrievalResult>> {
        let processed_query = self.query_processor.analyze_query(query).await?;
        
        let retriever = self.context_retriever.read().await;
        let mut results = retriever.retrieve_context(
            query,
            &processed_query.semantic_embedding,
            &processed_query.keywords,
        ).await?;

        results.truncate(limit);
        Ok(results)
    }

    pub async fn generate_response(&self, query: &str) -> Result<GeneratedResponse> {
        let retrieval_results = self.retrieve_context(query, self.config.retrieval.max_results).await?;
        
        // Convert retrieval results to ranked results for response generation
        let ranked_results: Vec<RankedResult> = retrieval_results.into_iter()
            .enumerate()
            .map(|(i, result)| RankedResult {
                retrieval_result: result,
                final_score: 1.0 - (i as f32 * 0.1), // Simple decreasing score
                score_breakdown: crate::rag::ScoreBreakdown {
                    semantic_score: 0.8,
                    keyword_score: 0.2,
                    recency_score: 0.0,
                    popularity_score: 0.0,
                    type_boost: 1.0,
                    diversity_penalty: 0.0,
                },
                rank: i + 1,
            })
            .collect();

        self.response_generator.generate_response(query, &ranked_results).await
    }

    async fn check_cache(&self, query: &str) -> Result<Option<QueryResult>> {
        let cache = self.query_cache.read().await;
        Ok(cache.get(query).cloned())
    }

    async fn cache_result(&self, query: &str, result: QueryResult) -> Result<()> {
        let mut cache = self.query_cache.write().await;
        
        // Implement simple LRU eviction if cache is full
        if cache.len() >= self.config.cache_size {
            // Remove oldest entry (simplified LRU)
            if let Some(key_to_remove) = cache.keys().next().cloned() {
                cache.remove(&key_to_remove);
            }
        }
        
        cache.insert(query.to_string(), result);
        Ok(())
    }

    async fn update_cache_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        // Update cache hit rate calculation
        let total = metrics.total_queries + 1;
        metrics.cache_hit_rate = (metrics.cache_hit_rate * (total - 1) as f64 + 1.0) / total as f64;
    }

    async fn update_query_metrics(&self, result: &QueryResult) {
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        
        if result.confidence_score > 0.5 {
            metrics.successful_queries += 1;
        } else {
            metrics.failed_queries += 1;
        }

        // Update average response time
        let total = metrics.total_queries;
        metrics.average_response_time_ms = (metrics.average_response_time_ms * (total - 1) as f64 + result.processing_time_ms as f64) / total as f64;
    }

    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let metrics = self.metrics.read().await;
        SystemMetrics {
            total_queries: metrics.total_queries,
            average_response_time_ms: metrics.average_response_time_ms,
            cache_hit_rate: metrics.cache_hit_rate,
            successful_queries: metrics.successful_queries,
            failed_queries: metrics.failed_queries,
        }
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.query_cache.write().await;
        cache.clear();
        info!("Query cache cleared");
    }

    pub async fn get_cache_size(&self) -> usize {
        let cache = self.query_cache.read().await;
        cache.len()
    }

    pub fn get_config(&self) -> &RAGConfig {
        &self.config
    }

    pub async fn update_popularity_scores(&self, node_access_counts: HashMap<String, u32>) {
        let mut ranker = self.result_ranker.write().await;
        ranker.update_popularity_scores(&node_access_counts);
        debug!("Updated popularity scores for {} nodes", node_access_counts.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, NodeType};

    fn create_test_node(name: &str, content: &str, node_type: NodeType) -> CodeNode {
        let now = chrono::Utc::now();
        CodeNode {
            id: Uuid::new_v4(),
            name: name.to_string(),
            node_type: Some(node_type),
            language: Some(Language::Rust),
            content: Some(content.to_string()),
            embedding: None,
            location: Location {
                file_path: "test.rs".to_string(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
            metadata: Metadata {
                attributes: std::collections::HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            complexity: None,
        }
    }

    #[tokio::test]
    async fn test_rag_system_initialization() {
        let config = RAGConfig::default();
        let rag_system = RAGSystem::new(config).await;
        
        assert!(rag_system.is_ok());
        let system = rag_system.unwrap();
        assert_eq!(system.get_cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_add_context_and_query() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config).await.unwrap();

        // Add test context
        let node = create_test_node("test_function", "fn test() -> i32 { 42 }", NodeType::Function);
        rag_system.add_context(node).await.unwrap();

        // Test query
        let result = rag_system.process_query("test function").await.unwrap();
        
        assert!(!result.response.is_empty());
        assert!(result.processing_time_ms < 1000); // Should be fast
        assert!(!result.query_id.is_nil());
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config).await.unwrap();

        let node = create_test_node("cached_function", "fn cached() {}", NodeType::Function);
        rag_system.add_context(node).await.unwrap();

        // First query
        let query = "cached function test";
        let result1 = rag_system.process_query(query).await.unwrap();
        
        // Second query (should be cached)
        let result2 = rag_system.process_query(query).await.unwrap();
        
        assert_eq!(result1.query_id, result2.query_id);
        assert_eq!(result1.response, result2.response);
        
        // Cache should have 1 entry
        assert_eq!(rag_system.get_cache_size().await, 1);
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config).await.unwrap();

        let node = create_test_node("perf_test", "fn performance_test() {}", NodeType::Function);
        rag_system.add_context(node).await.unwrap();

        let result = rag_system.process_query("performance test").await.unwrap();
        
        assert!(result.performance_metrics.query_processing_ms > 0);
        assert!(result.performance_metrics.context_retrieval_ms >= 0);
        assert!(result.performance_metrics.result_ranking_ms >= 0);
        assert!(result.performance_metrics.response_generation_ms > 0);
        assert_eq!(
            result.performance_metrics.total_processing_ms,
            result.processing_time_ms
        );
    }

    #[tokio::test]
    async fn test_system_metrics() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config).await.unwrap();

        let node = create_test_node("metrics_test", "fn metrics() {}", NodeType::Function);
        rag_system.add_context(node).await.unwrap();

        // Process a few queries
        rag_system.process_query("test query 1").await.unwrap();
        rag_system.process_query("test query 2").await.unwrap();

        let metrics = rag_system.get_system_metrics().await;
        
        assert_eq!(metrics.total_queries, 2);
        assert!(metrics.average_response_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_response_generation() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config).await.unwrap();

        let nodes = vec![
            create_test_node("read_file", "fn read_file(path: &str) -> String", NodeType::Function),
            create_test_node("write_file", "fn write_file(path: &str, content: &str)", NodeType::Function),
        ];

        for node in nodes {
            rag_system.add_context(node).await.unwrap();
        }

        let response = rag_system.generate_response("file operations").await.unwrap();
        
        assert!(!response.answer.is_empty());
        assert!(response.confidence > 0.0);
        assert!(!response.sources.is_empty());
        assert!(response.processing_time_ms < 200); // Sub-200ms requirement
    }
}
