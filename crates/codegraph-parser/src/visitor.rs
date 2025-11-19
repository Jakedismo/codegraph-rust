use crate::edge::CodeEdge;
use codegraph_core::{CodeNode, EdgeType, Language, Location, NodeId, NodeType, SharedStr};
use std::collections::HashMap;
use std::sync::Arc;
use tree_sitter::{Node, TreeCursor};

#[derive(Debug, Clone)]
pub struct SemanticRelationship {
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct CodeEntity {
    pub node: CodeNode,
    pub symbol_name: String,
    pub qualified_name: String,
    pub scope_path: Vec<String>,
    pub references: Vec<Location>,
    pub definitions: Vec<Location>,
}

pub struct AstToGraphConverter {
    pub language: Language,
    pub file_path: String,
    pub source: String,
    source_bytes: Arc<[u8]>,
    pub entities: Vec<CodeEntity>,
    pub relationships: Vec<SemanticRelationship>,
    symbol_table: HashMap<String, NodeId>,
    scope_stack: Vec<String>,
    current_scope: String,
}

impl AstToGraphConverter {
    pub fn new(language: Language, file_path: String, source: String) -> Self {
        let source_bytes: Arc<[u8]> = Arc::from(source.clone().into_bytes().into_boxed_slice());
        Self {
            language,
            file_path,
            source,
            source_bytes,
            entities: Vec::new(),
            relationships: Vec::new(),
            symbol_table: HashMap::new(),
            scope_stack: Vec::new(),
            current_scope: String::new(),
        }
    }

    pub fn convert(&mut self, root: Node) -> Result<(), Box<dyn std::error::Error>> {
        self.extract_entities(root)?;
        self.build_relationships()?;
        self.resolve_symbols()?;
        Ok(())
    }

    fn extract_entities(&mut self, node: Node) -> Result<(), Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        self.visit_for_entities(&mut cursor)?;
        Ok(())
    }

