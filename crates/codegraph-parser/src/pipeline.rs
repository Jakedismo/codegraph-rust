use crate::visitor::{AstToGraphConverter, CodeEntity, SemanticRelationship};
use crate::edge::CodeEdge;
use codegraph_core::{CodeNode, Language, NodeId, EdgeType};
use tree_sitter::{Parser, Tree};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use dashmap::DashMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct CrossFileReference {
    pub from_file: PathBuf,
    pub to_file: PathBuf,
    pub from_symbol: String,
    pub to_symbol: String,
    pub reference_type: EdgeType,
}

pub struct ConversionPipeline {
    parsers: HashMap<Language, Parser>,
    global_symbol_table: Arc<DashMap<String, (NodeId, PathBuf)>>,
    file_entities: HashMap<PathBuf, Vec<CodeEntity>>,
    cross_file_refs: Vec<CrossFileReference>,
    processed_files: HashSet<PathBuf>,
}

impl ConversionPipeline {
    pub fn new() -> Result<Self> {
        let mut parsers = HashMap::new();
        
        if let Ok(mut parser) = Parser::new() {
            let language = tree_sitter_rust::language();
            parser.set_language(language).map_err(|e| {
                format!("Failed to set Rust language: {:?}", e)
            })?;
            parsers.insert(Language::Rust, parser);
        }

        Ok(Self {
            parsers,
            global_symbol_table: Arc::new(DashMap::new()),
            file_entities: HashMap::new(),
            cross_file_refs: Vec::new(),
            processed_files: HashSet::new(),
        })
    }

    pub fn process_file(&mut self, file_path: &Path, source: String) -> Result<ConversionResult> {
        let language = self.detect_language(file_path)?;
        let file_path_buf = file_path.to_path_buf();

        if self.processed_files.contains(&file_path_buf) {
            return Err(format!("File already processed: {}", file_path.display()).into());
        }

        let parser = self.parsers.get_mut(&language).ok_or_else(|| {
            format!("No parser available for language: {:?}", language)
        })?;

        let tree = parser.parse(&source, None).ok_or_else(|| {
            format!("Failed to parse file: {}", file_path.display())
        })?;

        let mut converter = AstToGraphConverter::new(
            language.clone(),
            file_path.to_string_lossy().to_string(),
            source,
        );

        converter.convert(tree.root_node()).map_err(|e| {
            format!("Conversion failed: {}", e)
        })?;

        self.register_symbols(&converter, &file_path_buf)?;
        self.file_entities.insert(file_path_buf.clone(), converter.entities.clone());
        self.processed_files.insert(file_path_buf);

        Ok(ConversionResult {
            nodes: converter.get_nodes(),
            edges: converter.get_edges(),
            entities: converter.entities,
            relationships: converter.relationships,
        })
    }

    pub fn resolve_cross_file_links(&mut self) -> Result<Vec<CodeEdge>> {
        let mut cross_file_edges = Vec::new();
        
        for (file_path, entities) in &self.file_entities {
            for entity in entities {
                let potential_refs = self.find_potential_cross_file_references(entity, file_path)?;
                
                for (target_symbol, target_node_id, target_file) in potential_refs {
                    let edge_type = self.infer_cross_file_edge_type(&entity.node.content, &target_symbol);
                    
                    cross_file_edges.push(CodeEdge::new(
                        entity.node.id,
                        target_node_id,
                        edge_type,
                    ).with_metadata("cross_file".to_string(), "true".to_string())
                     .with_metadata("target_file".to_string(), target_file.to_string_lossy().to_string()));
                }
            }
        }

        Ok(cross_file_edges)
    }

    pub fn build_dependency_graph(&self) -> Result<DependencyGraph> {
        let mut graph = DependencyGraph::new();
        
        for file_path in &self.processed_files {
            graph.add_file(file_path.clone());
        }

        for cross_ref in &self.cross_file_refs {
            graph.add_dependency(cross_ref.from_file.clone(), cross_ref.to_file.clone());
        }

        graph.compute_metrics()?;
        Ok(graph)
    }

    pub fn optimize_memory_usage(&mut self) {
        self.global_symbol_table.clear();
        self.file_entities.clear();
        self.cross_file_refs.clear();
    }

