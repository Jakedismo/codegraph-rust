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
#[cfg(feature = "experimental")]
pub use watcher::*;
