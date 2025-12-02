// ABOUTME: LATS executor implementing Language Agent Tree Search algorithm
// ABOUTME: Orchestrates 4-phase search: selection, expansion, evaluation, backpropagation

use super::provider_router::ProviderRouter;
use super::search_tree::{NodeId, SearchNode, SearchTree, ToolAction};
use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::autoagents::executor::ExecutorError;
use crate::autoagents::executor_trait::AgentExecutorTrait;
use async_trait::async_trait;
use codegraph_ai::llm_provider::LLMProvider;
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::config_manager::CodeGraphConfig;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_tools::GraphToolExecutor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Default per-iteration timeout in seconds
const DEFAULT_ITERATION_TIMEOUT_SECS: u64 = 60;

/// High score threshold for early termination
const HIGH_SCORE_THRESHOLD: f32 = 0.9;

/// LATS algorithm configuration
#[derive(Debug, Clone)]
pub struct LATSConfig {
    /// Beam width: maximum nodes to expand per iteration
    pub beam_width: usize,
    /// Maximum tree depth
    pub max_depth: usize,
    /// UCT exploration weight (typically sqrt(2) ≈ 1.414)
    pub exploration_weight: f32,
    /// Context tier for this executor
    pub tier: ContextTier,
    /// Per-iteration timeout in seconds (0 = unlimited)
    pub iteration_timeout_secs: u64,
    /// Maximum tree nodes before termination (0 = auto-calculate from beam_width * max_depth * 2)
    pub max_tree_nodes: usize,
}

impl Default for LATSConfig {
    fn default() -> Self {
        Self {
            beam_width: 3,
            max_depth: 5,
            exploration_weight: 1.414, // sqrt(2)
            tier: ContextTier::Medium,
            iteration_timeout_secs: DEFAULT_ITERATION_TIMEOUT_SECS,
            max_tree_nodes: 0, // Auto-calculate
        }
    }
}

impl LATSConfig {
    /// Create LATS config from CodeGraph config and tier
    pub fn from_codegraph_config(_config: &CodeGraphConfig, tier: ContextTier) -> Self {
        // Read configuration from environment variables
        let iteration_timeout_secs = std::env::var("CODEGRAPH_LATS_ITERATION_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_ITERATION_TIMEOUT_SECS);

        let max_tree_nodes = std::env::var("CODEGRAPH_AGENT_MAX_TREE_NODES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Self {
            tier,
            iteration_timeout_secs,
            max_tree_nodes,
            ..Default::default()
        }
    }

    /// Calculate effective max tree nodes (auto-calculate if 0)
    pub fn effective_max_tree_nodes(&self) -> usize {
        if self.max_tree_nodes == 0 {
            self.beam_width * self.max_depth * 2
        } else {
            self.max_tree_nodes
        }
    }

    /// Get iteration timeout as Duration (returns very large if 0)
    pub fn iteration_timeout(&self) -> Duration {
        if self.iteration_timeout_secs == 0 {
            Duration::from_secs(u64::MAX / 2)
        } else {
            Duration::from_secs(self.iteration_timeout_secs)
        }
    }
}

/// Expanded node with action results
#[derive(Debug, Clone)]
struct ExpandedNode {
    parent_id: NodeId,
    thought: String,
    action: ToolAction,
    observation: serde_json::Value,
}

/// Evaluated node with quality score
#[derive(Debug, Clone)]
struct EvaluatedNode {
    node: ExpandedNode,
    score: f32,
    reasoning: String,
    is_solution: bool,
}

/// Selection phase output from LLM
#[derive(Debug, Serialize, Deserialize)]
struct SelectionOutput {
    selected_node_ids: Vec<NodeId>,
    reasoning: String,
}

/// Expansion phase output from LLM
#[derive(Debug, Serialize, Deserialize)]
struct ExpansionOutput {
    actions: Vec<ExpansionAction>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExpansionAction {
    thought: String,
    tool_name: String,
    parameters: serde_json::Value,
    reasoning: String,
}

/// Evaluation phase output from LLM
#[derive(Debug, Serialize, Deserialize)]
struct EvaluationOutput {
    score: f32,
    reasoning: String,
    is_solution: bool,
}

/// Backpropagation phase output from LLM
#[derive(Debug, Serialize, Deserialize)]
struct BackpropUpdate {
    new_score: f32,
    reasoning: String,
}

/// Synthesis phase output from LLM
#[derive(Debug, Serialize, Deserialize)]
struct SynthesisOutput {
    answer: String,
    findings: String,
    steps_taken: String,
}

/// LATS executor implementing Language Agent Tree Search
///
/// This executor uses a 4-phase iterative algorithm:
/// 1. Selection: Choose promising nodes using UCT
/// 2. Expansion: Generate new thought-action pairs
/// 3. Evaluation: Score the quality of expanded nodes
/// 4. Backpropagation: Update ancestor scores
///
/// Phase 2 implementation: Skeleton with placeholder logic.
/// Full implementation in Phase 3.
pub struct LATSExecutor {
    config: LATSConfig,
    provider_router: ProviderRouter,
    tool_executor: Arc<GraphToolExecutor>,
}

impl LATSExecutor {
    /// Create a new LATS executor
    ///
    /// # Arguments
    /// * `config` - CodeGraph configuration
    /// * `default_provider` - Default LLM provider (for Phase 2, used for all phases)
    /// * `tool_executor` - Graph tool executor for running analysis tools
    /// * `tier` - Context tier for this executor
    pub fn new(
        config: Arc<CodeGraphConfig>,
        default_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
    ) -> Self {
        let lats_config = LATSConfig::from_codegraph_config(&config, tier);
        let provider_router = ProviderRouter::new(&config, default_provider);

        info!(
            target: "lats::executor",
            beam_width = lats_config.beam_width,
            max_depth = lats_config.max_depth,
            exploration_weight = lats_config.exploration_weight,
            tier = ?lats_config.tier,
            "LATS executor initialized"
        );

        Self {
            config: lats_config,
            provider_router,
            tool_executor,
        }
    }

