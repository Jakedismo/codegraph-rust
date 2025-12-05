use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// REVOLUTIONARY: Python extractor with unified node+edge extraction
pub struct PythonExtractor;

impl PythonExtractor {
    /// Extract nodes and edges in single AST traversal for maximum speed
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = PythonCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor);
        collector.into_result()
    }
}

impl super::LanguageExtractor for PythonExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        PythonExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls, EdgeType::Extends]
    }

    fn language() -> Language {
        Language::Python
    }
}

struct PythonCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    current_class_id: Option<NodeId>,
}

impl<'a> PythonCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
            current_class_id: None,
        }
    }

    fn span_for(&self, node: &Node) -> Span {
        Span {
            start_byte: node.start_byte() as u32,
            end_byte: node.end_byte() as u32,
        }
    }

    fn into_result(self) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
        }
    }

    fn walk(&mut self, cursor: &mut TreeCursor) {
        let node = cursor.node();

        match node.kind() {
            // Function definitions
            "function_definition" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(&node);
                    let mut code =
                        CodeNode::new(name, Some(NodeType::Function), Some(Language::Python), loc)
                            .with_content(self.node_text(&node))
                            .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                                &node,
                                self.content,
                            ));
                    code.span = Some(self.span_for(&node));

                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Class definitions
            "class_definition" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(&node);
                    let mut code =
                        CodeNode::new(name, Some(NodeType::Class), Some(Language::Python), loc)
                            .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    self.current_class_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Import statements
            "import_statement" | "import_from_statement" => {
                if let Some(name) = self.extract_import_name(&node) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Import),
                        Some(Language::Python),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    // Extract import edge
                    let edge = EdgeRelationship {
                        from: code.id,
                        to: name,
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("import_type".to_string(), "python_import".to_string());
                            meta.insert("source_file".to_string(), self.file_path.to_string());
                            meta
                        },
                        span: Some(self.span_for(&node)),
                    };
                    self.edges.push(edge);
                    self.nodes.push(code);
                }
            }

            // Function calls
            "call" => {
                if let Some(current_fn) = self.current_function_id {
                    if let Some(function_name) = self.extract_call_target(&node) {
                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: function_name,
                            edge_type: EdgeType::Calls,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("call_type".to_string(), "function_call".to_string());
                                meta.insert("source_file".to_string(), self.file_path.to_string());
                                meta
                            },
                            span: Some(self.span_for(&node)),
                        };
                        self.edges.push(edge);
                    }
                }
            }

            _ => {}
        }

        // Recurse into children
        if cursor.goto_first_child() {
            loop {
                self.walk(cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_import_name(&self, node: &Node) -> Option<String> {
        // For import_statement: import module_name
        // For import_from_statement: from module_name import ...
        if node.kind() == "import_statement" {
            self.child_text_by_kinds(*node, &["dotted_name", "identifier"])
        } else if node.kind() == "import_from_statement" {
            self.child_text_by_kinds(*node, &["dotted_name", "relative_import"])
        } else {
            None
        }
    }

    fn extract_call_target(&self, node: &Node) -> Option<String> {
        if let Some(function_node) = node.child_by_field_name("function") {
            return Some(self.node_text(&function_node));
        }
        self.child_text_by_kinds(*node, &["identifier", "attribute"])
    }

    fn location(&self, node: &Node) -> Location {
        Location {
            file_path: self.file_path.to_string(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        }
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes())
            .unwrap_or("")
            .to_string()
    }

    fn child_text_by_kinds(&self, node: Node, kinds: &[&str]) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let n = cursor.node();
                if kinds.iter().any(|k| n.kind() == *k) {
                    return Some(self.node_text(&n));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }
}

/// Backward compatibility function
#[derive(Debug, Default, Clone)]
pub struct PythonExtraction {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<crate::edge::CodeEdge>,
}

pub fn extract_python(_file_path: &str, _source: &str) -> PythonExtraction {
    // Stub for backward compatibility
    PythonExtraction::default()
}
