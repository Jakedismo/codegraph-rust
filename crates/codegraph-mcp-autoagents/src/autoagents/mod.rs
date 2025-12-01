// ABOUTME: AutoAgents integration module for CodeGraph MCP server
// ABOUTME: Provides tier-aware agentic workflows with ReAct pattern execution

#[cfg(feature = "autoagents-experimental")]
pub mod agent_builder;
#[cfg(feature = "autoagents-experimental")]
pub mod codegraph_agent;
#[cfg(feature = "autoagents-experimental")]
pub mod executor;
#[cfg(feature = "autoagents-experimental")]
pub mod executor_trait;
#[cfg(feature = "autoagents-experimental")]
pub mod react_executor;
#[cfg(feature = "autoagents-experimental")]
pub mod executor_factory;
#[cfg(feature = "autoagents-experimental")]
pub mod progress_notifier;
#[cfg(feature = "autoagents-experimental")]
pub mod tier_plugin;
#[cfg(feature = "autoagents-experimental")]
pub mod tools;
#[cfg(feature = "autoagents-experimental")]
pub mod prompt_selector;
#[cfg(feature = "autoagents-experimental")]
pub mod prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod agentic_api_surface_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod architecture_analysis_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod call_chain_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod code_search_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod context_builder_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod dependency_analysis_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod dependency_analysis_prompts_integration_example;
#[cfg(feature = "autoagents-experimental")]
pub mod semantic_question_prompts;

// Re-exports
#[cfg(feature = "autoagents-experimental")]
pub use agent_builder::{AgentHandle, CodeGraphAgentBuilder, CodeGraphChatAdapter};
#[cfg(feature = "autoagents-experimental")]
pub use codegraph_agent::CodeGraphAgentOutput;
#[cfg(feature = "autoagents-experimental")]
pub use executor::{CodeGraphExecutor, CodeGraphExecutorBuilder, ExecutorError};
#[cfg(feature = "autoagents-experimental")]
pub use executor_trait::AgentExecutorTrait;
#[cfg(feature = "autoagents-experimental")]
pub use react_executor::ReActExecutor;
#[cfg(feature = "autoagents-experimental")]
pub use executor_factory::AgentExecutorFactory;
#[cfg(feature = "autoagents-experimental")]
pub use tier_plugin::TierAwarePromptPlugin;
#[cfg(feature = "autoagents-experimental")]
pub use prompt_selector::{PromptSelector, PromptSelectorStats, PromptVerbosity};
