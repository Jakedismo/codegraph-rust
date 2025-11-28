// ABOUTME: LLM tool executor for SurrealDB graph analysis functions
// ABOUTME: Executes graph analysis tools by calling Rust SDK wrappers with validated parameters

use codegraph_graph::GraphFunctions;
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::{debug, info};

const TOOL_PROGRESS_LOG_TARGET: &str = "codegraph::mcp::tools";

use crate::error::McpError;
use crate::graph_tool_schemas::GraphToolSchemas;
use crate::Result;

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
    /// LRU cache for tool results (function_name + params → result)
    cache: Arc<Mutex<LruCache<String, JsonValue>>>,
    /// Cache statistics for observability
    cache_stats: Arc<Mutex<CacheStats>>,
    /// Whether caching is enabled
    cache_enabled: bool,
}

impl GraphToolExecutor {
    /// Create a new tool executor with GraphFunctions instance
    pub fn new(graph_functions: Arc<GraphFunctions>) -> Self {
        Self::with_cache(graph_functions, true, 100)
    }

    /// Create a new tool executor with custom cache configuration
    pub fn with_cache(
        graph_functions: Arc<GraphFunctions>,
        cache_enabled: bool,
        cache_size: usize,
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

        Self {
            graph_functions,
            cache,
            cache_stats,
            cache_enabled,
        }
    }

    /// Get current cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        self.cache_stats.lock().clone()
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
            "find_nodes_by_name" => self.execute_find_nodes_by_name(parameters.clone()).await?,
            _ => {
                return Err(
                    McpError::Protocol(format!("Tool not implemented: {}", tool_name)).into(),
                )
            }
        };

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

        log_tool_call_finish(tool_name, &result);
        Ok(result)
    }

    /// Execute get_transitive_dependencies
    async fn execute_get_transitive_dependencies(&self, params: JsonValue) -> Result<JsonValue> {
        let node_id = params["node_id"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing node_id".to_string()))?;

        let edge_type = params["edge_type"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing edge_type".to_string()))?;

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
        let from_node = params["from_node"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing from_node".to_string()))?;

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

        let result = self
            .graph_functions
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

        let edge_type = params["edge_type"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing edge_type".to_string()))?;

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

    /// Execute find_nodes_by_name
    async fn execute_find_nodes_by_name(&self, params: JsonValue) -> Result<JsonValue> {
        let needle = params["needle"]
            .as_str()
            .ok_or_else(|| McpError::Protocol("Missing needle".to_string()))?;

        let limit = params["limit"].as_i64().unwrap_or(10) as usize;

        let result = self
            .graph_functions
            .find_nodes_by_name(needle, limit)
            .await
            .map_err(|e| McpError::Protocol(format!("find_nodes_by_name failed: {}", e)))?;

        Ok(json!({
            "tool": "find_nodes_by_name",
            "parameters": {
                "needle": needle,
                "limit": limit
            },
            "result": result
        }))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_available() {
        let schemas = GraphToolExecutor::get_tool_schemas();
        assert_eq!(schemas.len(), 6);
    }

    #[test]
    fn test_tool_names() {
        let names = GraphToolExecutor::get_tool_names();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"get_transitive_dependencies".to_string()));
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
