// ABOUTME: Rig tool implementations wrapping GraphToolExecutor
// ABOUTME: 8 graph analysis tools implementing rig_core::Tool trait

use codegraph_mcp_tools::GraphToolExecutor;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use thiserror::Error;

/// Error type for graph tool operations
#[derive(Debug, Error)]
pub enum GraphToolError {
    #[error("Execution failed: {0}")]
    ExecutionError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
}

// ============================================================================
// Tool Arguments (with JsonSchema for Rig)
// ============================================================================

/// Arguments for get_transitive_dependencies tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TransitiveDepsArgs {
    /// Node ID to analyze dependencies for
    pub node_id: String,
    /// Type of edge to traverse (default: "Calls")
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
    /// Maximum depth to traverse (default: 3)
    #[serde(default = "default_depth")]
    pub depth: i32,
}

/// Arguments for detect_circular_dependencies tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DetectCyclesArgs {
    /// Type of edge to check for cycles
    pub edge_type: String,
}

/// Arguments for trace_call_chain tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TraceCallChainArgs {
    /// Starting node for the trace
    pub from_node: String,
    /// Maximum depth to trace (default: 5)
    #[serde(default = "default_max_depth")]
    pub max_depth: i32,
}

/// Arguments for calculate_coupling_metrics tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CouplingMetricsArgs {
    /// Node ID to calculate coupling for
    pub node_id: String,
}

/// Arguments for get_hub_nodes tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct HubNodesArgs {
    /// Minimum degree to consider as hub (default: 5)
    #[serde(default = "default_min_degree")]
    pub min_degree: i32,
}

/// Arguments for get_reverse_dependencies tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReverseDepsArgs {
    /// Node ID to find dependents for
    pub node_id: String,
    /// Type of edge to traverse (default: "Calls")
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
    /// Maximum depth to traverse (default: 3)
    #[serde(default = "default_depth")]
    pub depth: i32,
}

/// Arguments for semantic_code_search tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SemanticSearchArgs {
    /// Natural language search query
    pub query: String,
    /// Maximum results to return (default: 10)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Similarity threshold 0.0-1.0 (default: 0.6)
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

/// Arguments for find_complexity_hotspots tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ComplexityHotspotsArgs {
    /// Minimum complexity score to include (default: 5.0)
    #[serde(default = "default_min_complexity")]
    pub min_complexity: f32,
    /// Maximum results to return (default: 20)
    #[serde(default = "default_hotspot_limit")]
    pub limit: i32,
}

// Default value functions
fn default_edge_type() -> String {
    "Calls".to_string()
}
fn default_depth() -> i32 {
    3
}
fn default_max_depth() -> i32 {
    5
}
fn default_min_degree() -> i32 {
    5
}
fn default_limit() -> usize {
    10
}
fn default_threshold() -> f64 {
    0.6
}
fn default_min_complexity() -> f32 {
    5.0
}
fn default_hotspot_limit() -> i32 {
    20
}

// ============================================================================
// Tool Implementations
// ============================================================================

/// Get transitive dependencies for a node
#[derive(Clone)]
pub struct GetTransitiveDependencies {
    executor: Arc<GraphToolExecutor>,
}

impl GetTransitiveDependencies {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for GetTransitiveDependencies {
    const NAME: &'static str = "get_transitive_dependencies";

