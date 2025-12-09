// ABOUTME: LLM tool executor for SurrealDB graph analysis functions
// ABOUTME: Executes graph analysis tools by calling Rust SDK wrappers with validated parameters

use codegraph_core::config_manager::CodeGraphConfig;
use codegraph_graph::GraphFunctions;
use codegraph_mcp_core::debug_logger::DebugLogger;
use codegraph_mcp_core::error::{McpError, Result};
use codegraph_vector::reranking::{factory::create_reranker, RerankDocument, Reranker};
use codegraph_vector::EmbeddingGenerator;
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::{debug, info};

const TOOL_PROGRESS_LOG_TARGET: &str = "codegraph::mcp::tools";

use crate::graph_tool_schemas::GraphToolSchemas;

/// Statistics about LRU cache performance
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    /// Number of cache hits (successful lookups)
    pub hits: u64,
    /// Number of cache misses (lookups that required SurrealDB call)
    pub misses: u64,
    /// Number of entries evicted due to LRU policy
    pub evictions: u64,
    /// Current number of entries in cache
    pub current_size: usize,
    /// Maximum cache size (capacity)
    pub max_size: usize,
}

impl CacheStats {
    /// Calculate cache hit rate as percentage
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Executor for graph analysis tools
/// Receives tool calls from LLM and executes appropriate SurrealDB functions
pub struct GraphToolExecutor {
    graph_functions: Arc<GraphFunctions>,
    /// Configuration for embedding and reranking
    config: Arc<CodeGraphConfig>,
    /// Shared embedding generator (created once, reused for all queries)
    embedding_generator: Arc<EmbeddingGenerator>,
    /// LRU cache for tool results (function_name + params → result)
    cache: Arc<Mutex<LruCache<String, JsonValue>>>,
    /// Cache statistics for observability
    cache_stats: Arc<Mutex<CacheStats>>,
    /// Whether caching is enabled
    cache_enabled: bool,
    /// Reranker for semantic search result refinement
    reranker: Option<Arc<dyn Reranker>>,
    /// Maximum result size in bytes (derived from LLM context window)
    /// Prevents sending oversized results that would exceed model limits
    max_result_bytes: usize,
}

/// Default max result bytes when context window not specified (~200KB)
const DEFAULT_MAX_RESULT_BYTES: usize = 200_000;

impl GraphToolExecutor {
    /// Create a new tool executor with shared EmbeddingGenerator
    /// Uses CODEGRAPH_CONTEXT_WINDOW env var to derive max result size
    pub fn new(
        graph_functions: Arc<GraphFunctions>,
        config: Arc<CodeGraphConfig>,
        embedding_generator: Arc<EmbeddingGenerator>,
    ) -> Self {
        // Read context window from environment to derive max result bytes
        let context_window = std::env::var("CODEGRAPH_CONTEXT_WINDOW")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(128_000);

        Self::with_context_window(graph_functions, config, embedding_generator, context_window)
    }

    /// Create a new tool executor with explicit context window for result size limiting
    /// max_result_bytes = context_window * 2 (conservative estimate: ~50% of context for results at 4 bytes/token)
    pub fn with_context_window(
        graph_functions: Arc<GraphFunctions>,
        config: Arc<CodeGraphConfig>,
        embedding_generator: Arc<EmbeddingGenerator>,
        context_window: usize,
    ) -> Self {
        // Calculate max result bytes: use ~50% of context window, assuming ~4 chars per token
        // This leaves room for system prompt, conversation history, and response
        let max_result_bytes = context_window.saturating_mul(2);

        Self::with_limits(graph_functions, config, embedding_generator, true, 100, max_result_bytes)
    }

    /// Create a new tool executor with custom cache and result size configuration
    pub fn with_limits(
        graph_functions: Arc<GraphFunctions>,
        config: Arc<CodeGraphConfig>,
        embedding_generator: Arc<EmbeddingGenerator>,
        cache_enabled: bool,
        cache_size: usize,
        max_result_bytes: usize,
    ) -> Self {
        let capacity = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(100).unwrap());
        let cache = Arc::new(Mutex::new(LruCache::new(capacity)));
        let cache_stats = Arc::new(Mutex::new(CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size: cache_size,
        }));

        // Initialize reranker from config
        let reranker = create_reranker(&config.rerank).ok().flatten();

