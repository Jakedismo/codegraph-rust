# LATS Agent Architecture for CodeGraph MCP Server

**Version:** 1.0
**Date:** 2025-12-01
**Status:** Design Phase

## Executive Summary

This document specifies the architecture for adding LATS (Language Agent Tree Search) support to CodeGraph's MCP server. The design enables runtime switching between ReAct (fast) and LATS (high-quality) agent architectures via environment variable configuration, with support for multi-provider LLM allocation across LATS phases.

**Key Design Principles:**
- **Zero Breaking Changes**: Existing ReAct functionality remains untouched
- **Clean Separation**: LATS implementation isolated in dedicated modules
- **Configuration-Driven**: Runtime selection via `CODEGRAPH_AGENT_ARCHITECTURE`
- **Multi-Provider Routing**: Different LLMs for different LATS phases
- **Tier-Aware**: Integrates with existing `ContextTier` system
- **Extensible**: Foundation for future agent architectures (ToT, CoT-SC, Reflexion)

---

## 1. Crate Responsibility Mapping

### 1.1 codegraph-mcp-core

**New Types:**
```rust
// src/agent_architecture.rs (NEW FILE)
pub enum AgentArchitecture {
    ReAct,
    LATS,
    // Future: ToT, CoTSC, Reflexion
}

pub struct AgentConfig {
    pub architecture: AgentArchitecture,
    pub tier: ContextTier,
    pub analysis_type: AnalysisType,
}
```

**Responsibility:** Protocol-level types, no business logic

### 1.2 codegraph-mcp-autoagents

**New Modules:**

```
crates/codegraph-mcp-autoagents/src/
├── autoagents/
│   ├── agent_builder.rs (EXISTING - stays ReAct-only)
│   ├── lats_agent_builder.rs (NEW)
│   ├── lats_executor.rs (NEW)
│   ├── lats_config.rs (NEW)
│   ├── lats_prompts/ (NEW DIRECTORY)
│   │   ├── mod.rs
│   │   ├── selection_prompts.rs
│   │   ├── expansion_prompts.rs
│   │   ├── evaluation_prompts.rs
│   │   └── backprop_prompts.rs
│   ├── executor.rs (EXISTING - modify for multi-architecture)
│   └── provider_router.rs (NEW)
```

**Responsibility:**
- LATS algorithm implementation
- Multi-provider routing logic
- LATS-specific prompt generation
- Agent builder orchestration

### 1.3 codegraph-mcp-tools

**No Changes Required**
GraphToolExecutor remains unchanged - both ReAct and LATS use the same 7 tools.

### 1.4 codegraph-core

**Configuration Extensions:**

```rust
// config_manager.rs - extend LLMConfig
pub struct LLMConfig {
    // ... existing fields ...

    /// LATS-specific multi-provider configuration
    #[serde(default)]
    pub lats: Option<LATSProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LATSProviderConfig {
    /// Provider for selection phase (node scoring)
    #[serde(default)]
    pub selection_provider: Option<String>,
    pub selection_model: Option<String>,

    /// Provider for expansion phase (generating new thoughts)
    #[serde(default)]
    pub expansion_provider: Option<String>,
    pub expansion_model: Option<String>,

    /// Provider for evaluation phase (assessing quality)
    #[serde(default)]
    pub evaluation_provider: Option<String>,
    pub evaluation_model: Option<String>,

    /// Provider for backpropagation phase (updating scores)
    #[serde(default)]
    pub backprop_provider: Option<String>,
    pub backprop_model: Option<String>,

    /// LATS algorithm parameters
    #[serde(default = "default_lats_beam_width")]
    pub beam_width: usize,

    #[serde(default = "default_lats_max_depth")]
    pub max_depth: usize,

    #[serde(default = "default_lats_exploration_weight")]
    pub exploration_weight: f32,
}
```

---

## 2. Configuration Schema

### 2.1 TOML Configuration

**File:** `~/.codegraph/config.toml`

```toml
[llm]
enabled = true
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
context_window = 200000

# LATS multi-provider configuration (optional)
[llm.lats]
# Selection phase: Fast, cheap model for node scoring
selection_provider = "openai"
selection_model = "gpt-4o-mini"

# Expansion phase: Creative model for generating thoughts
expansion_provider = "anthropic"
expansion_model = "claude-3-5-sonnet-20241022"

# Evaluation phase: Reasoning model for quality assessment
evaluation_provider = "openai"
evaluation_model = "o1-preview"

# Backpropagation: Fast model for score updates
backprop_provider = "openai"
backprop_model = "gpt-4o-mini"

# LATS algorithm tuning
beam_width = 3
max_depth = 5
exploration_weight = 1.414  # sqrt(2) for UCT
```

