pub mod language;
pub mod parser;
pub mod visitor;
pub mod watcher;
pub mod diff;
pub mod semantic;
pub mod text_processor;

#[cfg(test)]
pub mod integration_tests;

#[cfg(test)]
mod tests;

pub use language::*;
pub use parser::*;
pub use visitor::*;
pub use watcher::*;
pub use diff::*;
pub use semantic::*;
pub use text_processor::*;