// ABOUTME: LATS tree search with graph-grounded candidate tool loops and branch transcripts

use crate::agent::api::{AgentEvent, RigAgentTrait};
use crate::tools::GraphToolFactory;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::Stream;
use futures::future::join_all;
use futures::stream;
use rig::completion::CompletionRequest;
use rig::message::{Message, UserContent};
use rig::{DynModel, operation::Completion};
use serde_json::Value;
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
    history: Vec<Message>,
    grounded: bool,
    terminal: bool,
}

impl SearchNode {
    fn new(parent_id: Option<usize>, content: String, depth: usize, history: Vec<Message>) -> Self {
        let grounded = has_graph_observation(&history);
        let terminal = grounded && final_answer(&content).is_some();
        Self {
            parent_id,
            content,
            children: Vec::new(),
            visits: 0,
            value_sum: 0.0,
            depth,
            history,
            grounded,
            terminal,
        }
    }

    fn uct_score(&self, parent_visits: usize, exploration_weight: f64) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY; // Explore unvisited nodes first
        }
        let exploitation = self.value_sum / self.visits as f64;
        let exploration =
            exploration_weight * ((parent_visits.max(1) as f64).ln() / self.visits as f64).sqrt();
        exploitation + exploration
    }
}

/// LATS agent that explores multiple reasoning paths
pub struct LatsAgent {
    model: DynModel<Completion>,
    agent: rig_agent::Agent,
    factory: GraphToolFactory,
    max_turns: usize,
    tier: ContextTier,
    max_output_tokens: u64,
    system_prompt: String,
}

/// System prompt for generating one candidate step of the search tree.
///
/// Candidates execute native tool calls through the same counted graph tools as ReAct.
const EXPANSION_SYSTEM_PROMPT: &str = "\
You are one step in a search over possible answers to a question about a codebase. \
You are given the question and the reasoning so far, and you write the single next step.

Use the graph tools to establish facts missing from this branch's conversation. \
Before answering, obtain graph evidence unless earlier tool observations already \
support the answer. Do not invent file paths, line numbers, symbols or tool results. \
Empty graph results are evidence of an unsuccessful lookup, not proof that something \
cannot exist. State coverage limits, truncation and failed lookups.

If the reasoning so far is enough to answer the question, write the final answer and \
begin it with \"Final answer:\". Otherwise write the next step: what to establish next, \
why it moves toward an answer, and what is still unknown. Several candidates are generated \
for the same position, so take a different angle when the candidate number is above 1. \
For an intermediate step, write a short paragraph of plain prose. For a final \
answer, follow the analysis agent's answer format after the \"Final answer:\" prefix.";

/// System prompt for scoring a candidate step. The caller keeps only the digits
/// of the reply, so the reply must be a bare integer.
const EVALUATION_SYSTEM_PROMPT: &str = "\
You score one proposed step toward answering a question about a codebase.

You receive this branch's complete conversation, including actual graph-tool \
observations. Check the proposed answer against those observations. Candidate text \
is not evidence by itself. Score from 0 to 100. A high score means the step addresses \
the question asked and makes only claims supported by that branch. Score low when it drifts \
from the question, repeats earlier reasoning without adding to it, or states specifics \
such as file paths, line numbers, or tool results that nothing in the step supports.

Reply with a single integer between 0 and 100 and nothing else: no words, no punctuation, \
no \"/100\".";

/// Score given to a candidate when the evaluator produced nothing usable.
const NEUTRAL_SCORE: f64 = 0.5;

fn final_answer(content: &str) -> Option<&str> {
    content
        .trim()
        .strip_prefix("Final answer:")
        .map(str::trim)
        .filter(|answer| !answer.is_empty())
}

/// Only actual successful graph observations count; prose and exhausted-budget notes do not.
fn has_graph_observation(history: &[Message]) -> bool {
    let names = codegraph_mcp_tools::GraphToolExecutor::get_tool_names();
    history.iter().any(|message| {
        let Message::User { content } = message else {
            return false;
        };
        content.iter().any(|content| {
            let UserContent::ToolResult(tool) = content else {
                return false;
            };
            names.iter().any(|name| name == tool.name.as_str())
                && tool.content.iter().any(|item| {
                    item.deserialize_json::<Value>().is_ok_and(|value| {
                        value["tool"].as_str() == Some(tool.name.as_str())
                            && value.get("result").is_some()
                            && value["_budget"]["exhausted"].as_bool() != Some(true)
                    })
                })
        })
    })
}

fn is_expandable(id: usize, nodes: &HashMap<usize, SearchNode>, max_depth: usize) -> bool {
    let node = &nodes[&id];
    if node.terminal || node.depth >= max_depth {
        return false;
    }
    node.children.is_empty()
        || node
            .children
            .iter()
            .any(|child| is_expandable(*child, nodes, max_depth))
}

fn select_leaf(nodes: &HashMap<usize, SearchNode>, max_depth: usize) -> Option<usize> {
    let mut current_id = 0;
    loop {
        if !is_expandable(current_id, nodes, max_depth) {
            return None;
        }
        let node = &nodes[&current_id];
        if node.children.is_empty() {
            return Some(current_id);
        }
        current_id = *node
            .children
            .iter()
            .filter(|id| is_expandable(**id, nodes, max_depth))
            .max_by(|a, b| {
                nodes[a]
                    .uct_score(node.visits, std::f64::consts::SQRT_2)
                    .total_cmp(&nodes[b].uct_score(node.visits, std::f64::consts::SQRT_2))
                    .then_with(|| b.cmp(a))
            })?;
    }
}

