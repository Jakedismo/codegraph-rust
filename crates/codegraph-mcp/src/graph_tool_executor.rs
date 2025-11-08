// ABOUTME: LLM tool executor for SurrealDB graph analysis functions
// ABOUTME: Executes graph analysis tools by calling Rust SDK wrappers with validated parameters

use codegraph_graph::GraphFunctions;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tracing::{debug, info};

use crate::error::McpError;
use crate::graph_tool_schemas::GraphToolSchemas;
use crate::Result;

/// Executor for graph analysis tools
/// Receives tool calls from LLM and executes appropriate SurrealDB functions
pub struct GraphToolExecutor {
    graph_functions: Arc<GraphFunctions>,
}

impl GraphToolExecutor {
    /// Create a new tool executor with GraphFunctions instance
    pub fn new(graph_functions: Arc<GraphFunctions>) -> Self {
        Self { graph_functions }
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
        info!("Executing graph tool: {}", tool_name);
        debug!("Tool parameters: {}", parameters);

        // Validate tool exists
        let _schema = GraphToolSchemas::get_by_name(tool_name)
            .ok_or_else(|| McpError::Protocol(format!("Unknown tool: {}", tool_name)))?;

        // Execute based on tool name
        let result = match tool_name {
            "get_transitive_dependencies" => {
                self.execute_get_transitive_dependencies(parameters).await?
            }
            "detect_circular_dependencies" => {
                self.execute_detect_circular_dependencies(parameters)
                    .await?
            }
            "trace_call_chain" => self.execute_trace_call_chain(parameters).await?,
            "calculate_coupling_metrics" => {
                self.execute_calculate_coupling_metrics(parameters).await?
            }
            "get_hub_nodes" => self.execute_get_hub_nodes(parameters).await?,
            "get_reverse_dependencies" => self.execute_get_reverse_dependencies(parameters).await?,
            _ => {
                return Err(
                    McpError::Protocol(format!("Tool not implemented: {}", tool_name)).into(),
                )
            }
        };

        info!("Tool execution complete: {}", tool_name);
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

    /// Get all available tool schemas for registration
    pub fn get_tool_schemas() -> Vec<crate::ToolSchema> {
        GraphToolSchemas::all()
    }

    /// Get tool names for listing
    pub fn get_tool_names() -> Vec<String> {
        GraphToolSchemas::tool_names()
    }
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
}
