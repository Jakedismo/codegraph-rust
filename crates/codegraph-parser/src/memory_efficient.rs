use codegraph_core::{CodeNode, Language, Location, NodeType, EdgeType, NodeId};
use codegraph_graph::CodeEdge;
use tree_sitter::{Node, TreeCursor, Range};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

#[derive(Debug)]
pub struct ZeroCopyAstProcessor<'a> {
    source_bytes: &'a [u8],
    language: Language,
    file_path: Arc<str>,
    symbol_cache: Arc<DashMap<&'a str, NodeId>>,
    position_cache: HashMap<NodeId, Range>,
    content_slices: HashMap<NodeId, &'a str>,
}

impl<'a> ZeroCopyAstProcessor<'a> {
    pub fn new(source_bytes: &'a [u8], language: Language, file_path: String) -> Self {
        Self {
            source_bytes,
            language,
            file_path: Arc::from(file_path),
            symbol_cache: Arc::new(DashMap::new()),
            position_cache: HashMap::new(),
            content_slices: HashMap::new(),
        }
    }

    pub fn process_tree_zero_copy(&mut self, root: Node<'a>) -> Result<ProcessingResult, Box<dyn std::error::Error>> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut relationships = Vec::new();

        let mut cursor = root.walk();
        self.process_node_zero_copy(&mut cursor, &mut nodes, &mut edges, &mut relationships)?;

        Ok(ProcessingResult {
            nodes,
            edges,
            relationships,
            memory_stats: MemoryStats {
                source_bytes_len: self.source_bytes.len(),
                cached_symbols: self.symbol_cache.len(),
                cached_positions: self.position_cache.len(),
                content_slices: self.content_slices.len(),
            },
        })
    }

    fn process_node_zero_copy(
        &mut self,
        cursor: &mut TreeCursor<'a>,
        nodes: &mut Vec<CodeNode>,
        edges: &mut Vec<CodeEdge>,
        relationships: &mut Vec<ZeroCopyRelationship>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = cursor.node();
        
        if let Some(entity) = self.create_zero_copy_entity(&node)? {
            let node_id = entity.id;
            nodes.push(entity);

            self.analyze_node_relationships(&node, node_id, relationships)?;
            
            self.position_cache.insert(node_id, node.range());
            
            if let Ok(content) = node.utf8_text(self.source_bytes) {
                self.content_slices.insert(node_id, content);
            }
        }

        if cursor.goto_first_child() {
            loop {
                self.process_node_zero_copy(cursor, nodes, edges, relationships)?;
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }

        Ok(())
    }

    fn create_zero_copy_entity(&mut self, node: &Node<'a>) -> Result<Option<CodeNode>, Box<dyn std::error::Error>> {
        let node_type = match self.map_node_type(node.kind()) {
            Some(nt) => nt,
            None => return Ok(None),
        };

        let content = node.utf8_text(self.source_bytes)?;
        let symbol_name = self.extract_symbol_name_zero_copy(node, content)?;
        
        if symbol_name.is_empty() {
            return Ok(None);
        }

        let location = Location {
            file_path: self.file_path.to_string(),
            line: node.start_position().row as u32 + 1,
            column: node.start_position().column as u32,
            end_line: Some(node.end_position().row as u32 + 1),
            end_column: Some(node.end_position().column as u32),
        };

        let code_node = CodeNode::new(
            symbol_name.to_string(),
            Some(node_type),
            Some(self.language.clone()),
            location,
        ).with_content(content.to_string());

        self.symbol_cache.insert(symbol_name, code_node.id);

        Ok(Some(code_node))
    }

    fn extract_symbol_name_zero_copy(&self, node: &Node<'a>, content: &'a str) -> Result<&'a str, Box<dyn std::error::Error>> {
        match node.kind() {
            "function_item" | "function_declaration" | "function_definition" => {
                self.extract_function_name_zero_copy(node)
            }
            "struct_item" | "class_declaration" | "class_definition" => {
                self.extract_class_name_zero_copy(node)
            }
            "use_declaration" | "import_statement" | "import_from_statement" => {
                Ok(content.lines().next().unwrap_or(content).trim())
            }
            _ => {
                for child in node.children(&mut node.walk()) {
                    if matches!(child.kind(), "identifier" | "name" | "type_identifier") {
                        return Ok(child.utf8_text(self.source_bytes)?);
                    }
                }
                Ok("")
            }
        }
    }

