// ABOUTME: Core MCP types and utilities shared by server, tools, and autoagents
// ABOUTME: Exposes protocol-facing data structures without server runtime

pub mod error;
pub mod context_aware_limits;
pub mod process;
pub mod protocol;
pub mod version;
pub mod message;
pub mod config_manager;
pub mod analysis;
pub mod debug_logger;

pub use codegraph_core::{CodeGraphError, NodeId};
pub use context_aware_limits::*;
pub use process::*;
pub use protocol::*;
pub use message::*;
pub use config_manager::*;
pub use error::{McpError, Result};
pub use version::*;
pub use debug_logger::*;
pub use analysis::*;
