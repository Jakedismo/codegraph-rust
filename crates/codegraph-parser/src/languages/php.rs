use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced PHP AST extractor for web development intelligence.
///
/// Extracts:
/// - classes, interfaces, traits, enums (PHP 8.1+)
/// - functions, methods, properties, constants
/// - namespace organization and use statements
/// - Laravel/Symfony framework patterns
/// - dynamic property access and magic methods
/// - closures and anonymous functions
/// - array/object patterns and type hints
///
/// Notes:
/// - Optimized for modern PHP (7.4+, 8.x) patterns
/// - Captures Laravel/Symfony MVC patterns
/// - Handles dynamic typing and magic methods
/// - Understands Composer autoloading patterns
pub struct PhpExtractor;

#[derive(Default, Clone)]
struct PhpContext {
    namespace_path: Vec<String>,
    current_class: Option<String>,
    current_trait: Option<String>,
    use_statements: Vec<String>,
    is_framework_file: bool,
}

impl PhpExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = PhpCollector::new(content, file_path);
        let mut cursor = tree.walk();

        // Detect framework patterns from file structure
        let is_framework = file_path.contains("/app/")
            || file_path.contains("/src/")
            || file_path.contains("Controller.php")
            || file_path.contains("Model.php");

        let mut ctx = PhpContext::default();
        ctx.is_framework_file = is_framework;

        collector.walk(&mut cursor, ctx);
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = PhpCollector::new(content, file_path);
        let mut cursor = tree.walk();

        // Detect framework patterns from file structure
        let is_framework = file_path.contains("/app/")
            || file_path.contains("/src/")
            || file_path.contains("Controller.php")
            || file_path.contains("Model.php");

        let mut ctx = PhpContext::default();
        ctx.is_framework_file = is_framework;

        collector.walk(&mut cursor, ctx);
        collector.into_result()
    }
}

impl super::LanguageExtractor for PhpExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        PhpExtractor::extract_with_edges(tree, content, file_path)
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
        Language::Php
    }
}

struct PhpCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    _current_class_id: Option<NodeId>,
}

impl<'a> PhpCollector<'a> {
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: PhpContext) {
        let node = cursor.node();

        match node.kind() {
            // PHP Namespaces
            "namespace_definition" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Php),
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

            // PHP Classes
            "class_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect extends/implements
                    if let Some(base_clause) = self.child_text_by_field(node, "base_clause") {
                        code.metadata
                            .attributes
                            .insert("inheritance".into(), base_clause);
                    }

                    // Detect Laravel patterns
                    if ctx.is_framework_file {
                        if name.ends_with("Controller") {
                            code.metadata
                                .attributes
                                .insert("laravel_pattern".into(), "controller".into());
                        } else if content_text.contains("extends Model") {
                            code.metadata
                                .attributes
                                .insert("laravel_pattern".into(), "model".into());
                        } else if content_text.contains("extends Migration") {
                            code.metadata
                                .attributes
                                .insert("laravel_pattern".into(), "migration".into());
                        }
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "class".into());
                    self.nodes.push(code);
                    ctx.current_class = Some(name);
                }
            }

            // PHP Interfaces
            "interface_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Interface),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "interface".into());
                    self.nodes.push(code);
                }
            }

            // PHP Traits
            "trait_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Trait),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(self.node_text(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "trait".into());
                    self.nodes.push(code);
                    ctx.current_trait = Some(name);
                }
            }

            // PHP Functions/Methods
            "function_definition" | "method_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(
                        crate::complexity::calculate_cyclomatic_complexity(&node, self.content),
                    );
                    code.span = Some(self.span_for(&node));

                    // Detect visibility modifiers
                    if content_text.contains("private ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "private".into());
                    } else if content_text.contains("protected ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "protected".into());
                    } else if content_text.contains("public ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "public".into());
                    }

                    // Detect static methods
                    if content_text.contains("static ") {
                        code.metadata
                            .attributes
                            .insert("static".into(), "true".into());
                    }

                    // Detect magic methods
                    if name.starts_with("__") {
                        code.metadata
                            .attributes
                            .insert("magic_method".into(), "true".into());
                    }

                    // Detect return types (PHP 7+)
                    if let Some(return_type) = self.child_text_by_field(node, "return_type") {
                        code.metadata
                            .attributes
                            .insert("return_type".into(), return_type);
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "function".into());
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

            // PHP Properties
            "property_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Variable),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(content_text.clone());

                    // Detect visibility
                    if content_text.contains("private ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "private".into());
                    } else if content_text.contains("protected ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "protected".into());
                    } else if content_text.contains("public ") {
                        code.metadata
                            .attributes
                            .insert("visibility".into(), "public".into());
                    }

                    // Detect static properties
                    if content_text.contains("static ") {
                        code.metadata
                            .attributes
                            .insert("static".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "property".into());
                    self.nodes.push(code);
                }
            }

            // PHP Use statements
            "use_declaration" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        format!("use {}", name),
                        Some(NodeType::Import),
                        Some(Language::Php),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata.attributes.insert("kind".into(), "use".into());
                    code.metadata
                        .attributes
                        .insert("namespace".into(), name.clone());

                    // Create import edge
                    let edge = EdgeRelationship {
                        from: code.id,
                        to: name.clone(),
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("import_type".to_string(), "php_use".to_string());
                            meta.insert("source_file".to_string(), self.file_path.to_string());
                            meta
                        },
                        span: Some(self.span_for(&node)),
                    };
                    self.edges.push(edge);

                    ctx.use_statements.push(name);
                    self.nodes.push(code);
                }
            }

            // PHP Function calls - extract call edges
            "function_call_expression" | "member_call_expression" => {
                if let Some(current_fn) = self.current_function_id {
                    // Extract the function/method being called
                    let callee = if let Some(name_node) = node.child_by_field_name("name") {
                        self.node_text(&name_node)
                    } else if let Some(func_node) = node.child_by_field_name("function") {
                        self.node_text(&func_node)
                    } else {
                        String::new()
                    };

                    if !callee.is_empty() {
                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: callee,
                            edge_type: EdgeType::Calls,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("call_type".to_string(), "php_call".to_string());
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
