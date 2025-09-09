use codegraph_core::{CodeNode, Language, Location, NodeType};
use tree_sitter::{Node, TreeCursor};

pub struct AstVisitor {
    pub language: Language,
    pub file_path: String,
    pub source: String,
    pub nodes: Vec<CodeNode>,
}

impl AstVisitor {
    pub fn new(language: Language, file_path: String, source: String) -> Self {
        Self {
            language,
            file_path,
            source,
            nodes: Vec::new(),
        }
    }

    pub fn visit(&mut self, node: Node) {
        let mut cursor = node.walk();
        self.visit_node(&mut cursor);
    }

    fn visit_node(&mut self, cursor: &mut TreeCursor) {
        let node = cursor.node();
        
        if let Some(code_node) = self.create_code_node(&node) {
            self.nodes.push(code_node);
        }

        if cursor.goto_first_child() {
            loop {
                self.visit_node(cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn create_code_node(&self, node: &Node) -> Option<CodeNode> {
        let node_type = self.map_node_type(node.kind())?;
        let name = self.extract_name(node)?;

        let location = Location {
            file_path: self.file_path.clone(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        };

        let content = node.utf8_text(self.source.as_bytes()).ok()?.to_string();

        Some(
            CodeNode::new(name, node_type, self.language.clone(), location)
                .with_content(content),
        )
    }

    fn map_node_type(&self, kind: &str) -> Option<NodeType> {
        match (&self.language, kind) {
            (Language::Rust, "function_item") => Some(NodeType::Function),
            (Language::Rust, "struct_item") => Some(NodeType::Struct),
            (Language::Rust, "enum_item") => Some(NodeType::Enum),
            (Language::Rust, "trait_item") => Some(NodeType::Trait),
            (Language::Rust, "mod_item") => Some(NodeType::Module),
            (Language::Rust, "use_declaration") => Some(NodeType::Import),
            
            (Language::TypeScript | Language::JavaScript, "function_declaration") => Some(NodeType::Function),
            (Language::TypeScript | Language::JavaScript, "class_declaration") => Some(NodeType::Class),
            (Language::TypeScript | Language::JavaScript, "interface_declaration") => Some(NodeType::Interface),
            (Language::TypeScript | Language::JavaScript, "import_statement") => Some(NodeType::Import),
            (Language::TypeScript, "type_alias_declaration") => Some(NodeType::Type),
            
            (Language::Python, "function_definition") => Some(NodeType::Function),
            (Language::Python, "class_definition") => Some(NodeType::Class),
            (Language::Python, "import_statement" | "import_from_statement") => Some(NodeType::Import),
            
            (Language::Go, "function_declaration") => Some(NodeType::Function),
            (Language::Go, "type_declaration") => Some(NodeType::Type),
            (Language::Go, "import_declaration") => Some(NodeType::Import),
            
            _ => None,
        }
    }

    fn extract_name(&self, node: &Node) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return child.utf8_text(self.source.as_bytes()).ok().map(String::from);
            }
        }

        node.utf8_text(self.source.as_bytes())
            .ok()
            .map(|text| text.lines().next().unwrap_or(text).to_string())
    }
}