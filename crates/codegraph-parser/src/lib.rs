#[cfg(feature = "experimental")]
pub mod diff;
pub mod edge;
pub mod fast_io;
pub mod file_collect;
pub mod language;
pub mod languages;
pub mod ai_pattern_learning;
pub mod ai_context_enhancement;
pub mod parallel_language_processor;
// pub mod semantic_caching; // Temporarily disabled for speed optimization"
pub mod speed_optimized_cache;
pub mod differential_ast_processor;
pub mod rust_advanced_extractor;
pub mod parser;
#[cfg(feature = "experimental")]
pub mod semantic;
pub mod text_processor;
pub mod visitor;
#[cfg(feature = "experimental")]
pub mod watcher;

#[cfg(test)]
pub mod integration_tests;

#[cfg(test)]
mod tests;

#[cfg(feature = "experimental")]
pub use diff::*;
pub use edge::*;
pub use language::*;
pub use languages::*;
pub use parser::*;
#[cfg(feature = "experimental")]
pub use semantic::*;
pub use text_processor::*;
pub use visitor::*;
pub use ai_pattern_learning::*;
pub use ai_context_enhancement::*;
pub use parallel_language_processor::*;
// pub use semantic_caching::*; // Temporarily disabled for speed optimization"
pub use speed_optimized_cache::*;
pub use differential_ast_processor::*;
pub use rust_advanced_extractor::*;
#[cfg(feature = "experimental")]
pub use watcher::*;
