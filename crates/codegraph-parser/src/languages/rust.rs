use crate::real_ai_integration::extract_with_real_ai_enhancement;
use codegraph_core::{
    CodeNode, EdgeRelationship, EdgeType, ExtractionResult, Language, Location, NodeId, NodeType,
};
use serde_json::json;
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced Rust AST extractor using tree-sitter-rust.
///
/// Extracts:
/// - structs, traits, impls, functions, enums, modules
/// - tracks trait implementations, generic parameters, lifetimes
/// - builds dependency info for `use` statements (stored in node metadata)
/// - handles macros, async functions, unsafe blocks
///
/// Notes:
/// - We encode rich details in `CodeNode.metadata.attributes` to avoid API changes.
/// - Names are kept simple; qualified names and contexts are added as metadata.
pub struct RustExtractor;

// REVOLUTIONARY: AI-enhanced extraction implemented below
// REVOLUTIONARY: AI-enhanced extraction implemented through real_ai_integration module

#[derive(Default, Clone)]
struct WalkContext {
    module_path: Vec<String>,
    current_impl_for: Option<String>,
    current_impl_trait: Option<String>,
}

impl RustExtractor {
    /// Extract only nodes for backward compatibility
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        Self::extract_with_edges(tree, content, file_path).nodes
    }

    /// REVOLUTIONARY: Extract BOTH nodes and edges in single AST traversal for maximum speed
    /// ENHANCED: Now includes AI pattern learning for improved accuracy
    pub fn extract_with_edges(tree: &Tree, content: &str, file_path: &str) -> ExtractionResult {
        // Traditional high-speed extraction
        let base_result = {
            let mut collector = Collector::new(content, file_path);
            let mut cursor = tree.walk();
            collector.walk(&mut cursor, WalkContext::default());
            collector.into_result()
        };

        // REVOLUTIONARY: AI-enhanced extraction using learned patterns
        extract_with_real_ai_enhancement(|| base_result, Language::Rust, file_path)
    }
}

struct Collector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
    current_node_id: Option<NodeId>, // Track current context for edge relationships
}

impl<'a> Collector<'a> {
    fn new(content: &'a str, file_path: &'a str) -> Self {
        Self {
            content,
            file_path,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_node_id: None,
        }
    }

