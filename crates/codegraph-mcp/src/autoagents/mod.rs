// ABOUTME: AutoAgents integration module for CodeGraph MCP server
// ABOUTME: Provides tier-aware agentic workflows with ReAct pattern execution

#[cfg(feature = "autoagents-experimental")]
pub mod tier_plugin;
#[cfg(feature = "autoagents-experimental")]
pub mod tools;
#[cfg(feature = "autoagents-experimental")]
pub mod agent_builder;
#[cfg(feature = "autoagents-experimental")]
pub mod executor;
#[cfg(feature = "autoagents-experimental")]
pub mod progress_notifier;
#[cfg(feature = "autoagents-experimental")]
pub mod codegraph_agent;

// Re-exports
#[cfg(feature = "autoagents-experimental")]
pub use tier_plugin::TierAwarePromptPlugin;
#[cfg(feature = "autoagents-experimental")]
pub use agent_builder::{CodeGraphChatAdapter, CodeGraphAgentBuilder, AgentHandle};
#[cfg(feature = "autoagents-experimental")]
pub use codegraph_agent::CodeGraphAgentOutput;
#[cfg(feature = "autoagents-experimental")]
pub use executor::{CodeGraphExecutor, CodeGraphExecutorBuilder, ExecutorError};
