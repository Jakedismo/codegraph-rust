// ABOUTME: Rig tool implementations wrapping CountingExecutor
// ABOUTME: 8 graph analysis tools implementing rig_core::Tool trait

use super::counting_executor::CountingExecutor;
use rig::tool::PortableTool as Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
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
    /// Node ID exactly as returned by an earlier tool result, such as semantic_code_search. A symbol name is not a node ID.
    pub node_id: String,
    /// Relationship to follow: "Calls", "Imports", "Uses", "Extends", "Implements", or "References" (default: "Calls")
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
    /// How many hops to follow, 1-10 (default: 3). Results grow quickly with depth.
    #[serde(default = "default_depth")]
    pub depth: i32,
}

/// Arguments for detect_circular_dependencies tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DetectCyclesArgs {
    /// Relationship to check for cycles: "Calls", "Imports", "Uses", "Extends", "Implements", or "References"
    pub edge_type: String,
}

/// Arguments for trace_call_chain tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TraceCallChainArgs {
    /// ID of the function the trace starts from, exactly as returned by an earlier tool result
    pub from_node: String,
    /// How many calls deep to follow, 1-10 (default: 5)
    #[serde(default = "default_max_depth")]
    pub max_depth: i32,
}

/// Arguments for calculate_coupling_metrics tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CouplingMetricsArgs {
    /// Node ID exactly as returned by an earlier tool result, such as semantic_code_search. A symbol name is not a node ID.
    pub node_id: String,
}

/// Arguments for get_hub_nodes tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct HubNodesArgs {
    /// Minimum number of connections a node needs to count as a hub (default: 5). Lower it to get more nodes.
    #[serde(default = "default_min_degree")]
    pub min_degree: i32,
}

/// Arguments for get_reverse_dependencies tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReverseDepsArgs {
    /// Node ID exactly as returned by an earlier tool result, such as semantic_code_search. A symbol name is not a node ID.
    pub node_id: String,
    /// Relationship to follow: "Calls", "Imports", "Uses", "Extends", "Implements", or "References" (default: "Calls")
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
    /// How many hops to follow, 1-10 (default: 3). Depth 1 gives direct dependents only.
    #[serde(default = "default_depth")]
    pub depth: i32,
}

/// Arguments for semantic_code_search tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SemanticSearchArgs {
    /// What to look for, as a description ("where HTTP requests are authenticated") or an identifier ("parse_config")
    pub query: String,
    /// Maximum results to return, 1-50 (default: 10)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Minimum similarity, 0.0-1.0 (default: 0.6). Lower it if a search returns nothing.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

/// Arguments for find_complexity_hotspots tool
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ComplexityHotspotsArgs {
    /// Minimum complexity score a function needs to be included (default: 5.0)
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
    executor: CountingExecutor,
}

impl GetTransitiveDependencies {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for GetTransitiveDependencies {
    const NAME: &'static str = "get_transitive_dependencies";

    type Error = GraphToolError;
    type Args = TransitiveDepsArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "List everything a node depends on, directly and indirectly, by following one relationship type outward to a given depth. Use it to learn what a piece of code needs in order to work."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TransitiveDepsArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl DetectCircularDependencies {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for DetectCircularDependencies {
    const NAME: &'static str = "detect_circular_dependencies";

    type Error = GraphToolError;
    type Args = DetectCyclesArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "Find dependency cycles across the whole codebase for one relationship type. Takes no node ID. Use it to check for circular imports or mutually recursive calls."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(DetectCyclesArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl TraceCallChain {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for TraceCallChain {
    const NAME: &'static str = "trace_call_chain";

    type Error = GraphToolError;
    type Args = TraceCallChainArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "Follow function calls outward from a starting function and return the call paths in order. Use it to see how execution proceeds from an entry point."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TraceCallChainArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl CalculateCouplingMetrics {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for CalculateCouplingMetrics {
    const NAME: &'static str = "calculate_coupling_metrics";

    type Error = GraphToolError;
    type Args = CouplingMetricsArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "Measure how coupled one node is. Returns afferent coupling Ca (how many nodes depend on it), efferent coupling Ce (how many it depends on), and instability I = Ce / (Ca + Ce): near 0 means many dependents and costly to change, near 1 means few dependents and cheap to change."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(CouplingMetricsArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl GetHubNodes {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for GetHubNodes {
    const NAME: &'static str = "get_hub_nodes";

    type Error = GraphToolError;
    type Args = HubNodesArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "List the most connected nodes in the whole codebase, by incoming plus outgoing edges. Takes no node ID. Use it to find the central components of an unfamiliar codebase."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(HubNodesArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl GetReverseDependencies {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for GetReverseDependencies {
    const NAME: &'static str = "get_reverse_dependencies";

    type Error = GraphToolError;
    type Args = ReverseDepsArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "List everything that depends on a node, directly and indirectly, by following one relationship type inward to a given depth. Use it to learn what a change to the node could break."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ReverseDepsArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl SemanticCodeSearch {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for SemanticCodeSearch {
    const NAME: &'static str = "semantic_code_search";

    type Error = GraphToolError;
    type Args = SemanticSearchArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "Search the codebase by meaning or by identifier. Returns matching nodes with their node ID, name, file path, line, and a code snippet. This is the way to obtain the node IDs the other tools require, so it is usually the first call."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(SemanticSearchArgs)).unwrap_or_default()
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
    executor: CountingExecutor,
}

impl FindComplexityHotspots {
    pub fn new(executor: CountingExecutor) -> Self {
        Self { executor }
    }
}

impl Tool for FindComplexityHotspots {
    const NAME: &'static str = "find_complexity_hotspots";

    type Error = GraphToolError;
    type Args = ComplexityHotspotsArgs;
    type Output = JsonValue;

    fn description(&self) -> String {
        "Rank the functions in the whole codebase that combine high complexity with high coupling. Takes no node ID. Use it to find code that is risky to change or worth refactoring."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ComplexityHotspotsArgs)).unwrap_or_default()
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
