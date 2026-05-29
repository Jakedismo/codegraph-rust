// ABOUTME: Scala language AST extractor for code intelligence
// ABOUTME: Extracts classes, objects, traits, def, val, packages, imports

use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Scala AST extractor for functional/OO code analysis
pub struct ScalaExtractor;

impl ScalaExtractor {
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = ScalaCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor);
        collector.into_result()
    }
}

impl super::LanguageExtractor for ScalaExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        ScalaExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls, EdgeType::Extends, EdgeType::Implements]
    }

    fn language() -> Language {
        Language::Scala
    }
}

struct ScalaCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_def_id: Option<NodeId>,
}

impl<'a> ScalaCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_def_id: None,
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
            // Package declaration
            "package_clause" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "package".into());
                    self.nodes.push(code);
                }
            }

            // Import statement
            "import_statement" => {
                let text = self.node_text(&node);
                if let Some(name) = self.extract_import_name(&node) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Import),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(text.clone());
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "import".into());

                    let edge = EdgeRelationship {
                        from: code.id,
                        to: name,
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut m = HashMap::new();
                            m.insert("import_type".into(), "scala_import".into());
                            m
                        },
                        span: Some(self.span_for(&node)),
                    };
                    self.edges.push(edge);
                    self.nodes.push(code);
                }
            }

            // Class definition
            "class_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "class".into());

                    // Extract extends/implements
                    self.extract_type_params(node, code.id, "extends");
                    self.nodes.push(code);
                }
            }

            // Object definition (singleton)
            "object_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "object".into());
                    code.metadata.attributes.insert("singleton".into(), "true".into());
                    self.nodes.push(code);
                }
            }

            // Trait definition
            "trait_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Trait),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "trait".into());
                    self.nodes.push(code);
                }
            }

            // Function/Method definition
            "method_definition" | "function_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Scala),
                        loc,
                    )
                    .with_content(self.node_text(&node))
                    .with_complexity(
                        crate::complexity::calculate_cyclomatic_complexity(&node, self.content),
                    );
                    code.span = Some(self.span_for(&node));
                    code.metadata.attributes.insert("kind".into(), "method".into());

                    self.current_def_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Call expression
            "call_expression" => {
                if let Some(current) = self.current_def_id {
                    if let Some(target) = self.extract_call_target(&node) {
                        let edge = EdgeRelationship {
                            from: current,
                            to: target,
                            edge_type: EdgeType::Calls,
                            metadata: {
                                let mut m = HashMap::new();
                                m.insert("call_type".into(), "scala_call".into());
                                m
                            },
                            span: Some(self.span_for(&node)),
                        };
                        self.edges.push(edge);
                    }
                }
            }

            _ => {}
        }

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
        let mut parts = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "identifier" | "wildcard" => parts.push(self.node_text(&child)),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("."))
        }
    }

    fn extract_call_target(&self, node: &Node) -> Option<String> {
        if let Some(field) = node.child_by_field_name("function") {
            return Some(self.node_text(&field));
        }
        None
    }

    fn extract_type_params(&mut self, node: Node, from_id: NodeId, relation: &str) {
        // Look for extends clause
        let text = self.node_text(&node).to_lowercase();
        if let Some(start) = text.find(relation) {
            let rest = &text[start + relation.len()..];
            if let Some(end) = rest.find(&[' ', '{', '[', '\n'][..]) {
                let type_name = rest[..end].trim();
                if !type_name.is_empty() {
                    self.edges.push(EdgeRelationship {
                        from: from_id,
                        to: type_name.to_string(),
                        edge_type: EdgeType::Extends,
                        metadata: HashMap::new(),
                        span: None,
                    });
                }
            }
        }
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

    fn span_for(&self, node: &Node) -> Span {
        Span {
            start_byte: node.start_byte() as u32,
            end_byte: node.end_byte() as u32,
        }
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes()).unwrap_or("").to_string()
    }
}
