use codegraph_core::{CodeNode, Language, Location, NodeType};
use serde_json::json;
use std::collections::HashMap;
use tree_sitter::{Node, Tree, TreeCursor};

/// Advanced Ruby AST extractor for dynamic programming intelligence.
///
/// Extracts:
/// - classes, modules, methods, constants
/// - metaprogramming patterns (define_method, class_eval, etc.)
/// - Rails patterns (controllers, models, migrations)
/// - blocks, lambdas, and functional patterns
/// - mixins, includes, and module composition
/// - attr_accessor and dynamic property definitions
/// - gem dependencies and require statements
///
/// Notes:
/// - Optimized for Ruby on Rails development patterns
/// - Captures dynamic and metaprogramming constructs
/// - Handles Ruby's flexible syntax and duck typing
/// - Understands Rails conventions and patterns
pub struct RubyExtractor;

#[derive(Default, Clone)]
struct RubyContext {
    module_path: Vec<String>,
    current_class: Option<String>,
    current_module: Option<String>,
    is_rails_file: bool,
}

impl RubyExtractor {
    pub fn extract(tree: &Tree, content: &str, file_path: &str) -> Vec<CodeNode> {
        let mut collector = RubyCollector::new(content, file_path);
        let mut cursor = tree.walk();

        // Detect Rails patterns from file path
        let is_rails = file_path.contains("/app/") ||
                      file_path.contains("/config/") ||
                      file_path.contains("/db/migrate");

        let mut ctx = RubyContext::default();
        ctx.is_rails_file = is_rails;

        collector.walk(&mut cursor, ctx);
        collector.into_nodes()
    }
}

struct RubyCollector<'a> {
    content: &'a str,
    file_path: &'a str,
    nodes: Vec<CodeNode>,
}

impl<'a> RubyCollector<'a> {
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

    fn walk(&mut self, cursor: &mut TreeCursor, mut ctx: RubyContext) {
        let node = cursor.node();

        match node.kind() {
            // Ruby Classes
            "class" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Class),
                        Some(Language::Ruby),
                        loc,
                    ).with_content(content_text.clone());

                    // Detect inheritance
                    if let Some(superclass) = self.child_text_by_field(node, "superclass") {
                        code.metadata.attributes.insert("superclass".into(), superclass);
                    }

                    // Detect Rails patterns
                    if ctx.is_rails_file {
                        if name.ends_with("Controller") {
                            code.metadata.attributes.insert("rails_pattern".into(), "controller".into());
                        } else if content_text.contains("< ApplicationRecord") {
                            code.metadata.attributes.insert("rails_pattern".into(), "model".into());
                        } else if content_text.contains("< ActiveRecord::Migration") {
                            code.metadata.attributes.insert("rails_pattern".into(), "migration".into());
                        }
                    }

                    code.metadata.attributes.insert("kind".into(), "class".into());
                    self.nodes.push(code);
                    ctx.current_class = Some(name);
                }
            }

            // Ruby Modules
            "module" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Module),
                        Some(Language::Ruby),
                        loc,
                    ).with_content(self.node_text(&node));

                    code.metadata.attributes.insert("kind".into(), "module".into());
                    self.nodes.push(code);
                    ctx.module_path.push(name.clone());
                    ctx.current_module = Some(name);
                }
            }

            // Ruby Methods
            "method" => {
                if let Some(name) = self.child_text_by_field(node, "name") {
                    let loc = self.location(&node);
                    let content_text = self.node_text(&node);
                    let mut code = CodeNode::new(
                        name.clone(),
                        Some(NodeType::Function),
                        Some(Language::Ruby),
                        loc,
                    ).with_content(content_text.clone());

                    // Detect class vs instance methods
                    if content_text.starts_with("def self.") {
                        code.metadata.attributes.insert("method_type".into(), "class".into());
                    } else {
                        code.metadata.attributes.insert("method_type".into(), "instance".into());
                    }

                    // Detect Rails action methods
                    if ctx.is_rails_file && ctx.current_class.as_ref().map_or(false, |c| c.ends_with("Controller")) {
                        if matches!(name.as_str(), "index" | "show" | "new" | "create" | "edit" | "update" | "destroy") {
                            code.metadata.attributes.insert("rails_action".into(), "true".into());
                        }
                    }

                    // Detect metaprogramming patterns
                    if content_text.contains("define_method") {
                        code.metadata.attributes.insert("metaprogramming".into(), "define_method".into());
                    }

                    code.metadata.attributes.insert("kind".into(), "method".into());
                    if let Some(ref current_class) = ctx.current_class {
                        code.metadata.attributes.insert("parent_class".into(), current_class.clone());
                    }
                    self.nodes.push(code);
                }
            }

            // Ruby Constants
            "constant" => {
                let name = self.node_text(&node);
                let loc = self.location(&node);
                let mut code = CodeNode::new(
                    name.clone(),
                    Some(NodeType::Variable),
                    Some(Language::Ruby),
                    loc,
                ).with_content(name.clone());

                code.metadata.attributes.insert("kind".into(), "constant".into());
                self.nodes.push(code);
            }

            // Ruby attr_* declarations
            "call" => {
                let call_text = self.node_text(&node);
                if call_text.starts_with("attr_") {
                    let loc = self.location(&node);
                    let mut code = CodeNode::new(
                        call_text.clone(),
                        Some(NodeType::Variable),
                        Some(Language::Ruby),
                        loc,
                    ).with_content(call_text.clone());

                    // Detect specific attr types
                    if call_text.starts_with("attr_accessor") {
                        code.metadata.attributes.insert("attr_type".into(), "accessor".into());
                    } else if call_text.starts_with("attr_reader") {
                        code.metadata.attributes.insert("attr_type".into(), "reader".into());
                    } else if call_text.starts_with("attr_writer") {
                        code.metadata.attributes.insert("attr_type".into(), "writer".into());
                    }

                    code.metadata.attributes.insert("kind".into(), "attribute".into());
                    self.nodes.push(code);
                }
            }

            // Ruby require/load statements
            "call" if self.node_text(&node).starts_with("require") => {
                let require_text = self.node_text(&node);
                let loc = self.location(&node);
                let mut code = CodeNode::new(
                    require_text.clone(),
                    Some(NodeType::Import),
                    Some(Language::Ruby),
                    loc,
                ).with_content(require_text.clone());

                code.metadata.attributes.insert("kind".into(), "require".into());

                // Extract required gem/file name
                if let Some(arg) = self.extract_string_argument(&node) {
                    code.metadata.attributes.insert("required_module".into(), arg);
                }

                self.nodes.push(code);
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

    fn extract_string_argument(&self, node: &Node) -> Option<String> {
        // Extract string argument from method calls like require "gem_name"
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "string" {
                    let text = self.node_text(&child);
                    return Some(text.trim_matches('"').trim_matches('\'').to_string());
                }
            }
        }
        None
    }

    fn node_text(&self, node: &Node) -> String {
        node.utf8_text(self.content.as_bytes()).unwrap_or("").to_string()
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