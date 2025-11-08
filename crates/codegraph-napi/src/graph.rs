// ABOUTME: Graph analysis functions exposed to Node.js via NAPI
// ABOUTME: Wraps SurrealDB GraphFunctions with TypeScript-friendly async APIs

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;

use crate::state::with_state;
use crate::types::*;

/// Helper to convert Rust graph types to NAPI types
#[cfg(feature = "cloud-surrealdb")]
mod converters {
    use super::*;
    use codegraph_graph;

    pub fn to_node_location(loc: &codegraph_graph::NodeLocation) -> NodeLocation {
        NodeLocation {
            file_path: loc.file_path.clone(),
            start_line: loc.start_line,
            end_line: loc.end_line,
        }
    }

    pub fn to_node_info(node: &codegraph_graph::NodeInfo) -> NodeInfo {
        NodeInfo {
            id: node.id.clone(),
            name: node.name.clone(),
            kind: node.kind.clone(),
            location: node.location.as_ref().map(to_node_location),
            language: node.language.clone(),
            content: node.content.clone(),
            metadata: node.metadata.as_ref().map(|v| v.to_string()),
        }
    }

    pub fn to_dependency_node(node: &codegraph_graph::DependencyNode) -> DependencyNode {
        DependencyNode {
            id: node.id.clone(),
            name: node.name.clone(),
            kind: node.kind.clone(),
            location: node.location.as_ref().map(to_node_location),
            language: node.language.clone(),
            content: node.content.clone(),
            metadata: node.metadata.as_ref().map(|v| v.to_string()),
            dependency_depth: node.dependency_depth,
            dependent_depth: node.dependent_depth,
        }
    }

    pub fn to_circular_dependency(cd: &codegraph_graph::CircularDependency) -> CircularDependency {
        CircularDependency {
            node1_id: cd.node1_id.clone(),
            node2_id: cd.node2_id.clone(),
            node1: to_node_info(&cd.node1),
            node2: to_node_info(&cd.node2),
            dependency_type: cd.dependency_type.clone(),
        }
    }

    pub fn to_caller_info(caller: &codegraph_graph::CallerInfo) -> CallerInfo {
        CallerInfo {
            id: caller.id.clone(),
            name: caller.name.clone(),
            kind: caller.kind.clone(),
        }
    }

    pub fn to_call_chain_node(node: &codegraph_graph::CallChainNode) -> CallChainNode {
        CallChainNode {
            id: node.id.clone(),
            name: node.name.clone(),
            kind: node.kind.clone(),
            location: node.location.as_ref().map(to_node_location),
            language: node.language.clone(),
            content: node.content.clone(),
            metadata: node.metadata.as_ref().map(|v| v.to_string()),
            call_depth: node.call_depth,
            called_by: node
                .called_by
                .as_ref()
                .map(|callers| callers.iter().map(to_caller_info).collect()),
        }
    }

    pub fn to_node_reference(node_ref: &codegraph_graph::NodeReference) -> NodeReference {
        NodeReference {
            id: node_ref.id.clone(),
            name: node_ref.name.clone(),
            kind: node_ref.kind.clone(),
            location: node_ref.location.as_ref().map(to_node_location),
        }
    }

    pub fn to_coupling_metrics(metrics: &codegraph_graph::CouplingMetrics) -> CouplingMetrics {
        CouplingMetrics {
            afferent_coupling: metrics.afferent_coupling,
            efferent_coupling: metrics.efferent_coupling,
            total_coupling: metrics.total_coupling,
            instability: metrics.instability,
            stability: metrics.stability,
            is_stable: metrics.is_stable,
            is_unstable: metrics.is_unstable,
            coupling_category: metrics.coupling_category.clone(),
        }
    }

    pub fn to_coupling_metrics_result(
        result: &codegraph_graph::CouplingMetricsResult,
    ) -> CouplingMetricsResult {
        CouplingMetricsResult {
            node: to_node_info(&result.node),
            metrics: to_coupling_metrics(&result.metrics),
            dependents: result.dependents.iter().map(to_node_reference).collect(),
            dependencies: result.dependencies.iter().map(to_node_reference).collect(),
        }
    }

    pub fn to_edge_type_count(etc: &codegraph_graph::EdgeTypeCount) -> EdgeTypeCount {
        EdgeTypeCount {
            edge_type: etc.edge_type.clone(),
            count: etc.count,
        }
    }

