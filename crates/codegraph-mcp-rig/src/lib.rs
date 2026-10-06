// ABOUTME: Rig-based agent backend for CodeGraph MCP server
// ABOUTME: Runs ReAct, LATS, and Reflexion agents over the graph tools using the Rig framework

pub mod adapter;
pub mod agent;
pub mod prompts;
pub mod tools;

// Re-exports for convenience
pub use agent::RigAgentOutput;
pub use agent::builder::RigAgentBuilder;
pub use agent::executor::RigExecutor;
pub use tools::ToolTrace;
