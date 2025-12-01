// ABOUTME: AutoAgents tool definitions for SurrealDB graph analysis
// ABOUTME: Type-safe wrappers using AutoAgents derive macros with stateful executor access

use crate::autoagents::tools::tool_executor_adapter::GraphToolExecutorAdapter;
use autoagents::core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT};
use autoagents_derive::{tool, ToolInput};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Parameters for get_transitive_dependencies
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct GetTransitiveDependenciesArgs {
    #[input(description = "The ID of the code node to analyze (e.g., 'nodes:123')")]
    node_id: String,
    #[input(description = "Type of dependency relationship to follow (default: 'Calls')")]
    #[serde(default = "default_edge_type")]
    edge_type: String,
    #[input(description = "Maximum traversal depth (1-10, default: 3)")]
    #[serde(default = "default_depth")]
    depth: i32,
}

fn default_edge_type() -> String {
    "Calls".to_string()
}

fn default_depth() -> i32 {
    3
}

/// Get transitive dependencies of a code node
#[tool(
    name = "get_transitive_dependencies",
    description = "Get all transitive dependencies of a code node up to specified depth. \
                   Follows dependency edges recursively to find all nodes this node depends on.",
    input = GetTransitiveDependenciesArgs,
)]
pub struct GetTransitiveDependencies {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl GetTransitiveDependencies {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for GetTransitiveDependencies {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: GetTransitiveDependenciesArgs = serde_json::from_value(args)?;

        // Call the actual executor
        let result = self
            .executor
            .execute_sync(
                "get_transitive_dependencies",
                serde_json::json!({
                    "node_id": typed_args.node_id,
                    "edge_type": typed_args.edge_type,
                    "depth": typed_args.depth
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for get_reverse_dependencies
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct GetReverseDependenciesArgs {
    #[input(description = "The ID of the code node to analyze")]
    node_id: String,
    #[input(description = "Type of dependency relationship (default: 'Calls')")]
    #[serde(default = "default_edge_type")]
    edge_type: String,
    #[input(description = "Maximum traversal depth (default: 3)")]
    #[serde(default = "default_depth")]
    depth: i32,
}

/// Get reverse dependencies (what depends on this node)
#[tool(
    name = "get_reverse_dependencies",
    description = "Get all nodes that depend on the specified node. Useful for impact analysis.",
    input = GetReverseDependenciesArgs,
)]
pub struct GetReverseDependencies {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl GetReverseDependencies {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for GetReverseDependencies {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: GetReverseDependenciesArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "get_reverse_dependencies",
                serde_json::json!({
                    "node_id": typed_args.node_id,
                    "edge_type": typed_args.edge_type,
                    "depth": typed_args.depth
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for trace_call_chain
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct TraceCallChainArgs {
    #[input(description = "Starting node ID for call chain tracing")]
    from_node: String,
    #[input(description = "Maximum depth to trace (default: 5)")]
    #[serde(default = "default_call_chain_depth")]
    max_depth: i32,
}

fn default_call_chain_depth() -> i32 {
    5
}

/// Trace call chain from a starting point
#[tool(
    name = "trace_call_chain",
    description = "Trace the execution flow from a starting function through all called functions.",
    input = TraceCallChainArgs,
)]
pub struct TraceCallChain {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl TraceCallChain {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for TraceCallChain {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: TraceCallChainArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "trace_call_chain",
                serde_json::json!({
                    "from_node": typed_args.from_node,
                    "max_depth": typed_args.max_depth
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for detect_cycles
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct DetectCyclesArgs {
    #[input(description = "Type of dependency edge to check (default: 'Calls')")]
    #[serde(default = "default_edge_type")]
    edge_type: String,
}

/// Detect circular dependencies
#[tool(
    name = "detect_cycles",
    description = "Detect circular dependencies and cycles in the codebase graph.",
    input = DetectCyclesArgs,
)]
pub struct DetectCycles {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl DetectCycles {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for DetectCycles {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: DetectCyclesArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "detect_circular_dependencies",
                serde_json::json!({
                    "edge_type": typed_args.edge_type
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for calculate_coupling
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct CalculateCouplingArgs {
    #[input(description = "Node ID to analyze coupling for")]
    node_id: String,
}

/// Calculate coupling metrics
#[tool(
    name = "calculate_coupling",
    description = "Calculate afferent/efferent coupling and instability metrics for a node.",
    input = CalculateCouplingArgs,
)]
pub struct CalculateCoupling {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl CalculateCoupling {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for CalculateCoupling {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: CalculateCouplingArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "calculate_coupling_metrics",
                serde_json::json!({
                    "node_id": typed_args.node_id
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for get_hub_nodes
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct GetHubNodesArgs {
    #[input(description = "Minimum degree (connections) to consider a node a hub (default: 5)")]
    #[serde(default = "default_min_degree")]
    min_degree: i32,
}

fn default_min_degree() -> i32 {
    5
}

/// Get highly connected hub nodes
#[tool(
    name = "get_hub_nodes",
    description = "Find highly connected hub nodes in the dependency graph.",
    input = GetHubNodesArgs,
)]
pub struct GetHubNodes {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl GetHubNodes {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for GetHubNodes {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: GetHubNodesArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "get_hub_nodes",
                serde_json::json!({
                    "min_degree": typed_args.min_degree
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

/// Parameters for semantic_code_search
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct SemanticCodeSearchArgs {
    #[input(description = "Natural language search query (e.g., 'authentication logic', 'error handling', 'JWT validation', 'user controller')")]
    query: String,
    #[input(description = "Maximum number of results (1-50, default: 10)")]
    #[serde(default = "default_search_limit")]
    limit: i32,
}

fn default_search_limit() -> i32 {
    10
}

/// Semantic code search using AI embeddings, full-text analysis, and graph enrichment
#[tool(
    name = "semantic_code_search",
    description = "Primary code discovery tool using AI embeddings + full-text analysis + graph enrichment. \
                   Accepts natural language queries (e.g., 'authentication logic', 'error handling code', 'database models'). \
                   Combines HNSW vector similarity (70%) with fuzzy text matching (30%) for comprehensive results. \
                   Enriches results with dependencies, dependents, and file context. \
                   Works for both conceptual searches and specific identifiers. \
                   Automatically applies reranking if configured.",
    input = SemanticCodeSearchArgs,
)]
pub struct SemanticCodeSearch {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl SemanticCodeSearch {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl ToolRuntime for SemanticCodeSearch {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: SemanticCodeSearchArgs = serde_json::from_value(args)?;

        let result = self
            .executor
            .execute_sync(
                "semantic_code_search",
                serde_json::json!({
                    "query": typed_args.query,
                    "limit": typed_args.limit
                }),
            )
            .map_err(|e| {
                ToolCallError::RuntimeError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )))
            })?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_deserialization() {
        let json = serde_json::json!({
            "node_id": "nodes:123",
            "edge_type": "Imports",
            "depth": 5
        });

        let args: GetTransitiveDependenciesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.node_id, "nodes:123");
        assert_eq!(args.edge_type, "Imports");
        assert_eq!(args.depth, 5_i32);
    }

    #[test]
    fn test_args_defaults() {
        let json = serde_json::json!({
            "node_id": "nodes:456"
        });

        let args: GetTransitiveDependenciesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.edge_type, "Calls");
        assert_eq!(args.depth, 3_i32);
    }
}
