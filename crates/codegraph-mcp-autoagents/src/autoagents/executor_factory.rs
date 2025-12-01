// ABOUTME: Factory for creating architecture-specific agent executors
// ABOUTME: Supports runtime selection via CODEGRAPH_AGENT_ARCHITECTURE

use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_tools::GraphToolExecutor;
use codegraph_ai::llm_provider::LLMProvider;
use crate::autoagents::executor_trait::AgentExecutorTrait;
use crate::autoagents::executor::ExecutorError;
use crate::autoagents::react_executor::ReActExecutor;
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
}

impl AgentExecutorFactory {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
        }
    }

    /// Create an executor for the specified architecture
    pub fn create(
        &self,
        architecture: AgentArchitecture,
    ) -> Result<Box<dyn AgentExecutorTrait>, ExecutorError> {
        match architecture {
            AgentArchitecture::ReAct => {
                Ok(Box::new(ReActExecutor::new(
                    self.llm_provider.clone(),
                    self.tool_executor.clone(),
                )))
            }
            AgentArchitecture::LATS => {
                // Placeholder for Phase 2 implementation
                Err(ExecutorError::BuildFailed(
                    "LATS architecture not yet implemented. Use 'react' architecture or unset CODEGRAPH_AGENT_ARCHITECTURE.".to_string()
                ))
            }
        }
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

    #[test]
    fn test_lats_not_implemented() {
        // LATS should return an error until Phase 2
        // We can't test this without mocking the providers,
        // but we can verify the architecture enum
        assert_eq!(
            format!("{}", AgentArchitecture::LATS),
            "lats"
        );
    }
}
