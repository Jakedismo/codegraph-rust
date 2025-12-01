pub mod connection;
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

// Server modules have moved to codegraph-mcp-server; no re-exports to avoid dependency cycles.