### 2.2 Environment Variables

```bash
# Architecture selection (required)
export CODEGRAPH_AGENT_ARCHITECTURE=lats  # or "react" (default)

# Single-provider LATS (simplest setup)
export ANTHROPIC_API_KEY=sk-ant-...

# Multi-provider LATS (advanced)
export CODEGRAPH_LATS_SELECTION_PROVIDER=openai
export CODEGRAPH_LATS_SELECTION_MODEL=gpt-4o-mini
export CODEGRAPH_LATS_EXPANSION_PROVIDER=anthropic
export CODEGRAPH_LATS_EXPANSION_MODEL=claude-3-5-sonnet-20241022
export CODEGRAPH_LATS_EVALUATION_PROVIDER=openai
export CODEGRAPH_LATS_EVALUATION_MODEL=o1-preview
export CODEGRAPH_LATS_BACKPROP_PROVIDER=openai
export CODEGRAPH_LATS_BACKPROP_MODEL=gpt-4o-mini

# LATS algorithm tuning
export CODEGRAPH_LATS_BEAM_WIDTH=3
export CODEGRAPH_LATS_MAX_DEPTH=5
export CODEGRAPH_LATS_EXPLORATION_WEIGHT=1.414
```

### 2.3 Configuration Precedence

1. Environment variables (highest priority)
2. `~/.codegraph/config.toml`
3. Default values (fallback)

**Single-Provider Fallback Logic:**
```rust
// If no LATS-specific providers configured, use primary LLM for all phases
if config.llm.lats.is_none() {
    use config.llm.provider for all LATS phases
}
```

---

## 3. Trait/Interface Design

### 3.1 Agent Executor Abstraction

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/executor.rs

use async_trait::async_trait;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;

/// Universal executor trait for all agent architectures
#[async_trait]
pub trait AgentExecutorTrait: Send + Sync {
    /// Execute agentic analysis with the given query
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError>;

    /// Get the architecture type this executor implements
    fn architecture(&self) -> AgentArchitecture;

    /// Get the context tier this executor is configured for
    fn tier(&self) -> ContextTier;
}

/// Factory for creating architecture-specific executors
pub struct AgentExecutorFactory {
    llm_provider: Arc<dyn LLMProvider>,
    tool_executor: Arc<GraphToolExecutor>,
    config: Arc<CodeGraphConfig>,
}

impl AgentExecutorFactory {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        config: Arc<CodeGraphConfig>,
    ) -> Self {
        Self { llm_provider, tool_executor, config }
    }

    /// Create executor based on configured architecture
    pub fn create(&self, architecture: AgentArchitecture) -> Result<Box<dyn AgentExecutorTrait>> {
        match architecture {
            AgentArchitecture::ReAct => {
                Ok(Box::new(ReActExecutor::new(
                    self.llm_provider.clone(),
                    self.tool_executor.clone(),
                )))
            }
            AgentArchitecture::LATS => {
                Ok(Box::new(LATSExecutor::new(
                    self.config.clone(),
                    self.tool_executor.clone(),
                )?))
            }
        }
    }
}
```

### 3.2 ReAct Executor Wrapper

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/react_executor.rs (NEW FILE)

/// Wrapper around existing CodeGraphExecutor for ReAct
pub struct ReActExecutor {
    inner: CodeGraphExecutor,
}

#[async_trait]
impl AgentExecutorTrait for ReActExecutor {
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        self.inner.execute(query, analysis_type).await
    }

    fn architecture(&self) -> AgentArchitecture {
        AgentArchitecture::ReAct
    }

    fn tier(&self) -> ContextTier {
        self.inner.detect_tier().await.unwrap_or(ContextTier::Medium)
    }
}
```

---

## 4. LATS Executor Implementation

### 4.1 Core Algorithm Structure

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/lats_executor.rs

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// LATS executor implementing Language Agent Tree Search
pub struct LATSExecutor {
    config: LATSConfig,
    provider_router: ProviderRouter,
    tool_executor: Arc<GraphToolExecutor>,
    tier: ContextTier,
}

impl LATSExecutor {
    pub fn new(
        config: Arc<CodeGraphConfig>,
        tool_executor: Arc<GraphToolExecutor>,
    ) -> Result<Self> {
        let tier = ContextTier::from_context_window(config.llm.context_window);
        let lats_config = LATSConfig::from_codegraph_config(&config)?;
        let provider_router = ProviderRouter::new(&config)?;

        Ok(Self {
            config: lats_config,
            provider_router,
            tool_executor,
            tier,
        })
    }
}

