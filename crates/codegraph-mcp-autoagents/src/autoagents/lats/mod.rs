// ABOUTME: LATS (Language Agent Tree Search) algorithm implementation
// ABOUTME: Provides tree search, provider routing, prompts, and executor for LATS execution

pub mod executor;
pub mod prompts;
pub mod provider_router;
pub mod search_tree;
pub mod tiered_prompts;

pub use executor::{LATSConfig, LATSExecutor, TerminationReason};
pub use prompts::LATSPrompts;
pub use provider_router::{LATSPhase, ProviderRouter, ProviderStats};
pub use search_tree::{NodeId, SearchNode, SearchTree, SearchTreeError, ToolAction};
pub use tiered_prompts::{build_expansion_prompt, build_synthesis_prompt, max_actions_for_tier};
