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
pub use tier_plugin::{TierAwarePromptPlugin, TierAwareAgentExt};
#[cfg(feature = "autoagents-experimental")]
pub use agent_builder::CodeGraphAgentBuilder;
#[cfg(feature = "autoagents-experimental")]
pub use executor::CodeGraphAgenticExecutor;
#[cfg(feature = "autoagents-experimental")]
pub use progress_notifier::McpProgressObserver;
