pub mod ai_context_enhancement;
pub mod ai_pattern_learning;
#[cfg(feature = "experimental")]
pub mod diff;
pub mod edge;
pub mod fast_io;
pub mod file_collect;
pub mod language;
pub mod languages;
pub mod parallel_language_processor;
// pub mod semantic_caching; // Temporarily disabled for speed optimization"
pub mod differential_ast_processor;
pub mod parser;
pub mod real_ai_integration;
pub mod rust_advanced_extractor;
#[cfg(feature = "experimental")]
pub mod semantic;
pub mod speed_optimized_cache;
pub mod text_processor;
pub mod visitor;
#[cfg(feature = "experimental")]
pub mod watcher;

#[cfg(all(test, feature = "experimental"))]
pub mod integration_tests;

#[cfg(test)]
mod tests;

pub use ai_context_enhancement::*;
pub use ai_pattern_learning::*;
#[cfg(feature = "experimental")]
pub use diff::*;
pub use edge::*;
pub use language::*;
pub use languages::*;
pub use parallel_language_processor::*;
pub use parser::*;
#[cfg(feature = "experimental")]
pub use semantic::*;
pub use text_processor::*;
pub use visitor::*;
// pub use semantic_caching::*; // Temporarily disabled for speed optimization"
pub use differential_ast_processor::*;
pub use real_ai_integration::*;
pub use rust_advanced_extractor::*;
pub use speed_optimized_cache::*;
#[cfg(feature = "experimental")]
pub use watcher::*;