    fn visit_for_entities(
        &mut self,
        cursor: &mut TreeCursor,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = cursor.node();

        self.enter_scope(&node);

        if let Some(entity) = self.create_entity(&node)? {
            self.entities.push(entity);
        }

        if cursor.goto_first_child() {
            loop {
                self.visit_for_entities(cursor)?;
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }

        self.exit_scope(&node);
        Ok(())
    }

    fn create_entity(
        &mut self,
        node: &Node,
    ) -> Result<Option<CodeEntity>, Box<dyn std::error::Error>> {
        let node_type = match self.map_node_type(node.kind()) {
            Some(nt) => nt,
            None => return Ok(None),
        };

        let symbol_name = match self.extract_name(node) {
            Some(name) => name,
            None => return Ok(None),
        };

        let location = self.node_to_location(node);
        let content = self.node_text(node);
        let qualified_name = self.build_qualified_name(&symbol_name);

        let code_node = CodeNode::new(
            symbol_name.clone(),
            Some(node_type.clone()),
            Some(self.language.clone()),
            location.clone(),
        )
        .with_content(content);

        let entity = CodeEntity {
            node: code_node.clone(),
            symbol_name: symbol_name.clone(),
            qualified_name: qualified_name.clone(),
            scope_path: self.scope_stack.clone(),
            references: Vec::new(),
            definitions: vec![location],
        };

        self.symbol_table.insert(qualified_name, code_node.id);

        Ok(Some(entity))
    }

    fn build_relationships(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let entities = self.entities.clone();

        for entity in &entities {
            self.find_relationships_for_entity(entity)?;
        }

        Ok(())
    }

    fn find_relationships_for_entity(
        &mut self,
        entity: &CodeEntity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match entity.node.node_type {
            Some(NodeType::Function) => self.analyze_function_relationships(entity)?,
            Some(NodeType::Class) | Some(NodeType::Struct) => {
                self.analyze_class_relationships(entity)?
            }
            Some(NodeType::Import) => self.analyze_import_relationships(entity)?,
            Some(NodeType::Variable) => self.analyze_variable_relationships(entity)?,
            _ => {}
        }
        Ok(())
    }

    fn analyze_function_relationships(
        &mut self,
        entity: &CodeEntity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let empty = SharedStr::default();
        let content = entity.node.content.as_ref().unwrap_or(&empty);

        for other_entity in &self.entities {
            if other_entity.node.id == entity.node.id {
                continue;
            }

            let other_name = &other_entity.symbol_name;

            if content.contains(&format!("{}(", other_name)) {
                self.relationships.push(SemanticRelationship {
                    from: entity.node.id,
                    to: other_entity.node.id,
                    edge_type: EdgeType::Calls,
                    context: format!(
                        "Function call from {} to {}",
                        entity.symbol_name, other_name
                    ),
                });
            }

            if content.contains(&format!("{}", other_name))
                && matches!(
                    other_entity.node.node_type,
                    Some(NodeType::Variable)
                        | Some(NodeType::Struct)
                        | Some(NodeType::Class)
                        | Some(NodeType::Import)
                )
            {
                self.relationships.push(SemanticRelationship {
                    from: entity.node.id,
                    to: other_entity.node.id,
                    edge_type: EdgeType::Uses,
                    context: format!("Function {} uses {}", entity.symbol_name, other_name),
                });
            }
        }
        Ok(())
    }

    fn analyze_class_relationships(
        &mut self,
        entity: &CodeEntity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let empty2 = SharedStr::default();
        let content = entity.node.content.as_ref().unwrap_or(&empty2);

        if self.language == Language::Rust {
            if content.contains("impl ") {
                for other_entity in &self.entities {
                    if matches!(other_entity.node.node_type, Some(NodeType::Trait))
                        && content.contains(&format!("impl {} for", other_entity.symbol_name))
                    {
                        self.relationships.push(SemanticRelationship {
                            from: entity.node.id,
                            to: other_entity.node.id,
                            edge_type: EdgeType::Implements,
                            context: format!(
                                "Struct {} implements trait {}",
                                entity.symbol_name, other_entity.symbol_name
                            ),
                        });
                    }
                }
            }
        } else if matches!(self.language, Language::TypeScript | Language::JavaScript) {
            if content.contains("extends ") || content.contains("implements ") {
                for other_entity in &self.entities {
                    if content.contains(&format!("extends {}", other_entity.symbol_name)) {
                        self.relationships.push(SemanticRelationship {
                            from: entity.node.id,
                            to: other_entity.node.id,
                            edge_type: EdgeType::Extends,
                            context: format!(
                                "Class {} extends {}",
                                entity.symbol_name, other_entity.symbol_name
                            ),
                        });
                    }
                    if content.contains(&format!("implements {}", other_entity.symbol_name)) {
                        self.relationships.push(SemanticRelationship {
                            from: entity.node.id,
                            to: other_entity.node.id,
                            edge_type: EdgeType::Implements,
                            context: format!(
                                "Class {} implements {}",
                                entity.symbol_name, other_entity.symbol_name
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn analyze_import_relationships(
        &mut self,
        entity: &CodeEntity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let empty3 = SharedStr::default();
        let content = entity.node.content.as_ref().unwrap_or(&empty3);

        for other_entity in &self.entities {
            if other_entity.node.id == entity.node.id {
                continue;
            }

            if content.contains(&other_entity.symbol_name) {
                self.relationships.push(SemanticRelationship {
                    from: entity.node.id,
                    to: other_entity.node.id,
                    edge_type: EdgeType::Imports,
                    context: format!(
                        "Import {} references {}",
                        entity.symbol_name, other_entity.symbol_name
                    ),
                });
            }
        }
        Ok(())
    }

    fn analyze_variable_relationships(
        &mut self,
        _entity: &CodeEntity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resolve_symbols(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut unresolved_references: Vec<(String, NodeId, SharedStr)> = Vec::new();

        for entity in &self.entities {
            let empty4 = SharedStr::default();
            let content = entity.node.content.as_ref().unwrap_or(&empty4);

            for (symbol, node_id) in &self.symbol_table {
                if entity.node.id != *node_id && content.contains(symbol) {
                    unresolved_references.push((symbol.clone(), entity.node.id, content.clone()));
                }
            }
        }

        for (symbol, referring_node_id, context) in unresolved_references {
            if let Some(&target_node_id) = self.symbol_table.get(&symbol) {
                let edge_type = self.infer_edge_type(&context, &symbol);
                self.relationships.push(SemanticRelationship {
                    from: referring_node_id,
                    to: target_node_id,
                    edge_type,
                    context: format!(
                        "Symbol resolution: {} -> {}",
                        referring_node_id, target_node_id
                    ),
                });
            }
        }

        Ok(())
    }

    fn infer_edge_type(&self, context: &str, symbol: &str) -> EdgeType {
        if context.contains(&format!("{}(", symbol)) {
            EdgeType::Calls
        } else if context.contains("import") || context.contains("use") {
            EdgeType::Imports
        } else if context.contains("extends") {
            EdgeType::Extends
        } else if context.contains("implements") {
            EdgeType::Implements
        } else {
            EdgeType::References
        }
    }

    fn enter_scope(&mut self, node: &Node) {
        match node.kind() {
            "function_item"
            | "function_declaration"
            | "function_definition"
            | "struct_item"
            | "class_declaration"
            | "class_definition"
            | "mod_item"
            | "module"
            | "namespace_declaration" => {
                if let Some(name) = self.extract_name(node) {
                    self.scope_stack.push(name.clone());
                    self.current_scope = self.scope_stack.join("::");
                }
            }
            _ => {}
        }
    }

    fn exit_scope(&mut self, node: &Node) {
        match node.kind() {
            "function_item"
            | "function_declaration"
            | "function_definition"
            | "struct_item"
            | "class_declaration"
            | "class_definition"
            | "mod_item"
            | "module"
            | "namespace_declaration" => {
                if !self.scope_stack.is_empty() {
                    self.scope_stack.pop();
                    self.current_scope = self.scope_stack.join("::");
                }
            }
            _ => {}
        }
    }

    fn build_qualified_name(&self, symbol_name: &str) -> String {
        if self.current_scope.is_empty() {
            format!("{}::{}", self.file_path, symbol_name)
        } else {
            format!(
                "{}::{}::{}",
                self.file_path, self.current_scope, symbol_name
            )
        }
    }

    fn map_node_type(&self, kind: &str) -> Option<NodeType> {
        match (&self.language, kind) {
            (Language::Rust, "function_item") => Some(NodeType::Function),
            (Language::Rust, "struct_item") => Some(NodeType::Struct),
            (Language::Rust, "enum_item") => Some(NodeType::Enum),
            (Language::Rust, "trait_item") => Some(NodeType::Trait),
            (Language::Rust, "mod_item") => Some(NodeType::Module),
            (Language::Rust, "use_declaration") => Some(NodeType::Import),
            (Language::Rust, "let_declaration") => Some(NodeType::Variable),
            (Language::Rust, "const_item") => Some(NodeType::Variable),
            (Language::Rust, "static_item") => Some(NodeType::Variable),

            (Language::TypeScript | Language::JavaScript, "function_declaration") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "method_definition") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "class_declaration") => {
                Some(NodeType::Class)
            }
            (Language::TypeScript | Language::JavaScript, "interface_declaration") => {
                Some(NodeType::Interface)
            }
            (Language::TypeScript | Language::JavaScript, "import_statement") => {
                None // Don't map the whole statement, map children
            }
            (Language::TypeScript | Language::JavaScript, "import_specifier") => {
                Some(NodeType::Import)
            }
            (Language::TypeScript | Language::JavaScript, "namespace_import") => {
                Some(NodeType::Import)
            }
            (Language::TypeScript | Language::JavaScript, "default_import") => {
                Some(NodeType::Import)
            }
            (Language::TypeScript | Language::JavaScript, "variable_declaration") => {
                Some(NodeType::Variable)
            }
            (Language::TypeScript, "type_alias_declaration") => Some(NodeType::Type),

            (Language::Python, "function_definition") => Some(NodeType::Function),
            (Language::Python, "class_definition") => Some(NodeType::Class),
            (Language::Python, "import_statement") => None, // Map children
            (Language::Python, "import_from_statement") => None, // Map children
            (Language::Python, "dotted_name") => Some(NodeType::Import),
            (Language::Python, "aliased_import") => Some(NodeType::Import),
            (Language::Python, "assignment") => Some(NodeType::Variable),

            (Language::Go, "function_declaration") => Some(NodeType::Function),
            (Language::Go, "method_declaration") => Some(NodeType::Function),
            (Language::Go, "type_declaration") => Some(NodeType::Type),
            (Language::Go, "import_declaration") => Some(NodeType::Import),
            (Language::Go, "var_declaration") => Some(NodeType::Variable),
            (Language::Go, "const_declaration") => Some(NodeType::Variable),

            (Language::Java, "method_declaration") => Some(NodeType::Function),
            (Language::Java, "constructor_declaration") => Some(NodeType::Function),
            (Language::Java, "class_declaration") => Some(NodeType::Class),
            (Language::Java, "interface_declaration") => Some(NodeType::Interface),
            (Language::Java, "enum_declaration") => Some(NodeType::Enum),
            (Language::Java, "import_declaration") => Some(NodeType::Import),
            (Language::Java, "variable_declarator") => Some(NodeType::Variable),
            (Language::Java, "field_declaration") => Some(NodeType::Variable),

            (Language::Cpp, "function_definition") => Some(NodeType::Function),
            (Language::Cpp, "function_declarator") => Some(NodeType::Function),
            (Language::Cpp, "class_specifier") => Some(NodeType::Class),
            (Language::Cpp, "struct_specifier") => Some(NodeType::Struct),
            (Language::Cpp, "enum_specifier") => Some(NodeType::Enum),
            (Language::Cpp, "preproc_include") => Some(NodeType::Import),
            (Language::Cpp, "declaration") => Some(NodeType::Variable),
            (Language::Cpp, "field_declaration") => Some(NodeType::Variable),

            _ => None,
        }
    }

    fn extract_name(&self, node: &Node) -> Option<String> {
        match node.kind() {
            "use_declaration" => self.extract_use_name(node),
            "import_specifier" | "default_import" | "namespace_import" => {
                self.extract_identifier_name(node)
            }
            "dotted_name" => self.extract_dotted_name(node),
            "aliased_import" => self.extract_aliased_import(node),
            _ => self.extract_identifier_name(node),
        }
    }

    fn extract_identifier_name(&self, node: &Node) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier" | "name" | "type_identifier" => {
                    return child.utf8_text(&self.source_bytes).ok().map(String::from);
                }
                _ => continue,
            }
        }
        None
    }

    fn extract_use_name(&self, node: &Node) -> Option<String> {
        let content = node.utf8_text(&self.source_bytes).ok()?;
        if let Some(start) = content.find("use ") {
            let use_part = &content[start + 4..];
            if let Some(end) = use_part.find(';') {
                Some(use_part[..end].trim().to_string())
            } else {
                Some(use_part.trim().to_string())
            }
        } else {
            None
        }
    }

    fn extract_dotted_name(&self, node: &Node) -> Option<String> {
        node.utf8_text(&self.source_bytes).ok().map(String::from)
    }

    fn extract_aliased_import(&self, node: &Node) -> Option<String> {
        // In "import foo as bar", we want "bar" (the alias) as the symbol name
        // because that's what is used in the code.
        for child in node.children(&mut node.walk()) {
            if child.kind() == "alias" {
                for grandchild in child.children(&mut child.walk()) {
                    if grandchild.kind() == "identifier" {
                        return grandchild
                            .utf8_text(&self.source_bytes)
                            .ok()
                            .map(String::from);
                    }
                }
            }
        }
        None
    }

    fn node_to_location(&self, node: &Node) -> Location {
        Location {
            file_path: self.file_path.clone(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        }
    }

    fn node_text(&self, node: &Node) -> SharedStr {
        let start = node.start_byte() as usize;
        let end = node.end_byte() as usize;
        SharedStr::from_arc_slice(self.source_bytes.clone(), start, end)
    }

    pub fn get_edges(&self) -> Vec<CodeEdge> {
        self.relationships
            .iter()
            .map(|rel| {
                CodeEdge::new(rel.from, rel.to, rel.edge_type.clone())
                    .with_metadata("context".to_string(), rel.context.clone())
            })
            .collect()
    }

    pub fn get_nodes(&self) -> Vec<CodeNode> {
        self.entities
            .iter()
            .map(|entity| entity.node.clone())
            .collect()
    }
}

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
        // Clean implementation without debug logging for better user experience
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
            CodeNode::new(name, Some(node_type), Some(self.language.clone()), location)
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

            (Language::TypeScript | Language::JavaScript, "function_declaration") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "function_expression") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "arrow_function") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "method_definition") => {
                Some(NodeType::Function)
            }
            (Language::TypeScript | Language::JavaScript, "class_declaration") => {
                Some(NodeType::Class)
            }
            (Language::TypeScript | Language::JavaScript, "interface_declaration") => {
                Some(NodeType::Interface)
            }
            (Language::TypeScript | Language::JavaScript, "import_statement") => {
                Some(NodeType::Import)
            }
            (Language::TypeScript | Language::JavaScript, "import_clause") => {
                Some(NodeType::Other("ImportClause".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "named_imports") => {
                Some(NodeType::Other("NamedImports".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "import_specifier") => {
                Some(NodeType::Other("ImportSpecifier".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "export_statement") => {
                Some(NodeType::Other("Export".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "variable_declaration") => {
                Some(NodeType::Variable)
            }
            (Language::TypeScript | Language::JavaScript, "variable_declarator") => {
                Some(NodeType::Variable)
            }
            (Language::TypeScript | Language::JavaScript, "identifier") => {
                Some(NodeType::Other("Identifier".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "comment") => {
                Some(NodeType::Other("Comment".to_string()))
            }
            (Language::TypeScript | Language::JavaScript, "program") => {
                Some(NodeType::Other("Program".to_string()))
            }
            (Language::TypeScript, "type_alias_declaration") => Some(NodeType::Type),

            (Language::Python, "function_definition") => Some(NodeType::Function),
            (Language::Python, "class_definition") => Some(NodeType::Class),
            (Language::Python, "import_statement" | "import_from_statement") => {
                Some(NodeType::Import)
            }

            (Language::Go, "function_declaration") => Some(NodeType::Function),
            (Language::Go, "type_declaration") => Some(NodeType::Type),
            (Language::Go, "import_declaration") => Some(NodeType::Import),

            _ => None,
        }
    }

    fn extract_name(&self, node: &Node) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return child
                    .utf8_text(self.source.as_bytes())
                    .ok()
                    .map(String::from);
            }
        }

        node.utf8_text(self.source.as_bytes())
            .ok()
            .map(|text| text.lines().next().unwrap_or(text).to_string())
    }
}
