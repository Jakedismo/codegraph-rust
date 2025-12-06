// ABOUTME: Tier-aware system prompts for Rig agents
// ABOUTME: 4-tier prompt selection (Small, Medium, Large, Massive) based on context window

mod tier_prompts;

pub use codegraph_mcp_core::analysis::AnalysisType;
pub use tier_prompts::{detect_tier, get_max_turns, get_tier_system_prompt};