#[async_trait]
impl AgentExecutorTrait for LATSExecutor {
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Phase 1: Initialize search tree
        let root = self.create_root_node(&query, analysis_type)?;
        let mut tree = SearchTree::new(root);

        // Phase 2: Iterative LATS search
        for depth in 0..self.config.max_depth {
            // Step 1: Selection - choose promising nodes to expand
            let selected_nodes = self.select_nodes(&tree).await?;

            if selected_nodes.is_empty() {
                break; // No more nodes to expand
            }

            // Step 2: Expansion - generate new thought/action candidates
            let expansions = self.expand_nodes(&selected_nodes, &tree).await?;

            // Step 3: Evaluation - assess quality of expanded nodes
            let evaluated = self.evaluate_nodes(&expansions).await?;

            // Step 4: Backpropagation - update scores up the tree
            self.backpropagate(&mut tree, &evaluated).await?;

            // Check for solution
            if self.is_solution_found(&tree)? {
                break;
            }
        }

        // Phase 3: Extract best path and synthesize final answer
        let best_path = self.extract_best_path(&tree)?;
        let output = self.synthesize_answer(best_path, analysis_type).await?;

        Ok(output)
    }

    fn architecture(&self) -> AgentArchitecture {
        AgentArchitecture::LATS
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }
}

// === LATS Phase Implementations ===

impl LATSExecutor {
    /// Phase 1: Node Selection using UCT algorithm
    async fn select_nodes(&self, tree: &SearchTree) -> Result<Vec<NodeId>> {
        let provider = self.provider_router.get_provider(LATSPhase::Selection).await?;

        let prompt = self.create_selection_prompt(tree)?;
        let response = provider.generate_chat(&[prompt], &GenerationConfig::default()).await?;

        let selected: SelectionOutput = serde_json::from_str(&response.content)?;

        Ok(selected.node_ids)
    }

    /// Phase 2: Node Expansion - generate new thoughts/actions
    async fn expand_nodes(
        &self,
        nodes: &[NodeId],
        tree: &SearchTree,
    ) -> Result<Vec<ExpandedNode>> {
        let provider = self.provider_router.get_provider(LATSPhase::Expansion).await?;

        let mut expansions = Vec::new();

        for node_id in nodes {
            let node = tree.get_node(*node_id)?;
            let prompt = self.create_expansion_prompt(node, tree)?;

            let response = provider.generate_chat(&[prompt], &GenerationConfig::default()).await?;

            let expansion: ExpansionOutput = serde_json::from_str(&response.content)?;

            // Execute tool calls for expansion
            for action in expansion.actions {
                let tool_result = self.tool_executor
                    .execute(&action.tool_name, action.parameters)
                    .await?;

                expansions.push(ExpandedNode {
                    parent_id: *node_id,
                    thought: action.reasoning.clone(),
                    action: action.clone(),
                    observation: tool_result,
                });
            }
        }

        Ok(expansions)
    }

    /// Phase 3: Node Evaluation - assess quality
    async fn evaluate_nodes(
        &self,
        nodes: &[ExpandedNode],
    ) -> Result<Vec<EvaluatedNode>> {
        let provider = self.provider_router.get_provider(LATSPhase::Evaluation).await?;

        let mut evaluated = Vec::new();

        for node in nodes {
            let prompt = self.create_evaluation_prompt(node)?;
            let response = provider.generate_chat(&[prompt], &GenerationConfig::default()).await?;

            let evaluation: EvaluationOutput = serde_json::from_str(&response.content)?;

            evaluated.push(EvaluatedNode {
                node: node.clone(),
                score: evaluation.score,
                reasoning: evaluation.reasoning,
                is_solution: evaluation.is_solution,
            });
        }

        Ok(evaluated)
    }

