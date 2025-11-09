// ABOUTME: AutoAgents tool definitions for SurrealDB graph analysis
// ABOUTME: Type-safe tool wrappers with derive macros replacing manual JSON schemas

pub mod graph_tools;
pub mod tool_executor_adapter;

pub use graph_tools::*;
pub use tool_executor_adapter::{GraphToolExecutorAdapter, GraphToolFactory};
