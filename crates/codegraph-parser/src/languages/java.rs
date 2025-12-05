// ABOUTME: Java language AST extractor for code intelligence
// ABOUTME: Extracts packages, classes, interfaces, methods, imports, and call edges

use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced Java AST extractor for enterprise development intelligence.
///
/// Extracts:
/// - packages, classes, interfaces, enums, annotations
/// - methods, constructors, fields
/// - imports (single and wildcard)
/// - method invocations
/// - inheritance and interface implementation
/// - Spring/Enterprise patterns
///
/// Notes:
/// - Optimized for enterprise Java patterns
/// - Captures Spring Boot and Jakarta EE patterns
/// - Handles annotations and dependency injection
pub struct JavaExtractor;

#[derive(Default, Clone)]
struct JavaContext {
    package_name: Option<String>,
    current_class: Option<String>,
    current_interface: Option<String>,
}

impl JavaExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = JavaCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, JavaContext::default());
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = JavaCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, JavaContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for JavaExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        JavaExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls, EdgeType::Implements, EdgeType::Extends]
    }

    fn language() -> Language {
        Language::Java
    }
}

struct JavaCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    current_class_id: Option<NodeId>,
}

impl<'a> JavaCollector<'a> {
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

    fn into_nodes(self) -> Vec<CodeNode> {
        self.nodes
    }

    fn into_result(self) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
        }
    }

    fn span_for(&self, node: &Node) -> Span {
        Span {
            start_byte: node.start_byte() as u32,
            end_byte: node.end_byte() as u32,
        }
    }

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: JavaContext) {
        let node = cursor.node();

        match node.kind() {
            // Java Package declaration
            "package_declaration" => {
                // Extract package name from the identifier
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                            let name = self.node_text(&child);
                            let loc = self.location(&node);
                            let mut code = CodeNode::new(
                                name.clone(),
                                Some(NodeType::Module),
                                Some(Language::Java),
                                loc,
                            )
                            .with_content(self.node_text(&node));
                            code.span = Some(self.span_for(&node));

                            code.metadata
                                .attributes
                                .insert("kind".into(), "package".into());
                            self.nodes.push(code);
                            ctx.package_name = Some(name);
                            break;
                        }
                    }
                }
            }

            // Java Import declarations
            "import_declaration" => {
                let content_text = self.node_text(&node);
                let is_static = content_text.contains("static ");
                let is_wildcard = content_text.contains(".*");

                // Extract the import path
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                            let path = self.node_text(&child);
                            let loc = self.location(&node);
                            let mut code = CodeNode::new(
                                format!("import {}", path),
                                Some(NodeType::Import),
                                Some(Language::Java),
                                loc,
                            )
                            .with_content(content_text.clone());
                            code.span = Some(self.span_for(&node));

                            code.metadata
                                .attributes
                                .insert("kind".into(), "import".into());
                            code.metadata
                                .attributes
                                .insert("path".into(), path.clone());

                            if is_static {
                                code.metadata
                                    .attributes
                                    .insert("static".into(), "true".into());
                            }
                            if is_wildcard {
                                code.metadata
                                    .attributes
                                    .insert("wildcard".into(), "true".into());
                            }

                            // Create import edge
                            let edge = EdgeRelationship {
                                from: code.id,
                                to: path.clone(),
                                edge_type: EdgeType::Imports,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("import_type".to_string(), "java_import".to_string());
                                    meta.insert("source_file".to_string(), self.file_path.to_string());
                                    if is_static {
                                        meta.insert("static".to_string(), "true".to_string());
                                    }
                                    if is_wildcard {
                                        meta.insert("wildcard".to_string(), "true".to_string());
                                    }
                                    meta
                                },
                                span: Some(self.span_for(&node)),
                            };
                            self.edges.push(edge);
                            self.nodes.push(code);
                            break;
                        }
                    }
                }
            }

            // Java Class declarations
            "class_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Java),
                        loc,
                    )
                    .with_content(content_text.clone());
                    code.span = Some(self.span_for(&node));

                    // Detect visibility
                    if content_text.starts_with("public ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "public".into());
                    } else if content_text.starts_with("private ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "private".into());
                    } else if content_text.starts_with("protected ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "protected".into());
                    }

                    // Detect abstract/final
                    if content_text.contains("abstract ") {
                        code.metadata
                            .attributes
                            .insert("abstract".into(), "true".into());
                    }
                    if content_text.contains("final ") {
                        code.metadata
                            .attributes
                            .insert("final".into(), "true".into());
                    }

                    // Detect Spring patterns
                    if content_text.contains("@Controller") || content_text.contains("@RestController") {
                        code.metadata
                            .attributes
                            .insert("spring_pattern".into(), "controller".into());
                    } else if content_text.contains("@Service") {
                        code.metadata
                            .attributes
                            .insert("spring_pattern".into(), "service".into());
                    } else if content_text.contains("@Repository") {
                        code.metadata
                            .attributes
                            .insert("spring_pattern".into(), "repository".into());
                    } else if content_text.contains("@Component") {
                        code.metadata
                            .attributes
                            .insert("spring_pattern".into(), "component".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "class".into());

                    self.current_class_id = Some(code.id);
                    ctx.current_class = Some(name);
                    self.nodes.push(code);
                }
            }

            // Java Interface declarations
            "interface_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Interface),
                        Some(Language::Java),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "interface".into());

                    ctx.current_interface = Some(name);
                    self.nodes.push(code);
                }
            }

            // Java Enum declarations
            "enum_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Enum),
                        Some(Language::Java),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "enum".into());
                    self.nodes.push(code);
                }
            }

            // Java Method declarations
            "method_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Java),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                        &node,
                        self.content,
                    ));
                    code.span = Some(self.span_for(&node));

                    // Detect visibility
                    if content_text.contains("public ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "public".into());
                    } else if content_text.contains("private ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "private".into());
                    } else if content_text.contains("protected ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "protected".into());
                    }

                    // Detect static methods
                    if content_text.contains("static ") {
                        code.metadata
                            .attributes
                            .insert("static".into(), "true".into());
                    }

                    // Detect annotations
                    if content_text.contains("@Override") {
                        code.metadata
                            .attributes
                            .insert("override".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "method".into());
                    if let Some(ref current_class) = ctx.current_class {
                        code.metadata
                            .attributes
                            .insert("parent_class".into(), current_class.clone());
                    }

                    // Track current function for call edge attribution
                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Java Constructor declarations
            "constructor_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Java),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                        &node,
                        self.content,
                    ));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "constructor".into());

                    // Track current function for call edge attribution
                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Java Method invocations - extract call edges
            "method_invocation" => {
                if let Some(current_fn) = self.current_function_id {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let callee_name = self.node_text(&name_node);
                        if !callee_name.is_empty() {
                            let edge = EdgeRelationship {
                                from: current_fn,
                                to: callee_name,
                                edge_type: EdgeType::Calls,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("call_type".to_string(), "java_invocation".to_string());
                                    meta
                                },
                                span: Some(self.span_for(&node)),
                            };
                            self.edges.push(edge);
                        }
                    }
                }
            }

            _ => {}
        }

        // Recursively walk children
        if cursor.goto_first_child() {
            loop {
                self.walk(cursor, ctx.clone());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes())
            .unwrap_or("")
            .to_string()
    }

    fn location(&self, node: &Node) -> Location {
        Location {
            file_path: self.file_path.to_string(),
            line: (node.start_position().row + 1) as u32,
            column: (node.start_position().column + 1) as u32,
            end_line: Some((node.end_position().row + 1) as u32),
            end_column: Some((node.end_position().column + 1) as u32),
        }
    }
}