    /// Phase 4: Backpropagation - update UCT scores
    async fn backpropagate(
        &self,
        tree: &mut SearchTree,
        evaluated_nodes: &[EvaluatedNode],
    ) -> Result<()> {
        let provider = self.provider_router.get_provider(LATSPhase::Backpropagation).await?;

        for eval_node in evaluated_nodes {
            // Add node to tree
            let node_id = tree.add_node(
                eval_node.node.parent_id,
                eval_node.node.thought.clone(),
                eval_node.node.action.clone(),
                eval_node.node.observation.clone(),
                eval_node.score,
            );

            // Update ancestor scores using UCT
            let mut current = node_id;
            while let Some(parent_id) = tree.get_parent(current) {
                let update_prompt = self.create_backprop_prompt(tree, current, parent_id)?;
                let response = provider.generate_chat(&[update_prompt], &GenerationConfig::default()).await?;

                let update: BackpropUpdate = serde_json::from_str(&response.content)?;

                tree.update_score(parent_id, update.new_score);
                current = parent_id;
            }
        }

        Ok(())
    }
}
```

### 4.2 Search Tree Data Structure

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/lats_executor.rs

use std::collections::HashMap;

pub type NodeId = usize;

#[derive(Debug, Clone)]
pub struct SearchNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub thought: String,
    pub action: Option<ToolAction>,
    pub observation: Option<JsonValue>,
    pub score: f32,
    pub visits: usize,
    pub children: Vec<NodeId>,
    pub depth: usize,
}

pub struct SearchTree {
    nodes: HashMap<NodeId, SearchNode>,
    root_id: NodeId,
    next_id: NodeId,
}

impl SearchTree {
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

    pub fn add_node(
        &mut self,
        parent_id: NodeId,
        thought: String,
        action: ToolAction,
        observation: JsonValue,
        score: f32,
    ) -> NodeId {
        let node_id = self.next_id;
        self.next_id += 1;

        let parent_depth = self.nodes[&parent_id].depth;

        let node = SearchNode {
            id: node_id,
            parent_id: Some(parent_id),
            thought,
            action: Some(action),
            observation: Some(observation),
            score,
            visits: 0,
            children: Vec::new(),
            depth: parent_depth + 1,
        };

        self.nodes.insert(node_id, node);
        self.nodes.get_mut(&parent_id).unwrap().children.push(node_id);

        node_id
    }

    pub fn get_node(&self, id: NodeId) -> Result<&SearchNode> {
        self.nodes.get(&id).ok_or_else(|| ExecutorError::NodeNotFound(id))
    }

    pub fn update_score(&mut self, id: NodeId, new_score: f32) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.score = new_score;
            node.visits += 1;
        }
    }

    /// Get all leaf nodes (potential expansion candidates)
    pub fn get_leaf_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|node| node.children.is_empty())
            .map(|node| node.id)
            .collect()
    }

    /// Calculate UCT score for node selection
    pub fn uct_score(&self, node_id: NodeId, exploration_weight: f32) -> f32 {
        let node = &self.nodes[&node_id];

        if node.visits == 0 {
            return f32::INFINITY; // Unvisited nodes get highest priority
        }

        let parent = node.parent_id
            .and_then(|pid| self.nodes.get(&pid));

        let parent_visits = parent.map(|p| p.visits).unwrap_or(1);

        // UCT formula: Q(s,a) + c * sqrt(ln(N(s)) / N(s,a))
        node.score + exploration_weight * ((parent_visits as f32).ln() / node.visits as f32).sqrt()
    }
}
```

---

## 5. Provider Router Design

### 5.1 Multi-Provider Routing

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/provider_router.rs

use codegraph_ai::llm_factory::create_llm_provider;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LATSPhase {
    Selection,
    Expansion,
    Evaluation,
    Backpropagation,
}

/// Routes LLM requests to appropriate providers based on LATS phase
pub struct ProviderRouter {
    providers: HashMap<LATSPhase, Arc<dyn LLMProvider>>,
    default_provider: Arc<dyn LLMProvider>,
    config: Arc<CodeGraphConfig>,
}

impl ProviderRouter {
    pub fn new(config: &CodeGraphConfig) -> Result<Self> {
        let mut providers = HashMap::new();

        // Create default provider (fallback for all phases)
        let default_provider = create_llm_provider(&config.llm)?;

        // Create phase-specific providers if configured
        if let Some(ref lats_config) = config.llm.lats {
            // Selection provider
            if let (Some(provider), Some(model)) =
                (&lats_config.selection_provider, &lats_config.selection_model)
            {
                let llm_config = Self::create_phase_config(provider, model, config);
                providers.insert(
                    LATSPhase::Selection,
                    create_llm_provider(&llm_config)?
                );
            }

            // Expansion provider
            if let (Some(provider), Some(model)) =
                (&lats_config.expansion_provider, &lats_config.expansion_model)
            {
                let llm_config = Self::create_phase_config(provider, model, config);
                providers.insert(
                    LATSPhase::Expansion,
                    create_llm_provider(&llm_config)?
                );
            }

            // Evaluation provider
            if let (Some(provider), Some(model)) =
                (&lats_config.evaluation_provider, &lats_config.evaluation_model)
            {
                let llm_config = Self::create_phase_config(provider, model, config);
                providers.insert(
                    LATSPhase::Evaluation,
                    create_llm_provider(&llm_config)?
                );
            }

            // Backpropagation provider
            if let (Some(provider), Some(model)) =
                (&lats_config.backprop_provider, &lats_config.backprop_model)
            {
                let llm_config = Self::create_phase_config(provider, model, config);
                providers.insert(
                    LATSPhase::Backpropagation,
                    create_llm_provider(&llm_config)?
                );
            }
        }

        Ok(Self {
            providers,
            default_provider,
            config: Arc::new(config.clone()),
        })
    }

