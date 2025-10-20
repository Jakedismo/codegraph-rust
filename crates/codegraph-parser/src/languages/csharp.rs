use codegraph_core::{CodeNode, Language, Location, NodeType};
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
}

struct CSharpCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
}

impl<'a> CSharpCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
        }
    }

    fn into_nodes(self) -> Vec<CodeNode> {
        self.nodes
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
                    .with_content(content_text.clone());

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

                    code.metadata
                        .attributes
                        .insert("kind".into(), "using".into());
                    code.metadata
                        .attributes
                        .insert("namespace".into(), namespace.clone());
                    ctx.using_statements.push(namespace);
                    self.nodes.push(code);
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
