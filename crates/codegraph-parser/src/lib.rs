pub mod ai_pattern_learning;
#[cfg(feature = "experimental")]
pub mod diff;
pub mod edge;
pub mod fast_io;
pub mod file_collect;
pub mod language;
pub mod languages;
pub mod parser;
pub mod real_ai_integration;
#[cfg(feature = "experimental")]
pub mod semantic;
pub mod visitor;
#[cfg(feature = "watcher-experimental")]
pub mod watcher;

#[cfg(all(test, feature = "experimental"))]
pub mod integration_tests;

#[cfg(test)]
mod tests;

pub use ai_pattern_learning::*;
#[cfg(feature = "experimental")]
pub use diff::*;
pub use edge::*;
pub use language::*;
pub use languages::*;
pub use parser::*;
pub use real_ai_integration::*;
#[cfg(feature = "experimental")]
pub use semantic::*;
pub use visitor::*;
#[cfg(feature = "watcher-experimental")]
pub use watcher::*;