    fn extract_function_name_zero_copy(&self, node: &Node<'a>) -> Result<&'a str, Box<dyn std::error::Error>> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Ok(child.utf8_text(self.source_bytes)?);
            }
        }
        Ok("")
    }

    fn extract_class_name_zero_copy(&self, node: &Node<'a>) -> Result<&'a str, Box<dyn std::error::Error>> {
        for child in node.children(&mut node.walk()) {
            if matches!(child.kind(), "identifier" | "type_identifier") {
                return Ok(child.utf8_text(self.source_bytes)?);
            }
        }
        Ok("")
    }

    fn analyze_node_relationships(
        &self,
        node: &Node<'a>,
        node_id: NodeId,
        relationships: &mut Vec<ZeroCopyRelationship>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = node.utf8_text(self.source_bytes)?;
        
        for (symbol, &target_id) in self.symbol_cache.iter() {
            if content.contains(symbol) {
                let edge_type = self.infer_relationship_type(node.kind(), content, symbol);
                relationships.push(ZeroCopyRelationship {
                    from: node_id,
                    to: target_id,
                    edge_type,
                    context_slice: content,
                });
            }
        }

        Ok(())
    }

    fn infer_relationship_type(&self, node_kind: &str, content: &str, symbol: &str) -> EdgeType {
        match node_kind {
            "function_item" | "function_declaration" | "function_definition" => {
                if content.contains(&format!("{}(", symbol)) {
                    EdgeType::Calls
                } else {
                    EdgeType::Uses
                }
            }
            "use_declaration" | "import_statement" | "import_from_statement" => EdgeType::Imports,
            "class_declaration" | "struct_item" => {
                if content.contains(&format!("extends {}", symbol)) {
                    EdgeType::Extends
                } else if content.contains(&format!("implements {}", symbol)) {
                    EdgeType::Implements
                } else {
                    EdgeType::References
                }
            }
            _ => EdgeType::References,
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
            (Language::Rust, "let_declaration" | "const_item" | "static_item") => Some(NodeType::Variable),
            
            (Language::TypeScript | Language::JavaScript, "function_declaration") => Some(NodeType::Function),
            (Language::TypeScript | Language::JavaScript, "method_definition") => Some(NodeType::Function),
            (Language::TypeScript | Language::JavaScript, "class_declaration") => Some(NodeType::Class),
            (Language::TypeScript | Language::JavaScript, "interface_declaration") => Some(NodeType::Interface),
            (Language::TypeScript | Language::JavaScript, "import_statement") => Some(NodeType::Import),
            (Language::TypeScript | Language::JavaScript, "variable_declaration") => Some(NodeType::Variable),
            (Language::TypeScript, "type_alias_declaration") => Some(NodeType::Type),
            
            (Language::Python, "function_definition") => Some(NodeType::Function),
            (Language::Python, "class_definition") => Some(NodeType::Class),
            (Language::Python, "import_statement" | "import_from_statement") => Some(NodeType::Import),
            (Language::Python, "assignment") => Some(NodeType::Variable),
            
            (Language::Go, "function_declaration" | "method_declaration") => Some(NodeType::Function),
            (Language::Go, "type_declaration") => Some(NodeType::Type),
            (Language::Go, "import_declaration") => Some(NodeType::Import),
            (Language::Go, "var_declaration" | "const_declaration") => Some(NodeType::Variable),
            
            _ => None,
        }
    }

    pub fn get_memory_usage(&self) -> MemoryUsage {
        MemoryUsage {
            source_bytes_size: self.source_bytes.len(),
            symbol_cache_entries: self.symbol_cache.len(),
            position_cache_entries: self.position_cache.len(),
            content_slices_count: self.content_slices.len(),
            estimated_heap_usage: self.calculate_heap_usage(),
        }
    }

    fn calculate_heap_usage(&self) -> usize {
        let symbol_cache_size = self.symbol_cache.len() * (std::mem::size_of::<&str>() + std::mem::size_of::<NodeId>());
        let position_cache_size = self.position_cache.len() * (std::mem::size_of::<NodeId>() + std::mem::size_of::<Range>());
        let content_slices_size = self.content_slices.len() * (std::mem::size_of::<NodeId>() + std::mem::size_of::<&str>());
        
        symbol_cache_size + position_cache_size + content_slices_size
    }
}

#[derive(Debug)]
pub struct ZeroCopyRelationship<'a> {
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
    pub context_slice: &'a str,
}

#[derive(Debug)]
pub struct ProcessingResult {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
    pub relationships: Vec<ZeroCopyRelationship<'static>>,
    pub memory_stats: MemoryStats,
}

#[derive(Debug)]
pub struct MemoryStats {
    pub source_bytes_len: usize,
    pub cached_symbols: usize,
    pub cached_positions: usize,
    pub content_slices: usize,
}

#[derive(Debug)]
pub struct MemoryUsage {
    pub source_bytes_size: usize,
    pub symbol_cache_entries: usize,
    pub position_cache_entries: usize,
    pub content_slices_count: usize,
    pub estimated_heap_usage: usize,
}

pub struct StreamingAstProcessor {
    buffer_size: usize,
    chunk_processor: Arc<RwLock<ChunkProcessor>>,
}

