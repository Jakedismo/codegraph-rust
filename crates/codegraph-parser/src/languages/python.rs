use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
    Span,
};
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// REVOLUTIONARY: Python extractor with unified node+edge extraction
/// Now with framework-aware pattern detection for Django, Flask, and FastAPI
pub struct PythonExtractor;

#[derive(Default, Clone)]
struct FrameworkContext {
    framework_type: Option<String>, // "django", "flask", "fastapi"
    is_api_view: bool,
    is_url_pattern: bool,
    is_model: bool,
    is_viewset: bool,
    current_decorator: Option<String>,
}

impl PythonExtractor {
    /// Extract nodes and edges in single AST traversal for maximum speed
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        let mut collector = PythonCollector::new(content, file_path);
        let mut cursor = tree.walk();
        collector.walk(&mut cursor, FrameworkContext::default());
        collector.into_result()
    }
}

impl super::LanguageExtractor for PythonExtractor {
    fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        PythonExtractor::extract_with_edges(tree, content, file_path)
    }

    fn supported_edge_types() -> &'static [EdgeType] {
        &[EdgeType::Imports, EdgeType::Calls, EdgeType::Extends]
    }

    fn language() -> Language {
        Language::Python
    }
}

struct PythonCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_function_id: Option<NodeId>,
    current_class_id: Option<NodeId>,
    framework_type: Option<String>,
}