    /// Get provider for specific LATS phase
    pub async fn get_provider(&self, phase: LATSPhase) -> Result<Arc<dyn LLMProvider>> {
        Ok(self.providers
            .get(&phase)
            .cloned()
            .unwrap_or_else(|| self.default_provider.clone()))
    }

    /// Create phase-specific LLM configuration
    fn create_phase_config(
        provider: &str,
        model: &str,
        base_config: &CodeGraphConfig,
    ) -> LLMConfig {
        let mut config = base_config.llm.clone();
        config.provider = provider.to_string();
        config.model = Some(model.to_string());
        config
    }

    /// Get statistics about provider allocation
    pub fn stats(&self) -> ProviderStats {
        ProviderStats {
            default_provider: self.default_provider.provider_name().to_string(),
            phase_providers: self.providers
                .iter()
                .map(|(phase, provider)| {
                    (format!("{:?}", phase), provider.provider_name().to_string())
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStats {
    pub default_provider: String,
    pub phase_providers: HashMap<String, String>,
}
```

---

## 6. Integration Points

### 6.1 CodeGraphAgentBuilder Extension

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/agent_builder.rs

impl CodeGraphAgentBuilder {
    // EXISTING METHOD - unchanged
    pub async fn build(self) -> Result<AgentHandle, AutoAgentsError> {
        // ... existing ReAct implementation ...
    }

    // NEW METHOD - for architecture factory
    pub fn architecture(&self) -> AgentArchitecture {
        AgentArchitecture::ReAct // This builder is ReAct-only
    }
}
```

### 6.2 Modified CodeGraphExecutor

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/executor.rs

pub struct CodeGraphExecutor {
    factory: AgentExecutorFactory,
    architecture: AgentArchitecture,
}

impl CodeGraphExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        config: Arc<CodeGraphConfig>,
    ) -> Self {
        let factory = AgentExecutorFactory::new(
            llm_provider,
            tool_executor,
            config.clone(),
        );

        // Detect architecture from config or environment
        let architecture = Self::detect_architecture(&config);

        Self { factory, architecture }
    }

    pub async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Create architecture-specific executor
        let executor = self.factory.create(self.architecture)?;

        // Execute with selected architecture
        executor.execute(query, analysis_type).await
    }

    fn detect_architecture(config: &CodeGraphConfig) -> AgentArchitecture {
        // 1. Check environment variable (highest priority)
        if let Ok(arch_str) = std::env::var("CODEGRAPH_AGENT_ARCHITECTURE") {
            return AgentArchitecture::parse(&arch_str)
                .unwrap_or(AgentArchitecture::ReAct);
        }

        // 2. Check config file
        // (future: add agent.architecture to config.toml)

        // 3. Default to ReAct
        AgentArchitecture::ReAct
    }
}
```

### 6.3 MCP Server Integration

```rust
// crates/codegraph-mcp-server/src/agentic_tools.rs

async fn handle_agentic_tool_call(
    tool_name: &str,
    arguments: JsonValue,
    state: &ServerState,
) -> Result<Vec<JsonValue>> {
    let query = arguments["query"]
        .as_str()
        .ok_or_else(|| McpError::InvalidParams("Missing query parameter".to_string()))?;

    let analysis_type = AnalysisType::from_tool_name(tool_name)?;

    // CodeGraphExecutor now handles architecture selection internally
    let executor = CodeGraphExecutor::new(
        state.llm_provider.clone(),
        state.tool_executor.clone(),
        state.config.clone(),
    );

    let output = executor.execute(query.to_string(), analysis_type).await?;

    Ok(vec![serde_json::to_value(output)?])
}
```

---

## 7. Feature Flags

### 7.1 Cargo.toml Features

```toml
# crates/codegraph-mcp-autoagents/Cargo.toml

[features]
default = ["autoagents-react"]

# Existing ReAct support (current production)
autoagents-react = []

# LATS support (new experimental feature)
autoagents-lats = ["autoagents-react"]  # LATS requires ReAct as fallback

# Future agent architectures
autoagents-tot = ["autoagents-react"]    # Tree of Thoughts
autoagents-cot-sc = ["autoagents-react"] # Chain of Thought Self-Consistency
autoagents-reflexion = ["autoagents-react"] # Reflexion
```

### 7.2 Conditional Compilation

```rust
// crates/codegraph-mcp-autoagents/src/autoagents/mod.rs

pub mod agent_builder;
pub mod executor;
pub mod tier_plugin;
pub mod prompt_selector;

#[cfg(feature = "autoagents-lats")]
pub mod lats_agent_builder;

#[cfg(feature = "autoagents-lats")]
pub mod lats_executor;

#[cfg(feature = "autoagents-lats")]
pub mod provider_router;
```

---

## 8. Migration Path

### Phase 1: Infrastructure Setup (Week 1)
**Tasks:**
1. Add `AgentArchitecture` enum to codegraph-mcp-core
2. Extend configuration schema in codegraph-core
3. Create `AgentExecutorTrait` abstraction
4. Wrap existing ReActExecutor with trait implementation
5. Add architecture detection to `CodeGraphExecutor`
6. Update unit tests to verify no regression

**Validation:** All existing tests pass, ReAct behavior unchanged

### Phase 2: LATS Core Implementation (Week 2-3)
**Tasks:**
1. Implement `SearchTree` data structure
2. Implement `ProviderRouter` for multi-provider support
3. Create LATS-specific prompt templates
4. Implement `LATSExecutor` with 4 phases
5. Add LATS configuration loading
6. Write unit tests for LATS components

**Validation:** LATS executor can be instantiated and configured

### Phase 3: Integration & Testing (Week 4)
**Tasks:**
1. Integrate LATS with `AgentExecutorFactory`
2. Test single-provider LATS mode
3. Test multi-provider LATS mode
4. Benchmark LATS vs ReAct performance
5. Add integration tests
6. Update documentation

**Validation:** LATS executes successfully, produces correct outputs

### Phase 4: Optimization & Production (Week 5)
**Tasks:**
1. Implement caching for LATS phases
2. Add telemetry and observability
3. Performance tuning (beam width, depth)
4. Production rollout with feature flag
5. Monitor quality metrics

**Validation:** LATS quality > ReAct, acceptable latency

---

## 9. Sequence Diagrams

### 9.1 LATS Execution Flow

```
┌─────────┐          ┌──────────────┐          ┌──────────────┐          ┌─────────────┐
│ MCP Tool│          │CodeGraphExec │          │LATSExecutor  │          │ProviderRouter│
└────┬────┘          └──────┬───────┘          └──────┬───────┘          └──────┬──────┘
     │                      │                          │                         │
     │  agentic_code_search │                          │                         │
     │─────────────────────>│                          │                         │
     │                      │                          │                         │
     │                      │  execute(query, type)    │                         │
     │                      │─────────────────────────>│                         │
     │                      │                          │                         │
     │                      │                          │  create_root_node()     │
     │                      │                          │──┐                      │
     │                      │                          │  │                      │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │                   ╔═════════════════════╗          │
     │                      │                   ║ LATS ITERATION LOOP ║          │
     │                      │                   ╚═════════════════════╝          │
     │                      │                          │                         │
     │                      │                          │  [Phase 1: Selection]   │
     │                      │                          │  get_provider(Selection)│
     │                      │                          │────────────────────────>│
     │                      │                          │                         │
     │                      │                          │  SelectionProvider      │
     │                      │                          │<────────────────────────│
     │                      │                          │                         │
     │                      │                          │  generate_chat(prompt)  │
     │                      │                          │──┐                      │
     │                      │                          │  │ "Which nodes expand?"│
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │                          │  [Phase 2: Expansion]   │
     │                      │                          │  get_provider(Expansion)│
     │                      │                          │────────────────────────>│
     │                      │                          │                         │
     │                      │                          │  ExpansionProvider      │
     │                      │                          │<────────────────────────│
     │                      │                          │                         │
     │                      │                          │  generate_chat(prompt)  │
     │                      │                          │──┐                      │
     │                      │                          │  │ "Generate actions"   │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │             ┌────────────┴────────────┐            │
     │                      │             │Execute tool calls via   │            │
     │                      │             │GraphToolExecutor        │            │
     │                      │             └────────────┬────────────┘            │
     │                      │                          │                         │
     │                      │                          │  [Phase 3: Evaluation]  │
     │                      │                          │  get_provider(Evaluation)│
     │                      │                          │────────────────────────>│
     │                      │                          │                         │
     │                      │                          │  EvaluationProvider     │
     │                      │                          │<────────────────────────│
     │                      │                          │                         │
     │                      │                          │  generate_chat(prompt)  │
     │                      │                          │──┐                      │
     │                      │                          │  │ "Score nodes"        │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │                          │  [Phase 4: Backprop]    │
     │                      │                          │  get_provider(Backprop) │
     │                      │                          │────────────────────────>│
     │                      │                          │                         │
     │                      │                          │  BackpropProvider       │
     │                      │                          │<────────────────────────│
     │                      │                          │                         │
     │                      │                          │  generate_chat(prompt)  │
     │                      │                          │──┐                      │
     │                      │                          │  │ "Update UCT scores"  │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │                   ╔═════════════════════╗          │
     │                      │                   ║ END ITERATION       ║          │
     │                      │                   ╚═════════════════════╝          │
     │                      │                          │                         │
     │                      │                          │  extract_best_path()    │
     │                      │                          │──┐                      │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │                          │  synthesize_answer()    │
     │                      │                          │──┐                      │
     │                      │                          │<─┘                      │
     │                      │                          │                         │
     │                      │  CodeGraphAgentOutput    │                         │
     │                      │<─────────────────────────│                         │
     │                      │                          │                         │
     │  MCP Response        │                          │                         │
     │<─────────────────────│                          │                         │
     │                      │                          │                         │
```

### 9.2 Provider Router Flow

```
┌─────────────┐          ┌──────────────┐          ┌─────────────┐          ┌──────────────┐
│LATSExecutor │          │ProviderRouter│          │LLMFactory   │          │LLMProviders  │
└──────┬──────┘          └──────┬───────┘          └──────┬──────┘          └──────┬───────┘
       │                        │                         │                         │
       │  get_provider(Selection)                        │                         │
       │───────────────────────>│                         │                         │
       │                        │                         │                         │
       │                        │  Has phase-specific     │                         │
       │                        │  provider configured?   │                         │
       │                        │──┐                      │                         │
       │                        │  │                      │                         │
       │                        │<─┘                      │                         │
       │                        │                         │                         │
       │           ┌────────────┴────────────┐            │                         │
       │           │YES: Return cached       │            │                         │
       │           │     phase provider      │            │                         │
       │           └────────────┬────────────┘            │                         │
       │                        │                         │                         │
       │           ┌────────────┴────────────┐            │                         │
       │           │NO:  Return default      │            │                         │
       │           │     provider (fallback) │            │                         │
       │           └────────────┬────────────┘            │                         │
       │                        │                         │                         │
       │  Arc<dyn LLMProvider>  │                         │                         │
       │<───────────────────────│                         │                         │
       │                        │                         │                         │
```

---

## 10. Critical Implementation Details

### 10.1 Error Handling

```rust
#[derive(Debug, Error)]
pub enum LATSError {
    #[error("LATS configuration invalid: {0}")]
    ConfigError(String),

    #[error("Provider not available for phase {phase:?}: {details}")]
    ProviderUnavailable { phase: LATSPhase, details: String },

    #[error("Search tree depth exceeded: max={max}, current={current}")]
    DepthExceeded { max: usize, current: usize },

    #[error("No solution found after {iterations} iterations")]
    NoSolution { iterations: usize },

    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Tool execution failed: {0}")]
    ToolError(String),

    #[error("LLM provider error: {0}")]
    LLMError(String),
}
```

### 10.2 State Management

- **Thread Safety:** `SearchTree` wrapped in `Arc<RwLock<SearchTree>>` for concurrent access
- **Provider Caching:** `ProviderRouter` caches provider instances per phase
- **Memory Management:** Implement tree pruning to prevent unbounded growth

### 10.3 Testing Strategy

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_lats_config_parsing() { /* ... */ }

    #[test]
    fn test_provider_router_fallback() { /* ... */ }

    #[test]
    fn test_search_tree_uct_scores() { /* ... */ }

    #[tokio::test]
    async fn test_lats_executor_single_provider() { /* ... */ }

    #[tokio::test]
    async fn test_lats_executor_multi_provider() { /* ... */ }
}
```

**Integration Tests:**
```bash
# Test ReAct (existing - should pass)
CODEGRAPH_AGENT_ARCHITECTURE=react cargo test --features autoagents-react

# Test LATS single-provider
CODEGRAPH_AGENT_ARCHITECTURE=lats cargo test --features autoagents-lats

# Test LATS multi-provider
export CODEGRAPH_LATS_SELECTION_PROVIDER=openai
export CODEGRAPH_LATS_EXPANSION_PROVIDER=anthropic
cargo test --features autoagents-lats test_multi_provider_lats
```

### 10.4 Performance Considerations

- **Latency:** LATS typically 3-5x slower than ReAct (more LLM calls)
- **Cost:** Multi-provider LATS reduces cost by using cheap models for selection/backprop
- **Caching:** Implement result caching for identical subproblems
- **Parallelization:** Expand multiple nodes in parallel where possible

### 10.5 Security & Observability

**Logging:**
```rust
tracing::info!(
    architecture = ?self.architecture,
    tier = ?self.tier,
    "LATS executor initialized"
);

tracing::debug!(
    phase = "selection",
    selected_nodes = selected.len(),
    "LATS selection phase completed"
);
```

**Metrics:**
- Total iterations
- Nodes expanded per iteration
- Average node score
- Provider call counts per phase
- Total execution time

---

## 11. Future Extensibility

### 11.1 Adding New Agent Architectures

To add Tree-of-Thoughts (ToT):

1. **Add to enum:**
```rust
pub enum AgentArchitecture {
    ReAct,
    LATS,
    ToT,  // NEW
}
```

2. **Implement executor:**
```rust
pub struct ToTExecutor { /* ... */ }

#[async_trait]
impl AgentExecutorTrait for ToTExecutor {
    async fn execute(&self, query: String, analysis_type: AnalysisType)
        -> Result<CodeGraphAgentOutput>
    {
        // ToT algorithm implementation
    }
}
```

3. **Register in factory:**
```rust
impl AgentExecutorFactory {
    pub fn create(&self, architecture: AgentArchitecture)
        -> Result<Box<dyn AgentExecutorTrait>>
    {
        match architecture {
            AgentArchitecture::ReAct => Ok(Box::new(ReActExecutor::new(...))),
            AgentArchitecture::LATS => Ok(Box::new(LATSExecutor::new(...))),
            AgentArchitecture::ToT => Ok(Box::new(ToTExecutor::new(...))),  // NEW
        }
    }
}
```

### 11.2 Hybrid Architectures

Future: Adaptive architecture selection based on query complexity:

```rust
pub enum AgentArchitecture {
    ReAct,
    LATS,
    ToT,
    Adaptive,  // Chooses best architecture per query
}

impl AdaptiveExecutor {
    async fn select_architecture(&self, query: &str) -> AgentArchitecture {
        // Use classifier model to predict optimal architecture
        // Simple queries -> ReAct
        // Complex queries -> LATS
        // Multi-step reasoning -> ToT
    }
}
```

---

## 12. Open Questions & Decisions Needed

1. **LATS Beam Width:** Default to 3 or make tier-dependent?
   - Recommendation: Start with 3, make configurable

2. **Evaluation Model:** Use reasoning model (o1) or standard model?
   - Recommendation: Reasoning model for better quality assessment

3. **Caching Strategy:** Cache at tool level or LATS node level?
   - Recommendation: Both - tool results + full node states

4. **Failure Handling:** What if a phase-specific provider is unavailable?
   - Recommendation: Graceful fallback to default provider

5. **Cost Controls:** How to prevent runaway LATS iterations?
   - Recommendation: Hard limits on depth + iteration count + timeout

---

## 13. Success Metrics

**Functional Requirements:**
- ✅ ReAct continues to work unchanged
- ✅ LATS executes with single provider
- ✅ LATS executes with multi-provider
- ✅ Architecture selection via environment variable
- ✅ All 7 MCP tools work with both architectures

**Quality Metrics:**
- LATS quality improvement: Target 15-25% better than ReAct
- LATS latency overhead: Acceptable if < 5x ReAct
- Cost reduction via multi-provider: Target 40-60% vs. single high-end model

**Operational Metrics:**
- No regressions in existing tests
- Clean separation of concerns
- Extensible for future architectures

---

## 14. References & Further Reading

1. **LATS Paper:** "Language Agent Tree Search Unifies Reasoning, Acting, and Planning in Language Models" (Zhou et al., 2024)
2. **AutoAgents Framework:** https://github.com/liquidos-ai/AutoAgents
3. **UCT Algorithm:** "Bandit Based Monte-Carlo Planning" (Kocsis & Szepesvári, 2006)
4. **CodeGraph Architecture:** CLAUDE.md in project root
5. **Tier-Aware Prompting:** `crates/codegraph-mcp-autoagents/src/autoagents/prompt_selector.rs`

---

## Appendix A: File Checklist

**New Files to Create:**
```
crates/codegraph-mcp-core/src/agent_architecture.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_agent_builder.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_executor.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_config.rs
crates/codegraph-mcp-autoagents/src/autoagents/provider_router.rs
crates/codegraph-mcp-autoagents/src/autoagents/react_executor.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_prompts/mod.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_prompts/selection_prompts.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_prompts/expansion_prompts.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_prompts/evaluation_prompts.rs
crates/codegraph-mcp-autoagents/src/autoagents/lats_prompts/backprop_prompts.rs
```

**Files to Modify:**
```
crates/codegraph-core/src/config_manager.rs (extend LLMConfig)
crates/codegraph-mcp-autoagents/src/autoagents/executor.rs (add trait abstraction)
crates/codegraph-mcp-autoagents/src/autoagents/mod.rs (export new modules)
crates/codegraph-mcp-autoagents/Cargo.toml (add feature flags)
```
