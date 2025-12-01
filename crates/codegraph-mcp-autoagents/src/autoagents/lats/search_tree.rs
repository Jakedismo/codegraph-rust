// ABOUTME: Search tree data structure for LATS algorithm
// ABOUTME: Implements UCT (Upper Confidence Bound for Trees) scoring for node selection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub type NodeId = usize;

#[derive(Error, Debug)]
pub enum SearchTreeError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Invalid node operation: {0}")]
    InvalidOperation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAction {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct SearchNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub thought: String,
    pub action: Option<ToolAction>,
    pub observation: Option<serde_json::Value>,
    pub score: f32,
    pub visits: usize,
    pub children: Vec<NodeId>,
    pub depth: usize,
}

impl SearchNode {
    pub fn new_root(thought: String) -> Self {
        Self {
            id: 0,
            parent_id: None,
            thought,
            action: None,
            observation: None,
            score: 0.0,
            visits: 0,
            children: Vec::new(),
            depth: 0,
        }
    }
}

pub struct SearchTree {
    nodes: HashMap<NodeId, SearchNode>,
    root_id: NodeId,
    next_id: NodeId,
}

impl SearchTree {
    /// Create a new search tree with a root node
    pub fn new(root: SearchNode) -> Self {
        let root_id = root.id;
        let mut nodes = HashMap::new();
        nodes.insert(root_id, root);

        Self {
            nodes,
            root_id,
            next_id: root_id + 1,
        }
    }

    /// Add a new node to the tree
    pub fn add_node(
        &mut self,
        parent_id: NodeId,
        thought: String,
        action: Option<ToolAction>,
        observation: Option<serde_json::Value>,
        score: f32,
    ) -> Result<NodeId, SearchTreeError> {
        // Validate parent exists
        let parent_depth = self
            .nodes
            .get(&parent_id)
            .ok_or(SearchTreeError::NodeNotFound(parent_id))?
            .depth;

        let node_id = self.next_id;
        self.next_id += 1;

        let node = SearchNode {
            id: node_id,
            parent_id: Some(parent_id),
            thought,
            action,
            observation,
            score,
            visits: 0,
            children: Vec::new(),
            depth: parent_depth + 1,
        };

        self.nodes.insert(node_id, node);

        // Update parent's children list
        self.nodes
            .get_mut(&parent_id)
            .ok_or(SearchTreeError::NodeNotFound(parent_id))?
            .children
            .push(node_id);

        Ok(node_id)
    }

    /// Get a reference to a node
    pub fn get_node(&self, id: NodeId) -> Result<&SearchNode, SearchTreeError> {
        self.nodes
            .get(&id)
            .ok_or(SearchTreeError::NodeNotFound(id))
    }

    /// Get a mutable reference to a node
    pub fn get_node_mut(&mut self, id: NodeId) -> Result<&mut SearchNode, SearchTreeError> {
        self.nodes
            .get_mut(&id)
            .ok_or(SearchTreeError::NodeNotFound(id))
    }

