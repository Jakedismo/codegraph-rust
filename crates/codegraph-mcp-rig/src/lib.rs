// ABOUTME: Rig-based agent backend for CodeGraph MCP server
// ABOUTME: Alternative to AutoAgents using the Rig framework for LLM orchestration

pub mod adapter;
pub mod agent;
pub mod prompts;
pub mod tools;

// Re-exports for convenience
pub use agent::builder::RigAgentBuilder;
pub use agent::executor::RigExecutor;
pub use agent::RigAgentOutput;
pub use tools::ToolTrace;
