use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// TypeScript/JavaScript extractor with framework-aware pattern detection
/// Supports: Express, NestJS, Next.js, React
pub struct TypeScriptExtractor;

#[derive(Default, Clone)]
struct FrameworkContext {
    framework_type: Option<String>, // "express", "nestjs", "nextjs", "react"
    is_route_handler: bool,
    is_api_route: bool,
    is_component: bool,
    current_decorator: Option<String>,
}

impl TypeScriptExtractor {
    pub fn extract_with_edges(
        tree: &Tree,
        content: &str,
        file_path: &str,
        language: Language,
    ) -> ExtractionResult {
        let mut collector = TypeScriptCollector::new(content, file_path, language);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, FrameworkContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for TypeScriptExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        TypeScriptExtractor::extract_with_edges(tree, content, file_path, Language::TypeScript)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[
            EdgeType::Imports,
            EdgeType::Calls,
            EdgeType::Extends,
            EdgeType::Implements,
        ]
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
    framework_type: Option<String>,
}

impl<'a> TypeScriptCollector<'a> {
    fn new(content: &'a str, file_path: &'a str, language: Language) -> Self {
        let framework_type = Self::detect_framework(content, file_path);
        Self {
            content,
            file_path,
            language,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
            framework_type,
        }
    }

    fn detect_framework(content: &str, file_path: &str) -> Option<String> {
        let lower = content.to_lowercase();
        let path_lower = file_path.to_lowercase();

        // Express indicators
        if lower.contains("from 'express'")
            || lower.contains("from \"express\"")
            || lower.contains("require('express')")
            || lower.contains("import express")
        {
            return Some("express".to_string());
        }

        // NestJS indicators
        if lower.contains("@nestjs")
            || lower.contains("@Controller")
            || lower.contains("@Injectable")
            || lower.contains("@Module")
        {
            return Some("nestjs".to_string());
        }

        // Next.js indicators
        if path_lower.contains("/pages/")
            || path_lower.contains("/app/")
            || lower.contains("getserverprops")
            || lower.contains("getstaticprops")
        {
            return Some("nextjs".to_string());
        }

        // React indicators
        if lower.contains("from 'react'")
            || lower.contains("from \"react\"")
            || lower.contains("jsx")
        {
            return Some("react".to_string());
        }

        None
    }

