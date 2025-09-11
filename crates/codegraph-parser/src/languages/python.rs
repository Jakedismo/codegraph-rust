use crate::edge::CodeEdge;
use codegraph_core::CodeNode;

#[derive(Debug, Default, Clone)]
pub struct PythonExtraction {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
}

pub fn extract_python(_file_path: &str, _source: &str) -> PythonExtraction {
    // Lightweight stub to keep compilation healthy for non-Rust paths.
    PythonExtraction::default()
}
