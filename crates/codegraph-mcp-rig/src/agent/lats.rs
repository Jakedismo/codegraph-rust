// ABOUTME: LATS (Language Agent Tree Search) implementation using Rig

use crate::agent::api::{AgentEvent, RigAgentTrait};
use crate::tools::GraphToolFactory;
use anyhow::Result;
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::future::join_all;
use futures::stream;
use futures::Stream;
use rig::completion::{CompletionModel, CompletionRequest, Message};
use rig::OneOrMany;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::{debug, info};

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
        let exploration = exploration_weight * ((parent_visits as f64).ln() / self.visits as f64).sqrt();
        exploitation + exploration
    }
}

/// LATS agent that explores multiple reasoning paths
pub struct LatsAgent<M: CompletionModel + Send + Sync> {
    pub(crate) model: M,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

impl<M: CompletionModel + Send + Sync> LatsAgent<M> {
    // --- Helper: Call Model ---
    async fn call_model(&self, prompt: String, system_prompt: String) -> Result<String> {
        let chat_history = vec![
            Message::User {
                content: OneOrMany::one(rig::message::UserContent::Text(prompt.into()))
            }
        ];

        let req = CompletionRequest {
            chat_history: OneOrMany::many(chat_history).expect("History not empty"),
            preamble: Some(system_prompt),
            documents: vec![],
            tools: vec![],
            temperature: Some(0.7), // Higher temp for diversity in generation
            max_tokens: Some(1024),
            additional_params: None,
            tool_choice: None,
        };

        let response = self.model.completion(req).await.map_err(|e| anyhow::anyhow!(e))?;
        
        // Extract text from AssistantContent
        // Simple debug format as fallback since we don't have direct access to internal enum
        // In real impl we would match on variants
        let text = format!("{:?}", response.choice);
        // Clean up debug formatting if it wraps in "Text(...)"
        let cleaned = text.trim_start_matches("Text(\"").trim_end_matches("\"").replace("\\n", "\n");
        Ok(cleaned)
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
            let best_child = node.children.iter()
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
    async fn expand_node(&self, leaf_id: usize, nodes: &mut HashMap<usize, SearchNode>, next_id: &mut usize, query: &str) -> Result<Vec<usize>> {
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
            let prompt = format!("Query: {}\n\nContext so far:\n{}\n\nGenerate candidate step #{} (Thought & Action or Final Answer):", query, context, i+1);
            futures.push(self.call_model(prompt, "You are a reasoning agent exploring possible solutions.".to_string()));
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
    async fn evaluate_node(&self, node_id: usize, nodes: &HashMap<usize, SearchNode>, query: &str) -> f64 {
        let node = nodes.get(&node_id).unwrap();
        let content = &node.content;
        
        // Use LLM to score the content relevance/correctness (0.0 to 1.0)
        let prompt = format!("Query: {}\n\nProposed Step:\n{}\n\nRate this step from 0 to 100 based on correctness and relevance to the query. Return ONLY the number.", query, content);
        
        match self.call_model(prompt, "You are an evaluator. Rate the reasoning quality.".to_string()).await {
            Ok(score_str) => {
                // Extract number
                let digits: String = score_str.chars().filter(|c| c.is_digit(10)).collect();
                let score = digits.parse::<f64>().unwrap_or(50.0); // Default to neutral on parse fail
                score / 100.0
            },
            Err(_) => 0.5
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
impl<M: CompletionModel + Send + Sync> RigAgentTrait for LatsAgent<M> {
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
            debug!("LATS Iteration {}/{}", i+1, iterations);
            
            // 1. Selection
            let leaf_id = self.select_leaf(&nodes);
            
            // 2. Expansion
            // Note: In real LATS, we would execute tools here if the node implies an action.
            // For this implementation, we simulate reasoning expansion.
            let new_ids = self.expand_node(leaf_id, &mut nodes, &mut next_id, query).await?;
            
            // 3. Evaluation & Backprop
            // Evaluate all new children (parallelizable)
            for child_id in new_ids {
                let score = self.evaluate_node(child_id, &nodes, query).await;
                self.backpropagate(child_id, score, &mut nodes);
            }
        }

        // Select best path
        let best_child_id = nodes.get(&0).unwrap().children.iter()
            .max_by(|&a, &b| {
                let node_a = nodes.get(a).unwrap();
                let node_b = nodes.get(b).unwrap();
                // Select by visit count (robustness)
                node_a.visits.cmp(&node_b.visits)
            });

        match best_child_id {
            Some(&id) => {
                let node = nodes.get(&id).unwrap();
                Ok(format!("[LATS Optimized Result]\n{}", node.content))
            },
            None => Ok("LATS failed to generate a solution.".to_string())
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
            Ok(AgentEvent::Thinking("LATS: Building search tree...".to_string())),
            Ok(AgentEvent::Thinking("LATS: Expanding reasoning paths...".to_string())),
            Ok(AgentEvent::Thinking("LATS: Evaluating candidates...".to_string())),
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