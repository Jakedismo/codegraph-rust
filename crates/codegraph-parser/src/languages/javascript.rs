use codegraph_core::{CodeNode, Language};
use tree_sitter::Node;

pub fn extract_js_ts_nodes(
    _language: Language,
    _file_path: &str,
    _source: &str,
    _root: Node,
) -> Vec<CodeNode> {
    // Lightweight stub for JS/TS; parser falls back to generic visitor where needed.
    Vec::new()
}