    pub fn to_hub_node(hub: &codegraph_graph::HubNode) -> HubNode {
        HubNode {
            node_id: hub.node_id.clone(),
            node: to_node_info(&hub.node),
            afferent_degree: hub.afferent_degree,
            efferent_degree: hub.efferent_degree,
            total_degree: hub.total_degree,
            incoming_by_type: hub
                .incoming_by_type
                .iter()
                .map(to_edge_type_count)
                .collect(),
            outgoing_by_type: hub
                .outgoing_by_type
                .iter()
                .map(to_edge_type_count)
                .collect(),
        }
    }
}

/// Get transitive dependencies of a node
#[napi]
pub async fn get_transitive_dependencies(
    node_id: String,
    edge_type: String,
    depth: Option<i32>,
) -> Result<Vec<DependencyNode>> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let depth = depth.unwrap_or(3);

        let nodes = gf
            .get_transitive_dependencies(&node_id, &edge_type, depth)
            .await
            .map_err(|e| {
                Error::from_reason(format!("get_transitive_dependencies failed: {}", e))
            })?;

        Ok(nodes.iter().map(converters::to_dependency_node).collect())
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}

/// Detect circular dependencies for a given edge type
#[napi]
pub async fn detect_circular_dependencies(edge_type: String) -> Result<Vec<CircularDependency>> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let cycles = gf
            .detect_circular_dependencies(&edge_type)
            .await
            .map_err(|e| {
                Error::from_reason(format!("detect_circular_dependencies failed: {}", e))
            })?;

        Ok(cycles
            .iter()
            .map(converters::to_circular_dependency)
            .collect())
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}

/// Trace the call chain starting from a node
#[napi]
pub async fn trace_call_chain(
    from_node: String,
    max_depth: Option<i32>,
) -> Result<Vec<CallChainNode>> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let max_depth = max_depth.unwrap_or(5);

        let chain = gf
            .trace_call_chain(&from_node, max_depth)
            .await
            .map_err(|e| Error::from_reason(format!("trace_call_chain failed: {}", e)))?;

        Ok(chain.iter().map(converters::to_call_chain_node).collect())
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}

/// Calculate coupling metrics for a node
#[napi]
pub async fn calculate_coupling_metrics(node_id: String) -> Result<CouplingMetricsResult> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let result = gf
            .calculate_coupling_metrics(&node_id)
            .await
            .map_err(|e| Error::from_reason(format!("calculate_coupling_metrics failed: {}", e)))?;

        Ok(converters::to_coupling_metrics_result(&result))
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}

/// Get hub nodes with degree >= min_degree
#[napi]
pub async fn get_hub_nodes(min_degree: Option<i32>) -> Result<Vec<HubNode>> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let min_degree = min_degree.unwrap_or(5);

        let hubs = gf
            .get_hub_nodes(min_degree)
            .await
            .map_err(|e| Error::from_reason(format!("get_hub_nodes failed: {}", e)))?;

        Ok(hubs.iter().map(converters::to_hub_node).collect())
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}

/// Get reverse dependencies (dependents) of a node
#[napi]
pub async fn get_reverse_dependencies(
    node_id: String,
    edge_type: String,
    depth: Option<i32>,
) -> Result<Vec<DependencyNode>> {
    #[cfg(feature = "cloud-surrealdb")]
    {
        let gf: Arc<codegraph_graph::GraphFunctions> = with_state(|state| {
            let gf = state.graph_functions.as_ref().ok_or_else(|| {
                Error::from_reason(
                    "GraphFunctions not initialized. Set SURREALDB_CONNECTION environment variable.",
                )
            })?;
            Ok(Arc::clone(gf))
        })
        .await?;

        let depth = depth.unwrap_or(3);

        let nodes = gf
            .get_reverse_dependencies(&node_id, &edge_type, depth)
            .await
            .map_err(|e| Error::from_reason(format!("get_reverse_dependencies failed: {}", e)))?;

        Ok(nodes.iter().map(converters::to_dependency_node).collect())
    }

    #[cfg(not(feature = "cloud-surrealdb"))]
    {
        Err(Error::from_reason(
            "cloud-surrealdb feature not enabled. Rebuild with --features cloud-surrealdb",
        ))
    }
}
