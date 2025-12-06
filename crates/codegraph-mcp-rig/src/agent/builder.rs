// ABOUTME: Rig agent builder with tier-aware configuration
// ABOUTME: Builds agents with graph tools and appropriate system prompts

#[allow(unused_imports)]
use crate::adapter::{get_context_window, get_max_turns, RigLLMAdapter, RigProvider};
use crate::prompts::{get_tier_system_prompt, AnalysisType};
use anyhow::{anyhow, Result};
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_tools::GraphToolExecutor;
use std::sync::Arc;

/// Builder for creating Rig-based code analysis agents
pub struct RigAgentBuilder {
    #[allow(dead_code)] // Used when provider features are enabled
    executor: Arc<GraphToolExecutor>,
    analysis_type: AnalysisType,
    tier: ContextTier,
    max_turns: usize,
}

impl RigAgentBuilder {
    /// Create a new agent builder
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        let context_window = get_context_window();
        let tier = ContextTier::from_context_window(context_window);
        let max_turns = get_max_turns();

        Self {
            executor,
            analysis_type: AnalysisType::SemanticQuestion,
            tier,
            max_turns,
        }
    }

    /// Set the analysis type for this agent
    pub fn analysis_type(mut self, analysis_type: AnalysisType) -> Self {
        self.analysis_type = analysis_type;
        self
    }

    /// Override the context tier
    pub fn tier(mut self, tier: ContextTier) -> Self {
        self.tier = tier;
        self
    }

    /// Override the max turns for tool loop
    pub fn max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Get the configured tier
    pub fn get_tier(&self) -> ContextTier {
        self.tier
    }

    /// Get the configured max turns
    pub fn get_max_turns(&self) -> usize {
        self.max_turns
    }

    /// Get the system prompt for the current configuration
    pub fn system_prompt(&self) -> String {
        get_tier_system_prompt(self.analysis_type, self.tier)
    }

    /// Build an OpenAI-based agent
    #[cfg(feature = "openai")]
    pub fn build_openai(self) -> Result<OpenAIAgent> {
        let client = RigLLMAdapter::openai_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let factory = GraphToolFactory::new(self.executor);

        info!(
            provider = "openai",
            model = %model,
            tier = ?self.tier,
            max_turns = self.max_turns,
            analysis_type = ?self.analysis_type,
            "Building Rig agent"
        );

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(self.tier.max_output_tokens())
            .tool(factory.transitive_dependencies())
            .tool(factory.circular_dependencies())
            .tool(factory.call_chain())
            .tool(factory.coupling_metrics())
            .tool(factory.hub_nodes())
            .tool(factory.reverse_dependencies())
            .tool(factory.semantic_search())
            .tool(factory.complexity_hotspots())
            .build();

        Ok(OpenAIAgent {
            agent,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    /// Build an Anthropic-based agent
    #[cfg(feature = "anthropic")]
    pub fn build_anthropic(self) -> Result<AnthropicAgent> {
        let client = RigLLMAdapter::anthropic_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let factory = GraphToolFactory::new(self.executor);

        info!(
            provider = "anthropic",
            model = %model,
            tier = ?self.tier,
            max_turns = self.max_turns,
            analysis_type = ?self.analysis_type,
            "Building Rig agent"
        );

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(self.tier.max_output_tokens())
            .tool(factory.transitive_dependencies())
            .tool(factory.circular_dependencies())
            .tool(factory.call_chain())
            .tool(factory.coupling_metrics())
            .tool(factory.hub_nodes())
            .tool(factory.reverse_dependencies())
            .tool(factory.semantic_search())
            .tool(factory.complexity_hotspots())
            .build();

        Ok(AnthropicAgent {
            agent,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    /// Build an Ollama-based agent
    #[cfg(feature = "ollama")]
    pub fn build_ollama(self) -> Result<OllamaAgent> {
        let client = RigLLMAdapter::ollama_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let factory = GraphToolFactory::new(self.executor);

        info!(
            provider = "ollama",
            model = %model,
            tier = ?self.tier,
            max_turns = self.max_turns,
            analysis_type = ?self.analysis_type,
            "Building Rig agent"
        );

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .tool(factory.transitive_dependencies())
            .tool(factory.circular_dependencies())
            .tool(factory.call_chain())
            .tool(factory.coupling_metrics())
            .tool(factory.hub_nodes())
            .tool(factory.reverse_dependencies())
            .tool(factory.semantic_search())
            .tool(factory.complexity_hotspots())
            .build();

        Ok(OllamaAgent {
            agent,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    /// Build agent for the detected provider
    pub fn build(self) -> Result<Box<dyn RigAgentTrait>> {
        let provider = RigLLMAdapter::provider()?;

        match provider {
            #[cfg(feature = "openai")]
            RigProvider::OpenAI => Ok(Box::new(self.build_openai()?)),
            #[cfg(feature = "anthropic")]
            RigProvider::Anthropic => Ok(Box::new(self.build_anthropic()?)),
            #[cfg(feature = "ollama")]
            RigProvider::Ollama => Ok(Box::new(self.build_ollama()?)),
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!("Provider {:?} not enabled in build features", provider)),
        }
    }
}

/// Trait for unified agent interface
#[async_trait::async_trait]
pub trait RigAgentTrait: Send + Sync {
    /// Execute the agent with the given query
    async fn execute(&self, query: &str) -> Result<String>;

    /// Get the configured tier
    fn tier(&self) -> ContextTier;

    /// Get max turns
    fn max_turns(&self) -> usize;
}

/// OpenAI-based Rig agent
#[cfg(feature = "openai")]
pub struct OpenAIAgent {
    agent: rig_core::agent::Agent<rig_core::providers::openai::CompletionModel>,
    max_turns: usize,
    tier: ContextTier,
}

#[cfg(feature = "openai")]
#[async_trait::async_trait]
impl RigAgentTrait for OpenAIAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig_core::completion::Chat;

        let response = self
            .agent
            .chat(query, vec![])
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }
}

/// Anthropic-based Rig agent
#[cfg(feature = "anthropic")]
pub struct AnthropicAgent {
    agent: rig_core::agent::Agent<rig_core::providers::anthropic::CompletionModel>,
    max_turns: usize,
    tier: ContextTier,
}

#[cfg(feature = "anthropic")]
#[async_trait::async_trait]
impl RigAgentTrait for AnthropicAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig_core::completion::Chat;

        let response = self
            .agent
            .chat(query, vec![])
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }
}

/// Ollama-based Rig agent
#[cfg(feature = "ollama")]
pub struct OllamaAgent {
    agent: rig_core::agent::Agent<rig_core::providers::ollama::CompletionModel>,
    max_turns: usize,
    tier: ContextTier,
}

#[cfg(feature = "ollama")]
#[async_trait::async_trait]
impl RigAgentTrait for OllamaAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig_core::completion::Chat;

        let response = self
            .agent
            .chat(query, vec![])
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_configuration() {
        // We can't fully test without GraphToolExecutor, but we can test the builder API
        fn _assert_builder_api() {
            // This just verifies the API compiles correctly
            let _: fn(Arc<GraphToolExecutor>) -> RigAgentBuilder = RigAgentBuilder::new;
        }
    }
}
