// ABOUTME: LATS (Language Agent Tree Search) implementation using Rig

use crate::agent::api::{AgentEvent, RigAgentTrait};
use crate::tools::GraphToolFactory;
use anyhow::Result;
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::Stream;
use futures::future::join_all;
use futures::stream;
use rig::completion::CompletionRequest;
use rig::{DynModel, operation::Completion};
use std::collections::HashMap;
use std::pin::Pin;
use tracing::{debug, info, warn};

// --- MCTS Data Structures ---

#[derive(Debug, Clone)]
struct SearchNode {
    parent_id: Option<usize>,
    content: String, // The thought/action/response at this step
    children: Vec<usize>,
    visits: usize,
    value_sum: f64,
    depth: usize,
}

impl SearchNode {
    fn new(parent_id: Option<usize>, content: String, depth: usize) -> Self {
        Self {
            parent_id,
            content,
            children: Vec::new(),
            visits: 0,
            value_sum: 0.0,
            depth,
        }
    }

    fn uct_score(&self, parent_visits: usize, exploration_weight: f64) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY; // Explore unvisited nodes first
        }
        let exploitation = self.value_sum / self.visits as f64;
        let exploration =
            exploration_weight * ((parent_visits as f64).ln() / self.visits as f64).sqrt();
        exploitation + exploration
    }
}

/// LATS agent that explores multiple reasoning paths
pub struct LatsAgent {
    pub(crate) model: DynModel<Completion>,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

/// System prompt for generating one candidate step of the search tree.
///
/// Expansion calls the model without tools, so the prompt says so: a candidate
/// that claims to have run a tool or cites code it has not seen is invented.
const EXPANSION_SYSTEM_PROMPT: &str = "\
You are one step in a search over possible answers to a question about a codebase. \
You are given the question and the reasoning so far, and you write the single next step.

You have no tools in this step and cannot look at the code. Work only from the question \
and the reasoning so far. Do not claim to have run a search or a tool, and do not invent \
file paths, line numbers, or symbol names that are not already in the text you were given.

If the reasoning so far is enough to answer the question, write the final answer and \
begin it with \"Final answer:\". Otherwise write the next step: what to establish next, \
why it moves toward an answer, and what is still unknown. Several candidates are generated \
for the same position, so take a different angle when the candidate number is above 1. \
Write a short paragraph of plain prose.";

/// System prompt for scoring a candidate step. The caller keeps only the digits
/// of the reply, so the reply must be a bare integer.
const EVALUATION_SYSTEM_PROMPT: &str = "\
You score one proposed step toward answering a question about a codebase.

Score from 0 to 100. A high score means the step addresses the question asked, follows \
from what is known, and makes only claims it can support. Score low when the step drifts \
from the question, repeats earlier reasoning without adding to it, or states specifics \
such as file paths, line numbers, or tool results that nothing in the step supports.

Reply with a single integer between 0 and 100 and nothing else: no words, no punctuation, \
no \"/100\".";

/// Score given to a candidate when the evaluator produced nothing usable.
const NEUTRAL_SCORE: f64 = 0.5;

/// Read the evaluator's reply as a score in `[0.0, 1.0]`.
///
/// Takes the first integer in the reply, so "85/100" reads as 85, and returns
/// `None` when the reply holds no integer or one above 100.
fn parse_score(reply: &str) -> Option<f64> {
    let digits: String = reply
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let score = digits.parse::<u32>().ok().filter(|s| *s <= 100)?;
    Some(f64::from(score) / 100.0)
}

impl LatsAgent {
    // --- Helper: Call Model ---
    async fn call_model(&self, prompt: String, system_prompt: String) -> Result<String> {
        // No temperature: reasoning models reject the parameter. The output cap
        // follows the tier because reasoning tokens count against it, and a
        // tight cap can use up the budget before any visible text is produced.
        let request = CompletionRequest::new(prompt)
            .preamble(system_prompt)
            .max_tokens(self.tier.max_output_tokens());
        let response = self.model.call(request).await?;
        Ok(response.text())
    }

    // --- MCTS Steps ---

    // 1. Selection
    fn select_leaf(&self, nodes: &HashMap<usize, SearchNode>) -> usize {
        let mut current_id = 0; // Start at root

        loop {
            let node = nodes.get(&current_id).expect("Node missing");
            if node.children.is_empty() {
                return current_id; // Leaf found
            }

            // Select child with highest UCT
            let parent_visits = node.visits;
            let best_child = node
                .children
                .iter()
                .max_by(|&a, &b| {
                    let node_a = nodes.get(a).unwrap();
                    let node_b = nodes.get(b).unwrap();
                    let uct_a = node_a.uct_score(parent_visits, 1.41);
                    let uct_b = node_b.uct_score(parent_visits, 1.41);
                    uct_a.partial_cmp(&uct_b).unwrap()
                })
                .unwrap();

            current_id = *best_child;
        }
    }

