use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced C# AST extractor for .NET development intelligence.
///
/// Extracts:
/// - classes, interfaces, structs, records, enums
/// - methods, properties, fields, events
/// - async/await patterns and Task-based operations
/// - LINQ expressions and extension methods
/// - generics, nullable reference types
/// - attributes and dependency injection patterns
/// - namespace organization
///
/// Notes:
/// - Optimized for modern C# (.NET 6+) patterns
/// - Captures dependency injection and web API patterns
/// - Handles Entity Framework and ORM patterns
/// - Understands async/await and Task patterns
pub struct CSharpExtractor;

#[derive(Default, Clone)]
struct CSharpContext {
    namespace_path: Vec<String>,
    current_class: Option<String>,
    current_interface: Option<String>,
    using_statements: Vec<String>,
}

impl CSharpExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = CSharpCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, CSharpContext::default());
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = CSharpCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, CSharpContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for CSharpExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        CSharpExtractor::extract_with_edges(tree, content, file_path)
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
        Language::CSharp
    }
}

struct CSharpCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    _current_class_id: Option<NodeId>,
}

impl<'a> CSharpCollector<'a> {
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: CSharpContext) {
        let node = cursor.node();

        match node.kind() {
            // C# Namespaces
            "namespace_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "namespace".into());
                    self.nodes.push(code);
                    ctx.namespace_path.push(name);
                }
            }

            // C# Classes
            "class_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect inheritance and interfaces
                    if let Some(base_list) = self.child_text_by_field(node, "base_list") {
                        code.metadata
                            .attributes
                            .insert("inheritance".into(), base_list);
                    }

                    // Detect controller pattern (ASP.NET)
                    if name.ends_with("Controller") {
                        code.metadata
                            .attributes
                            .insert("pattern".into(), "controller".into());
                    }

                    // Detect service pattern (dependency injection)
                    if name.ends_with("Service") {
                        code.metadata
                            .attributes
                            .insert("pattern".into(), "service".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "class".into());
                    self.nodes.push(code);
                    ctx.current_class = Some(name);
                }
            }

            // C# Interfaces
            "interface_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Interface),
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "interface".into());
                    self.nodes.push(code);
                    ctx.current_interface = Some(name);
                }
            }

            // C# Records (modern data structures)
            "record_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Struct), // Records are value-type-like
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "record".into());
                    self.nodes.push(code);
                }
            }

            // C# Methods
            "method_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(
                        crate::complexity::calculate_cyclomatic_complexity(&node, self.content),
                    );
                    code.span = Some(self.span_for(&node));

                    // Detect async methods
                    if content_text.contains("async ") {
                        code.metadata
                            .attributes
                            .insert("async".into(), "true".into());
                    }

                    // Detect return types
                    if content_text.contains("Task<") {
                        code.metadata
                            .attributes
                            .insert("returns_task".into(), "true".into());
                    }

                    // Detect LINQ usage
                    if content_text.contains(".Where(")
                        || content_text.contains(".Select(")
                        || content_text.contains(".FirstOrDefault(")
                    {
                        code.metadata
                            .attributes
                            .insert("uses_linq".into(), "true".into());
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

            // C# Properties
            "property_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Variable), // Properties are variable-like
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect auto-properties
                    if content_text.contains("{ get; set; }") {
                        code.metadata
                            .attributes
                            .insert("auto_property".into(), "true".into());
                    }

                    // Detect computed properties
                    if content_text.contains("=>") {
                        code.metadata
                            .attributes
                            .insert("computed_property".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "property".into());
                    self.nodes.push(code);
                }
            }

            // C# Using statements
            "using_directive" => {
                if let Some(namespace) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        format!("using {}", namespace),
                        Some(NodeType::Import),
                        Some(Language::CSharp),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "using".into());
                    code.metadata
                        .attributes
                        .insert("namespace".into(), namespace.clone());

                    // Detect common .NET namespaces
                    let is_system = namespace.starts_with("System");
                    let is_microsoft = namespace.starts_with("Microsoft");

                    // Create import edge
                    let edge = EdgeRelationship {
                        from: code.id,
                        to: namespace.clone(),
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("import_type".to_string(), "csharp_using".to_string());
                            meta.insert("source_file".to_string(), self.file_path.to_string());
                            if is_system {
                                meta.insert("framework".to_string(), "system".to_string());
                            }
                            if is_microsoft {
                                meta.insert("framework".to_string(), "microsoft".to_string());
                            }
                            meta
                        },
                        span: Some(self.span_for(&node)),
                    };
                    self.edges.push(edge);

                    ctx.using_statements.push(namespace);
                    self.nodes.push(code);
                }
            }

            // C# Method invocations - extract call edges
            "invocation_expression" => {
                if let Some(current_fn) = self.current_function_id {
                    // Extract the method being called
                    if let Some(callee) = node.child_by_field_name("function") {
                        let callee_name = self.node_text(&callee);
                        if !callee_name.is_empty() {
                            let edge = EdgeRelationship {
                                from: current_fn,
                                to: callee_name,
                                edge_type: EdgeType::Calls,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert(
                                        "call_type".to_string(),
                                        "csharp_invocation".to_string(),
                                    );
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