    type Error = GraphToolError;
    type Args = TransitiveDepsArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get all transitive dependencies of a node following specified edge types to a given depth".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TransitiveDepsArgs)).unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "node_id": args.node_id,
            "edge_type": args.edge_type,
            "depth": args.depth
        });

        self.executor
            .execute("get_transitive_dependencies", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Detect circular dependencies in the graph
#[derive(Clone)]
pub struct DetectCircularDependencies {
    executor: Arc<GraphToolExecutor>,
}

impl DetectCircularDependencies {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for DetectCircularDependencies {
    const NAME: &'static str = "detect_circular_dependencies";

    type Error = GraphToolError;
    type Args = DetectCyclesArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Detect circular dependencies (cycles) in the graph for a given edge type"
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(DetectCyclesArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "edge_type": args.edge_type
        });

        self.executor
            .execute("detect_circular_dependencies", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Trace call chain from a starting node
#[derive(Clone)]
pub struct TraceCallChain {
    executor: Arc<GraphToolExecutor>,
}

impl TraceCallChain {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for TraceCallChain {
    const NAME: &'static str = "trace_call_chain";

    type Error = GraphToolError;
    type Args = TraceCallChainArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Trace the call chain from a starting node to understand execution flow"
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TraceCallChainArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "from_node": args.from_node,
            "max_depth": args.max_depth
        });

        self.executor
            .execute("trace_call_chain", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Calculate coupling metrics for a node
#[derive(Clone)]
pub struct CalculateCouplingMetrics {
    executor: Arc<GraphToolExecutor>,
}

impl CalculateCouplingMetrics {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for CalculateCouplingMetrics {
    const NAME: &'static str = "calculate_coupling_metrics";

    type Error = GraphToolError;
    type Args = CouplingMetricsArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Calculate afferent and efferent coupling metrics for a node to assess its dependencies".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(CouplingMetricsArgs)).unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "node_id": args.node_id
        });

        self.executor
            .execute("calculate_coupling_metrics", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Get hub nodes with high connectivity
#[derive(Clone)]
pub struct GetHubNodes {
    executor: Arc<GraphToolExecutor>,
}

impl GetHubNodes {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for GetHubNodes {
    const NAME: &'static str = "get_hub_nodes";

    type Error = GraphToolError;
    type Args = HubNodesArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Find hub nodes with high connectivity (many incoming or outgoing edges)"
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(HubNodesArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "min_degree": args.min_degree
        });

        self.executor
            .execute("get_hub_nodes", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Get reverse dependencies (what depends on this node)
#[derive(Clone)]
pub struct GetReverseDependencies {
    executor: Arc<GraphToolExecutor>,
}

impl GetReverseDependencies {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for GetReverseDependencies {
    const NAME: &'static str = "get_reverse_dependencies";

    type Error = GraphToolError;
    type Args = ReverseDepsArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Find all nodes that depend on the specified node (reverse dependency analysis)"
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ReverseDepsArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "node_id": args.node_id,
            "edge_type": args.edge_type,
            "depth": args.depth
        });

        self.executor
            .execute("get_reverse_dependencies", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Semantic code search using embeddings
#[derive(Clone)]
pub struct SemanticCodeSearch {
    executor: Arc<GraphToolExecutor>,
}

impl SemanticCodeSearch {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for SemanticCodeSearch {
    const NAME: &'static str = "semantic_code_search";

    type Error = GraphToolError;
    type Args = SemanticSearchArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Search code semantically using natural language queries and vector embeddings"
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SemanticSearchArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "query": args.query,
            "limit": args.limit,
            "threshold": args.threshold
        });

        self.executor
            .execute("semantic_code_search", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

/// Find complexity hotspots in the codebase
#[derive(Clone)]
pub struct FindComplexityHotspots {
    executor: Arc<GraphToolExecutor>,
}

impl FindComplexityHotspots {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self { executor }
    }
}

impl Tool for FindComplexityHotspots {
    const NAME: &'static str = "find_complexity_hotspots";

    type Error = GraphToolError;
    type Args = ComplexityHotspotsArgs;
    type Output = JsonValue;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Find functions with high complexity and coupling that may benefit from refactoring"
                    .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ComplexityHotspotsArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = json!({
            "min_complexity": args.min_complexity,
            "limit": args.limit
        });

        self.executor
            .execute("find_complexity_hotspots", params)
            .await
            .map_err(|e| GraphToolError::ExecutionError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_have_json_schema() {
        // Verify all args generate valid schemas
        let _ = schemars::schema_for!(TransitiveDepsArgs);
        let _ = schemars::schema_for!(DetectCyclesArgs);
        let _ = schemars::schema_for!(TraceCallChainArgs);
        let _ = schemars::schema_for!(CouplingMetricsArgs);
        let _ = schemars::schema_for!(HubNodesArgs);
        let _ = schemars::schema_for!(ReverseDepsArgs);
        let _ = schemars::schema_for!(SemanticSearchArgs);
        let _ = schemars::schema_for!(ComplexityHotspotsArgs);
    }

    #[test]
    fn test_default_values() {
        assert_eq!(default_edge_type(), "Calls");
        assert_eq!(default_depth(), 3);
        assert_eq!(default_max_depth(), 5);
        assert_eq!(default_min_degree(), 5);
        assert_eq!(default_limit(), 10);
        assert_eq!(default_threshold(), 0.6);
        assert_eq!(default_min_complexity(), 5.0);
        assert_eq!(default_hotspot_limit(), 20);
    }
}
