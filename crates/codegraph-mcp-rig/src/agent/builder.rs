// ABOUTME: Rig agent builder with tier-aware configuration
// ABOUTME: Builds agents with graph tools and appropriate system prompts

#[allow(unused_imports)]
use crate::adapter::{get_context_window, get_model_name, RigLLMAdapter, RigProvider};
use crate::agent::api::RigAgentTrait;
#[allow(unused_imports)]
use crate::agent::lats::LatsAgent;
#[cfg(feature = "anthropic")]
use crate::agent::react::AnthropicAgent;
#[cfg(feature = "ollama")]
use crate::agent::react::OllamaAgent;
#[cfg(feature = "openai")]
use crate::agent::react::OpenAIAgent;
#[cfg(feature = "xai")]
use crate::agent::react::XAIAgent;
#[allow(unused_imports)]
use crate::agent::reflexion::ReflexionAgent;
use crate::prompts::{get_max_turns, get_tier_system_prompt, AnalysisType};
#[allow(unused_imports)] // Used when provider features are enabled
use crate::tools::GraphToolFactory;
use anyhow::{anyhow, Result};
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_tools::GraphToolExecutor;
#[allow(unused_imports)] // Used when provider features are enabled
use rig::client::CompletionClient;
use std::sync::Arc;
#[allow(unused_imports)] // Used when provider features are enabled
use tracing::info;

/// Builder for creating Rig-based code analysis agents
pub struct RigAgentBuilder {
    #[allow(dead_code)] // Used when provider features are enabled
    executor: Arc<GraphToolExecutor>,
    analysis_type: AnalysisType,
    tier: ContextTier,
    max_turns: usize,
    architecture: Option<AgentArchitecture>,
    #[allow(dead_code)]
    response_format: Option<serde_json::Value>,
}

impl RigAgentBuilder {
    /// Create a new agent builder
    /// Uses tier-aware max_turns based on context window size
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        let context_window = get_context_window();
        let tier = ContextTier::from_context_window(context_window);
        let max_turns = get_max_turns(tier);
        // Check env var immediately, but store as Option
        let architecture = Self::detect_architecture_from_env();

