// ABOUTME: LATS phase-specific prompt templates
// ABOUTME: Provides structured prompts for selection, expansion, evaluation, and backpropagation phases

use super::search_tree::{SearchNode, SearchTree};
use serde_json::Value as JsonValue;

/// LATS prompt templates for each phase of the algorithm
pub struct LATSPrompts;

impl LATSPrompts {
    /// Generate prompt for selection phase (choose nodes to expand)
    ///
    /// This phase uses UCT (Upper Confidence Bound for Trees) to balance
    /// exploration and exploitation when selecting promising nodes.
    ///
    /// # Arguments
    /// * `tree` - The current search tree
    /// * `query` - The original user query
    /// * `beam_width` - Maximum number of nodes to select
    ///
    /// # Returns
    /// A prompt that asks the LLM to select the most promising nodes
    pub fn selection_prompt(tree: &SearchTree, query: &str, beam_width: usize) -> String {
        let leaf_nodes = tree.get_leaf_nodes();

        // Build node descriptions
        let mut node_descriptions = String::new();
        for node_id in &leaf_nodes {
            if let Ok(node) = tree.get_node(*node_id) {
                node_descriptions.push_str(&format!(
                    "\nNode {}: (depth={}, score={:.3}, visits={})\n  Thought: {}\n",
                    node.id, node.depth, node.score, node.visits, node.thought
                ));

                if let Some(ref action) = node.action {
                    node_descriptions.push_str(&format!("  Action: {} - {}\n",
                        action.tool_name, action.reasoning));
                }

                if let Some(ref obs) = node.observation {
                    let obs_preview = obs.to_string();
                    let preview = if obs_preview.len() > 200 {
                        format!("{}...", &obs_preview[..200])
                    } else {
                        obs_preview
                    };
                    node_descriptions.push_str(&format!("  Observation: {}\n", preview));
                }
            }
        }

        format!(
            r#"You are analyzing a search tree for the following query:
Query: "{}"

Current leaf nodes in the search tree:
{}

Your task: Select up to {} of the most promising leaf nodes to expand next.

Consider:
1. Nodes that are making progress toward answering the query
2. Nodes with unexplored potential (low visit count)
3. Nodes with high scores indicating good quality
4. Balance between exploration (trying new paths) and exploitation (following good paths)

Respond with a JSON object:
{{
  "selected_node_ids": [1, 3, 5],
  "reasoning": "Explanation of why these nodes were selected"
}}

Select between 1 and {} nodes."#,
            query,
            node_descriptions,
            beam_width,
            beam_width
        )
    }

    /// Generate prompt for expansion phase (generate new thoughts/actions)
    ///
    /// This phase generates new thought-action pairs for selected nodes,
    /// proposing the next step in the reasoning process.
    ///
    /// # Arguments
    /// * `node` - The node to expand
    /// * `query` - The original user query
    /// * `available_tools` - List of tool names available for execution
    ///
    /// # Returns
    /// A prompt that asks the LLM to generate new actions
    pub fn expansion_prompt(
        node: &SearchNode,
        query: &str,
        available_tools: &[String],
    ) -> String {
        let tools_list = available_tools.join(", ");

        // Build path context
        let mut path_context = format!("Current node (depth {}):\n", node.depth);
        path_context.push_str(&format!("  Thought: {}\n", node.thought));

        if let Some(ref action) = node.action {
            path_context.push_str(&format!(
                "  Previous action: {} - {}\n",
                action.tool_name, action.reasoning
            ));
        }

        if let Some(ref obs) = node.observation {
            let obs_str = obs.to_string();
            let preview = if obs_str.len() > 300 {
                format!("{}...", &obs_str[..300])
            } else {
                obs_str
            };
            path_context.push_str(&format!("  Observation: {}\n", preview));
        }

        format!(
            r#"You are expanding a node in a search tree to answer this query:
Query: "{}"

{}

Available tools: {}

Your task: Generate 1-3 new thought-action pairs that could help answer the query.

Each thought should:
1. Build on the current node's progress
2. Propose a specific next step
3. Use one of the available tools

Respond with a JSON object:
{{
  "actions": [
    {{
      "thought": "Next reasoning step",
      "tool_name": "tool_name",
      "parameters": {{"param": "value"}},
      "reasoning": "Why this action helps answer the query"
    }}
  ]
}}

Generate between 1 and 3 actions."#,
            query,
            path_context,
            tools_list
        )
    }

