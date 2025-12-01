// ABOUTME: Daemon/watch functionality separated from MCP server runtime
// ABOUTME: Provides daemon types and utilities used by server when enabled

pub mod daemon;

pub use daemon::*;
pub use codegraph_mcp::{IndexerConfig, ProjectIndexer};