    fn register_symbols(&self, converter: &AstToGraphConverter, file_path: &PathBuf) -> Result<()> {
        for entity in &converter.entities {
            let global_symbol = format!("{}::{}", file_path.to_string_lossy(), entity.qualified_name);
            self.global_symbol_table.insert(
                global_symbol.clone(),
                (entity.node.id, file_path.clone()),
            );
            
            self.global_symbol_table.insert(
                entity.symbol_name.clone(),
                (entity.node.id, file_path.clone()),
            );
        }
        Ok(())
    }

    fn find_potential_cross_file_references(
        &self,
        entity: &CodeEntity,
        current_file: &PathBuf,
    ) -> Result<Vec<(String, NodeId, PathBuf)>> {
        let mut potential_refs = Vec::new();
        let empty = String::new();
        let content = entity.node.content.as_ref().unwrap_or(&empty);

        for (symbol, (node_id, file_path)) in self.global_symbol_table.iter() {
            if file_path != current_file && content.contains(symbol) {
                potential_refs.push((symbol.clone(), *node_id, file_path.clone()));
            }
        }

        Ok(potential_refs)
    }

    fn infer_cross_file_edge_type(&self, content: &Option<String>, symbol: &str) -> EdgeType {
        let empty2 = String::new();
        let content = content.as_ref().unwrap_or(&empty2);
        
        if content.contains("use ") || content.contains("import ") {
            EdgeType::Imports
        } else if content.contains(&format!("{}(", symbol)) {
            EdgeType::Calls
        } else if content.contains("extends") || content.contains("implements") {
            EdgeType::Implements
        } else {
            EdgeType::References
        }
    }

    fn detect_language(&self, file_path: &Path) -> Result<Language> {
        match file_path.extension().and_then(|s| s.to_str()) {
            Some("rs") => Ok(Language::Rust),
            Some("ts") => Ok(Language::TypeScript),
            Some("js") => Ok(Language::JavaScript),
            Some("py") => Ok(Language::Python),
            Some("go") => Ok(Language::Go),
            Some("java") => Ok(Language::Java),
            Some("cpp") | Some("cc") | Some("cxx") => Ok(Language::Cpp),
            _ => Ok(Language::Other("unknown".to_string())),
        }
    }
}

#[derive(Debug)]
pub struct ConversionResult {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
    pub entities: Vec<CodeEntity>,
    pub relationships: Vec<SemanticRelationship>,
}

#[derive(Debug)]
pub struct DependencyGraph {
    files: HashSet<PathBuf>,
    dependencies: HashMap<PathBuf, HashSet<PathBuf>>,
    metrics: DependencyMetrics,
}

