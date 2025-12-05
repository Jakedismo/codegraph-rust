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
    #[input(
        description = "Type of dependency relationship to follow (edge-types: 'Calls, Imports, Uses, Extends, Implements, References', default: 'Calls')"
    )]
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
    description = "ANSWERS: 'What does this code NEED to work?' Maps the complete chain of forward dependencies to any depth. \
                   USE WHEN: Planning changes and need to understand blast radius, identifying external library usage, \
                   finding configuration/infrastructure this code requires. \
                   UNIQUE VALUE: Shows the SUPPLY CHAIN - what breaks if a dependency disappears. \
                   FOLLOW-UP: Use get_reverse_dependencies to understand impact in the other direction. \
                   NOTE: Requires a node_id from semantic_code_search results.",
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
    #[input(
        description = "Type of dependency relationship (edge-types: 'Calls, Imports, Uses, Extends, Implements, References', default: 'Calls')"
    )]
    #[serde(default = "default_edge_type")]
    edge_type: String,
    #[input(description = "Maximum traversal depth (default: 3)")]
    #[serde(default = "default_depth")]
    depth: i32,
}

/// Get reverse dependencies (what depends on this node)
#[tool(
    name = "get_reverse_dependencies",
    description = "ANSWERS: 'What breaks if I change this code?' Reveals the complete IMPACT RADIUS - all callers, importers, and users. \
                   USE WHEN: Before refactoring to understand risk, assessing how widely a component is used, \
                   finding entry points that trigger this code. \
                   UNIQUE VALUE: Essential for safe refactoring - shows who depends ON this code (opposite of transitive deps). \
                   HIGH PRIORITY: Use IMMEDIATELY after semantic_code_search to understand code importance. \
                   NOTE: Requires a node_id from semantic_code_search results.",
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
    node_id: String,
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
    description = "ANSWERS: 'How does execution actually FLOW at runtime?' Traces the dynamic call path from an entry point through nested function calls. \
                   USE WHEN: Debugging to understand how data flows, mapping request handling from entry to exit, \
                   finding the sequence of operations a feature performs. \
                   UNIQUE VALUE: Shows RUNTIME behavior, not just static structure - different from dependency tools which show import/definition relationships. \
                   EXAMPLE: Entry point 'handleRequest' -> 'validateInput' -> 'queryDatabase' -> 'formatResponse'. \
                   NOTE: Requires a node_id from semantic_code_search results.",
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
                    "node_id": typed_args.node_id,
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
    #[input(
        description = "Type of dependency edge to check ((edge-types: 'Calls, Imports, Uses, Extends, Implements, References', default: 'Calls')"
    )]
    #[serde(default = "default_edge_type")]
    edge_type: String,
}

/// Detect circular dependencies
#[tool(
    name = "detect_cycles",
    description = "ANSWERS: 'Is there hidden coupling creating architectural fragility?' Detects circular dependency chains that indicate design problems. \
                   USE WHEN: Assessing architectural health, investigating why changes have unexpected side effects, \
                   finding tightly coupled module groups that should be refactored. \
                   UNIQUE VALUE: Finds ARCHITECTURAL SMELLS that other tools miss - cycles often cause: build issues, testing difficulties, and change propagation. \
                   EXAMPLE OUTPUT: A->B->C->A cycle in 'Imports' means these modules cannot be separated. \
                   BEST PRACTICE: Run after get_transitive_dependencies at depth>=3 to check for hidden cycles.",
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
    description = "ANSWERS: 'Is this component stable or volatile? High-impact or isolated?' Quantifies architectural quality with Ca/Ce/I metrics. \
                   METRICS EXPLAINED: Ca (Afferent) = incoming dependencies (how many USE this), Ce (Efferent) = outgoing dependencies (how many this USES), \
                   I (Instability) = Ce/(Ca+Ce), range 0-1: I=0 means stable/hard to change, I=1 means unstable/easy to change. \
                   USE WHEN: Deciding if a component is safe to modify, identifying architectural hotspots, \
                   validating that stable components (I near 0) don't depend on unstable ones (I near 1). \
                   INTERPRETATION: High Ca + Low I = critical foundation component (change carefully). Low Ca + High I = leaf component (safe to modify). \
                   NOTE: Requires a node_id from semantic_code_search results.",
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
    description = "ANSWERS: 'What are the critical architectural centers everything depends on?' Identifies highly-connected components that are potential god objects or architectural bottlenecks. \
                   USE WHEN: Starting architecture analysis (use BEFORE semantic_code_search for architectural overview), \
                   finding components where bugs have high blast radius, identifying refactoring candidates. \
                   UNIQUE VALUE: Reveals the SKELETON of the codebase - the core components everything else builds upon. \
                   INTERPRETATION: High-degree hub = either well-designed foundation OR problematic god object (use calculate_coupling to distinguish). \
                   GREAT STARTING POINT: Returns node_ids you can pass to other tools for deeper analysis. \
                   min_degree=5 finds moderately connected nodes, min_degree=10+ finds major architectural centers.",
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
    #[input(
        description = "Search query (e.g., How is authentication handled?, database models, error handling code etc.)"
    )]
    query: String,
    #[input(description = "Maximum number of results (1-50, default: 50)")]
    #[serde(default = "default_search_limit")]
    limit: i32,
    #[input(
        description = "Similarity threshold 0.0-1.0 (optional, default 0.6; lower to broaden matches)"
    )]
    #[serde(default = "default_search_threshold")]
    threshold: f64,
}

fn default_search_limit() -> i32 {
    10
}

fn default_search_threshold() -> f64 {
    0.6
}

/// Semantic code search using AI embeddings, full-text analysis, and graph enrichment
#[tool(
    name = "semantic_code_search",
    description = "DISCOVERY TOOL: Find code by natural language description. Returns node locations and IDs for deeper analysis. \
                   USE FOR: Locating code by concept ('authentication', 'error handling'), finding specific identifiers, starting investigations. \
                   OUTPUT: Provides node_ids (format: 'nodes:uuid') needed by other graph tools. \
                   LIMITATIONS: Shows WHERE code is, NOT how it connects. Cannot answer: 'What uses this?', 'What does this depend on?', 'Is this stable?' \
                   REQUIRED FOLLOW-UPS: After finding nodes, use get_reverse_dependencies (who uses this?), get_transitive_dependencies (what does it need?), \
                   or calculate_coupling (is it stable?) to understand relationships. Search alone provides incomplete answers. \
                   WORKFLOW: Search -> extract node_id -> analyze with graph tools -> synthesize complete picture.",
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
                    "limit": typed_args.limit,
                    "threshold": typed_args.threshold
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