    // 2. Expansion
    async fn expand_node(
        &self,
        leaf_id: usize,
        nodes: &mut HashMap<usize, SearchNode>,
        next_id: &mut usize,
        query: &str,
    ) -> Result<Vec<usize>> {
        let leaf = nodes.get(&leaf_id).unwrap();
        let depth = leaf.depth;

        if depth >= self.max_turns {
            return Ok(vec![]); // Max depth reached
        }

        let context = &leaf.content; // In real impl, trace back to root to build full context

        // Generate candidates (parallel)
        let n_candidates = 3;
        let mut futures = vec![];

        for i in 0..n_candidates {
            let prompt = format!(
                "Question:\n{}\n\nReasoning so far:\n{}\n\nWrite candidate next step #{}.",
                query,
                context,
                i + 1
            );
            futures.push(self.call_model(prompt, EXPANSION_SYSTEM_PROMPT.to_string()));
        }

        let results = join_all(futures).await;

        let mut new_child_ids = vec![];
        for res in results {
            if let Ok(content) = res {
                let id = *next_id;
                *next_id += 1;
                let child = SearchNode::new(Some(leaf_id), content, depth + 1);
                nodes.insert(id, child);
                new_child_ids.push(id);
            }
        }

        // Link to parent
        if let Some(leaf_mut) = nodes.get_mut(&leaf_id) {
            leaf_mut.children.extend(new_child_ids.clone());
        }

        Ok(new_child_ids)
    }

    // 3. Evaluation
    async fn evaluate_node(
        &self,
        node_id: usize,
        nodes: &HashMap<usize, SearchNode>,
        query: &str,
    ) -> f64 {
        let node = nodes.get(&node_id).unwrap();
        let content = &node.content;

        // Use LLM to score the content relevance/correctness (0.0 to 1.0)
        let prompt = format!("Question:\n{}\n\nProposed step:\n{}", query, content);

        match self
            .call_model(prompt, EVALUATION_SYSTEM_PROMPT.to_string())
            .await
        {
            Ok(score_str) => parse_score(&score_str).unwrap_or_else(|| {
                warn!(reply = %score_str, "LATS evaluator returned no score; using neutral");
                NEUTRAL_SCORE
            }),
            Err(e) => {
                warn!(error = %e, "LATS evaluator call failed; using neutral score");
                NEUTRAL_SCORE
            }
        }
    }

    // 4. Backpropagation
    fn backpropagate(&self, leaf_id: usize, score: f64, nodes: &mut HashMap<usize, SearchNode>) {
        let mut curr_id = Some(leaf_id);
        while let Some(id) = curr_id {
            if let Some(node) = nodes.get_mut(&id) {
                node.visits += 1;
                node.value_sum += score;
                curr_id = node.parent_id;
            } else {
                break;
            }
        }
    }
}

#[async_trait]
impl RigAgentTrait for LatsAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        info!("Starting LATS execution for query: {}", query);

        // Initialize Tree
        let mut nodes = HashMap::new();
        let root = SearchNode::new(None, format!("Start Query: {}", query), 0);
        nodes.insert(0, root);
        let mut next_id = 1;

        // MCTS Loop
        let iterations = 5; // Configurable?

        for i in 0..iterations {
            debug!("LATS Iteration {}/{}", i + 1, iterations);

            // 1. Selection
            let leaf_id = self.select_leaf(&nodes);

            // 2. Expansion
            // Note: In real LATS, we would execute tools here if the node implies an action.
            // For this implementation, we simulate reasoning expansion.
            let new_ids = self
                .expand_node(leaf_id, &mut nodes, &mut next_id, query)
                .await?;

            // 3. Evaluation & Backprop
            // Evaluate all new children (parallelizable)
            for child_id in new_ids {
                let score = self.evaluate_node(child_id, &nodes, query).await;
                self.backpropagate(child_id, score, &mut nodes);
            }
        }

        // Select best path
        let best_child_id = nodes.get(&0).unwrap().children.iter().max_by(|&a, &b| {
            let node_a = nodes.get(a).unwrap();
            let node_b = nodes.get(b).unwrap();
            // Select by visit count (robustness)
            node_a.visits.cmp(&node_b.visits)
        });

        match best_child_id {
            Some(&id) => {
                let node = nodes.get(&id).unwrap();
                Ok(format!("[LATS Optimized Result]\n{}", node.content))
            }
            None => Ok("LATS failed to generate a solution.".to_string()),
        }
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        // LATS is inherently iterative and non-linear, hard to stream linearly.
        // We will stream status updates.
        let response = self.execute(query).await?;

        let events = vec![
            Ok(AgentEvent::Thinking(
                "LATS: Building search tree...".to_string(),
            )),
            Ok(AgentEvent::Thinking(
                "LATS: Expanding reasoning paths...".to_string(),
            )),
            Ok(AgentEvent::Thinking(
                "LATS: Evaluating candidates...".to_string(),
            )),
            Ok(AgentEvent::OutputChunk(response)),
            Ok(AgentEvent::Done),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }

    fn take_tool_call_count(&self) -> usize {
        self.factory.take_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.factory.take_traces()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_score() {
        assert_eq!(parse_score("85"), Some(0.85));
        assert_eq!(parse_score(" 0\n"), Some(0.0));
        assert_eq!(parse_score("Score: 85/100"), Some(0.85));
        assert_eq!(parse_score("no score"), None);
        assert_eq!(parse_score(""), None);
        assert_eq!(parse_score("250"), None);
    }
}
