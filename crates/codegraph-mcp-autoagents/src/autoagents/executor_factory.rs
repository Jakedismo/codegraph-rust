// ABOUTME: Factory for creating architecture-specific agent executors
// ABOUTME: Supports runtime selection via CODEGRAPH_AGENT_ARCHITECTURE

use crate::autoagents::executor::ExecutorError;
use crate::autoagents::executor_trait::AgentExecutorTrait;
use crate::autoagents::react_executor::ReActExecutor;
use codegraph_ai::llm_provider::LLMProvider;
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::config_manager::CodeGraphConfig;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_tools::GraphToolExecutor;
use std::sync::Arc;

/// Factory for creating architecture-specific agent executors
///
/// This factory enables runtime selection of agent architectures based on
/// configuration or environment variables. Currently supports:
/// - ReAct (production-ready)
/// - LATS (placeholder for Phase 2)
pub struct AgentExecutorFactory {
    llm_provider: Arc<dyn LLMProvider>,
    tool_executor: Arc<GraphToolExecutor>,
    // Reserved for Phase 2 LATS implementation
    #[allow(dead_code)]
    config: Arc<CodeGraphConfig>,
}

impl AgentExecutorFactory {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        config: Arc<CodeGraphConfig>,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            config,
        }
    }

    /// Create an executor for the specified architecture
    pub fn create(
        &self,
        architecture: AgentArchitecture,
    ) -> Result<Box<dyn AgentExecutorTrait>, ExecutorError> {
        // Detect tier from LLM configuration
        let tier = self.detect_tier();

        match architecture {
            AgentArchitecture::ReAct => Ok(Box::new(ReActExecutor::new(
                self.llm_provider.clone(),
                self.tool_executor.clone(),
                tier,
            ))),
            #[cfg(feature = "autoagents-lats")]
            AgentArchitecture::LATS => {
                use crate::autoagents::lats::executor::LATSExecutor;

                Ok(Box::new(LATSExecutor::new(
                    self.config.clone(),
                    self.llm_provider.clone(),
                    self.tool_executor.clone(),
                    tier,
                )))
            }
            #[cfg(not(feature = "autoagents-lats"))]
            AgentArchitecture::LATS => Err(ExecutorError::BuildFailed(
                "LATS requires 'autoagents-lats' feature. Rebuild with --features autoagents-lats"
                    .to_string(),
            )),
        }
    }

    /// Detect ContextTier from LLM provider configuration
    fn detect_tier(&self) -> ContextTier {
        // Get context window from config if available
        // For Phase 1, default to Medium tier
        // TODO: In Phase 2, extract context window from LLM config
        // and use ContextTier::from_context_window()
        ContextTier::Medium
    }

    /// Detect architecture from environment or use default
    pub fn detect_architecture() -> AgentArchitecture {
        // 1. Check environment variable (highest priority)
        if let Ok(arch_str) = std::env::var("CODEGRAPH_AGENT_ARCHITECTURE") {
            if let Some(arch) = AgentArchitecture::parse(&arch_str) {
                tracing::info!(
                    architecture = %arch,
                    "Detected agent architecture from CODEGRAPH_AGENT_ARCHITECTURE"
                );
                return arch;
            } else {
                tracing::warn!(
                    value = %arch_str,
                    "Invalid CODEGRAPH_AGENT_ARCHITECTURE value, falling back to ReAct"
                );
            }
        }

        // 2. Default to ReAct
        tracing::debug!("Using default ReAct architecture");
        AgentArchitecture::ReAct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_architecture_default() {
        // Should default to ReAct when env var not set
        std::env::remove_var("CODEGRAPH_AGENT_ARCHITECTURE");
        let arch = AgentExecutorFactory::detect_architecture();
        assert_eq!(arch, AgentArchitecture::ReAct);
    }

    #[test]
    fn test_detect_architecture_from_env() {
        // Should parse from environment variable
        std::env::set_var("CODEGRAPH_AGENT_ARCHITECTURE", "lats");
        let arch = AgentExecutorFactory::detect_architecture();
        assert_eq!(arch, AgentArchitecture::LATS);

        // Cleanup
        std::env::remove_var("CODEGRAPH_AGENT_ARCHITECTURE");
    }

    #[test]
    fn test_detect_architecture_invalid_env() {
        // Should fall back to ReAct on invalid value
        std::env::set_var("CODEGRAPH_AGENT_ARCHITECTURE", "invalid");
        let arch = AgentExecutorFactory::detect_architecture();
        assert_eq!(arch, AgentArchitecture::ReAct);

        // Cleanup
        std::env::remove_var("CODEGRAPH_AGENT_ARCHITECTURE");
    }

    #[cfg(feature = "autoagents-lats")]
    #[test]
    fn test_lats_architecture_enum() {
        // Verify LATS architecture enum format
        assert_eq!(format!("{}", AgentArchitecture::LATS), "lats");
    }
}