    /// Phase 1: Node Selection using UCT algorithm
    ///
    /// Selects the most promising leaf nodes to expand based on UCT scores.
    async fn select_nodes(
        &self,
        tree: &SearchTree,
        _query: &str,
    ) -> Result<Vec<NodeId>, ExecutorError> {
        debug!(
            target: "lats::selection",
            leaf_count = tree.get_leaf_nodes().len(),
            "Starting selection phase"
        );

        // Get leaf nodes and calculate UCT scores
        let mut leaf_nodes = tree.get_leaf_nodes();

        // Sort by UCT score (highest first)
        leaf_nodes.sort_by(|a, b| {
            let uct_a = tree.uct_score(*a, self.config.exploration_weight);
            let uct_b = tree.uct_score(*b, self.config.exploration_weight);
            uct_b
                .partial_cmp(&uct_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Select top beam_width nodes
        let selected: Vec<NodeId> = leaf_nodes
            .into_iter()
            .take(self.config.beam_width)
            .collect();

        info!(
            target: "lats::selection",
            selected_count = selected.len(),
            "Selection phase completed"
        );

        Ok(selected)
    }

    /// Phase 2: Node Expansion - generate new thoughts/actions
    ///
    /// For each selected node, generates new thought-action pairs and executes them.
    async fn expand_nodes(
        &self,
        tree: &SearchTree,
        nodes: &[NodeId],
        query: &str,
    ) -> Result<Vec<ExpandedNode>, ExecutorError> {
        use super::prompts::LATSPrompts;
        use super::provider_router::LATSPhase;
        use codegraph_ai::llm_provider::{GenerationConfig, Message, MessageRole, ResponseFormat};

        debug!(
            target: "lats::expansion",
            node_count = nodes.len(),
            "Starting expansion phase"
        );

        let available_tools = vec![
            "get_transitive_dependencies".to_string(),
            "get_reverse_dependencies".to_string(),
            "trace_call_chain".to_string(),
            "detect_circular_dependencies".to_string(),
            "calculate_coupling_metrics".to_string(),
            "get_hub_nodes".to_string(),
        ];

        let mut expansions = Vec::new();

        for &node_id in nodes {
            let node = tree
                .get_node(node_id)
                .map_err(|e| ExecutorError::ExecutionFailed(e.to_string()))?;

            let provider = self.provider_router.get_provider(LATSPhase::Expansion);
            let prompt = LATSPrompts::expansion_prompt(node, query, &available_tools);

            let messages = vec![Message {
                role: MessageRole::User,
                content: prompt,
            }];

            let mut config = GenerationConfig::default();
            config.response_format = Some(ResponseFormat::JsonObject);
            config.temperature = 0.7; // Higher temperature for creativity in expansion

            let response = provider
                .generate_chat(&messages, &config)
                .await
                .map_err(|e| ExecutorError::ExecutionFailed(format!("LLM call failed: {}", e)))?;

            // Parse expansion actions (with error handling)
            let expansion: ExpansionOutput = match serde_json::from_str(&response.content) {
                Ok(exp) => exp,
                Err(e) => {
                    debug!(
                        target: "lats::expansion",
                        error = %e,
                        content = %response.content,
                        "Failed to parse expansion output, using default"
                    );
                    // Default: single no-op action
                    ExpansionOutput {
                        actions: vec![ExpansionAction {
                            thought: "Continue analysis".to_string(),
                            tool_name: "get_transitive_dependencies".to_string(),
                            parameters: serde_json::json!({"node_id": "main", "edge_type": "imports", "depth": 1}),
                            reasoning: "Fallback action due to parsing error".to_string(),
                        }],
                    }
                }
            };

            // Execute each action
            for action in expansion.actions {
                let observation = match self
                    .tool_executor
                    .execute(&action.tool_name, action.parameters.clone())
                    .await
                {
                    Ok(result) => result,
                    Err(e) => {
                        debug!(
                            target: "lats::expansion",
                            error = %e,
                            tool = %action.tool_name,
                            "Tool execution failed, using error observation"
                        );
                        serde_json::json!({"error": e.to_string()})
                    }
                };

                expansions.push(ExpandedNode {
                    parent_id: node_id,
                    thought: action.thought,
                    action: ToolAction {
                        tool_name: action.tool_name,
                        parameters: action.parameters,
                        reasoning: action.reasoning,
                    },
                    observation,
                });
            }
        }

        info!(
            target: "lats::expansion",
            expansions_count = expansions.len(),
            "Expansion phase completed"
        );

        Ok(expansions)
    }

    /// Phase 3: Node Evaluation - assess quality
    ///
    /// Evaluates each expanded node and assigns a quality score.
    async fn evaluate_nodes(
        &self,
        nodes: &[ExpandedNode],
        query: &str,
    ) -> Result<Vec<EvaluatedNode>, ExecutorError> {
        use super::prompts::LATSPrompts;
        use super::provider_router::LATSPhase;
        use codegraph_ai::llm_provider::{GenerationConfig, Message, MessageRole, ResponseFormat};

        debug!(
            target: "lats::evaluation",
            node_count = nodes.len(),
            "Starting evaluation phase"
        );

        let mut evaluated = Vec::new();

        for node in nodes {
            let provider = self.provider_router.get_provider(LATSPhase::Evaluation);

            // Create a temporary SearchNode for the prompt
            let temp_node = SearchNode {
                id: 0,
                parent_id: Some(node.parent_id),
                thought: node.thought.clone(),
                action: Some(node.action.clone()),
                observation: Some(node.observation.clone()),
                score: 0.0,
                visits: 0,
                children: Vec::new(),
                depth: 0,
            };

            let prompt = LATSPrompts::evaluation_prompt(&temp_node, &node.observation, query);

            let messages = vec![Message {
                role: MessageRole::User,
                content: prompt,
            }];

            let mut config = GenerationConfig::default();
            config.response_format = Some(ResponseFormat::JsonObject);
            config.temperature = 0.3; // Lower temperature for evaluation consistency

            let response = provider
                .generate_chat(&messages, &config)
                .await
                .map_err(|e| ExecutorError::ExecutionFailed(format!("LLM call failed: {}", e)))?;

            // Parse evaluation output (with error handling)
            let evaluation: EvaluationOutput = match serde_json::from_str(&response.content) {
                Ok(eval) => eval,
                Err(e) => {
                    debug!(
                        target: "lats::evaluation",
                        error = %e,
                        content = %response.content,
                        "Failed to parse evaluation output, using default"
                    );
                    // Default: moderate score
                    EvaluationOutput {
                        score: 0.5,
                        reasoning: "Default score due to parsing error".to_string(),
                        is_solution: false,
                    }
                }
            };

            // Clamp score to [0.0, 1.0]
            let score = evaluation.score.clamp(0.0, 1.0);

            evaluated.push(EvaluatedNode {
                node: node.clone(),
                score,
                reasoning: evaluation.reasoning,
                is_solution: evaluation.is_solution,
            });
        }

        info!(
            target: "lats::evaluation",
            evaluated_count = evaluated.len(),
            "Evaluation phase completed"
        );

        Ok(evaluated)
    }

    /// Phase 4: Backpropagation - update UCT scores
    ///
    /// Updates ancestor node scores based on children's performance.
    async fn backpropagate(
        &self,
        tree: &mut SearchTree,
        evaluated_nodes: &[EvaluatedNode],
    ) -> Result<(), ExecutorError> {
        debug!(
            target: "lats::backprop",
            node_count = evaluated_nodes.len(),
            "Starting backpropagation phase"
        );

        // Add each evaluated node to the tree
        for eval_node in evaluated_nodes {
            let node_id = tree
                .add_node(
                    eval_node.node.parent_id,
                    eval_node.node.thought.clone(),
                    Some(eval_node.node.action.clone()),
                    Some(eval_node.node.observation.clone()),
                    eval_node.score,
                )
                .map_err(|e| ExecutorError::ExecutionFailed(e.to_string()))?;

            // Propagate score upward to ancestors
            let mut current_id = node_id;
            while let Some(parent_id) = tree.get_parent(current_id) {
                let parent = tree
                    .get_node(parent_id)
                    .map_err(|e| ExecutorError::ExecutionFailed(e.to_string()))?;

                // Collect scores of all children
                let child_scores: Vec<f32> = parent
                    .children
                    .iter()
                    .filter_map(|child_id| tree.get_node(*child_id).ok().map(|child| child.score))
                    .collect();

                if child_scores.is_empty() {
                    break;
                }

                // Calculate new score: weighted average of max and mean child scores
                let max_child_score = child_scores
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max);
                let avg_child_score = child_scores.iter().sum::<f32>() / child_scores.len() as f32;

                // UCT-style update: favor best paths but consider average
                let new_score = 0.7 * max_child_score + 0.3 * avg_child_score;

                tree.update_score(parent_id, new_score);

                debug!(
                    target: "lats::backprop",
                    parent_id = parent_id,
                    new_score = new_score,
                    max_child = max_child_score,
                    avg_child = avg_child_score,
                    "Updated parent score"
                );

                current_id = parent_id;
            }
        }

        info!(
            target: "lats::backprop",
            "Backpropagation phase completed"
        );

        Ok(())
    }

    /// Extract the best path from the search tree
    fn extract_best_path(&self, tree: &SearchTree) -> Vec<NodeId> {
        tree.get_best_path()
    }

    /// Synthesize final answer from best path
    async fn synthesize_answer(
        &self,
        tree: &SearchTree,
        best_path: Vec<NodeId>,
        query: &str,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        use super::prompts::LATSPrompts;
        use super::provider_router::LATSPhase;
        use codegraph_ai::llm_provider::{GenerationConfig, Message, MessageRole, ResponseFormat};

        debug!(
            target: "lats::synthesis",
            path_length = best_path.len(),
            "Starting synthesis phase"
        );

        // Collect nodes along best path
        let path_nodes: Vec<&SearchNode> = best_path
            .iter()
            .filter_map(|id| tree.get_node(*id).ok())
            .collect();

        let provider = self.provider_router.get_provider(LATSPhase::Expansion);
        let prompt = LATSPrompts::synthesis_prompt(&path_nodes, query);

        let messages = vec![Message {
            role: MessageRole::User,
            content: prompt,
        }];

        let mut config = GenerationConfig::default();
        config.response_format = Some(ResponseFormat::JsonObject);
        config.temperature = 0.3; // Lower temperature for consistent synthesis

        let response = provider
            .generate_chat(&messages, &config)
            .await
            .map_err(|e| ExecutorError::ExecutionFailed(format!("LLM call failed: {}", e)))?;

        // Parse synthesis output (with error handling)
        let synthesis: SynthesisOutput = match serde_json::from_str(&response.content) {
            Ok(syn) => syn,
            Err(e) => {
                debug!(
                    target: "lats::synthesis",
                    error = %e,
                    content = %response.content,
                    "Failed to parse synthesis output, using default"
                );
                // Fallback: construct answer from path
                let answer = format!(
                    "Explored {} nodes in search tree with {} steps in best path",
                    tree.node_count(),
                    best_path.len()
                );
                SynthesisOutput {
                    answer,
                    findings: "LATS search completed".to_string(),
                    steps_taken: best_path.len().to_string(),
                }
            }
        };

        info!(
            target: "lats::synthesis",
            "Synthesis phase completed"
        );

        Ok(CodeGraphAgentOutput {
            answer: synthesis.answer,
            findings: synthesis.findings,
            steps_taken: synthesis.steps_taken,
        })
    }

    /// Check if a solution has been found, returning the termination reason
    fn check_termination(
        &self,
        tree: &SearchTree,
        evaluated_nodes: &[EvaluatedNode],
    ) -> Option<TerminationReason> {
        // Check 1: Any node marked as solution
        if evaluated_nodes.iter().any(|node| node.is_solution) {
            return Some(TerminationReason::SolutionFound);
        }

        // Check 2: Any leaf node has very high score
        let has_high_score_leaf = tree
            .get_leaf_nodes()
            .iter()
            .filter_map(|&id| tree.get_node(id).ok())
            .any(|node| node.score > HIGH_SCORE_THRESHOLD);

        if has_high_score_leaf {
            return Some(TerminationReason::HighScore);
        }

        // Check 3: Tree size limit exceeded
        let max_nodes = self.config.effective_max_tree_nodes();
        if tree.node_count() >= max_nodes {
            return Some(TerminationReason::TreeSizeExceeded);
        }

        None
    }

    /// Log early termination event when CODEGRAPH_DEBUG is set
    fn log_early_termination(&self, reason: &TerminationReason, tree: &SearchTree) {
        if std::env::var("CODEGRAPH_DEBUG").is_ok() {
            let reason_str = match reason {
                TerminationReason::HighScore => "high_score",
                TerminationReason::SolutionFound => "solution_found",
                TerminationReason::TreeSizeExceeded => "tree_size_exceeded",
                TerminationReason::IterationTimeout => "iteration_timeout",
            };

            info!(
                target: "lats_early_termination",
                reason = reason_str,
                tree_size = tree.node_count(),
                max_allowed = self.config.effective_max_tree_nodes(),
                "LATS search terminated early"
            );
        }
    }
}

/// Reason for early termination
#[derive(Debug, Clone, PartialEq)]
pub enum TerminationReason {
    /// Solution node marked with is_solution flag
    SolutionFound,
    /// Node achieved score > 0.9
    HighScore,
    /// Tree size limit exceeded
    TreeSizeExceeded,
    /// Per-iteration timeout occurred
    IterationTimeout,
}

#[async_trait]
impl AgentExecutorTrait for LATSExecutor {
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        info!(
            target: "lats::executor",
            analysis_type = ?analysis_type,
            query_len = query.len(),
            max_tree_nodes = self.config.effective_max_tree_nodes(),
            iteration_timeout_secs = self.config.iteration_timeout_secs,
            "Starting LATS execution"
        );

        // Phase 1: Initialize search tree with root node
        let root_thought = format!(
            "Beginning {} analysis for query: {}",
            match analysis_type {
                AnalysisType::CodeSearch => "code search",
                AnalysisType::DependencyAnalysis => "dependency analysis",
                AnalysisType::CallChainAnalysis => "call chain analysis",
                AnalysisType::ArchitectureAnalysis => "architecture analysis",
                AnalysisType::ApiSurfaceAnalysis => "API surface analysis",
                AnalysisType::ContextBuilder => "context building",
                AnalysisType::SemanticQuestion => "semantic question answering",
            },
            query
        );

        let root = SearchNode::new_root(root_thought);
        let mut tree = SearchTree::new(root);

        info!(
            target: "lats::executor",
            "Search tree initialized with root node"
        );

        // Phase 2: Iterative LATS search
        let mut termination_reason: Option<TerminationReason> = None;

        for iteration in 0..self.config.max_depth {
            debug!(
                target: "lats::executor",
                iteration = iteration,
                tree_size = tree.node_count(),
                "Starting LATS iteration"
            );

            // Check tree size before starting iteration
            if let Some(reason) = self.check_termination(&tree, &[]) {
                termination_reason = Some(reason);
                break;
            }

            // Step 1: Selection - choose promising nodes to expand
            let selected_nodes = self.select_nodes(&tree, &query).await?;

            if selected_nodes.is_empty() {
                info!(
                    target: "lats::executor",
                    iteration = iteration,
                    "No nodes to expand, terminating search"
                );
                break;
            }

            // Steps 2-4 wrapped in per-iteration timeout
            let iteration_timeout = self.config.iteration_timeout();
            let iteration_result = tokio::time::timeout(iteration_timeout, async {
                // Step 2: Expansion - generate new thought/action candidates
                let expansions = self.expand_nodes(&tree, &selected_nodes, &query).await?;

                // Step 3: Evaluation - assess quality of expanded nodes
                let evaluated = self.evaluate_nodes(&expansions, &query).await?;

                Ok::<_, ExecutorError>((expansions, evaluated))
            })
            .await;

            let (expansions, evaluated) = match iteration_result {
                Ok(Ok(result)) => result,
                Ok(Err(e)) => {
                    // Execution error - propagate
                    return Err(e);
                }
                Err(_elapsed) => {
                    // Iteration timeout - log and fall back to best solution
                    warn!(
                        target: "lats::executor",
                        iteration = iteration,
                        timeout_secs = self.config.iteration_timeout_secs,
                        "LATS iteration timed out"
                    );
                    termination_reason = Some(TerminationReason::IterationTimeout);
                    break;
                }
            };

            // Step 4: Backpropagation - update scores up the tree
            self.backpropagate(&mut tree, &evaluated).await?;

            // Check for solution or termination conditions
            if let Some(reason) = self.check_termination(&tree, &evaluated) {
                termination_reason = Some(reason);
                break;
            }

            debug!(
                target: "lats::executor",
                iteration = iteration,
                expansions = expansions.len(),
                "LATS iteration completed"
            );
        }

        // Log early termination if applicable
        if let Some(ref reason) = termination_reason {
            self.log_early_termination(reason, &tree);
        }

        // Phase 3: Extract best path and synthesize final answer
        let best_path = self.extract_best_path(&tree);
        let mut output = self.synthesize_answer(&tree, best_path, &query).await?;

        // Add warning if terminated due to tree size or timeout
        if let Some(reason) = &termination_reason {
            match reason {
                TerminationReason::TreeSizeExceeded => {
                    output.findings = format!(
                        "WARNING: Search tree size limit ({}) exceeded. Result may be incomplete.\n\n{}",
                        self.config.effective_max_tree_nodes(),
                        output.findings
                    );
                }
                TerminationReason::IterationTimeout => {
                    output.findings = format!(
                        "WARNING: Iteration timeout ({}s) occurred. Result based on partial exploration.\n\n{}",
                        self.config.iteration_timeout_secs,
                        output.findings
                    );
                }
                _ => {} // High score and solution found are success cases
            }
        }

        info!(
            target: "lats::executor",
            tree_size = tree.node_count(),
            termination_reason = ?termination_reason,
            "LATS execution completed"
        );

        Ok(output)
    }

    fn architecture(&self) -> AgentArchitecture {
        AgentArchitecture::LATS
    }

    fn tier(&self) -> ContextTier {
        self.config.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lats_config_default() {
        let config = LATSConfig::default();
        assert_eq!(config.beam_width, 3);
        assert_eq!(config.max_depth, 5);
        assert_eq!(config.exploration_weight, 1.414);
        assert_eq!(config.iteration_timeout_secs, DEFAULT_ITERATION_TIMEOUT_SECS);
        assert_eq!(config.max_tree_nodes, 0); // Auto-calculate
    }

    #[test]
    fn test_lats_config_from_codegraph() {
        let cg_config = CodeGraphConfig::default();
        let tier = ContextTier::Large;

        let lats_config = LATSConfig::from_codegraph_config(&cg_config, tier);
        assert_eq!(lats_config.tier, ContextTier::Large);
        assert_eq!(lats_config.beam_width, 3);
        assert_eq!(lats_config.max_depth, 5);
        // Note: iteration_timeout_secs may be set from env, just verify tier
    }

    // Env-based tests need special handling due to parallel execution
    // Using unique values to detect contamination

    #[test]
    fn test_lats_config_from_env_timeout() {
        // Use unique value unlikely to be set by other tests
        std::env::set_var("CODEGRAPH_LATS_ITERATION_TIMEOUT_SECS", "42");
        let cg_config = CodeGraphConfig::default();
        let lats_config = LATSConfig::from_codegraph_config(&cg_config, ContextTier::Medium);
        assert_eq!(lats_config.iteration_timeout_secs, 42);
        std::env::remove_var("CODEGRAPH_LATS_ITERATION_TIMEOUT_SECS");
    }

    #[test]
    fn test_lats_config_from_env_tree_nodes() {
        std::env::set_var("CODEGRAPH_AGENT_MAX_TREE_NODES", "99");
        let cg_config = CodeGraphConfig::default();
        let lats_config = LATSConfig::from_codegraph_config(&cg_config, ContextTier::Medium);
        assert_eq!(lats_config.max_tree_nodes, 99);
        std::env::remove_var("CODEGRAPH_AGENT_MAX_TREE_NODES");
    }

    #[test]
    fn test_effective_max_tree_nodes_auto_calculate() {
        let config = LATSConfig {
            beam_width: 3,
            max_depth: 5,
            max_tree_nodes: 0, // Auto-calculate
            ..Default::default()
        };
        // Auto-calculated: beam_width * max_depth * 2 = 3 * 5 * 2 = 30
        assert_eq!(config.effective_max_tree_nodes(), 30);
    }

    #[test]
    fn test_effective_max_tree_nodes_explicit() {
        let config = LATSConfig {
            beam_width: 3,
            max_depth: 5,
            max_tree_nodes: 100, // Explicit
            ..Default::default()
        };
        assert_eq!(config.effective_max_tree_nodes(), 100);
    }

    #[test]
    fn test_iteration_timeout_duration() {
        let config = LATSConfig {
            iteration_timeout_secs: 30,
            ..Default::default()
        };
        assert_eq!(config.iteration_timeout().as_secs(), 30);
    }

    #[test]
    fn test_iteration_timeout_zero_is_unlimited() {
        let config = LATSConfig {
            iteration_timeout_secs: 0,
            ..Default::default()
        };
        // Zero means unlimited - should be very large
        assert!(config.iteration_timeout().as_secs() > 1_000_000);
    }

    #[test]
    fn test_termination_reason_enum() {
        assert_eq!(
            format!("{:?}", TerminationReason::HighScore),
            "HighScore"
        );
        assert_eq!(
            format!("{:?}", TerminationReason::SolutionFound),
            "SolutionFound"
        );
        assert_eq!(
            format!("{:?}", TerminationReason::TreeSizeExceeded),
            "TreeSizeExceeded"
        );
        assert_eq!(
            format!("{:?}", TerminationReason::IterationTimeout),
            "IterationTimeout"
        );
    }

    #[test]
    fn test_high_score_threshold() {
        assert_eq!(HIGH_SCORE_THRESHOLD, 0.9);
    }
}
