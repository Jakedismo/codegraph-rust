// ABOUTME: Core MCP types and utilities shared by server, tools, and autoagents
// ABOUTME: Exposes protocol-facing data structures without server runtime

pub mod agent_architecture;
pub mod analysis;
pub mod config_manager;
pub mod context_aware_limits;
pub mod debug_logger;
pub mod error;
pub mod message;
pub mod process;
pub mod protocol;
pub mod version;

pub use agent_architecture::*;
pub use analysis::*;
pub use codegraph_core::{CodeGraphError, NodeId};
pub use config_manager::*;
pub use context_aware_limits::*;
pub use debug_logger::*;
pub use error::{McpError, Result};
pub use message::*;
pub use process::*;
pub use protocol::*;
pub use version::*;
