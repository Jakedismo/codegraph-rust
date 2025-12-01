pub mod connection;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod estimation;
pub mod heartbeat;
pub mod indexer;
pub mod transport;

pub use connection::*;
pub use codegraph_mcp_core::{
    context_aware_limits::{ContextAwareLimits, ContextTier},
    error::{McpError, Result},
    message::*,
    process::{ProcessInfo, ProcessManager, ProcessStatus},
    protocol::*,
    version::*,
    config_manager::*,
    debug_logger::*,
};
pub use estimation::{
    build_symbol_index, EmbeddingThroughputConfig, RepositoryEstimate, RepositoryEstimator,
    TimeEstimates,
};
#[cfg(feature = "embeddings")]
pub use codegraph_mcp_tools::{CacheStats, GraphToolExecutor, GraphToolSchemas, ToolSchema};
pub use heartbeat::*;
pub use indexer::{IndexStats, IndexerConfig, ProjectIndexer};
pub use transport::*;

// Re-export server modules to preserve existing paths
pub mod official_server {
    pub use codegraph_mcp_server::official_server::*;
}
#[cfg(feature = "server-http")]
pub mod http_server {
    pub use codegraph_mcp_server::http_server::*;
}
#[cfg(feature = "server-http")]
pub mod http_config {
    pub use codegraph_mcp_server::http_config::*;
}
pub mod prompt_selector {
    pub use codegraph_mcp_server::prompt_selector::*;
}
pub mod prompts {
    pub use codegraph_mcp_server::prompts::*;
}
pub mod agentic_api_surface_prompts {
    pub use codegraph_mcp_server::agentic_api_surface_prompts::*;
}
pub mod architecture_analysis_prompts {
    pub use codegraph_mcp_server::architecture_analysis_prompts::*;
}
pub mod call_chain_prompts {
    pub use codegraph_mcp_server::call_chain_prompts::*;
}
pub mod code_search_prompts {
    pub use codegraph_mcp_server::code_search_prompts::*;
}
pub mod context_builder_prompts {
    pub use codegraph_mcp_server::context_builder_prompts::*;
}
pub mod dependency_analysis_prompts {
    pub use codegraph_mcp_server::dependency_analysis_prompts::*;
}
pub mod dependency_analysis_prompts_integration_example {
    pub use codegraph_mcp_server::dependency_analysis_prompts_integration_example::*;
}
pub mod semantic_question_prompts {
    pub use codegraph_mcp_server::semantic_question_prompts::*;
}

// Daemon mode exports
#[cfg(feature = "daemon")]
pub use daemon::{
    BackoffConfig, CircuitBreakerConfig, CircuitState, DaemonState, DaemonStatus, HealthMonitor,
    PidFile, SessionMetrics, WatchConfig, WatchDaemon, WatchSession,
};
