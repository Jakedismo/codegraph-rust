// ABOUTME: LATS (Language Agent Tree Search) algorithm implementation
// ABOUTME: Provides tree search, provider routing, and prompts for LATS execution

pub mod search_tree;
pub mod provider_router;

pub use search_tree::{SearchTree, SearchNode, NodeId, ToolAction, SearchTreeError};
pub use provider_router::{ProviderRouter, LATSPhase, ProviderStats};