    /// Generate prompt for evaluation phase (score node quality)
    ///
    /// This phase assesses the quality of expanded nodes based on their
    /// contribution toward answering the original query.
    ///
    /// # Arguments
    /// * `node` - The node to evaluate
    /// * `observation` - The result from executing the node's action
    /// * `query` - The original user query
    ///
    /// # Returns
    /// A prompt that asks the LLM to score the node
    pub fn evaluation_prompt(node: &SearchNode, observation: &JsonValue, query: &str) -> String {
        let obs_str = serde_json::to_string_pretty(observation).unwrap_or_else(|_| "{}".to_string());

        let action_desc = if let Some(ref action) = node.action {
            format!("Tool: {} - {}", action.tool_name, action.reasoning)
        } else {
            "No action".to_string()
        };

        format!(
            r#"You are evaluating a node in a search tree for this query:
Query: "{}"

Node to evaluate:
  Thought: {}
  Action: {}
  Observation: {}

Your task: Evaluate how well this node contributes to answering the query.

Scoring criteria:
- 0.0-0.3: Not helpful, wrong direction, or irrelevant
- 0.3-0.5: Somewhat helpful, partial progress
- 0.5-0.7: Good progress, relevant information
- 0.7-0.9: Very helpful, substantial progress
- 0.9-1.0: Excellent, likely contains answer or key insight

Also determine if this node represents a complete solution to the query.

Respond with a JSON object:
{{
  "score": 0.75,
  "reasoning": "Explanation of the score",
  "is_solution": false
}}

Provide a score between 0.0 and 1.0."#,
            query,
            node.thought,
            action_desc,
            obs_str
        )
    }

    /// Generate prompt for backpropagation phase (update scores)
    ///
    /// This phase updates ancestor node scores based on their children's
    /// performance, using UCT-style value propagation.
    ///
    /// # Arguments
    /// * `node` - The node whose score should be updated
    /// * `child_scores` - Scores of the node's children
    ///
    /// # Returns
    /// A prompt that asks the LLM to compute updated score
    pub fn backprop_prompt(node: &SearchNode, child_scores: &[f32]) -> String {
        let avg_child_score = if child_scores.is_empty() {
            0.0
        } else {
            child_scores.iter().sum::<f32>() / child_scores.len() as f32
        };

        let max_child_score = child_scores
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        format!(
            r#"You are updating a node's score in a search tree.

Current node:
  Thought: {}
  Current score: {:.3}
  Visits: {}

Children statistics:
  Number of children: {}
  Average child score: {:.3}
  Maximum child score: {:.3}
  Child scores: {:?}

Your task: Compute an updated score for this node based on its children's performance.

The new score should:
1. Reflect the quality of the best path through this node
2. Consider both the maximum and average child scores
3. Be in the range [0.0, 1.0]

A common approach is to use the maximum child score with some smoothing:
new_score = 0.7 * max_child_score + 0.3 * avg_child_score

Respond with a JSON object:
{{
  "new_score": 0.75,
  "reasoning": "Explanation of how the score was computed"
}}

Provide a score between 0.0 and 1.0."#,
            node.thought,
            node.score,
            node.visits,
            child_scores.len(),
            avg_child_score,
            max_child_score,
            child_scores
        )
    }