impl<'a> PythonCollector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        // Detect framework from file path and content
        let framework_type = Self::detect_framework(content, file_path);
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_function_id: None,
            current_class_id: None,
            framework_type,
        }
    }

    fn detect_framework(content: &str, file_path: &str) -> Option<String> {
        let lower = content.to_lowercase();
        // Django indicators
        if lower.contains("from django") || lower.contains("import django") {
            return Some("django".to_string());
        }
        // Flask indicators
        if lower.contains("from flask") || lower.contains("import flask") || lower.contains("flask=") {
            return Some("flask".to_string());
        }
        // FastAPI indicators
        if lower.contains("from fastapi") || lower.contains("import fastapi") || lower.contains("fastapi=") {
            return Some("fastapi".to_string());
        }
        // Check file path for framework conventions
        if file_path.contains("/django/") || file_path.contains("\\django\\") {
            return Some("django".to_string());
        }
        if file_path.contains("/flask/") || file_path.contains("\\flask\\") {
            return Some("flask".to_string());
        }
        if file_path.contains("/fastapi/") || file_path.contains("\\fastapi\\") {
            return Some("fastapi".to_string());
        }
        None
    }

    fn span_for(&self, node: Node) -> Span {
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: FrameworkContext) {
        let node = cursor.node();
        let node_text = self.node_text(&node);
        let lower_text = node_text.to_lowercase();

        match node.kind() {
            // Decorator detection for framework patterns
            "decorator" => {
                ctx.current_decorator = Some(node_text.clone());
                if lower_text.contains("@app.route")
                    || lower_text.contains("@router.route")
                    || lower_text.contains("@api_view")
                {
                    ctx.is_api_view = true;
                }
                if lower_text.contains("route(") || lower_text.contains("get(") || lower_text.contains("post(") {
                    ctx.is_url_pattern = true;
                }
                if lower_text.contains("@api_view") {
                    ctx.is_api_view = true;
                }
            }

            // Django/Flask/FastAPI class-based views
            "class_definition" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(node);
                    let mut node_type = NodeType::Class;
                    let mut meta = HashMap::new();

                    // Framework-specific class detection
                    if let Some(ref fw) = self.framework_type {
                        match fw.as_str() {
                            "django" => {
                                if name.ends_with("View") || name.ends_with("ViewSet") || name.ends_with("APIView") {
                                    node_type = NodeType::Class;
                                    meta.insert("framework".to_string(), "django".to_string());
                                    meta.insert("pattern".to_string(), "view".to_string());
                                }
                                if name.ends_with("Model") || lower_text.contains("class meta:") {
                                    node_type = NodeType::Class;
                                    meta.insert("framework".to_string(), "django".to_string());
                                    meta.insert("pattern".to_string(), "model".to_string());
                                }
                            }
                            "fastapi" => {
                                if lower_text.contains("APIRouter") || lower_text.contains("Depends") {
                                    meta.insert("framework".to_string(), "fastapi".to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    let mut code = CodeNode::new(name, Some(node_type), Some(Language::Python), loc)
                        .with_content(node_text.clone());
                    code.span = Some(self.span_for(node));
                    if !meta.is_empty() {
                        code.metadata.attributes.extend(meta);
                    }

                    self.current_class_id = Some(code.id);
                    self.extract_base_classes(node, code.id);
                    self.nodes.push(code);
                }
            }

            // Django/Flask/FastAPI route handlers (function-based views)
            "function_definition" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(node);
                    let mut node_type = NodeType::Function;
                    let mut meta = HashMap::new();

                    // Framework-specific function detection
                    if let Some(ref fw) = self.framework_type {
                        match fw.as_str() {
                            "django" => {
                                if ctx.is_api_view || ctx.is_url_pattern {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "django".to_string());
                                    meta.insert("pattern".to_string(), "view".to_string());
                                    // Extract URL pattern from decorator
                                    if let Some(ref dec) = ctx.current_decorator {
                                        if let Some(pattern) = self.extract_url_pattern(dec) {
                                            meta.insert("route".to_string(), pattern);
                                        }
                                    }
                                }
                            }
                            "flask" => {
                                if ctx.is_api_view || ctx.is_url_pattern {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "flask".to_string());
                                    meta.insert("pattern".to_string(), "route".to_string());
                                    if let Some(ref dec) = ctx.current_decorator {
                                        if let Some(pattern) = self.extract_url_pattern(dec) {
                                            meta.insert("route".to_string(), pattern);
                                        }
                                    }
                                }
                            }
                            "fastapi" => {
                                // FastAPI functions are typically route handlers
                                if lower_text.contains("@app")
                                    || lower_text.contains("@router")
                                    || lower_text.contains("@get")
                                    || lower_text.contains("@post")
                                    || lower_text.contains("@put")
                                    || lower_text.contains("@delete")
                                    || lower_text.contains("@patch")
                                {
                                    node_type = NodeType::Function;
                                    meta.insert("framework".to_string(), "fastapi".to_string());
                                    meta.insert("pattern".to_string(), "endpoint".to_string());
                                    // Extract path from decorator
                                    if let Some(ref dec) = ctx.current_decorator {
                                        if let Some(path) = self.extract_url_pattern(dec) {
                                            meta.insert("route".to_string(), path);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    let mut code = CodeNode::new(name, Some(node_type), Some(Language::Python), loc)
                        .with_content(node_text.clone())
                        .with_complexity(crate::complexity::calculate_cyclomatic_complexity(&node, self.content));
                    code.span = Some(self.span_for(node));
                    if !meta.is_empty() {
                        code.metadata.attributes.extend(meta);
                    }

                    self.current_function_id = Some(code.id);
                    self.extract_type_hints(node, code.id);
                    self.nodes.push(code);
                }
            }

            // Import statements
            "import_statement" | "import_from_statement" => {
                if let Some(name) = self.extract_import_name(node) {
                    let loc = self.location(node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Import),
                        Some(Language::Python),
                        loc,
                    )
                    .with_content(node_text.clone());
                    code.span = Some(self.span_for(node));

                    // Framework-specific imports
                    let meta: HashMap<String, String> = {
                        let mut m = HashMap::new();
                        m.insert("import_type".to_string(), "python_import".to_string());
                        m.insert("source_file".to_string(), self.file_path.to_string());
                        m
                    };
                    code.metadata.attributes = meta.clone();

                    let edge = EdgeRelationship {
                        from: code.id,
                        to: name,
                        edge_type: EdgeType::Imports,
                        metadata: meta,
                        span: Some(self.span_for(node)),
                    };
                    self.edges.push(edge);
                    self.nodes.push(code);
                }
            }

            // Function calls
            "call" => {
                if let Some(current_fn) = self.current_function_id {
                    if let Some(function_name) = self.extract_call_target(node) {
                        let mut meta = HashMap::new();
                        meta.insert("call_type".to_string(), "function_call".to_string());
                        meta.insert("source_file".to_string(), self.file_path.to_string());

                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: function_name,
                            edge_type: EdgeType::Calls,
                            metadata: meta,
                            span: Some(self.span_for(node)),
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

    /// Extract URL path pattern from decorator
    fn extract_url_pattern(&self, decorator: &str) -> Option<String> {
        // Match patterns like @app.route("/path"), @router.get("/api"), @get("/users")
        if let Some(start) = decorator.find('"') {
            let rest = &decorator[start + 1..];
            if let Some(end) = rest.find('"') {
                return Some(rest[..end].to_string());
            }
        }
        if let Some(start) = decorator.find('\'') {
            let rest = &decorator[start + 1..];
            if let Some(end) = rest.find('\'') {
                return Some(rest[..end].to_string());
            }
        }
        None
    }

    fn extract_import_name(&self, node: Node) -> Option<String> {
        if node.kind() == "import_statement" {
            self.child_text_by_kinds(node, &["dotted_name", "identifier"])
        } else if node.kind() == "import_from_statement" {
            self.child_text_by_kinds(node, &["dotted_name", "relative_import"])
        } else {
            None
        }
    }

    fn extract_call_target(&self, node: Node) -> Option<String> {
        if let Some(function_node) = node.child_by_field_name("function") {
            return Some(self.node_text(&function_node));
        }
        self.child_text_by_kinds(node, &["identifier", "attribute"])
    }

    fn location(&self, node: Node) -> Location {
        Location {
            file_path: self.file_path.to_string(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        }
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes()).unwrap_or("").to_string()
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

    fn extract_type_hints(&mut self, node: Node, from_id: NodeId) {
        if let Some(parameters) = node.child_by_field_name("parameters") {
            let mut cursor = parameters.walk();
            if cursor.goto_first_child() {
                loop {
                    let param = cursor.node();
                    match param.kind() {
                        "typed_parameter" | "typed_default_parameter" => {
                            if let Some(type_node) = param.child_by_field_name("type") {
                                self.add_reference_edge(from_id, type_node, "parameter_type");
                            }
                        }
                        _ => {}
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        if let Some(return_type) = node.child_by_field_name("return_type") {
            self.add_reference_edge(from_id, return_type, "return_type");
        }
    }

    fn extract_base_classes(&mut self, node: Node, from_id: NodeId) {
        if let Some(superclasses) = node.child_by_field_name("superclasses") {
            let mut cursor = superclasses.walk();
            if cursor.goto_first_child() {
                loop {
                    let superclass = cursor.node();
                    if superclass.kind() == "identifier" || superclass.kind() == "attribute" {
                        self.add_reference_edge(from_id, superclass, "base_class");
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    fn add_reference_edge(&mut self, from_id: NodeId, node: Node, kind: &str) {
        let name = self.node_text(&node);
        if !name.is_empty() {
            self.edges.push(EdgeRelationship {
                from: from_id,
                to: name,
                edge_type: EdgeType::References,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("kind".to_string(), kind.to_string());
                    meta.insert("source_file".to_string(), self.file_path.to_string());
                    meta
                },
                span: Some(self.span_for(node)),
            });
        }
    }
}

/// Backward compatibility function
#[derive(Debug, Default, Clone)]
pub struct PythonExtraction {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<crate::edge::CodeEdge>,
}

pub fn extract_python(_file_path: &str, _source: &str) -> PythonExtraction {
    // Stub for backward compatibility
    PythonExtraction::default()
}
