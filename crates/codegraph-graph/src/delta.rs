use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{CodeEdge, CodeGraph};
use codegraph_core::{CodeNode, EdgeType, GraphStore, NodeId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOperation {
    AddNode(CodeNode),
    RemoveNode(NodeId),
    UpdateNode(NodeId, CodeNode),
    AddEdge(NodeId, NodeId, EdgeType, HashMap<String, String>),
    RemoveEdge(NodeId, NodeId, EdgeType),
    UpdateEdge(NodeId, NodeId, EdgeType, HashMap<String, String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDelta {
    pub id: Uuid,
    pub operations: Vec<DeltaOperation>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub file_path: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeltaComputationResult {
    pub delta: GraphDelta,
    pub affected_nodes: HashSet<NodeId>,
    pub affected_edges: HashSet<(NodeId, NodeId, EdgeType)>,
    pub performance_stats: DeltaPerformanceStats,
}

#[derive(Debug, Clone)]
pub struct DeltaPerformanceStats {
    pub nodes_compared: usize,
    pub edges_compared: usize,
    pub operations_generated: usize,
    pub computation_duration_ms: u64,
    pub memory_usage_bytes: usize,
}

pub struct GraphDeltaProcessor {
    node_comparator: NodeComparator,
    edge_comparator: EdgeComparator,
    optimization_threshold: usize,
}

impl GraphDeltaProcessor {
    pub fn new() -> Self {
        Self {
            node_comparator: NodeComparator::new(),
            edge_comparator: EdgeComparator::new(),
            optimization_threshold: 1000, // Nodes threshold for optimization
        }
    }

    pub fn with_optimization_threshold(mut self, threshold: usize) -> Self {
        self.optimization_threshold = threshold;
        self
    }

    pub async fn compute_delta(
        &self,
        old_nodes: &[CodeNode],
        new_nodes: &[CodeNode],
        file_path: Option<String>,
        content_hash: Option<String>,
    ) -> Result<DeltaComputationResult> {
        let start_time = std::time::Instant::now();

        info!(
            "Computing graph delta for {} -> {} nodes",
            old_nodes.len(),
            new_nodes.len()
        );

        // Build lookup maps for efficient comparison
        let old_node_map = self.build_node_map(old_nodes);
        let new_node_map = self.build_node_map(new_nodes);

        let mut operations = Vec::new();
        let mut affected_nodes = HashSet::new();
        let mut affected_edges = HashSet::new();

        // Use different strategies based on the size of the change
        if old_nodes.len() + new_nodes.len() > self.optimization_threshold {
            // For large graphs, use optimized comparison
            self.compute_delta_optimized(
                &old_node_map,
                &new_node_map,
                &mut operations,
                &mut affected_nodes,
                &mut affected_edges,
            )
            .await?;
        } else {
            // For smaller graphs, use comprehensive comparison
            self.compute_delta_comprehensive(
                &old_node_map,
                &new_node_map,
                &mut operations,
                &mut affected_nodes,
                &mut affected_edges,
            )
            .await?;
        }

        let computation_duration = start_time.elapsed();

        let delta = GraphDelta {
            id: Uuid::new_v4(),
            operations,
            timestamp: chrono::Utc::now(),
            file_path,
            content_hash,
        };

        let stats = DeltaPerformanceStats {
            nodes_compared: old_nodes.len() + new_nodes.len(),
            edges_compared: 0, // TODO: Implement edge counting
            operations_generated: delta.operations.len(),
            computation_duration_ms: computation_duration.as_millis() as u64,
            memory_usage_bytes: std::mem::size_of_val(&delta)
                + std::mem::size_of_val(&affected_nodes)
                + std::mem::size_of_val(&affected_edges),
        };

        info!(
            "Delta computation completed: {} operations in {}ms",
            delta.operations.len(),
            stats.computation_duration_ms
        );

        Ok(DeltaComputationResult {
            delta,
            affected_nodes,
            affected_edges,
            performance_stats: stats,
        })
    }

    fn build_node_map<'a>(&self, nodes: &'a [CodeNode]) -> HashMap<NodeId, &'a CodeNode> {
        nodes.iter().map(|node| (node.id.clone(), node)).collect()
    }

    async fn compute_delta_comprehensive(
        &self,
        old_nodes: &HashMap<NodeId, &CodeNode>,
        new_nodes: &HashMap<NodeId, &CodeNode>,
        operations: &mut Vec<DeltaOperation>,
        affected_nodes: &mut HashSet<NodeId>,
        affected_edges: &mut HashSet<(NodeId, NodeId, EdgeType)>,
    ) -> Result<()> {
        // Find removed nodes
        for (old_id, old_node) in old_nodes {
            if !new_nodes.contains_key(old_id) {
                operations.push(DeltaOperation::RemoveNode(old_id.clone()));
                affected_nodes.insert(old_id.clone());
                debug!("Node removed: {} ({})", old_node.name.as_str(), old_id);
            }
        }

        // Find added and updated nodes
        for (new_id, new_node) in new_nodes {
            match old_nodes.get(new_id) {
                None => {
                    // New node
                    operations.push(DeltaOperation::AddNode((*new_node).clone()));
                    affected_nodes.insert(new_id.clone());
                    debug!("Node added: {} ({})", new_node.name.as_str(), new_id);
                }
                Some(old_node) => {
                    // Check if node changed
                    if self.node_comparator.has_changed(old_node, new_node) {
                        operations.push(DeltaOperation::UpdateNode(
                            new_id.clone(),
                            (*new_node).clone(),
                        ));
                        affected_nodes.insert(new_id.clone());
                        debug!("Node updated: {} ({})", new_node.name.as_str(), new_id);
                    }
                }
            }
        }

        // TODO: Implement edge comparison
        // This would require maintaining edge information in the nodes or having separate edge data

        Ok(())
    }

    async fn compute_delta_optimized(
        &self,
        old_nodes: &HashMap<NodeId, &CodeNode>,
        new_nodes: &HashMap<NodeId, &CodeNode>,
        operations: &mut Vec<DeltaOperation>,
        affected_nodes: &mut HashSet<NodeId>,
        affected_edges: &mut HashSet<(NodeId, NodeId, EdgeType)>,
    ) -> Result<()> {
        // For large graphs, use parallel processing and heuristics
        use rayon::prelude::*;

        // Create thread-safe collections for parallel processing
        let operations_map: Arc<DashMap<NodeId, DeltaOperation>> = Arc::new(DashMap::new());
        let affected_nodes_set: Arc<DashMap<NodeId, ()>> = Arc::new(DashMap::new());

        // Process removals
        old_nodes.par_iter().for_each(|(old_id, old_node)| {
            if !new_nodes.contains_key(old_id) {
                operations_map.insert(old_id.clone(), DeltaOperation::RemoveNode(old_id.clone()));
                affected_nodes_set.insert(old_id.clone(), ());
            }
        });

        // Process additions and updates
        new_nodes
            .par_iter()
            .for_each(|(new_id, new_node)| match old_nodes.get(new_id) {
                None => {
                    operations_map
                        .insert(new_id.clone(), DeltaOperation::AddNode((*new_node).clone()));
                    affected_nodes_set.insert(new_id.clone(), ());
                }
                Some(old_node) => {
                    if self.node_comparator.has_changed(old_node, new_node) {
                        operations_map.insert(
                            new_id.clone(),
                            DeltaOperation::UpdateNode(new_id.clone(), (*new_node).clone()),
                        );
                        affected_nodes_set.insert(new_id.clone(), ());
                    }
                }
            });

        // Collect results without moving out of Arc
        operations.extend(operations_map.iter().map(|entry| entry.value().clone()));
        affected_nodes.extend(affected_nodes_set.iter().map(|entry| entry.key().clone()));

        Ok(())
    }

    pub async fn apply_delta(
        &self,
        graph: &mut CodeGraph,
        delta: &GraphDelta,
    ) -> Result<DeltaApplicationResult> {
        let start_time = std::time::Instant::now();
        let mut applied_operations = 0;
        let mut failed_operations = 0;
        let mut warnings = Vec::new();

        info!(
            "Applying delta {} with {} operations",
            delta.id,
            delta.operations.len()
        );

        for operation in &delta.operations {
            match self.apply_single_operation(graph, operation).await {
                Ok(_) => applied_operations += 1,
                Err(e) => {
                    failed_operations += 1;
                    let warning = format!("Failed to apply operation {:?}: {}", operation, e);
                    warn!("{}", warning);
                    warnings.push(warning);
                }
            }
        }

        let duration = start_time.elapsed();

        info!(
            "Delta application completed: {}/{} operations successful in {}ms",
            applied_operations,
            delta.operations.len(),
            duration.as_millis()
        );

        Ok(DeltaApplicationResult {
            applied_operations,
            failed_operations,
            warnings,
            duration_ms: duration.as_millis() as u64,
        })
    }

    async fn apply_single_operation(
        &self,
        graph: &mut CodeGraph,
        operation: &DeltaOperation,
    ) -> Result<()> {
        match operation {
            DeltaOperation::AddNode(node) => {
                graph.add_node(node.clone()).await?;
            }
            DeltaOperation::RemoveNode(node_id) => {
                graph.remove_node(*node_id).await?;
            }
            DeltaOperation::UpdateNode(node_id, new_node) => {
                // For updates, we remove and re-add to ensure consistency
                if graph.get_node(*node_id).await?.is_some() {
                    graph.remove_node(*node_id).await?;
                }
                graph.add_node(new_node.clone()).await?;
            }
            DeltaOperation::AddEdge(from, to, edge_type, metadata) => {
                let mut edge = CodeEdge::new(from.clone(), to.clone(), edge_type.clone());
                for (key, value) in metadata {
                    edge = edge.with_metadata(key.clone(), value.clone());
                }
                graph.add_edge(edge).await?;
            }
            DeltaOperation::RemoveEdge(from, to, edge_type) => {
                let _ = graph
                    .remove_edge(from.clone(), to.clone(), Some(edge_type.clone()))
                    .await?;
            }
            DeltaOperation::UpdateEdge(from, to, edge_type, metadata) => {
                // Remove old edge and add new one with updated metadata
                let _ = graph
                    .remove_edge(from.clone(), to.clone(), Some(edge_type.clone()))
                    .await?;
                let mut edge = CodeEdge::new(from.clone(), to.clone(), edge_type.clone());
                for (key, value) in metadata {
                    edge = edge.with_metadata(key.clone(), value.clone());
                }
                graph.add_edge(edge).await?;
            }
        }
        Ok(())
    }

    pub fn estimate_delta_size(
        &self,
        old_nodes: &[CodeNode],
        new_nodes: &[CodeNode],
    ) -> DeltaSizeEstimate {
        let old_count = old_nodes.len();
        let new_count = new_nodes.len();

        let estimated_operations = if old_count == 0 {
            new_count // All additions
        } else if new_count == 0 {
            old_count // All deletions
        } else {
            // Heuristic: assume some percentage of changes
            let max_changes = std::cmp::min(old_count, new_count);
            let additions = new_count.saturating_sub(old_count);
            let deletions = old_count.saturating_sub(new_count);
            let modifications = max_changes / 4; // Assume 25% modification rate

            additions + deletions + modifications
        };

        let complexity = if estimated_operations < 100 {
            DeltaComplexity::Low
        } else if estimated_operations < 1000 {
            DeltaComplexity::Medium
        } else {
            DeltaComplexity::High
        };

        DeltaSizeEstimate {
            estimated_operations,
            complexity,
            recommended_strategy: if estimated_operations > self.optimization_threshold {
                DeltaStrategy::Optimized
            } else {
                DeltaStrategy::Comprehensive
            },
        }
    }
}

pub struct NodeComparator {
    ignore_fields: HashSet<String>,
}

impl NodeComparator {
    pub fn new() -> Self {
        let mut ignore_fields = HashSet::new();
        // Fields that might change but don't affect semantic meaning
        ignore_fields.insert("last_modified".to_string());
        ignore_fields.insert("parse_time".to_string());

        Self { ignore_fields }
    }

    pub fn has_changed(&self, old: &CodeNode, new: &CodeNode) -> bool {
        // Compare key fields that matter for semantic analysis
        old.name != new.name
            || old.node_type != new.node_type
            || old.content != new.content
            || old.location.line != new.location.line
            || old.location.column != new.location.column
            || old.location.end_line != new.location.end_line
            || old.location.end_column != new.location.end_column
            || old.metadata.attributes != new.metadata.attributes
    }

    fn metadata_changed(
        &self,
        old_metadata: &Option<HashMap<String, String>>,
        new_metadata: &Option<HashMap<String, String>>,
    ) -> bool {
        match (old_metadata, new_metadata) {
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
            (Some(old), Some(new)) => {
                if old.len() != new.len() {
                    return true;
                }

                for (key, old_value) in old {
                    if self.ignore_fields.contains(key) {
                        continue;
                    }

                    if new.get(key) != Some(old_value) {
                        return true;
                    }
                }

                false
            }
        }
    }
}

pub struct EdgeComparator;

impl EdgeComparator {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub struct DeltaApplicationResult {
    pub applied_operations: usize,
    pub failed_operations: usize,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct DeltaSizeEstimate {
    pub estimated_operations: usize,
    pub complexity: DeltaComplexity,
    pub recommended_strategy: DeltaStrategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeltaComplexity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeltaStrategy {
    Comprehensive,
    Optimized,
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::Language;

    #[test]
    fn test_node_comparator() {
        let comparator = NodeComparator::new();

        let node1 = CodeNode {
            id: Some("test1".to_string()),
            name: Some("function1".to_string()),
            node_type: "function".to_string(),
            content: Some("fn test() {}".to_string()),
            ..Default::default()
        };

        let node2 = CodeNode {
            id: Some("test1".to_string()),
            name: Some("function1".to_string()),
            node_type: "function".to_string(),
            content: Some("fn test() { println!(\"hello\"); }".to_string()),
            ..Default::default()
        };

        assert!(comparator.has_changed(&node1, &node2));
    }

    #[tokio::test]
    async fn test_delta_computation() {
        let processor = GraphDeltaProcessor::new();

        let old_nodes = vec![CodeNode {
            id: Some("node1".to_string()),
            name: Some("function1".to_string()),
            node_type: "function".to_string(),
            ..Default::default()
        }];

        let new_nodes = vec![CodeNode {
            id: Some("node1".to_string()),
            name: Some("function1_modified".to_string()),
            node_type: "function".to_string(),
            ..Default::default()
        }];

        let result = processor
            .compute_delta(&old_nodes, &new_nodes, Some("test.rs".to_string()), None)
            .await
            .unwrap();

        assert_eq!(result.delta.operations.len(), 1);
        assert!(matches!(
            result.delta.operations[0],
            DeltaOperation::UpdateNode(_, _)
        ));
    }

    #[test]
    fn test_delta_size_estimation() {
        let processor = GraphDeltaProcessor::new();

        let old_nodes = vec![
            CodeNode {
                id: Some("1".to_string()),
                ..Default::default()
            },
            CodeNode {
                id: Some("2".to_string()),
                ..Default::default()
            },
        ];

        let new_nodes = vec![
            CodeNode {
                id: Some("1".to_string()),
                ..Default::default()
            },
            CodeNode {
                id: Some("3".to_string()),
                ..Default::default()
            },
        ];

        let estimate = processor.estimate_delta_size(&old_nodes, &new_nodes);
        assert_eq!(estimate.complexity, DeltaComplexity::Low);
        assert_eq!(estimate.recommended_strategy, DeltaStrategy::Comprehensive);
    }
}