/// Prefer a grounded final answer, then the best grounded leaf, not the root's first step.
fn best_node(nodes: &HashMap<usize, SearchNode>) -> Option<usize> {
    nodes
        .iter()
        .filter(|(id, node)| {
            **id != 0 && node.grounded && (node.terminal || node.children.is_empty())
        })
        .max_by(|(a_id, a), (b_id, b)| {
            a.terminal
                .cmp(&b.terminal)
                .then_with(|| {
                    (a.value_sum / a.visits.max(1) as f64)
                        .total_cmp(&(b.value_sum / b.visits.max(1) as f64))
                })
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| b_id.cmp(a_id))
        })
        .map(|(id, _)| *id)
}

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
    pub(crate) fn new(
        model: DynModel<Completion>,
        factory: GraphToolFactory,
        max_turns: usize,
        tier: ContextTier,
        max_output_tokens: u64,
        system_prompt: String,
    ) -> Self {
        let agent = factory
            .agent_builder(model.clone())
            .preamble(format!(
                "{system_prompt}\n\n# LATS candidate\n{EXPANSION_SYSTEM_PROMPT}"
            ))
            .max_tokens(max_output_tokens)
            .build();
        Self {
            model,
            agent,
            factory,
            max_turns,
            tier,
            max_output_tokens,
            system_prompt,
        }
    }

    // --- Helper: Call Model ---
    async fn call_model(&self, prompt: String, system_prompt: String) -> Result<String> {
        // No temperature: reasoning models reject the parameter. The output cap
        // follows the tier because reasoning tokens count against it, and a
        // tight cap can use up the budget before any visible text is produced.
        let request = CompletionRequest::new(prompt)
            .preamble(system_prompt)
            .max_tokens(self.max_output_tokens);
        let response = self.model.call(request).await?;
        Ok(response.text())
    }

    // --- MCTS Steps ---

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

        let history = &leaf.history;

        // Generate candidates (parallel)
        let n_candidates = 3;
        let mut futures = vec![];

        for i in 0..n_candidates {
            let prompt = format!(
                "Question:\n{}\n\nWrite candidate next step #{} using only this branch's history and graph-tool observations.",
                query,
                i + 1
            );
            futures.push(async move {
                let response = self
                    .agent
                    .prompt(prompt)
                    .history(history.clone())
                    .max_turns(self.max_turns)
                    .await?;
                let mut branch_history = history.clone();
                branch_history.extend(response.messages.unwrap_or_default());
                Ok::<_, anyhow::Error>((response.output, branch_history))
            });
        }

        let results = join_all(futures).await;

        let mut new_child_ids = vec![];
        let mut first_error = None;
        for res in results {
            match res {
                Ok((content, history)) => {
                    let id = *next_id;
                    *next_id += 1;
                    let child = SearchNode::new(Some(leaf_id), content, depth + 1, history);
                    nodes.insert(id, child);
                    new_child_ids.push(id);
                }
                Err(error) => {
                    warn!(%error, "LATS candidate tool loop failed");
                    first_error.get_or_insert(error);
                }
            }
        }
        if new_child_ids.is_empty() {
            return Err(first_error
                .unwrap_or_else(|| anyhow!("No LATS candidates were generated"))
                .context("All LATS candidate tool loops failed"));
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
        if !node.grounded {
            return 0.0;
        }
        let content = &node.content;

        // Use LLM to score the content relevance/correctness (0.0 to 1.0)
        let prompt = format!(
            "Question:\n{}\n\nBranch conversation and graph observations:\n{}\n\nProposed step:\n{}",
            query,
            serde_json::to_string(&node.history).expect("Rig messages are serializable"),
            content
        );

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
        if self.max_turns == 0 {
            return Err(anyhow!(
                "LATS requires a positive search and tool-round budget"
            ));
        }
        let root = SearchNode::new(None, format!("Start Query: {}", query), 0, vec![]);
        nodes.insert(0, root);
        let mut next_id = 1;

        // MCTS Loop
        let iterations = self.max_turns;

        for i in 0..iterations {
            debug!("LATS Iteration {}/{}", i + 1, iterations);

            // 1. Selection
            let Some(leaf_id) = select_leaf(&nodes, self.max_turns) else {
                break;
            };

            // 2. Expansion
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

        let id = best_node(&nodes)
            .ok_or_else(|| anyhow!("LATS did not obtain a successful graph-tool observation"))?;
        let node = &nodes[&id];
        if let Some(answer) = final_answer(&node.content) {
            return Ok(answer.to_string());
        }

        // Turn the selected evidence-bearing branch into an answer if the search budget
        // ended on an intermediate step. The same tool registry and byte budget apply.
        let response = self
            .agent
            .prompt(format!(
                "Answer the original question now from this branch's graph observations: {query}. \
                 State any missing evidence. Return the answer, not another candidate step."
            ))
            .preamble(&self.system_prompt)
            .history(node.history.clone())
            .max_turns(self.max_turns)
            .await?;
        Ok(final_answer(&response.output)
            .unwrap_or(&response.output)
            .to_string())
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
#[path = "lats_tests.rs"]
mod tests;