    fn into_result(self) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
        }
    }

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: FrameworkContext) {
        let node = cursor.node();
        let node_text = self.node_text(&node);
        let lower_text = node_text.to_lowercase();

        match node.kind() {
            // Decorators for NestJS patterns
            "decorator" => {
                ctx.current_decorator = Some(node_text.clone());
                if lower_text.contains("@get")
                    || lower_text.contains("@post")
                    || lower_text.contains("@put")
                    || lower_text.contains("@delete")
                    || lower_text.contains("@patch")
                {
                    ctx.is_route_handler = true;
                }
                if lower_text.contains("@Controller") {
                    ctx.is_route_handler = true;
                }
            }

            // Express route handlers (app.get, router.get, etc.)
            "method_definition" => {
                if let Some(name) = self.extract_method_name(&node) {
                    let loc = self.location(&node);
                    let mut node_type = NodeType::Function;
                    let mut meta = HashMap::new();

                    // Framework-specific detection
                    if let Some(ref fw) = self.framework_type {
                        match fw.as_str() {
                            "express" => {
                                // Express methods: get, post, put, delete, patch, use
                                if ["get", "post", "put", "delete", "patch", "use", "head", "options"]
                                    .contains(&name.as_str())
                                {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "express".to_string());
                                    meta.insert("pattern".to_string(), "route".to_string());
                                }
                            }
                            "nestjs" => {
                                if ctx.is_route_handler {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "nestjs".to_string());
                                    meta.insert("pattern".to_string(), "endpoint".to_string());
                                }
                            }
                            "nextjs" => {
                                if name == "GET"
                                    || name == "POST"
                                    || name == "PUT"
                                    || name == "DELETE"
                                    || name == "PATCH"
                                {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "nextjs".to_string());
                                    meta.insert("pattern".to_string(), "apiroute".to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    let mut code = CodeNode::new(
                        name,
                        Some(node_type),
                        Some(self.language.clone()),
                        loc,
                    )
                    .with_content(node_text.clone());
                    code.span = Some(self.span_for(&node));
                    if !meta.is_empty() {
                        code.metadata.attributes.extend(meta);
                    }

                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Functions (arrow functions, function expressions)
            "function_declaration" | "function_expression" | "arrow_function" => {
                if let Some(name) = self.extract_function_name(&node) {
                    let loc = self.location(&node);
                    let mut node_type = NodeType::Function;
                    let mut meta = HashMap::new();

                    // Framework detection for functions
                    if let Some(ref fw) = self.framework_type {
                        match fw.as_str() {
                            "express" => {
                                // Check if it's an Express route handler
                                if lower_text.contains("app.get")
                                    || lower_text.contains("router.get")
                                    || lower_text.contains("app.post")
                                    || lower_text.contains("router.post")
                                {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "express".to_string());
                                    meta.insert("pattern".to_string(), "route_handler".to_string());
                                }
                            }
                            "react" => {
                                // React components are PascalCase functions
                                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                                    && !name.starts_with("use")
                                {
                                    node_type = NodeType::Class;
                                    meta.insert("framework".to_string(), "react".to_string());
                                    meta.insert("pattern".to_string(), "component".to_string());
                                }
                                // React hooks
                                if name.starts_with("use") {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "react".to_string());
                                    meta.insert("pattern".to_string(), "hook".to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    let mut code = CodeNode::new(
                        name,
                        Some(node_type),
                        Some(self.language.clone()),
                        loc,
                    )
                    .with_content(node_text.clone())
                    .with_complexity(crate::complexity::calculate_cyclomatic_complexity(&node, self.content));
                    code.span = Some(self.span_for(&node));
                    if !meta.is_empty() {
                        code.metadata.attributes.extend(meta);
                    }

                    self.current_function_id = Some(code.id);
                    self.nodes.push(code);
                }
            }

            // Class declarations (React components, NestJS controllers)
            "class_declaration" => {
                if let Some(name) = self.extract_class_name(&node) {
                    let loc = self.location(&node);
                    let mut node_type = NodeType::Class;
                    let mut meta = HashMap::new();

                    if let Some(ref fw) = self.framework_type {
                        match fw.as_str() {
                            "nestjs" => {
                                if lower_text.contains("@controller")
                                    || lower_text.contains("@injectable")
                                    || lower_text.contains("@service")
                                {
                                    node_type = NodeType::Class;
                                    meta.insert("framework".to_string(), "nestjs".to_string());
                                    if lower_text.contains("@controller") {
                                        meta.insert("pattern".to_string(), "controller".to_string());
                                    } else if lower_text.contains("@service") {
                                        meta.insert("pattern".to_string(), "service".to_string());
                                    } else {
                                        meta.insert("pattern".to_string(), "injectable".to_string());
                                    }
                                }
                            }
                            "react" => {
                                if name.ends_with("Component") || name.ends_with("Page") {
                                    node_type = NodeType::Class;
                                    meta.insert("framework".to_string(), "react".to_string());
                                    meta.insert("pattern".to_string(), "component".to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    let mut code = CodeNode::new(
                        name,
                        Some(node_type),
                        Some(self.language.clone()),
                        loc,
                    )
                    .with_content(node_text.clone());
                    code.span = Some(self.span_for(&node));
                    if !meta.is_empty() {
                        code.metadata.attributes.extend(meta);
                    }

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
                    .with_content(node_text.clone());
                    code.span = Some(self.span_for(&node));

                    let meta: HashMap<String, String> = {
                        let mut m = HashMap::new();
                        m.insert("import_type".to_string(), "es_import".to_string());
                        m.insert("source_file".to_string(), self.file_path.to_string());
                        m
                    };
                    code.metadata.attributes = meta.clone();

                    let edge = EdgeRelationship {
                        from: code.id,
                        to: name,
                        edge_type: EdgeType::Imports,
                        metadata: meta,
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
                        let meta: HashMap<String, String> = {
                            let mut m = HashMap::new();
                            m.insert("call_type".to_string(), "function_call".to_string());
                            m.insert("source_file".to_string(), self.file_path.to_string());
                            m
                        };
                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: function_name,
                            edge_type: EdgeType::Calls,
                            metadata: meta,
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
                self.walk(cursor, ctx.clone());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_method_name(&self, node: &Node) -> Option<String> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.node_text(&name_node));
        }
        None
    }

    fn extract_function_name(&self, node: &Node) -> Option<String> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.node_text(&name_node));
        }
        self.child_text_by_kinds(node, &["identifier", "property_identifier"])
    }

    fn extract_class_name(&self, node: &Node) -> Option<String> {
        self.child_text_by_kinds(node, &["type_identifier", "identifier"])
    }

    fn extract_import_source(&self, node: &Node) -> Option<String> {
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
        self.child_text_by_kinds(node, &["identifier", "member_expression"])
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

    fn child_text_by_kinds(&self, node: &Node, kinds: &[&str]) -> Option<String> {
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
    Vec::new()
}