        if let Some(ref reranker) = reranker {
            info!(
                "Reranker initialized: {} ({})",
                reranker.model_name(),
                reranker.provider_name()
            );
        }

        info!(
            "GraphToolExecutor initialized with max_result_bytes: {} ({:.1}MB)",
            max_result_bytes,
            max_result_bytes as f64 / 1_000_000.0
        );

        Self {
            graph_functions,
            config,
            embedding_generator,
            cache,
            cache_stats,
            cache_enabled,
            reranker,
            max_result_bytes,
        }
    }

    /// Create a new tool executor with custom cache configuration (legacy compatibility)
    pub fn with_cache(
        graph_functions: Arc<GraphFunctions>,
        config: Arc<CodeGraphConfig>,
        embedding_generator: Arc<EmbeddingGenerator>,
        cache_enabled: bool,
        cache_size: usize,
    ) -> Self {
        Self::with_limits(graph_functions, config, embedding_generator, cache_enabled, cache_size, DEFAULT_MAX_RESULT_BYTES)
    }

    /// Get current cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        self.cache_stats.lock().clone()
    }

    /// Expose underlying graph functions for read-only summaries
    pub fn graph_functions(&self) -> Arc<GraphFunctions> {
        self.graph_functions.clone()
    }

    /// Clear the cache and reset statistics
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
        let mut stats = self.cache_stats.lock();
        stats.hits = 0;
        stats.misses = 0;
        stats.evictions = 0;
        stats.current_size = 0;
    }

    /// Truncate result if it exceeds max_result_bytes to prevent context window overflow
    /// Returns truncated result with warning metadata if truncation occurred
    fn truncate_if_oversized(&self, tool_name: &str, result: JsonValue) -> JsonValue {
        let serialized = result.to_string();
        let result_bytes = serialized.len();

        if result_bytes <= self.max_result_bytes {
            return result;
        }

        // Result is oversized - need to truncate
        let overflow_ratio = result_bytes as f64 / self.max_result_bytes as f64;
        tracing::warn!(
            tool = tool_name,
            result_bytes = result_bytes,
            max_bytes = self.max_result_bytes,
            overflow_ratio = format!("{:.1}x", overflow_ratio),
            "Tool result exceeds max_result_bytes limit, truncating"
        );

        // Strategy: Try to truncate the "result" field if it's an array
        if let Some(result_array) = result.get("result").and_then(|r| r.as_array()) {
            // Calculate how many items we can keep
            let item_count = result_array.len();
            if item_count == 0 {
                return result;
            }

            // Estimate bytes per item (rough average)
            let bytes_per_item = result_bytes / item_count;
            let max_items = self.max_result_bytes / bytes_per_item.max(1);
            let keep_items = max_items.min(item_count).max(1); // Keep at least 1

            // Create truncated result
            let truncated_array: Vec<JsonValue> = result_array.iter().take(keep_items).cloned().collect();
            let truncated_count = item_count - keep_items;

            tracing::info!(
                tool = tool_name,
                original_items = item_count,
                kept_items = keep_items,
                truncated_items = truncated_count,
                "Truncated result array to fit context window"
            );

            // Reconstruct the result with truncation metadata
            let mut truncated_result = result.clone();
            if let Some(obj) = truncated_result.as_object_mut() {
                obj.insert("result".to_string(), JsonValue::Array(truncated_array));
                obj.insert("_truncated".to_string(), json!({
                    "original_items": item_count,
                    "kept_items": keep_items,
                    "truncated_items": truncated_count,
                    "reason": "Result exceeded context window limit",
                    "max_bytes": self.max_result_bytes
                }));
            }

            return truncated_result;
        }

        // Fallback: If we can't smart-truncate, just return with a warning
        // This shouldn't happen often since most results are arrays
        tracing::error!(
            tool = tool_name,
            result_bytes = result_bytes,
            "Cannot truncate non-array result, returning as-is (may cause context overflow)"
        );
        result
    }

    /// Generate a cache key from project, tool name, and parameters
    fn cache_key(project_id: &str, tool_name: &str, parameters: &JsonValue) -> String {
        // Create deterministic key from project + function name + serialized params
        format!("{}:{}:{}", project_id, tool_name, parameters.to_string())
    }

    /// Execute a tool call from LLM
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to execute
    /// * `parameters` - JSON parameters for the tool
    ///
    /// # Returns
    /// JSON result from the tool execution
    pub async fn execute(&self, tool_name: &str, parameters: JsonValue) -> Result<JsonValue> {
        log_tool_call_start(tool_name, &parameters);

        let exec_result: Result<JsonValue> = async {
            // Validate tool exists
            let _schema = GraphToolSchemas::get_by_name(tool_name)
                .ok_or_else(|| McpError::Protocol(format!("Unknown tool: {}", tool_name)))?;

            let project_id = self.graph_functions.project_id();

            // Check cache if enabled
            if self.cache_enabled {
                let cache_key = Self::cache_key(project_id, tool_name, &parameters);

                // Try cache lookup
                {
                    let mut cache = self.cache.lock();
                    if let Some(cached_result) = cache.get(&cache_key) {
                        // Cache hit
                        let mut stats = self.cache_stats.lock();
                        stats.hits += 1;
                        debug!("Cache hit for {}: {}", tool_name, cache_key);
                        let cached = cached_result.clone();
                        log_tool_call_finish(tool_name, &cached);
                        return Ok(cached);
                    }
                }

                // Cache miss - record it
                {
                    let mut stats = self.cache_stats.lock();
                    stats.misses += 1;
                }
                debug!("Cache miss for {}: {}", tool_name, cache_key);
            }

            // Execute based on tool name
            let result = match tool_name {
                "get_transitive_dependencies" => {
                    self.execute_get_transitive_dependencies(parameters.clone())
                        .await?
                }
                "detect_circular_dependencies" => {
                    self.execute_detect_circular_dependencies(parameters.clone())
                        .await?
                }
                "trace_call_chain" => self.execute_trace_call_chain(parameters.clone()).await?,
                "calculate_coupling_metrics" => {
                    self.execute_calculate_coupling_metrics(parameters.clone())
                        .await?
                }
                "get_hub_nodes" => self.execute_get_hub_nodes(parameters.clone()).await?,
                "get_reverse_dependencies" => {
                    self.execute_get_reverse_dependencies(parameters.clone())
                        .await?
                }
                "semantic_code_search" => {
                    self.execute_semantic_code_search(parameters.clone())
                        .await?
                }
                "find_complexity_hotspots" => {
                    self.execute_find_complexity_hotspots(parameters.clone())
                        .await?
                }
                _ => {
                    return Err(
                        McpError::Protocol(format!("Tool not implemented: {}", tool_name)).into(),
                    );
                }
            };

            // Apply result size limiting to prevent context window overflow
            let result = self.truncate_if_oversized(tool_name, result);

            // Cache the result if enabled
            if self.cache_enabled {
                let cache_key = Self::cache_key(project_id, tool_name, &parameters);
                let mut cache = self.cache.lock();
                let was_evicted = cache.len() >= cache.cap().get();
                cache.put(cache_key, result.clone());

                // Update stats
                let mut stats = self.cache_stats.lock();
                if was_evicted {
                    stats.evictions += 1;
                }
                stats.current_size = cache.len();
            }

            Ok(result)
        }
        .await;

        match exec_result {
            Ok(result) => {
                log_tool_call_finish(tool_name, &result);
                Ok(result)
            }
            Err(err) => {
                DebugLogger::log_tool_error(tool_name, &parameters, &format!("{}", err));
                Err(err)
            }
        }
    }

    /// Execute get_transitive_dependencies
    async fn execute_get_transitive_dependencies(&self, params: JsonValue) -> Result<JsonValue> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing node_id".to_string()))?;

        // Default to "Calls" if edge_type not provided (for LATS compatibility)
        let edge_type = params["edge_type"].as_str().unwrap_or("Calls");

        let depth = params["depth"].as_i64().unwrap_or(3) as i32;

        let result = self
            .graph_functions
            .get_transitive_dependencies(node_id, edge_type, depth)
            .await
            .map_err(|e| {
                McpError::Protocol(format!("get_transitive_dependencies failed: {}", e))
            })?;

        Ok(json!({
            "tool": "get_transitive_dependencies",
            "parameters": {
                "node_id": node_id,
                "edge_type": edge_type,
                "depth": depth
            },
            "result": result
        }))
    }

    /// Execute detect_circular_dependencies
    async fn execute_detect_circular_dependencies(&self, params: JsonValue) -> Result<JsonValue> {
        let edge_type = params["edge_type"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing edge_type".to_string()))?;

        let result = self
            .graph_functions
            .detect_circular_dependencies(edge_type)
            .await
            .map_err(|e| {
                McpError::Protocol(format!("detect_circular_dependencies failed: {}", e))
            })?;

        Ok(json!({
            "tool": "detect_circular_dependencies",
            "parameters": {
                "edge_type": edge_type
            },
            "result": result
        }))
    }

    /// Execute trace_call_chain
    async fn execute_trace_call_chain(&self, params: JsonValue) -> Result<JsonValue> {
        // Accept both "from_node" (canonical) and "node_id" (common pattern) for compatibility
        let from_node = params["from_node"]
            .as_str()
            .or_else(|| params["node_id"].as_str())
            .ok_or_else(|| McpError::Protocol("Missing from_node or node_id".to_string()))?;

        let max_depth = params["max_depth"].as_i64().unwrap_or(5) as i32;

        let result = self
            .graph_functions
            .trace_call_chain(from_node, max_depth)
            .await
            .map_err(|e| McpError::Protocol(format!("trace_call_chain failed: {}", e)))?;

        Ok(json!({
            "tool": "trace_call_chain",
            "parameters": {
                "from_node": from_node,
                "max_depth": max_depth
            },
            "result": result
        }))
    }

    /// Execute calculate_coupling_metrics
    async fn execute_calculate_coupling_metrics(&self, params: JsonValue) -> Result<JsonValue> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing node_id".to_string()))?;

        // Optional project_id override to reduce mismatches between indexing and runtime
        let gf = if let Some(project_id) = params["project_id"].as_str() {
            self.graph_functions.with_project_id(project_id)
        } else {
            (*self.graph_functions).clone()
        };

        let result = gf
            .calculate_coupling_metrics(node_id)
            .await
            .map_err(|e| McpError::Protocol(format!("calculate_coupling_metrics failed: {}", e)))?;

        Ok(json!({
            "tool": "calculate_coupling_metrics",
            "parameters": {
                "node_id": node_id
            },
            "result": result
        }))
    }

    /// Execute get_hub_nodes
    async fn execute_get_hub_nodes(&self, params: JsonValue) -> Result<JsonValue> {
        let min_degree = params["min_degree"].as_i64().unwrap_or(5) as i32;

        let result = self
            .graph_functions
            .get_hub_nodes(min_degree)
            .await
            .map_err(|e| McpError::Protocol(format!("get_hub_nodes failed: {}", e)))?;

        Ok(json!({
            "tool": "get_hub_nodes",
            "parameters": {
                "min_degree": min_degree
            },
            "result": result
        }))
    }

    /// Execute get_reverse_dependencies
    async fn execute_get_reverse_dependencies(&self, params: JsonValue) -> Result<JsonValue> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing node_id".to_string()))?;

        // Default to "Calls" if edge_type not provided (for LATS compatibility)
        let edge_type = params["edge_type"].as_str().unwrap_or("Calls");

        let depth = params["depth"].as_i64().unwrap_or(3) as i32;

        let result = self
            .graph_functions
            .get_reverse_dependencies(node_id, edge_type, depth)
            .await
            .map_err(|e| McpError::Protocol(format!("get_reverse_dependencies failed: {}", e)))?;

        Ok(json!({
            "tool": "get_reverse_dependencies",
            "parameters": {
                "node_id": node_id,
                "edge_type": edge_type,
                "depth": depth
            },
            "result": result
        }))
    }

    /// Execute semantic code search with HNSW, full-text, and graph enrichment
    /// Accepts natural language queries for comprehensive semantic search
    async fn execute_semantic_code_search(&self, params: JsonValue) -> Result<JsonValue> {
        let query_text = params["query"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing query".to_string()))?;

        let limit = params["limit"].as_i64().unwrap_or(10) as usize;
        let threshold = params["threshold"]
            .as_f64()
            .or_else(|| {
                std::env::var("CODEGRAPH_SEMSEARCH_THRESHOLD")
                    .ok()?
                    .parse::<f64>()
                    .ok()
            })
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or(0.6);

        // Step 1: Generate embedding using shared EmbeddingGenerator
        let query_embedding = self
            .embedding_generator
            .generate_text_embedding(query_text)
            .await
            .map_err(|e| McpError::Protocol(format!("Embedding generation failed: {}", e)))?;

        // Step 2: Get embedding dimension from shared generator (auto-detected)
        let dimension = self.embedding_generator.dimension();

        // Step 3: Call semantic search function with graph enrichment (always enabled)
        let include_graph_context = true; // Always enabled per requirements

        let candidates = self
            .graph_functions
            .semantic_search_with_context(
                query_text,
                &query_embedding,
                dimension,
                limit,
                threshold as f32,
                include_graph_context,
            )
            .await
            .map_err(|e| {
                McpError::Protocol(format!("semantic_search_with_context failed: {}", e))
            })?;

        // Step 4: Apply reranking if configured (Jina OR LM Studio)
        let final_results = self.apply_reranking(query_text, candidates).await?;

        Ok(json!({
            "tool": "semantic_code_search",
            "parameters": {
                "query": query_text,
                "limit": limit,
                "dimension": dimension,
                "threshold": threshold
            },
            "result": final_results
        }))
    }

    /// Execute find_complexity_hotspots - find functions with high complexity and coupling
    async fn execute_find_complexity_hotspots(&self, params: JsonValue) -> Result<JsonValue> {
        let min_complexity = params["min_complexity"].as_f64().unwrap_or(5.0) as f32;
        let limit = params["limit"].as_i64().unwrap_or(20) as i32;

        let result = self
            .graph_functions
            .get_complexity_hotspots(min_complexity, limit)
            .await
            .map_err(|e| McpError::Protocol(format!("find_complexity_hotspots failed: {}", e)))?;

        Ok(json!({
            "tool": "find_complexity_hotspots",
            "parameters": {
                "min_complexity": min_complexity,
                "limit": limit
            },
            "result": result
        }))
    }

    /// Apply reranking if configured using text-based reranking system
    async fn apply_reranking(
        &self,
        query: &str,
        candidates: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        if let Some(ref reranker) = self.reranker {
            let top_n = self.config.rerank.top_n;

            // Convert candidates to RerankDocuments
            let documents: Vec<RerankDocument> = candidates
                .iter()
                .enumerate()
                .map(|(idx, candidate)| RerankDocument {
                    id: idx.to_string(),
                    text: Self::extract_text_from_candidate(candidate),
                    metadata: Some(candidate.clone()),
                })
                .collect();

            // Rerank using the configured provider
            let results = reranker
                .rerank(query, documents, top_n)
                .await
                .map_err(|e| McpError::Protocol(format!("Reranking failed: {}", e)))?;

            // Convert back to original format
            let reranked: Vec<serde_json::Value> =
                results.into_iter().filter_map(|r| r.metadata).collect();

            Ok(reranked)
        } else {
            // No reranking configured
            Ok(candidates)
        }
    }

    /// Extract text content from a candidate for reranking
    fn extract_text_from_candidate(candidate: &serde_json::Value) -> String {
        let mut text_parts = Vec::new();

        if let Some(name) = candidate.get("name").and_then(|v| v.as_str()) {
            text_parts.push(name.to_string());
        }
        if let Some(content) = candidate.get("content").and_then(|v| v.as_str()) {
            text_parts.push(content.to_string());
        }
        if let Some(file_path) = candidate.get("file_path").and_then(|v| v.as_str()) {
            text_parts.push(format!("File: {}", file_path));
        }

        text_parts.join(" ")
    }

    /// Get all available tool schemas for registration
    pub fn get_tool_schemas() -> Vec<crate::ToolSchema> {
        GraphToolSchemas::all()
    }

    /// Get tool names for listing
    pub fn get_tool_names() -> Vec<String> {
        GraphToolSchemas::tool_names()
    }
}