        Self {
            executor,
            analysis_type: AnalysisType::SemanticQuestion,
            tier,
            max_turns,
            architecture,
            response_format: None,
        }
    }

    /// Set required response format (JSON schema)
    pub fn response_format(mut self, schema: serde_json::Value) -> Self {
        self.response_format = Some(schema);
        self
    }

    /// Detect architecture from environment only
    fn detect_architecture_from_env() -> Option<AgentArchitecture> {
        if let Ok(arch_str) = std::env::var("CODEGRAPH_AGENT_ARCHITECTURE") {
            if let Some(arch) = AgentArchitecture::parse(&arch_str) {
                return Some(arch);
            }
        }
        None
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

    /// Override the architecture
    pub fn architecture(mut self, architecture: AgentArchitecture) -> Self {
        self.architecture = Some(architecture);
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

    /// Get max output tokens, respecting MCP_CODE_AGENT_MAX_OUTPUT_TOKENS env var
    /// This ensures the LLM produces answers within Claude Code's limits
    #[allow(dead_code)]
    fn get_max_output_tokens(&self) -> u64 {
        // Check for environment variable override first
        if let Ok(val) = std::env::var("MCP_CODE_AGENT_MAX_OUTPUT_TOKENS") {
            if let Ok(tokens) = val.parse::<u64>() {
                tracing::info!(
                    "Rig agent using MCP_CODE_AGENT_MAX_OUTPUT_TOKENS={}",
                    tokens
                );
                return tokens;
            }
        }

        // Fall back to tier-based defaults
        self.tier.max_output_tokens()
    }

    /// Resolve architecture using heuristic if not explicitly set
    fn resolve_architecture(&self) -> AgentArchitecture {
        if let Some(arch) = self.architecture {
            return arch;
        }

        // Heuristic: Use LATS for complex/deep analysis types
        match self.analysis_type {
            AnalysisType::ArchitectureAnalysis |
            AnalysisType::ComplexityAnalysis |
            AnalysisType::SemanticQuestion => {
                info!("Selecting LATS architecture for complex analysis: {:?}", self.analysis_type);
                AgentArchitecture::LATS
            },
            _ => AgentArchitecture::ReAct,
        }
    }

    /// Build agent for the detected provider and architecture
    pub fn build(self) -> Result<Box<dyn RigAgentTrait>> {
        let provider = RigLLMAdapter::provider()?;
        let architecture = self.resolve_architecture();

        match architecture {
            AgentArchitecture::ReAct | AgentArchitecture::Rig => self.build_react(provider),
            AgentArchitecture::LATS => self.build_lats(provider),
            AgentArchitecture::Reflexion => self.build_reflexion(provider),
        }
    }

    fn build_react(self, provider: RigProvider) -> Result<Box<dyn RigAgentTrait>> {
        match provider {
            #[cfg(feature = "openai")]
            RigProvider::OpenAI => Ok(Box::new(self.build_openai_react()?)),
            #[cfg(feature = "anthropic")]
            RigProvider::Anthropic => Ok(Box::new(self.build_anthropic_react()?)),
            #[cfg(feature = "ollama")]
            RigProvider::Ollama => Ok(Box::new(self.build_ollama_react()?)),
            #[cfg(feature = "xai")]
            RigProvider::XAI => Ok(Box::new(self.build_xai_react()?)),
            #[cfg(feature = "openai")]
            RigProvider::LMStudio => Ok(Box::new(self.build_lmstudio_react()?)),
            #[cfg(feature = "openai")]
            RigProvider::OpenAICompatible { ref base_url } => {
                Ok(Box::new(self.build_openai_compatible_react(base_url)?))
            }
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!(
                "Provider {:?} not enabled in build features",
                provider
            )),
        }
    }

    fn build_lats(self, provider: RigProvider) -> Result<Box<dyn RigAgentTrait>> {
        let model_name = get_model_name();
        let factory = GraphToolFactory::new(self.executor.clone());

        match provider {
            #[cfg(feature = "openai")]
            RigProvider::OpenAI => {
                let client = RigLLMAdapter::openai_client();
                let model = client.completion_model(&model_name);
                Ok(Box::new(LatsAgent {
                    model,
                    factory,
                    max_turns: self.max_turns,
                    tier: self.tier,
                }))
            }
            #[cfg(feature = "anthropic")]
            RigProvider::Anthropic => {
                let client = RigLLMAdapter::anthropic_client();
                let model = client.completion_model(&model_name);
                Ok(Box::new(LatsAgent {
                    model,
                    factory,
                    max_turns: self.max_turns,
                    tier: self.tier,
                }))
            }
            // Add other providers as needed, mostly mimicking the above pattern
             #[allow(unreachable_patterns)]
            _ => {
                let _ = model_name;
                let _ = factory;
                Err(anyhow!("LATS not yet supported for provider {:?}", provider))
            },
        }
    }

    fn build_reflexion(self, provider: RigProvider) -> Result<Box<dyn RigAgentTrait>> {
        // Reflexion wraps a ReAct agent
        let inner = self.build_react(provider)?;
        Ok(Box::new(ReflexionAgent {
            inner,
            max_retries: 2, // Default to 2 retries
        }))
    }

    // --- ReAct Builders (Internal) ---

    #[cfg(feature = "openai")]
    fn build_openai_react(self) -> Result<OpenAIAgent> {
        let client = RigLLMAdapter::openai_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let max_output_tokens = self.get_max_output_tokens();
        let factory = GraphToolFactory::new(self.executor);

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(max_output_tokens)
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
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    #[cfg(feature = "anthropic")]
    fn build_anthropic_react(self) -> Result<AnthropicAgent> {
        let client = RigLLMAdapter::anthropic_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let max_output_tokens = self.get_max_output_tokens();
        let factory = GraphToolFactory::new(self.executor);

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(max_output_tokens)
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
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    #[cfg(feature = "ollama")]
    fn build_ollama_react(self) -> Result<OllamaAgent> {
        let client = RigLLMAdapter::ollama_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let factory = GraphToolFactory::new(self.executor);

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
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    #[cfg(feature = "xai")]
    fn build_xai_react(self) -> Result<XAIAgent> {
        let client = RigLLMAdapter::xai_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let max_output_tokens = self.get_max_output_tokens();
        let factory = GraphToolFactory::new(self.executor);

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(max_output_tokens)
            .tool(factory.transitive_dependencies())
            .tool(factory.circular_dependencies())
            .tool(factory.call_chain())
            .tool(factory.coupling_metrics())
            .tool(factory.hub_nodes())
            .tool(factory.reverse_dependencies())
            .tool(factory.semantic_search())
            .tool(factory.complexity_hotspots())
            .build();

        Ok(XAIAgent {
            agent,
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    #[cfg(feature = "openai")]
    fn build_lmstudio_react(self) -> Result<OpenAIAgent> {
        let client = RigLLMAdapter::lmstudio_client();
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let max_output_tokens = self.get_max_output_tokens();
        let factory = GraphToolFactory::new(self.executor);

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(max_output_tokens)
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
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }

    #[cfg(feature = "openai")]
    fn build_openai_compatible_react(self, base_url: &str) -> Result<OpenAIAgent> {
        let client = RigLLMAdapter::openai_compatible_client(base_url);
        let model = get_model_name();
        let system_prompt = self.system_prompt();
        let max_output_tokens = self.get_max_output_tokens();
        let factory = GraphToolFactory::new(self.executor);

        let agent = client
            .agent(&model)
            .preamble(&system_prompt)
            .max_tokens(max_output_tokens)
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
            factory,
            max_turns: self.max_turns,
            tier: self.tier,
        })
    }
}