#[derive(Debug, Default)]
pub struct DependencyMetrics {
    pub total_files: usize,
    pub total_dependencies: usize,
    pub cyclic_dependencies: Vec<Vec<PathBuf>>,
    pub fan_out: HashMap<PathBuf, usize>,
    pub fan_in: HashMap<PathBuf, usize>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            files: HashSet::new(),
            dependencies: HashMap::new(),
            metrics: DependencyMetrics::default(),
        }
    }

    pub fn add_file(&mut self, file: PathBuf) {
        self.files.insert(file);
    }

    pub fn add_dependency(&mut self, from: PathBuf, to: PathBuf) {
        self.dependencies.entry(from).or_insert_with(HashSet::new).insert(to);
    }

    pub fn compute_metrics(&mut self) -> Result<()> {
        self.metrics.total_files = self.files.len();
        self.metrics.total_dependencies = self.dependencies.values().map(|deps| deps.len()).sum();

        for (file, deps) in &self.dependencies {
            self.metrics.fan_out.insert(file.clone(), deps.len());
            
            for dep in deps {
                *self.metrics.fan_in.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        self.metrics.cyclic_dependencies = self.find_cycles()?;

        Ok(())
    }

    fn find_cycles(&self) -> Result<Vec<Vec<PathBuf>>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut current_path = Vec::new();

        for file in &self.files {
            if !visited.contains(file) {
                self.dfs_find_cycles(
                    file,
                    &mut visited,
                    &mut rec_stack,
                    &mut current_path,
                    &mut cycles,
                );
            }
        }

        Ok(cycles)
    }

    fn dfs_find_cycles(
        &self,
        file: &PathBuf,
        visited: &mut HashSet<PathBuf>,
        rec_stack: &mut HashSet<PathBuf>,
        current_path: &mut Vec<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        visited.insert(file.clone());
        rec_stack.insert(file.clone());
        current_path.push(file.clone());

        if let Some(deps) = self.dependencies.get(file) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.dfs_find_cycles(dep, visited, rec_stack, current_path, cycles);
                } else if rec_stack.contains(dep) {
                    if let Some(start_idx) = current_path.iter().position(|f| f == dep) {
                        let cycle = current_path[start_idx..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        current_path.pop();
        rec_stack.remove(file);
    }

    pub fn get_metrics(&self) -> &DependencyMetrics {
        &self.metrics
    }
}

pub struct ZeroCopySymbolResolver<'a> {
    source_bytes: &'a [u8],
    symbol_positions: HashMap<&'a str, Vec<(usize, usize)>>,
    resolved_cache: HashMap<&'a str, NodeId>,
}

impl<'a> ZeroCopySymbolResolver<'a> {
    pub fn new(source_bytes: &'a [u8]) -> Self {
        Self {
            source_bytes,
            symbol_positions: HashMap::new(),
            resolved_cache: HashMap::new(),
        }
    }

    pub fn index_symbols(&mut self, symbols: &[&'a str]) -> Result<()> {
        for symbol in symbols {
            let positions = self.find_symbol_positions(symbol);
            self.symbol_positions.insert(symbol, positions);
        }
        Ok(())
    }

    pub fn resolve_symbol(&self, symbol: &str) -> Option<NodeId> {
        self.resolved_cache.get(symbol).copied()
    }

    fn find_symbol_positions(&self, symbol: &str) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        let symbol_bytes = symbol.as_bytes();
        let mut start = 0;

        while let Some(pos) = self.find_next_occurrence(&self.source_bytes[start..], symbol_bytes) {
            let absolute_pos = start + pos;
            positions.push((absolute_pos, absolute_pos + symbol_bytes.len()));
            start = absolute_pos + symbol_bytes.len();
        }

        positions
    }

    fn find_next_occurrence(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_conversion_pipeline_basic() {
        let mut pipeline = ConversionPipeline::new().unwrap();
        
        let rust_code = r#"
use std::collections::HashMap;

struct Config {
    name: String,
}

fn process_config(config: Config) -> HashMap<String, String> {
    let mut result = HashMap::new();
    result.insert("name".to_string(), config.name);
    result
}
        "#;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        
        let result = pipeline.process_file(&file_path, rust_code.to_string()).unwrap();
        
        assert!(!result.nodes.is_empty());
        assert!(!result.edges.is_empty());
        assert!(result.nodes.iter().any(|n| n.name == "Config"));
        assert!(result.nodes.iter().any(|n| n.name == "process_config"));
    }

    #[test]
    fn test_zero_copy_symbol_resolver() {
        let source = "fn hello() { world(); }";
        let mut resolver = ZeroCopySymbolResolver::new(source.as_bytes());
        
        resolver.index_symbols(&["hello", "world"]).unwrap();
        
        let hello_positions = resolver.symbol_positions.get("hello").unwrap();
        assert_eq!(hello_positions.len(), 1);
        assert_eq!(hello_positions[0], (3, 8)); // "hello" starts at position 3
    }

    #[test]
    fn test_dependency_graph_cycles() {
        let mut graph = DependencyGraph::new();
        let file_a = PathBuf::from("a.rs");
        let file_b = PathBuf::from("b.rs");
        let file_c = PathBuf::from("c.rs");

        graph.add_file(file_a.clone());
        graph.add_file(file_b.clone());
        graph.add_file(file_c.clone());

        graph.add_dependency(file_a.clone(), file_b.clone());
        graph.add_dependency(file_b.clone(), file_c.clone());
        graph.add_dependency(file_c.clone(), file_a.clone()); // Creates a cycle

        graph.compute_metrics().unwrap();

        assert_eq!(graph.metrics.cyclic_dependencies.len(), 1);
        assert_eq!(graph.metrics.total_files, 3);
    }
}
