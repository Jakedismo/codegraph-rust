// ABOUTME: Rig agent construction and execution
// ABOUTME: Tier-aware agent builder and executor with conversation memory

pub mod api;
pub mod builder;
pub mod executor;
pub mod lats;
pub mod react;
pub mod reflexion;

pub use api::{AgentEvent, RigAgentTrait};
pub use builder::RigAgentBuilder;
pub use executor::{ConversationTurn, RigExecutor};

use serde::{Deserialize, Serialize};

/// Output from Rig agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigAgentOutput {
    /// Final response from the agent
    pub response: String,
    /// Number of tool calls made during execution
    pub tool_calls: usize,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Tool call trace for deterministic post-processing
    pub tool_traces: Vec<crate::tools::ToolTrace>,
}
