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

/// Unified extraction dispatch for all supported languages
///
/// Routes to the appropriate language extractor based on the Language enum.
/// Returns None for unsupported languages.
pub fn extract_for_language(
    language: &Language,
    tree: &Tree,
    content: &str,
    file_path: &str,
) -> Option<ExtractionResult> {
    match language {
        Language::Rust => Some(<RustExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::TypeScript => Some(
            <TypeScriptExtractor as LanguageExtractor>::extract_with_edges(
                tree, content, file_path,
            ),
        ),
        Language::JavaScript => {
            // JavaScript uses TypeScript extractor with JS-specific handling
            Some(TypeScriptExtractor::extract_with_edges(
                tree,
                content,
                file_path,
                Language::JavaScript,
            ))
        }
        Language::Python => Some(<PythonExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Go => Some(<GoExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Java => Some(<JavaExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Cpp => Some(<CppExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Swift => Some(<SwiftExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::CSharp => Some(<CSharpExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Ruby => Some(<RubyExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        Language::Php => Some(<PhpExtractor as LanguageExtractor>::extract_with_edges(
            tree, content, file_path,
        )),
        // Languages without dedicated extractors yet
        _ => None,
    }
}
