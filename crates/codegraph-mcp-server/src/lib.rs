// ABOUTME: MCP server entry (stdio/http) using core, tools, and autoagents crates
// ABOUTME: Thin runtime layer wiring transports and handlers

pub mod official_server;
#[cfg(feature = "server-http")]
pub mod http_config;
#[cfg(feature = "server-http")]
pub mod http_server;
pub mod prompt_selector;
pub mod prompts;
pub mod agentic_api_surface_prompts;
pub mod architecture_analysis_prompts;
pub mod call_chain_prompts;
pub mod code_search_prompts;
pub mod context_builder_prompts;
pub mod dependency_analysis_prompts;
pub mod dependency_analysis_prompts_integration_example;
pub mod semantic_question_prompts;

pub use official_server::*;
#[cfg(feature = "server-http")]
pub use http_config::*;
#[cfg(feature = "server-http")]
pub use http_server::*;
pub use prompt_selector::*;