    /// Get the parent ID of a node
    pub fn get_parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(&id).and_then(|node| node.parent_id)
    }

    /// Update the score of a node and increment its visit count
    pub fn update_score(&mut self, id: NodeId, new_score: f32) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.score = new_score;
            node.visits += 1;
        }
    }

    /// Get all leaf nodes (nodes without children)
    pub fn get_leaf_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|node| node.children.is_empty())
            .map(|node| node.id)
            .collect()
    }

    /// Calculate UCT (Upper Confidence Bound for Trees) score for node selection
    ///
    /// UCT formula: Q(s,a) + c * sqrt(ln(N(s)) / N(s,a))
    /// where:
    /// - Q(s,a) is the node's current score
    /// - c is the exploration weight (typically sqrt(2) ≈ 1.414)
    /// - N(s) is the parent's visit count
    /// - N(s,a) is the node's visit count
    ///
    /// Unvisited nodes return f32::INFINITY to ensure they are explored first.
    pub fn uct_score(&self, node_id: NodeId, exploration_weight: f32) -> f32 {
        let node = match self.nodes.get(&node_id) {
            Some(n) => n,
            None => return f32::NEG_INFINITY,
        };

        // Unvisited nodes get highest priority
        if node.visits == 0 {
            return f32::INFINITY;
        }

        // Get parent visit count
        let parent_visits = node
            .parent_id
            .and_then(|pid| self.nodes.get(&pid))
            .map(|p| p.visits)
            .unwrap_or(1);

        // UCT formula: exploitation + exploration
        let exploitation = node.score;
        let exploration =
            exploration_weight * ((parent_visits as f32).ln() / node.visits as f32).sqrt();

        exploitation + exploration
    }

    /// Extract the best path from root to the highest-scoring leaf node
    pub fn get_best_path(&self) -> Vec<NodeId> {
        // Start from root
        let mut path = vec![self.root_id];
        let mut current_id = self.root_id;

        // Traverse down, always picking the highest-scoring child
        loop {
            let current = match self.nodes.get(&current_id) {
                Some(n) => n,
                None => break,
            };

            if current.children.is_empty() {
                // Reached a leaf node
                break;
            }

            // Find child with highest score (using exploitation only, not UCT)
            let best_child = current
                .children
                .iter()
                .filter_map(|&child_id| {
                    self.nodes.get(&child_id).map(|child| (child_id, child))
                })
                .max_by(|(_, a), (_, b)| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, _)| id);

            match best_child {
                Some(child_id) => {
                    path.push(child_id);
                    current_id = child_id;
                }
                None => break,
            }
        }

        path
    }

    /// Get the root node ID
    pub fn root_id(&self) -> NodeId {
        self.root_id
    }

    /// Get the total number of nodes in the tree
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tree() {
        let root = SearchNode::new_root("Initial thought".to_string());
        let tree = SearchTree::new(root);

        assert_eq!(tree.root_id(), 0);
        assert_eq!(tree.node_count(), 1);
        assert!(tree.get_node(0).is_ok());
    }

    #[test]
    fn test_add_node() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let action = ToolAction {
            tool_name: "test_tool".to_string(),
            parameters: serde_json::json!({"param": "value"}),
            reasoning: "Test reasoning".to_string(),
        };

        let child_id = tree
            .add_node(
                0,
                "Child thought".to_string(),
                Some(action),
                Some(serde_json::json!({"result": "success"})),
                0.5,
            )
            .unwrap();

        assert_eq!(child_id, 1);
        assert_eq!(tree.node_count(), 2);

        let child = tree.get_node(child_id).unwrap();
        assert_eq!(child.parent_id, Some(0));
        assert_eq!(child.depth, 1);
        assert_eq!(child.score, 0.5);
    }

    #[test]
    fn test_add_node_invalid_parent() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let result = tree.add_node(
            999, // Non-existent parent
            "Child".to_string(),
            None,
            None,
            0.0,
        );

        assert!(result.is_err());
        assert!(matches!(result, Err(SearchTreeError::NodeNotFound(999))));
    }

    #[test]
    fn test_update_score() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        tree.update_score(0, 0.8);

        let node = tree.get_node(0).unwrap();
        assert_eq!(node.score, 0.8);
        assert_eq!(node.visits, 1);

        tree.update_score(0, 0.9);
        let node = tree.get_node(0).unwrap();
        assert_eq!(node.score, 0.9);
        assert_eq!(node.visits, 2);
    }

    #[test]
    fn test_get_leaf_nodes() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        // Root is initially a leaf
        assert_eq!(tree.get_leaf_nodes(), vec![0]);

        // Add children
        let _child1 = tree
            .add_node(0, "Child 1".to_string(), None, None, 0.5)
            .unwrap();
        let _child2 = tree
            .add_node(0, "Child 2".to_string(), None, None, 0.6)
            .unwrap();

        // Now children are leaves
        let mut leaves = tree.get_leaf_nodes();
        leaves.sort();
        assert_eq!(leaves, vec![1, 2]); // child1=1, child2=2

        // Add grandchild
        let _grandchild = tree
            .add_node(1, "Grandchild".to_string(), None, None, 0.7)
            .unwrap();

        // Now child2 and grandchild are leaves
        let mut leaves = tree.get_leaf_nodes();
        leaves.sort();
        assert_eq!(leaves, vec![2, 3]); // child2 and grandchild
    }

    #[test]
    fn test_uct_score_unvisited() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let child_id = tree
            .add_node(0, "Child".to_string(), None, None, 0.5)
            .unwrap();

        // Unvisited node should return INFINITY
        assert_eq!(tree.uct_score(child_id, 1.414), f32::INFINITY);
    }

    #[test]
    fn test_uct_score_calculation() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        // Update root visits
        tree.update_score(0, 0.0);
        tree.update_score(0, 0.0);
        tree.update_score(0, 0.0); // 3 visits

        let child_id = tree
            .add_node(0, "Child".to_string(), None, None, 0.5)
            .unwrap();

        tree.update_score(child_id, 0.5); // 1 visit

        let uct = tree.uct_score(child_id, 1.414);

        // UCT = 0.5 + 1.414 * sqrt(ln(3) / 1)
        // UCT ≈ 0.5 + 1.414 * sqrt(1.0986)
        // UCT ≈ 0.5 + 1.414 * 1.048
        // UCT ≈ 1.98
        assert!((uct - 1.98).abs() < 0.1);
    }

    #[test]
    fn test_get_best_path_simple() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let _child1 = tree
            .add_node(0, "Child 1".to_string(), None, None, 0.3)
            .unwrap();
        let child2 = tree
            .add_node(0, "Child 2".to_string(), None, None, 0.7)
            .unwrap();

        let _grandchild1 = tree
            .add_node(child2, "Grandchild 1".to_string(), None, None, 0.5)
            .unwrap();
        let grandchild2 = tree
            .add_node(child2, "Grandchild 2".to_string(), None, None, 0.9)
            .unwrap();

        let best_path = tree.get_best_path();

        // Should follow: root -> child2 (0.7) -> grandchild2 (0.9)
        assert_eq!(best_path, vec![0, child2, grandchild2]);
    }

    #[test]
    fn test_get_best_path_equal_scores() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let child1 = tree
            .add_node(0, "Child 1".to_string(), None, None, 0.5)
            .unwrap();
        let child2 = tree
            .add_node(0, "Child 2".to_string(), None, None, 0.5)
            .unwrap();

        let best_path = tree.get_best_path();

        // Should pick one of the children (implementation picks the last one with max score)
        assert!(best_path == vec![0, child1] || best_path == vec![0, child2]);
    }

    #[test]
    fn test_get_parent() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        let child = tree
            .add_node(0, "Child".to_string(), None, None, 0.5)
            .unwrap();
        let grandchild = tree
            .add_node(child, "Grandchild".to_string(), None, None, 0.7)
            .unwrap();

        assert_eq!(tree.get_parent(0), None); // Root has no parent
        assert_eq!(tree.get_parent(child), Some(0));
        assert_eq!(tree.get_parent(grandchild), Some(child));
    }

    #[test]
    fn test_exploration_vs_exploitation() {
        let root = SearchNode::new_root("Root".to_string());
        let mut tree = SearchTree::new(root);

        // Parent with some visits
        tree.update_score(0, 0.0);
        tree.update_score(0, 0.0);
        tree.update_score(0, 0.0);
        tree.update_score(0, 0.0); // 4 visits

        // Child 1: high score, many visits (exploitation favored)
        let child1 = tree
            .add_node(0, "Child 1".to_string(), None, None, 0.8)
            .unwrap();
        tree.update_score(child1, 0.8);
        tree.update_score(child1, 0.8);
        tree.update_score(child1, 0.8); // 3 visits

        // Child 2: lower score, one visit (exploration should boost)
        let child2 = tree
            .add_node(0, "Child 2".to_string(), None, None, 0.4)
            .unwrap();
        tree.update_score(child2, 0.4); // 1 visit

        let uct1 = tree.uct_score(child1, 1.414);
        let uct2 = tree.uct_score(child2, 1.414);

        // Child 2 should have higher UCT due to exploration bonus
        assert!(
            uct2 > uct1,
            "Less-visited node should have higher UCT score"
        );
    }
}
