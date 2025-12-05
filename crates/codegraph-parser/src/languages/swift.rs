use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced Swift AST extractor for iOS/macOS development intelligence.
///
/// Extracts:
/// - classes, structs, protocols, extensions, enums
/// - functions, methods, computed properties, property wrappers
/// - SwiftUI views and view modifiers
/// - protocol conformance and inheritance relationships
/// - async/await patterns and error handling
/// - closures and functional programming constructs
/// - optionals and unwrapping patterns
///
/// Notes:
/// - Optimized for iOS/macOS development patterns
/// - Captures SwiftUI declarative syntax and state management
/// - Handles Swift's unique protocol-oriented programming model
pub struct SwiftExtractor;

#[derive(Default, Clone)]
struct SwiftContext {
    current_type: Option<String>,
    current_protocol: Option<String>,
    is_swiftui_view: bool,
    property_wrappers: Vec<String>,
}

impl SwiftExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = SwiftCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, SwiftContext::default());
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = SwiftCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, SwiftContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for SwiftExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        SwiftExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[
            EdgeType::Imports,
            EdgeType::Calls,
            EdgeType::Implements,
            EdgeType::Extends,
        ]
    }

    fn language() -> Language {
        Language::Swift
    }
}

struct SwiftCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    _current_class_id: Option<NodeId>,
}

impl<'a> SwiftCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
            _current_class_id: None,
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: SwiftContext) {
        let node = cursor.node();

        match node.kind() {
            // Swift Classes
            "class_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    // Detect inheritance and protocol conformance
                    if let Some(inheritance) = self.child_text_by_field(node, "inheritance") {
                        code.metadata
                            .attributes
                            .insert("inheritance".into(), inheritance);
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "class".into());
                    self.nodes.push(code);
                    ctx.current_type = Some(name);
                }
            }

            // Swift Structs (including SwiftUI Views)
            "struct_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Struct),
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect SwiftUI Views
                    ctx.is_swiftui_view = content_text.contains(": View")
                        || content_text.contains("var body: some View");

                    if ctx.is_swiftui_view {
                        code.metadata
                            .attributes
                            .insert("swiftui_view".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "struct".into());
                    self.nodes.push(code);
                    ctx.current_type = Some(name);
                }
            }

            // Swift Protocols
            "protocol_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Interface), // Protocols are similar to interfaces
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "protocol".into());
                    self.nodes.push(code);
                    ctx.current_protocol = Some(name);
                }
            }

            // Swift Extensions
            "extension_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "type") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        format!("{} (extension)", name),
                        Some(NodeType::Module), // Extensions add functionality
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "extension".into());
                    code.metadata.attributes.insert("extends".into(), name);
                    self.nodes.push(code);
                }
            }

            // Swift Functions/Methods
            "function_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(
                        crate::complexity::calculate_cyclomatic_complexity(&node, self.content),
                    );
                    code.span = Some(self.span_for(&node));

                    // Detect async functions
                    if content_text.contains("async ") {
                        code.metadata
                            .attributes
                            .insert("async".into(), "true".into());
                    }

                    // Detect throwing functions
                    if content_text.contains("throws") {
                        code.metadata
                            .attributes
                            .insert("throws".into(), "true".into());
                    }

                    // Detect property wrappers
                    for wrapper in &ctx.property_wrappers {
                        if content_text.contains(&format!("@{}", wrapper)) {
                            code.metadata
                                .attributes
                                .insert("property_wrapper".into(), wrapper.clone());
                        }
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "function".into());
                    if let Some(ref current_type) = ctx.current_type {
                        code.metadata
                            .attributes
                            .insert("parent_type".into(), current_type.clone());
                    }

                    // Track current function for call edge attribution
                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Swift Enums (including associated values)
            "enum_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Enum),
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect if enum has associated values
                    if content_text.contains("case ") && content_text.contains("(") {
                        code.metadata
                            .attributes
                            .insert("has_associated_values".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "enum".into());
                    self.nodes.push(code);
                    ctx.current_type = Some(name);
                }
            }

            // Swift Property Wrappers (@State, @Published, etc.)
            "attribute" => {
                if let Some(attr_name) = self.node_text(&node).strip_prefix('@') {
                    ctx.property_wrappers.push(attr_name.to_string());
                }
            }

            // Swift Imports
            "import_declaration" => {
                if let Some(module) = self.child_text_by_field(node, "module") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        format!("import {}", module),
                        Some(NodeType::Import),
                        Some(Language::Swift),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "import".into());
                    code.metadata
                        .attributes
                        .insert("module".into(), module.clone());

                    // Detect framework imports
                    let is_framework =
                        module == "SwiftUI" || module == "UIKit" || module == "Foundation";
                    if is_framework {
                        code.metadata
                            .attributes
                            .insert("framework".into(), module.clone());
                    }

                    // Create import edge
                    let edge = EdgeRelationship {
                        from: code.id,
                        to: module.clone(),
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("import_type".to_string(), "swift_import".to_string());
                            meta.insert("source_file".to_string(), self.file_path.to_string());
                            if is_framework {
                                meta.insert("framework".to_string(), "true".to_string());
                            }
                            meta
                        },
                        span: Some(self.span_for(&node)),
                    };
                    self.edges.push(edge);
                    self.nodes.push(code);
                }
            }

            // Swift Call Expressions - extract call edges
            "call_expression" => {
                if let Some(current_fn) = self.current_function_id {
                    // Extract the function being called
                    if let Some(callee) = node.child_by_field_name("function") {
                        let callee_name = self.node_text(&callee);
                        if !callee_name.is_empty() {
                            let edge = EdgeRelationship {
                                from: current_fn,
                                to: callee_name,
                                edge_type: EdgeType::Calls,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("call_type".to_string(), "swift_call".to_string());
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

    fn child_text_by_field(&self, node: Node, field_name: &str) -> Option<String> {
        node.child_by_field_name(field_name)
            .map(|child| self.node_text(&child))
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