    /// REVOLUTIONARY: Return both nodes and edges from single AST traversal
    fn into_result(self) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
        }
    }

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: WalkContext) {
        let node = cursor.node();

        match node.kind() {
            // Modules
            "mod_item" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Rust),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    let qn = self.qname(&ctx.module_path, &name);
                    code.metadata
                        .attributes
                        .insert("kind".into(), "module".into());
                    code.metadata.attributes.insert("qualified_name".into(), qn);
                    self.nodes.push(code);

                    // push module path for nested items only if there's a body
                    let has_body = node.child_by_field_name("body").is_some();
                    if has_body {
                        ctx.module_path.push(name);
                    }

                    if cursor.goto_first_child() {
                        loop {
                            self.walk(cursor, ctx.clone());
                            if !cursor.goto_next_sibling() {
                                break;
                            }
                        }
                        cursor.goto_parent();
                    }

                    if has_body {
                        ctx.module_path.pop();
                    }
                    return; // already recursed children
                }
            }

            // Imports (use)
            "use_declaration" => {
                let text = self.node_text(&node);
                let imports = parse_use_declaration(text.as_ref());
                let name = imports
                    .iter()
                    .map(|i| i.full_path.clone())
                    .next()
                    .unwrap_or_else(|| "use".into());
                let loc = self.location(&node);
                let mut code =
                    CodeNode::new(name, Some(NodeType::Import), Some(Language::Rust), loc)
                        .with_content(self.node_text(&node));
                // Store full parsed import list and graph-friendly edges as metadata
                code.metadata
                    .attributes
                    .insert("imports".into(), json!(imports).to_string());
                code.metadata.attributes.insert(
                    "qualified_name".into(),
                    self.qname(&ctx.module_path, code.name.as_str()),
                );

                // REVOLUTIONARY: Extract import edges during same AST traversal
                let import_node_id = code.id;
                for import in &imports {
                    let edge = EdgeRelationship {
                        from: import_node_id,
                        to: import.full_path.clone(),
                        edge_type: EdgeType::Imports,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("import_type".to_string(), "use_declaration".to_string());
                            meta.insert("source_file".to_string(), self.file_path.to_string());
                            meta
                        },
                    };
                    self.edges.push(edge);
                }

                self.nodes.push(code);
            }

            // Trait
            "trait_item" => {
                if let Some(name) = self.child_text_by_kinds(node, &["type_identifier"]) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Trait),
                        Some(Language::Rust),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    let (generics, lifetimes) = self.collect_generics_and_lifetimes(node);
                    code.metadata
                        .attributes
                        .insert("generics".into(), json!(generics).to_string());
                    code.metadata
                        .attributes
                        .insert("lifetimes".into(), json!(lifetimes).to_string());
                    code.metadata
                        .attributes
                        .insert("qualified_name".into(), self.qname(&ctx.module_path, &name));
                    self.nodes.push(code);
                }
            }

            // Struct
            "struct_item" => {
                if let Some(name) = self.child_text_by_kinds(node, &["type_identifier"]) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Struct),
                        Some(Language::Rust),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    let (generics, lifetimes) = self.collect_generics_and_lifetimes(node);
                    code.metadata
                        .attributes
                        .insert("generics".into(), json!(generics).to_string());
                    code.metadata
                        .attributes
                        .insert("lifetimes".into(), json!(lifetimes).to_string());
                    code.metadata
                        .attributes
                        .insert("qualified_name".into(), self.qname(&ctx.module_path, &name));
                    self.nodes.push(code);
                }
            }

            // Enum
            "enum_item" => {
                if let Some(name) = self.child_text_by_kinds(node, &["type_identifier"]) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Enum),
                        Some(Language::Rust),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    let (generics, lifetimes) = self.collect_generics_and_lifetimes(node);
                    code.metadata
                        .attributes
                        .insert("generics".into(), json!(generics).to_string());
                    code.metadata
                        .attributes
                        .insert("lifetimes".into(), json!(lifetimes).to_string());
                    code.metadata
                        .attributes
                        .insert("qualified_name".into(), self.qname(&ctx.module_path, &name));
                    self.nodes.push(code);
                }
            }

            // Impl (inherent or trait impl)
            "impl_item" => {
                let loc = self.location(&node);
                let text = self.node_text(&node);
                let impl_info = parse_impl_signature(&node, self.content)
                    .unwrap_or_else(|| parse_impl_signature_text(&text));

                let mut code = CodeNode::new(
                    impl_info.for_type.clone().unwrap_or_else(|| "impl".into()),
                    Some(NodeType::Other("impl".into())),
                    Some(Language::Rust),
                    loc,
                )
                .with_content(text.clone());
                if let Some(for_type) = &impl_info.for_type {
                    code.metadata
                        .attributes
                        .insert("impl_for".into(), for_type.clone());
                }
                if let Some(trait_name) = &impl_info.trait_name {
                    code.metadata
                        .attributes
                        .insert("impl_trait".into(), trait_name.clone());
                }
                code.metadata.attributes.insert(
                    "inherent".into(),
                    json!(impl_info.trait_name.is_none()).to_string(),
                );
                code.metadata
                    .attributes
                    .insert("generics".into(), json!(impl_info.generics).to_string());
                code.metadata
                    .attributes
                    .insert("lifetimes".into(), json!(impl_info.lifetimes).to_string());
                code.metadata.attributes.insert(
                    "qualified_name".into(),
                    self.qname(&ctx.module_path, code.name.as_str()),
                );
                self.nodes.push(code);

                // Walk methods inside impl with context
                let mut next_ctx = ctx.clone();
                next_ctx.current_impl_for = impl_info.for_type;
                next_ctx.current_impl_trait = impl_info.trait_name;

                if cursor.goto_first_child() {
                    loop {
                        self.walk(cursor, next_ctx.clone());
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                    cursor.goto_parent();
                }
                return; // done own recursion
            }

            // Functions (includes methods inside impl)
            "function_item" => {
                if let Some(name) = self.child_text_by_kinds(node, &["identifier"]) {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Rust),
                        loc,
                    )
                    .with_content(self.node_text(&node));
                    let (generics, lifetimes) = self.collect_generics_and_lifetimes(node);
                    code.metadata
                        .attributes
                        .insert("generics".into(), json!(generics).to_string());
                    code.metadata
                        .attributes
                        .insert("lifetimes".into(), json!(lifetimes).to_string());
                    let is_async =
                        self.node_text(&node).contains("async fn") || has_child_kind(node, "async");
                    let is_unsafe = self.node_text(&node).contains("unsafe fn")
                        || has_child_kind(node, "unsafe");
                    let has_unsafe_block = subtree_contains_kind(&node, "unsafe_block");
                    code.metadata
                        .attributes
                        .insert("async".into(), json!(is_async).to_string());
                    code.metadata
                        .attributes
                        .insert("unsafe".into(), json!(is_unsafe).to_string());
                    code.metadata.attributes.insert(
                        "has_unsafe_block".into(),
                        json!(has_unsafe_block).to_string(),
                    );
                    if let Some(for_type) = &ctx.current_impl_for {
                        code.metadata
                            .attributes
                            .insert("method_of".into(), for_type.clone());
                    }
                    if let Some(trait_name) = &ctx.current_impl_trait {
                        code.metadata
                            .attributes
                            .insert("implements_trait".into(), trait_name.clone());
                    }
                    code.metadata
                        .attributes
                        .insert("qualified_name".into(), self.qname_with_impl(&ctx, &name));

                    // REVOLUTIONARY: Set function context for edge extraction during AST traversal
                    let function_node_id = code.id;
                    self.current_node_id = Some(function_node_id);

                    self.nodes.push(code);
                }
            }

            // Macros
            "macro_invocation" => {
                // Try to get macro name: often in child field "macro" -> identifier or scoped_identifier
                let macro_name = self
                    .child_by_field_text(node, "macro")
                    .or_else(|| {
                        self.child_text_by_kinds(node, &["identifier", "scoped_identifier"])
                    })
                    .unwrap_or_else(|| "macro".into());
                let loc = self.location(&node);
                let mut code = CodeNode::new(
                    macro_name,
                    Some(NodeType::Other("macro_invocation".into())),
                    Some(Language::Rust),
                    loc,
                )
                .with_content(self.node_text(&node));
                code.metadata.attributes.insert(
                    "qualified_name".into(),
                    self.qname(&ctx.module_path, code.name.as_str()),
                );
                self.nodes.push(code);
            }

            // Function calls (extract edges during AST traversal for MAXIMUM SPEED)
            "call_expression" => {
                if let Some(current_fn) = self.current_node_id {
                    if let Some(function_name) = self.extract_call_target(&node) {
                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: function_name.clone(),
                            edge_type: EdgeType::Calls,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("call_type".to_string(), "function_call".to_string());
                                meta.insert("source_file".to_string(), self.file_path.to_string());
                                meta
                            },
                        };
                        self.edges.push(edge);
                    }
                }
            }

            // Method calls (method.function() syntax)
            "method_call_expression" => {
                if let Some(current_fn) = self.current_node_id {
                    if let Some(method_name) = self.child_text_by_kinds(node, &["field_identifier"])
                    {
                        let edge = EdgeRelationship {
                            from: current_fn,
                            to: method_name.clone(),
                            edge_type: EdgeType::Calls,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("call_type".to_string(), "method_call".to_string());
                                meta.insert("source_file".to_string(), self.file_path.to_string());
                                meta
                            },
                        };
                        self.edges.push(edge);
                    }
                }
            }

            _ => {}
        }

        // Recurse into children generically
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

    fn location(&self, node: &Node) -> Location {
        Location {
            file_path: self.file_path.to_string(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        }
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes())
            .unwrap_or("")
            .to_string()
    }

    fn child_text_by_kinds(&self, node: Node, kinds: &[&str]) -> Option<String> {
        let mut c = node.walk();
        if c.goto_first_child() {
            loop {
                let n = c.node();
                if kinds.iter().any(|k| n.kind() == *k) {
                    return Some(self.node_text(&n));
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn child_by_field_text(&self, node: Node, field: &str) -> Option<String> {
        node.child_by_field_name(field).map(|n| self.node_text(&n))
    }

    fn collect_generics_and_lifetimes(&self, node: Node) -> (Vec<String>, Vec<String>) {
        let mut generics = Vec::new();
        let mut lifetimes = Vec::new();

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let n = cursor.node();
                match n.kind() {
                    "type_parameters" => {
                        self.collect_type_parameters(n, &mut generics, &mut lifetimes);
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        (generics, lifetimes)
    }

    fn collect_type_parameters(
        &self,
        node: Node,
        generics: &mut Vec<String>,
        lifetimes: &mut Vec<String>,
    ) {
        let mut c = node.walk();
        if c.goto_first_child() {
            loop {
                let n = c.node();
                match n.kind() {
                    "type_parameter" => {
                        if let Some(t) = self.child_text_by_kinds(n, &["type_identifier"]) {
                            generics.push(t);
                        }
                    }
                    "lifetime_parameter" => {
                        if let Some(lf) = self.child_text_by_kinds(n, &["lifetime"]) {
                            lifetimes.push(lf);
                        }
                    }
                    "const_parameter" => {
                        if let Some(id) = self.child_text_by_kinds(n, &["identifier"]) {
                            generics.push(format!("const {}", id));
                        }
                    }
                    _ => {}
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn qname(&self, path: &[String], name: &str) -> String {
        if path.is_empty() {
            format!("{}::{}", self.file_path, name)
        } else {
            format!("{}::{}::{}", self.file_path, path.join("::"), name)
        }
    }

    fn qname_with_impl(&self, ctx: &WalkContext, name: &str) -> String {
        match (&ctx.current_impl_for, &ctx.current_impl_trait) {
            (Some(t), Some(tr)) => format!("{}::impl<{} for {}>::{}", self.file_path, tr, t, name),
            (Some(t), None) => format!("{}::impl<{}>::{}", self.file_path, t, name),
            _ => self.qname(&ctx.module_path, name),
        }
    }

    /// Extract function call target for edge relationships (FASTEST edge extraction)
    fn extract_call_target(&self, node: &Node) -> Option<String> {
        // Try different patterns for function calls
        if let Some(function_node) = node.child_by_field_name("function") {
            // Direct function call: func_name()
            if function_node.kind() == "identifier" {
                return Some(self.node_text(&function_node));
            }
            // Scoped call: mod::func_name()
            if function_node.kind() == "scoped_identifier" {
                return Some(self.node_text(&function_node));
            }
            // Field access: obj.method()
            if function_node.kind() == "field_expression" {
                if let Some(field) = function_node.child_by_field_name("field") {
                    return Some(self.node_text(&field));
                }
            }
        }

        // Fallback: extract any identifier from the call
        self.child_text_by_kinds(
            *node,
            &["identifier", "scoped_identifier", "field_identifier"],
        )
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct ImportItem {
    full_path: String,
    alias: Option<String>,
    is_glob: bool,
}

fn parse_use_declaration(text: &str) -> Vec<ImportItem> {
    // Handles common forms: use a::b; use a::b as c; use a::{b, c as d, *}; use super::a; use crate::x::y;
    let mut s = text.trim();
    if s.starts_with("use ") {
        s = &s[4..];
    }
    if let Some(idx) = s.rfind(';') {
        s = &s[..idx];
    }

    // Brace group
    if let Some(open) = s.find('{') {
        let prefix = s[..open].trim().trim_end_matches("::");
        let rest = &s[open + 1..];
        let close = rest.rfind('}').unwrap_or(rest.len());
        let inside = &rest[..close];
        let mut out = Vec::new();
        for part in inside.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            if p == "*" {
                out.push(ImportItem {
                    full_path: format!("{}::*", prefix),
                    alias: None,
                    is_glob: true,
                });
                continue;
            }
            let (name, alias) = split_alias(p);
            out.push(ImportItem {
                full_path: format!("{}::{}", prefix, name),
                alias,
                is_glob: false,
            });
        }
        return out;
    }

    let (name, alias) = split_alias(s.trim());
    vec![ImportItem {
        full_path: name.to_string(),
        alias,
        is_glob: name.ends_with("::*"),
    }]
}

fn split_alias(s: &str) -> (String, Option<String>) {
    // e.g., "foo as bar" -> ("foo", Some("bar"))
    if let Some(idx) = s.rfind(" as ") {
        (
            s[..idx].trim().to_string(),
            Some(s[idx + 4..].trim().to_string()),
        )
    } else {
        (s.trim().to_string(), None)
    }
}

#[derive(Default, Debug)]
struct ImplInfo {
    trait_name: Option<String>,
    for_type: Option<String>,
    generics: Vec<String>,
    lifetimes: Vec<String>,
}

fn parse_impl_signature(node: &Node, content: &str) -> Option<ImplInfo> {
    // Prefer AST-based extraction using fields when available
    let mut info = ImplInfo::default();
    if let Some(tp) = node.child_by_field_name("type_parameters") {
        // Collect generics and lifetimes
        let mut c = tp.walk();
        if c.goto_first_child() {
            loop {
                let n = c.node();
                match n.kind() {
                    "type_parameter" => {
                        if let Some(t) = child_text_by_kinds(n, content, &["type_identifier"]) {
                            info.generics.push(t);
                        }
                    }
                    "lifetime_parameter" => {
                        if let Some(lf) = child_text_by_kinds(n, content, &["lifetime"]) {
                            info.lifetimes.push(lf);
                        }
                    }
                    _ => {}
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    if let Some(tr) = node.child_by_field_name("trait") {
        info.trait_name = Some(tr.utf8_text(content.as_bytes()).ok()?.to_string());
    }
    if let Some(ty) = node.child_by_field_name("type") {
        info.for_type = Some(ty.utf8_text(content.as_bytes()).ok()?.to_string());
    }
    Some(info)
}

fn parse_impl_signature_text(text: &str) -> ImplInfo {
    // Fallback robust text-based parse
    // Matches forms like: impl<T> Trait for Type { ... } and impl<T> Type { ... }
    let mut info = ImplInfo::default();
    // generics
    if let Some(start) = text.find('<') {
        if let Some(end) = matching_angle(text, start) {
            let inside = &text[start + 1..end];
            for tok in inside.split(',') {
                let t = tok.trim();
                if t.is_empty() {
                    continue;
                }
                if t.starts_with('\'') {
                    info.lifetimes.push(t.to_string());
                } else if t.starts_with("const ") {
                    info.generics.push(t.to_string());
                } else {
                    // remove bounds (after ':') for a clean name
                    let name = t.split(':').next().unwrap_or(t).trim().to_string();
                    info.generics.push(name);
                }
            }
        }
    }
    let s = text.replace('\n', " ");
    if let Some(idx) = s.find(" for ") {
        // trait impl
        let head = s.trim_start_matches("impl").trim();
        let trait_part = head[..idx - 0].trim();
        let after_for = &s[idx + 5..];
        let ty = after_for.split('{').next().unwrap_or(after_for).trim();
        info.trait_name = Some(trait_part.to_string());
        info.for_type = Some(ty.trim_end().to_string());
    } else {
        // inherent impl: impl Type { ... }
        if let Some(after_impl) = s.strip_prefix("impl ") {
            let ty = after_impl.split('{').next().unwrap_or(after_impl).trim();
            info.for_type = Some(ty.to_string());
        }
    }
    info
}

fn matching_angle(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in text.chars().enumerate().skip(open) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == kind {
                return true;
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

fn subtree_contains_kind(node: &Node, kind: &str) -> bool {
    // DFS
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if n.kind() == kind {
            return true;
        }
        let mut w = n.walk();
        if w.goto_first_child() {
            loop {
                stack.push(w.node());
                if !w.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    false
}

fn child_text_by_kinds(node: Node, content: &str, kinds: &[&str]) -> Option<String> {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            let n = c.node();
            if kinds.iter().any(|k| n.kind() == *k) {
                return n.utf8_text(content.as_bytes()).ok().map(|s| s.to_string());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}
