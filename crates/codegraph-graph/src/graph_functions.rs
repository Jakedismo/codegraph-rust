// ABOUTME: Rust SDK wrappers for SurrealDB graph analysis functions
// ABOUTME: Provides type-safe interfaces for LLM-powered graph analysis tools

use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::{engine::any::Any, Surreal};
use tracing::{debug, error};

/// Wrapper for SurrealDB graph analysis functions
/// Provides type-safe Rust interfaces for calling SurrealDB functions
#[derive(Clone)]
pub struct GraphFunctions {
    db: Arc<Surreal<Any>>,
}

impl GraphFunctions {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }

    /// Get transitive dependencies of a node up to specified depth
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to analyze
    /// * `edge_type` - The type of edge to follow (e.g., "Calls", "Imports")
    /// * `depth` - Maximum depth to traverse (1-10, defaults to 3)
    ///
    /// # Returns
    /// Vector of nodes representing transitive dependencies
    pub async fn get_transitive_dependencies(
        &self,
        node_id: &str,
        edge_type: &str,
        depth: i32,
    ) -> Result<Vec<DependencyNode>> {
        debug!(
            "Calling fn::get_transitive_dependencies({}, {}, {})",
            node_id, edge_type, depth
        );

        let result: Vec<DependencyNode> = self
            .db
            .query("RETURN fn::get_transitive_dependencies($node_id, $edge_type, $depth)")
            .bind(("node_id", node_id.to_string()))
            .bind(("edge_type", edge_type.to_string()))
            .bind(("depth", depth))
            .await
            .map_err(|e| {
                error!("Failed to call get_transitive_dependencies: {}", e);
                CodeGraphError::Database(format!("get_transitive_dependencies failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize dependencies: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }

    /// Detect circular dependencies for a given edge type
    ///
    /// # Arguments
    /// * `edge_type` - The type of edge to analyze (e.g., "Imports", "Uses")
    ///
    /// # Returns
    /// Vector of circular dependency pairs (A <-> B relationships)
    pub async fn detect_circular_dependencies(
        &self,
        edge_type: &str,
    ) -> Result<Vec<CircularDependency>> {
        debug!("Calling fn::detect_circular_dependencies({})", edge_type);

        let result: Vec<CircularDependency> = self
            .db
            .query("RETURN fn::detect_circular_dependencies($edge_type)")
            .bind(("edge_type", edge_type.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to call detect_circular_dependencies: {}", e);
                CodeGraphError::Database(format!("detect_circular_dependencies failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize circular dependencies: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }

    /// Trace the call chain starting from a node
    ///
    /// # Arguments
    /// * `from_node` - The ID of the starting node
    /// * `max_depth` - Maximum depth to traverse (1-10, defaults to 5)
    ///
    /// # Returns
    /// Vector of nodes in the call chain with depth information
    pub async fn trace_call_chain(
        &self,
        from_node: &str,
        max_depth: i32,
    ) -> Result<Vec<CallChainNode>> {
        debug!("Calling fn::trace_call_chain({}, {})", from_node, max_depth);

        let result: Vec<CallChainNode> = self
            .db
            .query("RETURN fn::trace_call_chain($from_node, $max_depth)")
            .bind(("from_node", from_node.to_string()))
            .bind(("max_depth", max_depth))
            .await
            .map_err(|e| {
                error!("Failed to call trace_call_chain: {}", e);
                CodeGraphError::Database(format!("trace_call_chain failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize call chain: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }

    /// Calculate coupling metrics for a node
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to analyze
    ///
    /// # Returns
    /// Coupling metrics including afferent, efferent, and instability
    pub async fn calculate_coupling_metrics(&self, node_id: &str) -> Result<CouplingMetricsResult> {
        debug!("Calling fn::calculate_coupling_metrics({})", node_id);

        let results: Vec<CouplingMetricsResult> = self
            .db
            .query("RETURN fn::calculate_coupling_metrics($node_id)")
            .bind(("node_id", node_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to call calculate_coupling_metrics: {}", e);
                CodeGraphError::Database(format!("calculate_coupling_metrics failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize coupling metrics: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        results
            .into_iter()
            .next()
            .ok_or_else(|| CodeGraphError::Database("No coupling metrics returned".to_string()))
    }

    /// Get hub nodes with degree >= min_degree
    ///
    /// # Arguments
    /// * `min_degree` - Minimum total degree (defaults to 5)
    ///
    /// # Returns
    /// Vector of highly connected hub nodes sorted by degree (descending)
    pub async fn get_hub_nodes(&self, min_degree: i32) -> Result<Vec<HubNode>> {
        debug!("Calling fn::get_hub_nodes({})", min_degree);

        let result: Vec<HubNode> = self
            .db
            .query("RETURN fn::get_hub_nodes($min_degree)")
            .bind(("min_degree", min_degree))
            .await
            .map_err(|e| {
                error!("Failed to call get_hub_nodes: {}", e);
                CodeGraphError::Database(format!("get_hub_nodes failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize hub nodes: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }

    /// Get reverse dependencies (dependents) of a node
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to analyze
    /// * `edge_type` - The type of edge to follow
    /// * `depth` - Maximum depth to traverse (1-10, defaults to 3)
    ///
    /// # Returns
    /// Vector of nodes that depend on the target node
    pub async fn get_reverse_dependencies(
        &self,
        node_id: &str,
        edge_type: &str,
        depth: i32,
    ) -> Result<Vec<DependencyNode>> {
        debug!(
            "Calling fn::get_reverse_dependencies({}, {}, {})",
            node_id, edge_type, depth
        );

        let result: Vec<DependencyNode> = self
            .db
            .query("RETURN fn::get_reverse_dependencies($node_id, $edge_type, $depth)")
            .bind(("node_id", node_id.to_string()))
            .bind(("edge_type", edge_type.to_string()))
            .bind(("depth", depth))
            .await
            .map_err(|e| {
                error!("Failed to call get_reverse_dependencies: {}", e);
                CodeGraphError::Database(format!("get_reverse_dependencies failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize reverse dependencies: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }
}

// ============================================================================
// Type Definitions for Function Results
// ============================================================================

/// Node with dependency depth information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub location: Option<NodeLocation>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub dependency_depth: Option<i32>,
    pub dependent_depth: Option<i32>,
}

/// Circular dependency pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircularDependency {
    pub node1_id: String,
    pub node2_id: String,
    pub node1: NodeInfo,
    pub node2: NodeInfo,
    pub dependency_type: String,
}

/// Call chain node with caller information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallChainNode {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub location: Option<NodeLocation>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub call_depth: Option<i32>,
    pub called_by: Option<Vec<CallerInfo>>,
}

/// Coupling metrics result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingMetricsResult {
    pub node: NodeInfo,
    pub metrics: CouplingMetrics,
    pub dependents: Vec<NodeReference>,
    pub dependencies: Vec<NodeReference>,
}

/// Coupling metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingMetrics {
    pub afferent_coupling: i32,
    pub efferent_coupling: i32,
    pub total_coupling: i32,
    pub instability: f64,
    pub stability: f64,
    pub is_stable: bool,
    pub is_unstable: bool,
    pub coupling_category: String,
}

/// Hub node with degree information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubNode {
    pub node_id: String,
    pub node: NodeInfo,
    pub afferent_degree: i32,
    pub efferent_degree: i32,
    pub total_degree: i32,
    pub incoming_by_type: Vec<EdgeTypeCount>,
    pub outgoing_by_type: Vec<EdgeTypeCount>,
}

/// Edge type count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTypeCount {
    pub edge_type: String,
    pub count: i32,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub location: Option<NodeLocation>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Node reference (minimal info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReference {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub location: Option<NodeLocation>,
}

/// Caller information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerInfo {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
}

/// Node location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLocation {
    pub file_path: String,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_node_serialization() {
        let node = DependencyNode {
            id: "test:1".to_string(),
            name: "test_function".to_string(),
            kind: Some("function".to_string()),
            location: Some(NodeLocation {
                file_path: "test.rs".to_string(),
                start_line: Some(10),
                end_line: Some(20),
            }),
            language: Some("rust".to_string()),
            content: None,
            metadata: None,
            dependency_depth: Some(1),
            dependent_depth: None,
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("test_function"));
    }
}
