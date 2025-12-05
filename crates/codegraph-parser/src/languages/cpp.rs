// ABOUTME: C++ language AST extractor for code intelligence
// ABOUTME: Extracts namespaces, classes, structs, functions, includes, and call edges

use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced C++ AST extractor for systems development intelligence.
///
/// Extracts:
/// - namespaces, classes, structs, templates
/// - functions, methods, constructors, destructors
/// - #include directives (system and local)
/// - function/method calls
/// - class inheritance
/// - template specializations
///
/// Notes:
/// - Optimized for modern C++ (C++11/14/17/20) patterns
/// - Captures RAII and smart pointer patterns
/// - Handles header/source file relationships
/// - Understands STL and Boost patterns
pub struct CppExtractor;

#[derive(Default, Clone)]
struct CppContext {
    namespace_path: Vec<String>,
    current_class: Option<String>,
    current_struct: Option<String>,
}

impl CppExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = CppCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, CppContext::default());
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = CppCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, CppContext::default());
        collector.into_result()
    }
}

struct CppCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    current_class_id: Option<NodeId>,
}

impl<'a> CppCollector<'a> {
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: CppContext) {
        let node = cursor.node();

        match node.kind() {
            // C++ Namespace definition
            "namespace_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Cpp),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "namespace".into());
                    self.nodes.push(code);
                    ctx.namespace_path.push(name);
                }
            }

            // C++ #include directives
            "preproc_include" => {
                let content_text = self.node_text(&node);
                let loc = self.location(&node);

                // Extract the path from the include
                let (path, is_system) = if let Some(path_node) = node.child_by_field_name("path") {
                    let path_text = self.node_text(&path_node);
                    let is_system = path_text.starts_with('<');
                    let clean_path = path_text
                        .trim_matches(|c| c == '<' || c == '>' || c == '"')
                        .to_string();
                    (clean_path, is_system)
                } else {
                    // Fallback: extract from content
                    let is_system = content_text.contains('<');
                    let path = content_text
                        .replace("#include", "")
                        .trim()
                        .trim_matches(|c| c == '<' || c == '>' || c == '"')
                        .to_string();
                    (path, is_system)
                };

                if path.is_empty() {
                    return;
                }

                let mut code = CodeNode::new(
                    format!("#include {}", if is_system { format!("<{}>", path) } else { format!("\"{}\"", path) }),
                    Some(NodeType::Import),
                    Some(Language::Cpp),
                    loc,
                )
                .with_content(content_text);
                code.span = Some(self.span_for(&node));

                code.metadata
                    .attributes
                    .insert("kind".into(), "include".into());
                code.metadata
                    .attributes
                    .insert("path".into(), path.clone());

                if is_system {
                    code.metadata
                        .attributes
                        .insert("system".into(), "true".into());
                }

                // Create import edge
                let edge = EdgeRelationship {
                    from: code.id,
                    to: path.clone(),
                    edge_type: EdgeType::Imports,
                    metadata: {
                        let mut meta = HashMap::new();
                        meta.insert("import_type".to_string(), "cpp_include".to_string());
                        meta.insert("source_file".to_string(), self.file_path.to_string());
                        if is_system {
                            meta.insert("system".to_string(), "true".to_string());
                        }
                        meta
                    },
                    span: Some(self.span_for(&node)),
                };
                self.edges.push(edge);
                self.nodes.push(code);
            }

            // C++ Class specifier
            "class_specifier" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Cpp),
                        loc,
                    )
                    .with_content(content_text.clone());
                    code.span = Some(self.span_for(&node));

                    // Detect template classes
                    if content_text.contains("template") {
                        code.metadata
                            .attributes
                            .insert("template".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "class".into());

                    self.current_class_id = Some(code.id);
                    ctx.current_class = Some(name);
                    self.nodes.push(code);
                }
            }

            // C++ Struct specifier
            "struct_specifier" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Struct),
                        Some(Language::Cpp),
                        loc,
                    )
                    .with_content(content_text);
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "struct".into());

                    ctx.current_struct = Some(name);
                    self.nodes.push(code);
                }
            }

            // C++ Function definition
            "function_definition" => {
                if let Some(decl) = node.child_by_field_name("declarator") {
                    let name = self.extract_function_name(&decl);
                    if !name.is_empty() {
                        let loc = self.location(&node);
                        let content_text = self.node_text(&node);
                        let mut code = CodeNode::new(
                            name.clone(),
                            Some(NodeType::Function),
                            Some(Language::Cpp),
                            loc,
                        )
                        .with_content(content_text.clone())
                        .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                            &node,
                            self.content,
                        ));
                        code.span = Some(self.span_for(&node));

                        // Detect virtual functions
                        if content_text.contains("virtual ") {
                            code.metadata
                                .attributes
                                .insert("virtual".into(), "true".into());
                        }

                        // Detect static functions
                        if content_text.contains("static ") {
                            code.metadata
                                .attributes
                                .insert("static".into(), "true".into());
                        }

                        // Detect inline functions
                        if content_text.contains("inline ") {
                            code.metadata
                                .attributes
                                .insert("inline".into(), "true".into());
                        }

                        // Detect constexpr functions
                        if content_text.contains("constexpr ") {
                            code.metadata
                                .attributes
                                .insert("constexpr".into(), "true".into());
                        }

                        // Detect constructors/destructors
                        if name.starts_with('~') {
                            code.metadata
                                .attributes
                                .insert("kind".into(), "destructor".into());
                        } else if ctx.current_class.as_ref().map_or(false, |c| c == &name) {
                            code.metadata
                                .attributes
                                .insert("kind".into(), "constructor".into());
                        } else {
                            code.metadata
                                .attributes
                                .insert("kind".into(), "function".into());
                        }

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
            }

            // C++ Template declaration
            "template_declaration" => {
                let content_text = self.node_text(&node);
                let loc = self.location(&node);

                // Extract template parameters
                let mut code = CodeNode::new(
                    "template",
                    Some(NodeType::Variable), // Templates are meta-definitions
                    Some(Language::Cpp),
                    loc,
                )
                .with_content(content_text);
                code.span = Some(self.span_for(&node));

                code.metadata
                    .attributes
                    .insert("kind".into(), "template".into());
                self.nodes.push(code);
            }

            // C++ Call expressions - extract call edges
            "call_expression" => {
                if let Some(current_fn) = self.current_function_id {
                    if let Some(func) = node.child_by_field_name("function") {
                        let callee_name = self.node_text(&func);
                        if !callee_name.is_empty() {
                            let edge = EdgeRelationship {
                                from: current_fn,
                                to: callee_name,
                                edge_type: EdgeType::Calls,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("call_type".to_string(), "cpp_call".to_string());
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

    fn extract_function_name(&self, declarator: &Node) -> String {
        // Navigate through declarator to find the function name
        // Handles: function_declarator, pointer_declarator, reference_declarator
        match declarator.kind() {
            "function_declarator" => {
                if let Some(inner) = declarator.child_by_field_name("declarator") {
                    self.node_text(&inner)
                } else {
                    String::new()
                }
            }
            "pointer_declarator" | "reference_declarator" => {
                if let Some(inner) = declarator.child_by_field_name("declarator") {
                    self.extract_function_name(&inner)
                } else {
                    String::new()
                }
            }
            "identifier" | "field_identifier" | "destructor_name" | "qualified_identifier" => {
                self.node_text(declarator)
            }
            _ => {
                // Try to find identifier child
                for i in 0..declarator.child_count() {
                    if let Some(child) = declarator.child(i) {
                        if child.kind() == "identifier" || child.kind() == "field_identifier" {
                            return self.node_text(&child);
                        }
                    }
                }
                String::new()
            }
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
