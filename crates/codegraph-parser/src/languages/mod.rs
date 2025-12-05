// ABOUTME: Language extractor modules and shared infrastructure
// ABOUTME: Provides unified extraction interface for all supported languages

pub mod extractor_utils;
pub mod javascript;
pub mod python;
pub mod rust;
// Revolutionary universal language support
pub mod cpp;
pub mod csharp;
pub mod go;
pub mod java;
pub mod php;
pub mod ruby;
pub mod swift;

use codegraph_core::{EdgeType, ExtractionResult, Language};
use tree_sitter::Tree;

/// Trait for language-specific AST extractors
///
/// All language extractors implement this trait to provide unified
/// extraction of code nodes and relationship edges.
pub trait LanguageExtractor {
    /// Extract nodes and edges in a single AST traversal
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult;

    /// List of edge types this extractor can produce
    fn supported_edge_types() -> &'static [EdgeType];

    /// Language identifier
    fn language() -> Language;
}

// Re-export extractors for convenience
pub use cpp::CppExtractor;
pub use csharp::CSharpExtractor;
pub use go::GoExtractor;
pub use java::JavaExtractor;
pub use javascript::TypeScriptExtractor;
pub use php::PhpExtractor;
pub use python::PythonExtractor;
pub use ruby::RubyExtractor;
pub use rust::RustExtractor;
pub use swift::SwiftExtractor;