impl StreamingAstProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            chunk_processor: Arc::new(RwLock::new(ChunkProcessor::new())),
        }
    }

    pub fn process_large_file(
        &self,
        file_path: &str,
        language: Language,
    ) -> Result<Vec<CodeNode>, Box<dyn std::error::Error>> {
        use std::fs::File;
        use memmap2::MmapOptions;

        let file = File::open(file_path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        
        let mut nodes = Vec::new();
        let mut offset = 0;
        
        while offset < mmap.len() {
            let end = std::cmp::min(offset + self.buffer_size, mmap.len());
            let chunk = &mmap[offset..end];
            
            let chunk_nodes = self.process_chunk(chunk, language.clone(), offset)?;
            nodes.extend(chunk_nodes);
            
            offset = end;
        }

        Ok(nodes)
    }

    fn process_chunk(
        &self,
        chunk: &[u8],
        language: Language,
        offset: usize,
    ) -> Result<Vec<CodeNode>, Box<dyn std::error::Error>> {
        let mut processor = self.chunk_processor.write();
        processor.process_chunk(chunk, language, offset)
    }
}

pub struct ChunkProcessor {
    parser: Option<tree_sitter::Parser>,
    current_language: Option<Language>,
}

impl ChunkProcessor {
    pub fn new() -> Self {
        Self {
            parser: None,
            current_language: None,
        }
    }

    pub fn process_chunk(
        &mut self,
        chunk: &[u8],
        language: Language,
        offset: usize,
    ) -> Result<Vec<CodeNode>, Box<dyn std::error::Error>> {
        self.ensure_parser_for_language(&language)?;
        
        let parser = self.parser.as_mut().unwrap();
        let source = std::str::from_utf8(chunk)?;
        
        if let Some(tree) = parser.parse(source, None) {
            let mut processor = ZeroCopyAstProcessor::new(
                chunk,
                language,
                format!("chunk_at_offset_{}", offset),
            );
            
            let result = processor.process_tree_zero_copy(tree.root_node())?;
            Ok(result.nodes)
        } else {
            Ok(Vec::new())
        }
    }

    fn ensure_parser_for_language(&mut self, language: &Language) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_language.as_ref() != Some(language) {
            let mut parser = tree_sitter::Parser::new();
            
            match language {
                Language::Rust => {
                    parser.set_language(tree_sitter_rust::language().into())?;
                }
                Language::TypeScript => {
                    parser.set_language(tree_sitter_typescript::language_typescript().into())?;
                }
                Language::JavaScript => {
                    parser.set_language(tree_sitter_javascript::language().into())?;
                }
                Language::Python => {
                    parser.set_language(tree_sitter_python::language().into())?;
                }
                Language::Go => {
                    parser.set_language(tree_sitter_go::language().into())?;
                }
                _ => {
                    return Err("Unsupported language for chunk processing".into());
                }
            }
            
            self.parser = Some(parser);
            self.current_language = Some(language.clone());
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::{Parser, Language as TSLanguage};

    fn create_test_parser() -> Parser {
        let mut parser = Parser::new();
        parser.set_language(tree_sitter_rust::language().into()).unwrap();
        parser
    }

    #[test]
    fn test_zero_copy_processing() {
        let source = "fn hello_world() { println!(\"Hello, world!\"); }";
        let mut parser = create_test_parser();
        
        if let Some(tree) = parser.parse(source, None) {
            let mut processor = ZeroCopyAstProcessor::new(
                source.as_bytes(),
                Language::Rust,
                "test.rs".to_string(),
            );
            
            let result = processor.process_tree_zero_copy(tree.root_node()).unwrap();
            
            assert!(!result.nodes.is_empty());
            assert!(result.nodes.iter().any(|n| n.name == "hello_world"));
            
            let memory_usage = processor.get_memory_usage();
            assert_eq!(memory_usage.source_bytes_size, source.len());
            assert!(memory_usage.estimated_heap_usage > 0);
        }
    }

    #[test]
    fn test_streaming_processor() {
        let processor = StreamingAstProcessor::new(1024);
        
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(&file_path, "fn test() {} struct Test {}").unwrap();
        
        let nodes = processor.process_large_file(
            file_path.to_str().unwrap(),
            Language::Rust,
        ).unwrap();
        
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_memory_efficiency() {
        let large_source = "fn test() {}\n".repeat(1000);
        let mut parser = create_test_parser();
        
        if let Some(tree) = parser.parse(&large_source, None) {
            let mut processor = ZeroCopyAstProcessor::new(
                large_source.as_bytes(),
                Language::Rust,
                "large_test.rs".to_string(),
            );
            
            let result = processor.process_tree_zero_copy(tree.root_node()).unwrap();
            let memory_usage = processor.get_memory_usage();
            
            let efficiency_ratio = memory_usage.estimated_heap_usage as f64 / large_source.len() as f64;
            assert!(efficiency_ratio < 1.0, "Heap usage should be less than source size");
        }
    }
}