fn log_tool_call_start(tool_name: &str, parameters: &JsonValue) {
    info!(
        target: TOOL_PROGRESS_LOG_TARGET,
        tool = tool_name,
        "Tool call started"
    );
    debug!(
        target: TOOL_PROGRESS_LOG_TARGET,
        tool = tool_name,
        "Tool input payload: {}",
        parameters
    );

    // Debug logging to file if enabled
    DebugLogger::log_tool_start(tool_name, parameters);
}

fn log_tool_call_finish(tool_name: &str, result: &JsonValue) {
    info!(
        target: TOOL_PROGRESS_LOG_TARGET,
        tool = tool_name,
        "Tool call completed"
    );
    debug!(
        target: TOOL_PROGRESS_LOG_TARGET,
        tool = tool_name,
        "Tool output payload: {}",
        result
    );

    // Debug logging to file if enabled
    DebugLogger::log_tool_finish(tool_name, result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_available() {
        let schemas = GraphToolExecutor::get_tool_schemas();
        assert_eq!(schemas.len(), 8);
    }

    #[test]
    fn test_tool_names() {
        let names = GraphToolExecutor::get_tool_names();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&"get_transitive_dependencies".to_string()));
        assert!(names.contains(&"find_complexity_hotspots".to_string()));
    }

    #[test]
    fn test_parameter_extraction() {
        let params = json!({
            "node_id": "nodes:123",
            "edge_type": "Calls",
            "depth": 5
        });

        assert_eq!(params["node_id"].as_str().unwrap(), "nodes:123");
        assert_eq!(params["edge_type"].as_str().unwrap(), "Calls");
        assert_eq!(params["depth"].as_i64().unwrap(), 5);
    }

    // === Cache Tests ===

    #[test]
    fn test_cache_key_generation() {
        let params1 = json!({
            "node_id": "nodes:123",
            "edge_type": "Calls",
            "depth": 3
        });
        let params2 = json!({
            "node_id": "nodes:123",
            "edge_type": "Calls",
            "depth": 3
        });
        let params3 = json!({
            "node_id": "nodes:456",
            "edge_type": "Calls",
            "depth": 3
        });

        let project = "proj-a";

        let key1 = GraphToolExecutor::cache_key(project, "get_transitive_dependencies", &params1);
        let key2 = GraphToolExecutor::cache_key(project, "get_transitive_dependencies", &params2);
        let key3 = GraphToolExecutor::cache_key(project, "get_transitive_dependencies", &params3);

        // Same params should generate same key
        assert_eq!(key1, key2);
        // Different params should generate different key
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_key_includes_project_scope() {
        let params = json!({
            "node_id": "nodes:123"
        });

        let key_a = GraphToolExecutor::cache_key("proj-a", "get_hub_nodes", &params);
        let key_b = GraphToolExecutor::cache_key("proj-b", "get_hub_nodes", &params);

        assert_ne!(key_a, key_b);
        assert!(
            key_a.starts_with("proj-a:"),
            "Project scope should prefix cache key"
        );
    }

    #[test]
    fn test_cache_stats_initialization() {
        let stats = CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size: 100,
        };

        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_calculation() {
        let stats = CacheStats {
            hits: 75,
            misses: 25,
            evictions: 5,
            current_size: 50,
            max_size: 100,
        };

        assert_eq!(stats.hit_rate(), 75.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_no_requests() {
        let stats = CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size: 100,
        };

        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_log_tool_call_start_captures_info_and_debug() {
        let logs = capture_logs(|| {
            let params = serde_json::json!({
                "node_id": "nodes:123",
                "edge_type": "Calls"
            });
            log_tool_call_start("get_transitive_dependencies", &params);
        });

        assert!(logs.contains("Tool call started"));
        assert!(logs.contains("Tool input payload"));
    }

    #[test]
    fn test_log_tool_call_finish_captures_info_and_debug() {
        let logs = capture_logs(|| {
            let result = serde_json::json!({
                "tool": "detect_cycles",
                "result": "ok"
            });
            log_tool_call_finish("detect_cycles", &result);
        });

        assert!(logs.contains("Tool call completed"));
        assert!(logs.contains("Tool output payload"));
    }

    fn capture_logs<F>(f: F) -> String
    where
        F: FnOnce(),
    {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        use tracing::subscriber::with_default;
        use tracing_subscriber::EnvFilter;

        #[derive(Clone)]
        struct BufferWriter {
            inner: Arc<Mutex<Vec<u8>>>,
        }

        impl BufferWriter {
            fn new() -> Self {
                Self {
                    inner: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn into_string(&self) -> String {
                let bytes = self.inner.lock().unwrap().clone();
                String::from_utf8(bytes).unwrap()
            }
        }

        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriter {
            type Writer = BufferGuard;

            fn make_writer(&'a self) -> Self::Writer {
                BufferGuard {
                    inner: self.inner.clone(),
                }
            }
        }

        struct BufferGuard {
            inner: Arc<Mutex<Vec<u8>>>,
        }

        impl Write for BufferGuard {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.inner.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let writer = BufferWriter::new();

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("debug"))
            .with_ansi(false)
            .without_time()
            .with_writer(writer.clone())
            .finish();

        with_default(subscriber, f);

        writer.into_string()
    }
}
