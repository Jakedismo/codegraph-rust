// ABOUTME: AutoAgents tool definitions for SurrealDB graph analysis
// ABOUTME: Type-safe wrappers using AutoAgents derive macros

use autoagents::core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT};
use autoagents_derive::{tool, ToolInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use crate::autoagents::tools::tool_executor_adapter::GraphToolExecutorAdapter;

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
    depth: usize,
}

fn default_edge_type() -> String {
    "Calls".to_string()
}

fn default_depth() -> usize {
    3
}

/// Get transitive dependencies of a code node
#[tool(
    name = "get_transitive_dependencies",
    description = "Get all transitive dependencies of a code node up to specified depth. \
                   Follows dependency edges recursively to find all nodes this node depends on.",
    input = GetTransitiveDependenciesArgs,
)]
pub struct GetTransitiveDependencies {}

impl ToolRuntime for GetTransitiveDependencies {
    fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolCallError> {
        let typed_args: GetTransitiveDependenciesArgs = serde_json::from_value(args)?;

        // TODO: Need executor instance - for now return placeholder
        Ok(serde_json::json!({
            "status": "not_implemented",
            "params": {
                "node_id": typed_args.node_id,
                "edge_type": typed_args.edge_type,
                "depth": typed_args.depth,
            }
        }))
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
        assert_eq!(args.depth, 5);
    }

    #[test]
    fn test_args_defaults() {
        let json = serde_json::json!({
            "node_id": "nodes:456"
        });

        let args: GetTransitiveDependenciesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.edge_type, "Calls");
        assert_eq!(args.depth, 3);
    }
}