    /// Generate prompt for final answer synthesis
    ///
    /// This combines the best path through the search tree into a coherent answer.
    ///
    /// # Arguments
    /// * `path_nodes` - Nodes along the best path
    /// * `query` - The original user query
    ///
    /// # Returns
    /// A prompt that asks the LLM to synthesize the final answer
    pub fn synthesis_prompt(path_nodes: &[&SearchNode], query: &str) -> String {
        let mut path_summary = String::new();
        for (i, node) in path_nodes.iter().enumerate() {
            path_summary.push_str(&format!("\nStep {}:\n", i + 1));
            path_summary.push_str(&format!("  Thought: {}\n", node.thought));

            if let Some(ref action) = node.action {
                path_summary.push_str(&format!(
                    "  Action: {} - {}\n",
                    action.tool_name, action.reasoning
                ));
            }

            if let Some(ref obs) = node.observation {
                let obs_str = obs.to_string();
                let preview = if obs_str.len() > 500 {
                    format!("{}...", &obs_str[..500])
                } else {
                    obs_str
                };
                path_summary.push_str(&format!("  Result: {}\n", preview));
            }
        }

        format!(
            r#"You are synthesizing the final answer from a search tree exploration.

Original query: "{}"

Best path through the search tree:
{}

Your task: Synthesize a comprehensive answer to the query based on this path.

The answer should:
1. Directly address the original query
2. Incorporate key findings from each step
3. Be clear, concise, and well-organized
4. Highlight the most important insights

Respond with a JSON object:
{{
  "answer": "Comprehensive answer to the query",
  "findings": "Key findings and insights",
  "steps_taken": "{}"
}}

Provide a complete, professional response."#,
            query,
            path_summary,
            path_nodes.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autoagents::lats::search_tree::{SearchNode, SearchTree, ToolAction};

    #[test]
    fn test_selection_prompt() {
        let root = SearchNode::new_root("Initial analysis".to_string());
        let tree = SearchTree::new(root);

        let prompt = LATSPrompts::selection_prompt(&tree, "Test query", 3);

        assert!(prompt.contains("Test query"));
        assert!(prompt.contains("Select up to 3"));
        assert!(prompt.contains("JSON object"));
    }

    #[test]
    fn test_expansion_prompt() {
        let node = SearchNode::new_root("Test thought".to_string());
        let tools = vec!["tool1".to_string(), "tool2".to_string()];

        let prompt = LATSPrompts::expansion_prompt(&node, "Test query", &tools);

        assert!(prompt.contains("Test query"));
        assert!(prompt.contains("Test thought"));
        assert!(prompt.contains("tool1, tool2"));
        assert!(prompt.contains("1-3 new thought-action pairs"));
    }

    #[test]
    fn test_evaluation_prompt() {
        let mut node = SearchNode::new_root("Test thought".to_string());
        node.action = Some(ToolAction {
            tool_name: "test_tool".to_string(),
            parameters: serde_json::json!({"param": "value"}),
            reasoning: "Test reasoning".to_string(),
        });

        let observation = serde_json::json!({"result": "success"});
        let prompt = LATSPrompts::evaluation_prompt(&node, &observation, "Test query");

        assert!(prompt.contains("Test query"));
        assert!(prompt.contains("test_tool"));
        assert!(prompt.contains("score"));
        assert!(prompt.contains("is_solution"));
    }

    #[test]
    fn test_backprop_prompt() {
        let node = SearchNode::new_root("Test thought".to_string());
        let child_scores = vec![0.5, 0.7, 0.6];

        let prompt = LATSPrompts::backprop_prompt(&node, &child_scores);

        assert!(prompt.contains("Test thought"));
        assert!(prompt.contains("0.5"));
        assert!(prompt.contains("new_score"));
    }

    #[test]
    fn test_synthesis_prompt() {
        let node1 = SearchNode::new_root("First thought".to_string());
        let mut node2 = SearchNode::new_root("Second thought".to_string());
        node2.action = Some(ToolAction {
            tool_name: "test_tool".to_string(),
            parameters: serde_json::json!({}),
            reasoning: "Test".to_string(),
        });

        let nodes = vec![&node1, &node2];
        let prompt = LATSPrompts::synthesis_prompt(&nodes, "Test query");

        assert!(prompt.contains("Test query"));
        assert!(prompt.contains("First thought"));
        assert!(prompt.contains("Second thought"));
        assert!(prompt.contains("answer"));
        assert!(prompt.contains("findings"));
    }
}
