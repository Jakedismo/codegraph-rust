// ABOUTME: MCP server entry (stdio/http) using core, tools, and Rig agent crates
// ABOUTME: Thin runtime layer wiring transports and handlers

pub mod agent_cli;
pub mod agent_hooks;
pub mod agentic_schemas;
pub mod agentic_tools;
#[cfg(feature = "server-http")]
pub mod http_config;
#[cfg(feature = "server-http")]
pub mod http_server;
pub mod official_server;
pub mod project_init;
pub mod prompts;

pub use codegraph_mcp_core::analysis::AnalysisType;
#[cfg(feature = "server-http")]
pub use http_config::*;
#[cfg(feature = "server-http")]
pub use http_server::*;
pub use official_server::*;
