#[cfg(feature = "ai-enhanced")]
pub mod agentic_api_surface_prompts;
#[cfg(feature = "ai-enhanced")]
pub mod agentic_orchestrator;
#[cfg(feature = "ai-enhanced")]
pub mod architecture_analysis_prompts;
#[cfg(feature = "ai-enhanced")]
pub mod call_chain_prompts;
#[cfg(feature = "ai-enhanced")]
pub mod code_search_prompts;
pub mod connection;
pub mod context_aware_limits;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "ai-enhanced")]
pub mod context_builder_prompts;
#[cfg(feature = "ai-enhanced")]
pub mod dependency_analysis_prompts;
pub mod error;
pub mod estimation;
pub mod graph_tool_executor;
pub mod graph_tool_schemas;
pub mod heartbeat;
pub mod indexer;
pub mod message;
pub mod official_server;
pub mod process;
pub mod prompt_selector;
pub mod protocol;
#[cfg(feature = "ai-enhanced")]
pub mod semantic_question_prompts;
pub mod transport;
pub mod version;

#[cfg(feature = "server-http")]
pub mod http_config;
#[cfg(feature = "server-http")]
pub mod http_server;

#[cfg(feature = "autoagents-experimental")]
pub mod autoagents;

pub mod config_manager;
pub mod prompts;

#[cfg(feature = "ai-enhanced")]
pub use agentic_orchestrator::{
    AgenticConfig, AgenticOrchestrator, AgenticResult, ReasoningStep, ToolCallStats,
};
pub use connection::*;
pub use context_aware_limits::{ContextAwareLimits, ContextTier};
pub use error::{McpError, Result};
pub use estimation::{
    build_symbol_index, EmbeddingThroughputConfig, RepositoryEstimate, RepositoryEstimator,
    TimeEstimates,
};
pub use graph_tool_executor::{CacheStats, GraphToolExecutor};
pub use graph_tool_schemas::{GraphToolSchemas, ToolSchema};
pub use heartbeat::*;
pub use indexer::{IndexStats, IndexerConfig, ProjectIndexer};
pub use message::*;
pub use process::{ProcessInfo, ProcessManager, ProcessStatus};
pub use prompt_selector::{AnalysisType, PromptSelector, PromptSelectorStats, PromptVerbosity};
pub use protocol::*;
pub use transport::*;
pub use version::*;

// Daemon mode exports
#[cfg(feature = "daemon")]
pub use daemon::{
    BackoffConfig, CircuitBreakerConfig, CircuitState, DaemonState, DaemonStatus, HealthMonitor,
    PidFile, SessionMetrics, WatchConfig, WatchDaemon, WatchSession,
};

#[cfg(feature = "qwen-integration")]
pub use qwen::{QwenClient, QwenConfig, QwenResult};
