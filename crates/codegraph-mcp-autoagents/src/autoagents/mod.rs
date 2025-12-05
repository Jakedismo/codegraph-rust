// ABOUTME: AutoAgents integration module for CodeGraph MCP server
// ABOUTME: Provides tier-aware agentic workflows with ReAct pattern execution

#[cfg(feature = "autoagents-experimental")]
pub mod agent_builder;
#[cfg(feature = "autoagents-experimental")]
pub mod agentic_api_surface_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod architecture_analysis_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod call_chain_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod code_search_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod codegraph_agent;
#[cfg(feature = "autoagents-experimental")]
pub mod complexity_analysis_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod context_builder_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod dependency_analysis_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod dependency_analysis_prompts_integration_example;
#[cfg(feature = "autoagents-experimental")]
pub mod executor;
#[cfg(feature = "autoagents-experimental")]
pub mod executor_factory;
#[cfg(feature = "autoagents-experimental")]
pub mod executor_trait;
#[cfg(all(feature = "autoagents-experimental", feature = "autoagents-lats"))]
pub mod lats;
#[cfg(feature = "autoagents-experimental")]
pub mod progress_notifier;
#[cfg(feature = "autoagents-experimental")]
pub mod prompt_selector;
#[cfg(feature = "autoagents-experimental")]
pub mod prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod react_executor;
#[cfg(feature = "autoagents-experimental")]
pub mod semantic_question_prompts;
#[cfg(feature = "autoagents-experimental")]
pub mod startup_context;
#[cfg(feature = "autoagents-experimental")]
pub mod tier_plugin;
#[cfg(feature = "autoagents-experimental")]
pub mod tools;

// Re-exports
#[cfg(feature = "autoagents-experimental")]
pub use agent_builder::{AgentHandle, CodeGraphAgentBuilder, CodeGraphChatAdapter};
#[cfg(feature = "autoagents-experimental")]
pub use codegraph_agent::CodeGraphAgentOutput;
#[cfg(feature = "autoagents-experimental")]
pub use executor::{
    is_context_overflow_error, transform_context_overflow, CodeGraphExecutor,
    CodeGraphExecutorBuilder, ExecutorError,
};
#[cfg(feature = "autoagents-experimental")]
pub use executor_factory::AgentExecutorFactory;
#[cfg(feature = "autoagents-experimental")]
pub use executor_trait::AgentExecutorTrait;
#[cfg(all(feature = "autoagents-experimental", feature = "autoagents-lats"))]
pub use lats::{
    LATSConfig, LATSExecutor, LATSPhase, LATSPrompts, NodeId, ProviderRouter, ProviderStats,
    SearchNode, SearchTree, SearchTreeError, TerminationReason, ToolAction,
};
#[cfg(feature = "autoagents-experimental")]
pub use progress_notifier::{ProgressCallback, ProgressNotifier, ProgressStage};
#[cfg(feature = "autoagents-experimental")]
pub use prompt_selector::{PromptSelector, PromptSelectorStats, PromptVerbosity};
#[cfg(feature = "autoagents-experimental")]
pub use react_executor::ReActExecutor;
#[cfg(feature = "autoagents-experimental")]
pub use startup_context::{build_startup_context, StartupContext, StartupContextRender};
#[cfg(feature = "autoagents-experimental")]
pub use tier_plugin::TierAwarePromptPlugin;
