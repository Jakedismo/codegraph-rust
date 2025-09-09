pub mod processor;
pub mod parser;

pub use processor::{CodeProcessor, CodeInput, CodeMetadata};

use std::hash::Hash;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
    Cpp,
    Csharp,
    Other(u16), // For extensibility
}

impl CodeLanguage {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" | "pyx" | "pyi" => Self::Python,
            "js" | "jsx" => Self::JavaScript,
            "ts" | "tsx" => Self::TypeScript,
            "java" => Self::Java,
            "go" => Self::Go,
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "h" => Self::Cpp,
            "cs" => Self::Csharp,
            _ => Self::Other(0),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Java => "java",
            Self::Go => "go",
            Self::Cpp => "cpp",
            Self::Csharp => "csharp",
            Self::Other(_) => "other",
        }
    }

    pub fn tree_sitter_language(&self) -> Option<&'static str> {
        match self {
            Self::Rust => Some("rust"),
            Self::Python => Some("python"),
            Self::JavaScript => Some("javascript"),
            Self::TypeScript => Some("typescript"),
            Self::Java => Some("java"),
            Self::Go => Some("go"),
            Self::Cpp => Some("cpp"),
            Self::Csharp => Some("c_sharp"),
            Self::Other(_) => None,
        }
    }

    pub fn supports_ast_parsing(&self) -> bool {
        self.tree_sitter_language().is_some()
    }
}

#[derive(Debug, Clone)]
pub struct ASTFeatures {
    pub node_types: Vec<String>,
    pub depth: usize,
    pub complexity_score: f32,
    pub function_count: usize,
    pub class_count: usize,
    pub import_count: usize,
}

#[derive(Debug, Clone)]
pub struct ControlFlowGraph {
    pub nodes: Vec<CFGNode>,
    pub edges: Vec<CFGEdge>,
    pub entry_node: usize,
    pub exit_nodes: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CFGNode {
    pub id: usize,
    pub node_type: CFGNodeType,
    pub source_range: (usize, usize),
}

#[derive(Debug, Clone)]
pub enum CFGNodeType {
    Entry,
    Exit,
    Statement,
    Condition,
    Loop,
    FunctionCall,
    Return,
}

#[derive(Debug, Clone)]
pub struct CFGEdge {
    pub from: usize,
    pub to: usize,
    pub edge_type: CFGEdgeType,
}

#[derive(Debug, Clone)]
pub enum CFGEdgeType {
    Sequential,
    True,
    False,
    Exception,
}

impl Default for ASTFeatures {
    fn default() -> Self {
        Self {
            node_types: Vec::new(),
            depth: 0,
            complexity_score: 0.0,
            function_count: 0,
            class_count: 0,
            import_count: 0,
        }
    }
}

impl Default for ControlFlowGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            entry_node: 0,
            exit_nodes: Vec::new(),
        }
    }
}