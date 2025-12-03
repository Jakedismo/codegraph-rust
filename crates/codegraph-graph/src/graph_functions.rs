// ABOUTME: Rust SDK wrappers for SurrealDB graph analysis functions
// ABOUTME: Provides type-safe interfaces for LLM-powered graph analysis tools

use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::json;
use std::sync::Arc;
use surrealdb::{engine::any::Any, Surreal, Value as SurrealValue};
use tracing::{debug, error, warn};

/// Wrapper for SurrealDB graph analysis functions
/// Provides type-safe Rust interfaces for calling SurrealDB functions
#[derive(Clone)]
pub struct GraphFunctions {
    db: Arc<Surreal<Any>>,
    project_id: String,
}

impl GraphFunctions {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self {
            db,
            project_id: Self::default_project_id(),
        }
    }

    pub fn new_with_project_id(db: Arc<Surreal<Any>>, project_id: impl Into<String>) -> Self {
        Self {
            db,
            project_id: project_id.into(),
        }
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    fn default_project_id() -> String {
        std::env::var("CODEGRAPH_PROJECT_ID")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|p| p.display().to_string())
            })
            .unwrap_or_else(|| "default-project".to_string())
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
            "Calling fn::get_transitive_dependencies({}, {}, {}, project={})",
            node_id, edge_type, depth, self.project_id
        );

        let result: Vec<DependencyNode> = self
            .db
            .query(
                "RETURN fn::get_transitive_dependencies($project_id, $node_id, $edge_type, $depth)",
            )
            .bind(("project_id", self.project_id.clone()))
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
        debug!(
            "Calling fn::detect_circular_dependencies({}, project={})",
            edge_type, self.project_id
        );

        let result: Vec<CircularDependency> = self
            .db
            .query("RETURN fn::detect_circular_dependencies($project_id, $edge_type)")
            .bind(("project_id", self.project_id.clone()))
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
        debug!(
            "Calling fn::trace_call_chain({}, {}, project={})",
            from_node, max_depth, self.project_id
        );

        let result: Vec<CallChainNode> = self
            .db
            .query("RETURN fn::trace_call_chain($project_id, $from_node, $max_depth)")
            .bind(("project_id", self.project_id.clone()))
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
        debug!(
            "Calling fn::calculate_coupling_metrics({}, project={})",
            node_id, self.project_id
        );

        let results: Vec<CouplingMetricsResult> = self
            .db
            .query("RETURN fn::calculate_coupling_metrics($project_id, $node_id)")
            .bind(("project_id", self.project_id.clone()))
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
        debug!(
            "Calling fn::get_hub_nodes({}, project={})",
            min_degree, self.project_id
        );

        let result: Vec<HubNode> = self
            .db
            .query("RETURN fn::get_hub_nodes($project_id, $min_degree)")
            .bind(("project_id", self.project_id.clone()))
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
            "Calling fn::get_reverse_dependencies({}, {}, {}, project={})",
            node_id, edge_type, depth, self.project_id
        );

        let result: Vec<DependencyNode> = self
            .db
            .query("RETURN fn::get_reverse_dependencies($project_id, $node_id, $edge_type, $depth)")
            .bind(("project_id", self.project_id.clone()))
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

    /// Count nodes for the current project (used for health checks)
    pub async fn count_nodes_for_project(&self) -> Result<usize> {
        let mut response = self
            .db
            .query("SELECT VALUE count() FROM nodes WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!("count_nodes_for_project query failed: {}", e))
            })?;

        let count: Option<usize> = response
            .take(0)
            .map_err(|e| CodeGraphError::Database(format!("Failed to deserialize count: {}", e)))?;

        Ok(count.unwrap_or(0))
    }

    /// Find nodes by (partial) name within the current project
    pub async fn find_nodes_by_name(
        &self,
        needle: &str,
        limit: usize,
    ) -> Result<Vec<NodeReference>> {
        let max = limit.clamp(1, 50) as i64;

        debug!(
            "Calling fn::find_nodes_by_name({}, project={}, limit={})",
            needle, self.project_id, max
        );

        let result: Vec<NodeReference> = self
            .db
            .query("RETURN fn::find_nodes_by_name($project_id, $needle, $limit)")
            .bind(("project_id", self.project_id.clone()))
            .bind(("needle", needle.to_string()))
            .bind(("limit", max))
            .await
            .map_err(|e| {
                error!("Failed to call find_nodes_by_name: {}", e);
                CodeGraphError::Database(format!("find_nodes_by_name failed: {}", e))
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to deserialize find_nodes_by_name results: {}", e);
                CodeGraphError::Database(format!("Deserialization failed: {}", e))
            })?;

        Ok(result)
    }

    /// Comprehensive semantic search with HNSW vector search, full-text, and graph enrichment
    ///
    /// Calls fn::semantic_search_with_context in SurrealDB which combines:
    /// - HNSW vector similarity search
    /// - Full-text search using code_analyzer
    /// - Graph enrichment with dependencies and file context
    ///
    /// # Parameters
    /// - `query_text`: Original search query
    /// - `query_embedding`: Pre-generated embedding vector
    /// - `dimension`: Embedding dimension (384,768,1024,1536,2048,2560,3072,4096)
    /// - `limit`: Maximum results
    /// - `threshold`: Minimum similarity score (0.0-1.0)
    /// - `include_graph_context`: Whether to enrich with graph data
    pub async fn semantic_search_with_context(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        dimension: usize,
        limit: usize,
        threshold: f32,
        include_graph_context: bool,
    ) -> Result<Vec<serde_json::Value>> {
        let skip_chunking = std::env::var("CODEGRAPH_EMBEDDING_SKIP_CHUNKING")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !skip_chunking {
            return self
                .semantic_search_chunks_with_context(
                    query_text,
                    query_embedding,
                    dimension,
                    limit,
                    threshold,
                    include_graph_context,
                )
                .await;
        }
        debug!(
            "Calling fn::semantic_search_with_context(project={}, query='{}', dim={}, limit={}, threshold={})",
            self.project_id, query_text, dimension, limit, threshold
        );

        // Convert embedding to Value
        let embedding_value: serde_json::Value =
            serde_json::to_value(query_embedding).map_err(|e| {
                CodeGraphError::Database(format!("Failed to serialize embedding: {}", e))
            })?;

        let mut response = self
            .db
            .query("RETURN fn::semantic_search_with_context($project_id, $query_embedding, $query_text, $dimension, $limit, $threshold, $include_graph_context)")
            .bind(("project_id", self.project_id.clone()))
            .bind(("query_embedding", embedding_value))
            .bind(("query_text", query_text.to_string()))
            .bind(("dimension", dimension as i64))
            .bind(("limit", limit as i64))
            .bind(("threshold", threshold as f64))
            .bind(("include_graph_context", include_graph_context))
            .await
            .map_err(|e| {
                error!("Failed to call semantic_search_with_context: {}", e);
                CodeGraphError::Database(format!("semantic_search_with_context failed: {}", e))
            })?;

        // Workaround for SurrealDB 2.x SDK bug (GitHub #4921):
        // Direct deserialization to serde_json::Value fails with "invalid type: enum"
        // Instead, get raw SurrealDB Value and serialize via serde_json::to_value
        let raw_value: SurrealValue = response.take(0).map_err(|e| {
            error!("Failed to get raw value from semantic_search_with_context: {}", e);
            CodeGraphError::Database(format!("Failed to get raw value: {}", e))
        })?;

        // Convert SurrealDB Value to serde_json::Value via Serialize trait
        let json_value = serde_json::to_value(&raw_value).map_err(|e| {
            error!("Failed to serialize semantic_search_with_context result: {}", e);
            CodeGraphError::Database(format!("Serialization failed: {}", e))
        })?;

        // Extract Vec from the JSON value
        let result: Vec<serde_json::Value> = match json_value {
            serde_json::Value::Array(arr) => arr,
            serde_json::Value::Null => Vec::new(),
            other => vec![other],
        };

        Ok(result)
    }

    async fn semantic_search_chunks_with_context(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        dimension: usize,
        limit: usize,
        threshold: f32,
        include_graph_context: bool,
    ) -> Result<Vec<serde_json::Value>> {
        debug!(
            "Calling fn::semantic_search_chunks_with_context(project={}, query='{}', dim={}, limit={}, threshold={})",
            self.project_id, query_text, dimension, limit, threshold
        );

        let embedding_value: serde_json::Value =
            serde_json::to_value(query_embedding).map_err(|e| {
                CodeGraphError::Database(format!("Failed to serialize embedding: {}", e))
            })?;

        let mut response = self
            .db
            .query("RETURN fn::semantic_search_chunks_with_context($project_id, $query_embedding, $query_text, $dimension, $limit, $threshold, $include_graph_context)")
            .bind(("project_id", self.project_id.clone()))
            .bind(("query_embedding", embedding_value))
            .bind(("query_text", query_text.to_string()))
            .bind(("dimension", dimension as i64))
            .bind(("limit", limit as i64))
            .bind(("threshold", threshold as f64))
            .bind(("include_graph_context", include_graph_context))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("semantic_search_chunks_with_context") {
                    warn!(
                        "semantic_search_chunks_with_context missing in DB; falling back to fn::semantic_search_with_context"
                    );
                    CodeGraphError::Database("MISSING_CHUNK_FN".into())
                } else {
                    error!("Failed to call semantic_search_chunks_with_context: {}", msg);
                    CodeGraphError::Database(format!("semantic_search_chunks_with_context failed: {}", msg))
                }
            })?;

        // Workaround for SurrealDB 2.x SDK bug (GitHub #4921):
        // Direct deserialization to serde_json::Value fails with "invalid type: enum"
        // Instead, get raw SurrealDB Value and serialize via serde_json::to_value
        let raw_value: SurrealValue = response.take(0).map_err(|e| {
            error!(
                "Failed to get raw value from semantic_search_chunks_with_context: {}",
                e
            );
            CodeGraphError::Database(format!("Failed to get raw value: {}", e))
        })?;

        // Convert SurrealDB Value to serde_json::Value via Serialize trait
        let json_value = serde_json::to_value(&raw_value).map_err(|e| {
            error!(
                "Failed to serialize semantic_search_chunks_with_context result: {}",
                e
            );
            CodeGraphError::Database(format!("Serialization failed: {}", e))
        })?;

        // Extract Vec from the JSON value
        let result: Vec<serde_json::Value> = match json_value {
            serde_json::Value::Array(arr) => arr,
            serde_json::Value::Null => Vec::new(),
            other => vec![other],
        };

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

    #[cfg(feature = "surrealdb")]
    #[tokio::test]
    async fn count_nodes_for_project_filters_by_project() {
        use surrealdb::opt::auth::Root;

        let db: Surreal<Any> = Surreal::init();
        db.connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .ok(); // mem engine ignores auth

        // Two projects, only one should be counted
        db.query("CREATE nodes CONTENT $doc")
            .bind((
                "doc",
                json!({
                    "id": "nodes:a1",
                    "name": "A1",
                    "project_id": "proj-a"
                }),
            ))
            .await
            .unwrap();

        db.query("CREATE nodes CONTENT $doc")
            .bind((
                "doc",
                json!({
                    "id": "nodes:b1",
                    "name": "B1",
                    "project_id": "proj-b"
                }),
            ))
            .await
            .unwrap();

        let gf = GraphFunctions::new_with_project_id(Arc::new(db), "proj-a");
        let count = gf.count_nodes_for_project().await.unwrap();

        assert_eq!(count, 1, "Should only count nodes in proj-a");
    }
}
