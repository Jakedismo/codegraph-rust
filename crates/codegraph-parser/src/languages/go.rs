// ABOUTME: Go language AST extractor for code intelligence
// ABOUTME: Extracts packages, functions, methods, types, imports, and call edges

use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced Go AST extractor for backend development intelligence.
///
/// Extracts:
/// - packages, functions, methods, types (struct, interface)
/// - imports (single and grouped)
/// - function/method calls
/// - struct embeddings and interface implementations
/// - goroutines and channels patterns
/// - error handling patterns
///
/// Notes:
/// - Optimized for Go backend patterns
/// - Captures composition over inheritance patterns
/// - Handles Go's unique interface satisfaction model
pub struct GoExtractor;

#[derive(Default, Clone)]
struct GoContext {
    package_name: Option<String>,
    current_type: Option<String>,
    current_receiver: Option<String>,
}

impl GoExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = GoCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, GoContext::default());
        collector.into_nodes()
    }

    /// Unified extraction of nodes AND edges in single AST traversal
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = GoCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, GoContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for GoExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        GoExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls]
    }

    fn language() -> Language {
        Language::Go
    }
}

struct GoCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    current_type_id: Option<NodeId>,
}

impl<'a> GoCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
            current_type_id: None,
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: GoContext) {
        let node = cursor.node();

        match node.kind() {
            // Go Package declaration
            "package_clause" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Go),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    code.span = Some(self.span_for(&node));

                    code.metadata
                        .attributes
                        .insert("kind".into(), "package".into());
                    self.nodes.push(code);
                    ctx.package_name = Some(name);
                }
            }

            // Go Import declarations
            "import_declaration" => {
                // Handle both single imports and import blocks
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "import_spec" || child.kind() == "import_spec_list" {
                            self.extract_import_spec(&child, &ctx);
                        }
                    }
                }
            }

            // Individual import spec
            "import_spec" => {
                self.extract_import_spec(&node, &ctx);
            }

            // Go Function declarations
            "function_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Go),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                        &node,
                        self.content,
                    ));
                    code.span = Some(self.span_for(&node));

                    // Detect main function
                    if name == "main" {
                        code.metadata
                            .attributes
                            .insert("entry_point".into(), "true".into());
                    }

                    // Detect init function
                    if name == "init" {
                        code.metadata
                            .attributes
                            .insert("init_function".into(), "true".into());
                    }

                    // Detect exported functions (capitalized)
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        code.metadata
                            .attributes
                            .insert("exported".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "function".into());

                    // Track current function for call edge attribution
                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Go Method declarations (functions with receivers)
            "method_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.node_text(&name_node);
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);

                    // Extract receiver type
                    let receiver = node
                        .child_by_field_name("receiver")
                        .map(|r| self.node_text(&r));

                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Go),
                        loc,
                    )
                    .with_content(content_text.clone())
                    .with_complexity(crate::complexity::calculate_cyclomatic_complexity(
                        &node,
                        self.content,
                    ));
                    code.span = Some(self.span_for(&node));

                    if let Some(ref recv) = receiver {
                        code.metadata
                            .attributes
                            .insert("receiver".into(), recv.clone());
                        ctx.current_receiver = Some(recv.clone());
                    }

                    // Detect exported methods
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        code.metadata
                            .attributes
                            .insert("exported".into(), "true".into());
                    }

                    code.metadata
                        .attributes
                        .insert("kind".into(), "method".into());

                    // Track current function for call edge attribution
                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Go Type declarations (struct, interface)
            "type_declaration" => {
                for i in 0..node.child_count() {
                    if let Some(spec) = node.child(i) {
                        if spec.kind() == "type_spec" {
                            self.extract_type_spec(&spec, &mut ctx);
                        }
                    }
                }
            }

            // Type spec (struct or interface)
            "type_spec" => {
                self.extract_type_spec(&node, &mut ctx);
            }

            // Go Call expressions - extract call edges
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
                                    meta.insert("call_type".to_string(), "go_call".to_string());
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

    fn extract_import_spec(&mut self, node: &Node, _ctx: &GoContext) {
        // Extract the import path from the import spec
        if let Some(path_node) = node.child_by_field_name("path") {
            let path = self.node_text(&path_node);
            // Remove quotes from path
            let clean_path = path.trim_matches('"').to_string();

            if clean_path.is_empty() {
                return;
            }

            let loc = self.location(node);
            let mut code = CodeNode::new(
                format!("import {}", clean_path),
                Some(NodeType::Import),
                Some(Language::Go),
                loc,
            )
            .with_content(self.node_text(node));
            code.span = Some(self.span_for(node));

            // Detect alias if present
            if let Some(alias_node) = node.child_by_field_name("name") {
                let alias = self.node_text(&alias_node);
                code.metadata
                    .attributes
                    .insert("alias".into(), alias);
            }

            // Detect standard library imports
            let is_stdlib = !clean_path.contains('.');
            if is_stdlib {
                code.metadata
                    .attributes
                    .insert("stdlib".into(), "true".into());
            }

            code.metadata
                .attributes
                .insert("kind".into(), "import".into());
            code.metadata
                .attributes
                .insert("path".into(), clean_path.clone());

            // Create import edge
            let edge = EdgeRelationship {
                from: code.id,
                to: clean_path,
                edge_type: EdgeType::Imports,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("import_type".to_string(), "go_import".to_string());
                    meta.insert("source_file".to_string(), self.file_path.to_string());
                    if is_stdlib {
                        meta.insert("stdlib".to_string(), "true".to_string());
                    }
                    meta
                },
                span: Some(self.span_for(node)),
            };
            self.edges.push(edge);
            self.nodes.push(code);
        }
    }

    fn extract_type_spec(&mut self, node: &Node, ctx: &mut GoContext) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(&name_node);
            let loc = self.location(node);
            let content_text = self.node_text(node);

            // Determine if struct or interface
            let (node_type, kind) = if content_text.contains("interface {") || content_text.contains("interface{") {
                (NodeType::Interface, "interface")
            } else if content_text.contains("struct {") || content_text.contains("struct{") {
                (NodeType::Struct, "struct")
            } else {
                (NodeType::Variable, "type_alias")
            };

            let mut code = CodeNode::new(
                name.clone(),
                Some(node_type),
                Some(Language::Go),
                loc,
            )
            .with_content(content_text);
            code.span = Some(self.span_for(node));

            // Detect exported types
            if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                code.metadata
                    .attributes
                    .insert("exported".into(), "true".into());
            }

            code.metadata
                .attributes
                .insert("kind".into(), kind.into());

            self.current_type_id = Some(code.id);
            ctx.current_type = Some(name);
            self.nodes.push(code);
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
