// ABOUTME: Adapters bridging CodeGraph infrastructure to Rig framework
// ABOUTME: Environment variable based provider selection

mod llm_adapter;

pub use llm_adapter::{
    get_context_window, get_max_turns, get_model_name, RigLLMAdapter, RigProvider,
};
