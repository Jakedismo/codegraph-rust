use super::{InvalidationResult, UpdateRequest};
use crate::embedding::EmbeddingError;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub location: SymbolLocation,
    pub scope: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolType {
    Function,
    Class,
    Variable,
    Constant,
    Module,
    Interface,
    Trait,
    Struct,
    Enum,
    Type,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolLocation {
    pub file_path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub byte_offset: u32,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub from_symbol: Symbol,
    pub to_symbol: Symbol,
    pub dependency_type: DependencyType,
    pub strength: DependencyStrength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyType {
    Import,
    FunctionCall,
    Inheritance,
    Implementation,
    Usage,
    TypeReference,
    FieldAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyStrength {
    Weak,    // Comment references, optional dependencies
    Medium,  // Function calls, variable usage
    Strong,  // Direct inheritance, required imports
    Critical, // Core definitions, breaking changes
}

#[derive(Debug, Clone)]
pub struct DependencyGraph {
    symbols: HashMap<Symbol, SymbolNode>,
    file_to_symbols: HashMap<PathBuf, HashSet<Symbol>>,
    forward_deps: HashMap<Symbol, HashSet<Dependency>>,
    reverse_deps: HashMap<Symbol, HashSet<Dependency>>,
}

#[derive(Debug, Clone)]
pub struct SymbolNode {
    pub symbol: Symbol,
    pub last_modified: std::time::SystemTime,
    pub version: u32,
    pub metadata: SymbolMetadata,
}

#[derive(Debug, Clone)]
pub struct SymbolMetadata {
    pub visibility: Visibility,
    pub is_exported: bool,
    pub documentation: Option<String>,
    pub annotations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            file_to_symbols: HashMap::new(),
            forward_deps: HashMap::new(),
            reverse_deps: HashMap::new(),
        }
    }

    pub fn add_symbol(&mut self, symbol: Symbol, metadata: SymbolMetadata) {
        let node = SymbolNode {
            symbol: symbol.clone(),
            last_modified: std::time::SystemTime::now(),
            version: 1,
            metadata,
        };

        self.symbols.insert(symbol.clone(), node);
        
        self.file_to_symbols
            .entry(symbol.location.file_path.clone())
            .or_insert_with(HashSet::new)
            .insert(symbol);
    }

    pub fn add_dependency(&mut self, dep: Dependency) {
        self.forward_deps
            .entry(dep.from_symbol.clone())
            .or_insert_with(HashSet::new)
            .insert(dep.clone());

        self.reverse_deps
            .entry(dep.to_symbol.clone())
            .or_insert_with(HashSet::new)
            .insert(dep);
    }

    pub fn remove_symbol(&mut self, symbol: &Symbol) -> bool {
        if let Some(node) = self.symbols.remove(symbol) {
            // Remove from file mapping
            if let Some(symbols) = self.file_to_symbols.get_mut(&node.symbol.location.file_path) {
                symbols.remove(symbol);
                if symbols.is_empty() {
                    self.file_to_symbols.remove(&node.symbol.location.file_path);
                }
            }

            // Remove all dependencies involving this symbol
            self.forward_deps.remove(symbol);
            self.reverse_deps.remove(symbol);

            // Remove from other symbols' dependencies
            for deps in self.forward_deps.values_mut() {
                deps.retain(|dep| &dep.to_symbol != symbol);
            }
            for deps in self.reverse_deps.values_mut() {
                deps.retain(|dep| &dep.from_symbol != symbol);
            }

            true
        } else {
            false
        }
    }

    pub fn get_dependencies(&self, symbol: &Symbol) -> Vec<&Dependency> {
        self.forward_deps
            .get(symbol)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    pub fn get_dependents(&self, symbol: &Symbol) -> Vec<&Dependency> {
        self.reverse_deps
            .get(symbol)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    pub fn get_symbols_in_file(&self, file_path: &PathBuf) -> HashSet<Symbol> {
        self.file_to_symbols
            .get(file_path)
            .cloned()
            .unwrap_or_default()
    }

    pub fn compute_transitive_dependents(&self, symbol: &Symbol, max_depth: usize) -> HashSet<Symbol> {
        let mut visited = HashSet::new();
        let mut stack = vec![(symbol.clone(), 0)];
        let mut dependents = HashSet::new();

        while let Some((current_symbol, depth)) = stack.pop() {
            if depth > max_depth || visited.contains(&current_symbol) {
                continue;
            }

            visited.insert(current_symbol.clone());

            if let Some(deps) = self.reverse_deps.get(&current_symbol) {
                for dep in deps {
                    dependents.insert(dep.from_symbol.clone());
                    stack.push((dep.from_symbol.clone(), depth + 1));
                }
            }
        }

        dependents
    }

    pub fn find_critical_path(&self, from: &Symbol, to: &Symbol) -> Option<Vec<Symbol>> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        
        if self.dfs_path(from, to, &mut visited, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn dfs_path(&self, current: &Symbol, target: &Symbol, visited: &mut HashSet<Symbol>, path: &mut Vec<Symbol>) -> bool {
        if current == target {
            path.push(current.clone());
            return true;
        }

        if visited.contains(current) {
            return false;
        }

        visited.insert(current.clone());
        path.push(current.clone());

        if let Some(deps) = self.forward_deps.get(current) {
            for dep in deps {
                if self.dfs_path(&dep.to_symbol, target, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
    }

    pub fn analyze_impact(&self, changed_symbols: &[Symbol]) -> ImpactAnalysis {
        let mut affected_files = HashSet::new();
        let mut affected_symbols = HashSet::new();
        let mut cascade_depth = 0;

        for symbol in changed_symbols {
            let dependents = self.compute_transitive_dependents(symbol, 10);
            
            for dependent in &dependents {
                affected_files.insert(dependent.location.file_path.clone());
            }
            
            affected_symbols.extend(dependents);
            cascade_depth = cascade_depth.max(self.compute_cascade_depth(symbol));
        }

        ImpactAnalysis {
            affected_files,
            affected_symbols,
            cascade_depth,
            change_categories: self.categorize_changes(changed_symbols),
        }
    }

    fn compute_cascade_depth(&self, symbol: &Symbol) -> usize {
        let mut max_depth = 0;
        let mut visited = HashSet::new();
        let mut stack = vec![(symbol.clone(), 0)];

        while let Some((current, depth)) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current.clone());
            max_depth = max_depth.max(depth);

            if let Some(deps) = self.reverse_deps.get(&current) {
                for dep in deps {
                    stack.push((dep.from_symbol.clone(), depth + 1));
                }
            }
        }

        max_depth
    }

    fn categorize_changes(&self, symbols: &[Symbol]) -> Vec<ChangeCategory> {
        let mut categories = Vec::new();

        for symbol in symbols {
            match &symbol.symbol_type {
                SymbolType::Interface | SymbolType::Trait => {
                    categories.push(ChangeCategory::ApiChange);
                }
                SymbolType::Function => {
                    if self.is_public_function(symbol) {
                        categories.push(ChangeCategory::ApiChange);
                    } else {
                        categories.push(ChangeCategory::Implementation);
                    }
                }
                SymbolType::Type | SymbolType::Struct | SymbolType::Enum => {
                    categories.push(ChangeCategory::DataStructure);
                }
                _ => {
                    categories.push(ChangeCategory::Implementation);
                }
            }
        }

        categories.sort();
        categories.dedup();
        categories
    }

    fn is_public_function(&self, symbol: &Symbol) -> bool {
        self.symbols
            .get(symbol)
            .map(|node| node.metadata.visibility == Visibility::Public)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct ImpactAnalysis {
    pub affected_files: HashSet<PathBuf>,
    pub affected_symbols: HashSet<Symbol>,
    pub cascade_depth: usize,
    pub change_categories: Vec<ChangeCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeCategory {
    Implementation,
    DataStructure,
    ApiChange,
    Breaking,
}

pub struct DependencyTracker {
    graph: DependencyGraph,
    parsers: HashMap<String, Box<dyn DependencyParser + Send + Sync>>,
}

impl DependencyTracker {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Box<dyn DependencyParser + Send + Sync>> = HashMap::new();
        
        // Register language-specific dependency parsers
        parsers.insert("rust".to_string(), Box::new(RustDependencyParser::new()));
        parsers.insert("python".to_string(), Box::new(PythonDependencyParser::new()));
        parsers.insert("javascript".to_string(), Box::new(JavaScriptDependencyParser::new()));
        parsers.insert("typescript".to_string(), Box::new(TypeScriptDependencyParser::new()));

        Self {
            graph: DependencyGraph::new(),
            parsers,
        }
    }

    pub async fn update_dependencies(&mut self, request: &UpdateRequest, _result: &InvalidationResult) -> Result<(), EmbeddingError> {
        match &request.change_type {
            super::ChangeType::Added | super::ChangeType::Modified => {
                self.parse_and_update_file(&request.file_path).await?;
            }
            super::ChangeType::Deleted => {
                self.remove_file_dependencies(&request.file_path);
            }
            super::ChangeType::Moved(new_path) => {
                self.move_file_dependencies(&request.file_path, new_path);
            }
        }

        Ok(())
    }

    pub fn get_graph(&self) -> &DependencyGraph {
        &self.graph
    }

    async fn parse_and_update_file(&mut self, file_path: &PathBuf) -> Result<(), EmbeddingError> {
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let language = match extension {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            _ => return Ok(()), // Unsupported language
        };

        if let Some(parser) = self.parsers.get(language) {
            let source = tokio::fs::read_to_string(file_path).await
                .map_err(|e| EmbeddingError::InferenceError(e.to_string()))?;

            let parse_result = parser.parse_dependencies(&source, file_path.clone())?;
            
            // Remove old symbols for this file
            let old_symbols = self.graph.get_symbols_in_file(file_path);
            for symbol in old_symbols {
                self.graph.remove_symbol(&symbol);
            }

            // Add new symbols and dependencies
            for symbol_info in parse_result.symbols {
                self.graph.add_symbol(symbol_info.symbol, symbol_info.metadata);
            }

            for dependency in parse_result.dependencies {
                self.graph.add_dependency(dependency);
            }
        }

        Ok(())
    }

    fn remove_file_dependencies(&mut self, file_path: &PathBuf) {
        let symbols = self.graph.get_symbols_in_file(file_path);
        for symbol in symbols {
            self.graph.remove_symbol(&symbol);
        }
    }

    fn move_file_dependencies(&mut self, old_path: &PathBuf, new_path: &PathBuf) {
        // This would need to update all symbol locations
        // For now, just remove and re-parse
        self.remove_file_dependencies(old_path);
        // Would trigger re-parsing of new_path in real implementation
    }
}

pub trait DependencyParser {
    fn parse_dependencies(&self, source: &str, file_path: PathBuf) -> Result<DependencyParseResult, EmbeddingError>;
}

#[derive(Debug)]
pub struct DependencyParseResult {
    pub symbols: Vec<SymbolInfo>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub struct SymbolInfo {
    pub symbol: Symbol,
    pub metadata: SymbolMetadata,
}

// Language-specific parsers
struct RustDependencyParser;
struct PythonDependencyParser;
struct JavaScriptDependencyParser;
struct TypeScriptDependencyParser;

impl RustDependencyParser {
    fn new() -> Self { Self }
}

impl DependencyParser for RustDependencyParser {
    fn parse_dependencies(&self, _source: &str, _file_path: PathBuf) -> Result<DependencyParseResult, EmbeddingError> {
        // Simplified implementation - would use syn or similar for real parsing
        Ok(DependencyParseResult {
            symbols: vec![],
            dependencies: vec![],
        })
    }
}

impl PythonDependencyParser { fn new() -> Self { Self } }
impl JavaScriptDependencyParser { fn new() -> Self { Self } }
impl TypeScriptDependencyParser { fn new() -> Self { Self } }

impl DependencyParser for PythonDependencyParser {
    fn parse_dependencies(&self, _source: &str, _file_path: PathBuf) -> Result<DependencyParseResult, EmbeddingError> {
        Ok(DependencyParseResult { symbols: vec![], dependencies: vec![] })
    }
}

impl DependencyParser for JavaScriptDependencyParser {
    fn parse_dependencies(&self, _source: &str, _file_path: PathBuf) -> Result<DependencyParseResult, EmbeddingError> {
        Ok(DependencyParseResult { symbols: vec![], dependencies: vec![] })
    }
}

impl DependencyParser for TypeScriptDependencyParser {
    fn parse_dependencies(&self, _source: &str, _file_path: PathBuf) -> Result<DependencyParseResult, EmbeddingError> {
        Ok(DependencyParseResult { symbols: vec![], dependencies: vec![] })
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SymbolMetadata {
    fn default() -> Self {
        Self {
            visibility: Visibility::Private,
            is_exported: false,
            documentation: None,
            annotations: Vec::new(),
        }
    }
}