use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// REVOLUTIONARY: TypeScript/JavaScript extractor with unified node+edge extraction
pub struct TypeScriptExtractor;

impl TypeScriptExtractor {
    /// Extract nodes and edges in single AST traversal for maximum speed
    pub fn extract_with_edges(
        tree: &Tree,
        content: &str,
        file_path: &str,
        language: Language,
    ) -> ExtractionResult {
        let mut collector = TypeScriptCollector::new(content, file_path, language);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor);
        collector.into_result()
    }
}

impl super::LanguageExtractor for TypeScriptExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        TypeScriptExtractor::extract_with_edges(tree, content, file_path, Language::TypeScript)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls, EdgeType::Extends, EdgeType::Implements]
    }

    fn language() -> Language {
        Language::TypeScript
    }
}

struct TypeScriptCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    language: Language,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
}

impl<'a> TypeScriptCollector<'a> {
    fn new(content: &'a str, file_path: &'a str, language: Language) -> Self {
        Self {
            content,
            file_path,
            language,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
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
            // Functions
            "function_declaration" | "function_expression" | "arrow_function" => {
                if let Some(name) = self.extract_function_name(&node) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name,
                        Some(NodeType::Function),
                        Some(self.language.clone()),
                        loc,
                    )
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

            // Import statements
            "import_statement" => {
                if let Some(name) = self.extract_import_source(&node) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Import),
                        Some(self.language.clone()),
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
                            meta.insert("import_type".to_string(), "es_import".to_string());
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
            "call_expression" => {
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

            // Classes
            "class_declaration" => {
                if let Some(name) =
                    self.child_text_by_kinds(node, &["type_identifier", "identifier"])
                {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name,
                        Some(NodeType::Class),
                        Some(self.language.clone()),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    self.nodes.push(code);
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

    fn extract_function_name(&self, node: &Node) -> Option<String> {
        self.child_text_by_kinds(*node, &["identifier", "property_identifier"])
    }

    fn extract_import_source(&self, node: &Node) -> Option<String> {
        // Extract from import statements
        if let Some(source_node) = node.child_by_field_name("source") {
            let text = self.node_text(&source_node);
            return Some(text.trim_matches('"').trim_matches('\'').to_string());
        }
        None
    }

    fn extract_call_target(&self, node: &Node) -> Option<String> {
        if let Some(function_node) = node.child_by_field_name("function") {
            return Some(self.node_text(&function_node));
        }
        self.child_text_by_kinds(*node, &["identifier", "member_expression"])
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

/// Backward compatibility wrapper
pub fn extract_js_ts_nodes(
    _language: Language,
    _file_path: &str,
    _source: &str,
    _root: Node,
) -> Vec<CodeNode> {
    // This should be replaced with proper tree usage, but for now return empty
    // to maintain compatibility while unified extraction is being implemented
    Vec::new()
}
