use crate::AstToGraphConverter;
use codegraph_core::{EdgeType, Language, NodeType};
use std::path::PathBuf;
use tempfile::TempDir;
use tree_sitter::Parser;

// Additional test modules can be added here as needed

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_rust_conversion() {
        let rust_code = r#"
use std::collections::HashMap;

pub struct Config {
    name: String,
    values: HashMap<String, i32>,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self {
            name,
            values: HashMap::new(),
        }
    }

    pub fn add_value(&mut self, key: String, value: i32) {
        self.values.insert(key, value);
    }

    pub fn get_value(&self, key: &str) -> Option<&i32> {
        self.values.get(key)
    }
}

pub fn create_config(name: &str) -> Config {
    Config::new(name.to_string())
}

pub fn process_config(mut config: Config) -> HashMap<String, i32> {
    config.add_value("default".to_string(), 42);
    config.values
}
        "#;

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(tree_sitter_rust::LANGUAGE.into_raw()() as *const _)
        };
        parser.set_language(&language).unwrap();

        if let Some(tree) = parser.parse(rust_code, None) {
            let mut converter = AstToGraphConverter::new(
                Language::Rust,
                "test.rs".to_string(),
                rust_code.to_string(),
            );

            converter.convert(tree.root_node()).unwrap();

            let nodes = converter.get_nodes();
            let edges = converter.get_edges();

            // Verify we extracted the correct entities
            assert!(nodes.iter().any(|n| &*n.name == "Config"));
            assert!(nodes.iter().any(|n| &*n.name == "new"));
            assert!(nodes.iter().any(|n| &*n.name == "add_value"));
            assert!(nodes.iter().any(|n| &*n.name == "get_value"));
            assert!(nodes.iter().any(|n| &*n.name == "create_config"));
            assert!(nodes.iter().any(|n| &*n.name == "process_config"));

            // Verify we have relationships
            assert!(!edges.is_empty());

            // Verify relationship types
            assert!(edges.iter().any(|e| matches!(e.edge_type, EdgeType::Calls)));
            assert!(edges.iter().any(|e| matches!(e.edge_type, EdgeType::Uses)));

            println!("Extracted {} nodes and {} edges", nodes.len(), edges.len());

            for node in &nodes {
                println!("Node: {} ({:?})", node.name, node.node_type);
            }

            for edge in &edges {
                println!(
                    "Edge: {:?} -> {:?} ({:?})",
                    edge.from, edge.to, edge.edge_type
                );
            }
        }
    }

    #[test]
    fn test_typescript_conversion() {
        let ts_code = r#"
import { Component } from 'react';

interface UserProps {
    name: string;
    age: number;
}

class User extends Component<UserProps> {
    constructor(props: UserProps) {
        super(props);
    }

    getName(): string {
        return this.props.name;
    }

    getAge(): number {
        return this.props.age;
    }
}

function createUser(name: string, age: number): User {
    return new User({ name, age });
}

export default User;
        "#;

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into_raw()() as *const _,
            )
        };
        parser.set_language(&language).unwrap();

        if let Some(tree) = parser.parse(ts_code, None) {
            let mut converter = AstToGraphConverter::new(
                Language::TypeScript,
                "User.tsx".to_string(),
                ts_code.to_string(),
            );

            converter.convert(tree.root_node()).unwrap();

            let nodes = converter.get_nodes();
            let edges = converter.get_edges();

            // Verify TypeScript-specific entities
            assert!(nodes.iter().any(
                |n| &*n.name == "UserProps" && matches!(n.node_type, Some(NodeType::Interface))
            ));
            assert!(nodes
                .iter()
                .any(|n| &*n.name == "User" && matches!(n.node_type, Some(NodeType::Class))));
            assert!(nodes.iter().any(
                |n| &*n.name == "createUser" && matches!(n.node_type, Some(NodeType::Function))
            ));

            // Verify inheritance relationship
            assert!(edges
                .iter()
                .any(|e| matches!(e.edge_type, EdgeType::Extends)));

            println!(
                "TypeScript: Extracted {} nodes and {} edges",
                nodes.len(),
                edges.len()
            );
        }
    }

    #[test]
    fn test_python_conversion() {
        let py_code = r#"
import json
from typing import Dict, List, Optional

class DataProcessor:
    def __init__(self, name: str):
        self.name = name
        self.data: Dict[str, List[int]] = {}

    def add_data(self, key: str, values: List[int]) -> None:
        self.data[key] = values

    def get_data(self, key: str) -> Optional[List[int]]:
        return self.data.get(key)

    def process_all(self) -> Dict[str, int]:
        result = {}
        for key, values in self.data.items():
            result[key] = sum(values)
        return result

def create_processor(name: str) -> DataProcessor:
    return DataProcessor(name)

def save_to_file(processor: DataProcessor, filename: str) -> None:
    data = processor.process_all()
    with open(filename, 'w') as f:
        json.dump(data, f)
        "#;

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(tree_sitter_python::LANGUAGE.into_raw()() as *const _)
        };
        parser.set_language(&language).unwrap();

        if let Some(tree) = parser.parse(py_code, None) {
            let mut converter = AstToGraphConverter::new(
                Language::Python,
                "processor.py".to_string(),
                py_code.to_string(),
            );

            converter.convert(tree.root_node()).unwrap();

            let nodes = converter.get_nodes();
            let edges = converter.get_edges();

            // Verify Python-specific entities
            assert!(nodes.iter().any(
                |n| &*n.name == "DataProcessor" && matches!(n.node_type, Some(NodeType::Class))
            ));
            assert!(nodes.iter().any(|n| &*n.name == "create_processor"
                && matches!(n.node_type, Some(NodeType::Function))));
            assert!(nodes
                .iter()
                .any(|n| &*n.name == "save_to_file"
                    && matches!(n.node_type, Some(NodeType::Function))));

            // Verify import relationships
            assert!(edges
                .iter()
                .any(|e| matches!(e.edge_type, EdgeType::Imports)));

            println!(
                "Python: Extracted {} nodes and {} edges",
                nodes.len(),
                edges.len()
            );
        }
    }

    #[test]
    #[cfg(feature = "experimental")]
    #[ignore = "ConversionPipeline not currently exported"]
    fn test_cross_language_pipeline() {
        let mut pipeline = ConversionPipeline::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Create test files
        let rust_file = temp_dir.path().join("lib.rs");
        std::fs::write(
            &rust_file,
            "pub fn hello() -> String { \"Hello\".to_string() }",
        )
        .unwrap();

        let ts_file = temp_dir.path().join("app.ts");
        std::fs::write(
            &ts_file,
            "export function greet(name: string): string { return `Hello, ${name}`; }",
        )
        .unwrap();

        // Process files
        let rust_result = pipeline
            .process_file(&rust_file, std::fs::read_to_string(&rust_file).unwrap())
            .unwrap();
        let ts_result = pipeline
            .process_file(&ts_file, std::fs::read_to_string(&ts_file).unwrap())
            .unwrap();

        assert!(!rust_result.nodes.is_empty());
        assert!(!ts_result.nodes.is_empty());

        // Build dependency graph
        let dep_graph = pipeline.build_dependency_graph().unwrap();
        let metrics = dep_graph.get_metrics();

        assert_eq!(metrics.total_files, 2);

        println!(
            "Cross-language pipeline processed {} files",
            metrics.total_files
        );
    }

    #[test]
    #[cfg(feature = "experimental")]
    #[ignore = "ZeroCopyAstProcessor not currently exported"]
    fn test_memory_efficient_processing() {
        let large_rust_code = generate_large_rust_file(1000);

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(tree_sitter_rust::LANGUAGE.into_raw()() as *const _)
        };
        parser.set_language(&language).unwrap();

        if let Some(tree) = parser.parse(&large_rust_code, None) {
            let mut processor = ZeroCopyAstProcessor::new(
                large_rust_code.as_bytes(),
                Language::Rust,
                "large.rs".to_string(),
            );

            let result = processor.process_tree_zero_copy(tree.root_node()).unwrap();
            let memory_usage = processor.get_memory_usage();

            assert!(!result.nodes.is_empty());
            assert!(result.nodes.len() >= 1000); // Should find at least 1000 functions

            // Verify memory efficiency
            let efficiency =
                memory_usage.estimated_heap_usage as f64 / large_rust_code.len() as f64;
            assert!(
                efficiency < 0.5,
                "Memory usage should be less than 50% of source size"
            );

            println!(
                "Memory efficiency: {:.2}% (heap: {} bytes, source: {} bytes)",
                efficiency * 100.0,
                memory_usage.estimated_heap_usage,
                large_rust_code.len()
            );
        }
    }
}

fn generate_large_rust_file(num_functions: usize) -> String {
    let mut code = String::new();
    code.push_str("use std::collections::HashMap;\n\n");

    for i in 0..num_functions {
        code.push_str(&format!(
            "pub fn function_{}() -> i32 {{\n    let mut map = HashMap::new();\n    map.insert({}, {});\n    {}\n}}\n\n",
            i, i, i * 2, i
        ));
    }

    code
}

#[cfg(test)]
mod benchmark_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_conversion_performance() {
        let iterations = 100;
        let rust_code = r#"
use std::collections::HashMap;

pub struct Config {
    name: String,
    values: HashMap<String, i32>,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self {
            name,
            values: HashMap::new(),
        }
    }

    pub fn process(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
}

pub fn create_default() -> Config {
    Config::new("default".to_string())
}
        "#;

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(tree_sitter_rust::LANGUAGE.into_raw()() as *const _)
        };
        parser.set_language(&language).unwrap();
        let tree = parser.parse(rust_code, None).unwrap();

        let start = Instant::now();

        for _ in 0..iterations {
            let mut converter = AstToGraphConverter::new(
                Language::Rust,
                "benchmark.rs".to_string(),
                rust_code.to_string(),
            );

            converter.convert(tree.root_node()).unwrap();
            let _nodes = converter.get_nodes();
            let _edges = converter.get_edges();
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        println!(
            "Benchmark: {} iterations in {:?} (avg: {:?}/iteration)",
            iterations, elapsed, avg_time
        );

        // Should be reasonably fast
        assert!(
            avg_time.as_millis() < 100,
            "Conversion should be under 100ms per iteration"
        );
    }

    #[test]
    #[cfg(feature = "experimental")]
    #[ignore = "ZeroCopyAstProcessor not currently exported"]
    fn benchmark_zero_copy_vs_regular() {
        let rust_code = generate_large_rust_file(100);

        let mut parser = Parser::new();
        let language = unsafe {
            tree_sitter::Language::from_raw(tree_sitter_rust::LANGUAGE.into_raw()() as *const _)
        };
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&rust_code, None).unwrap();

        // Benchmark regular conversion
        let start = Instant::now();
        let mut converter = AstToGraphConverter::new(
            Language::Rust,
            "benchmark.rs".to_string(),
            rust_code.clone(),
        );
        converter.convert(tree.root_node()).unwrap();
        let regular_time = start.elapsed();

        // Benchmark zero-copy conversion
        let start = Instant::now();
        let mut processor = ZeroCopyAstProcessor::new(
            rust_code.as_bytes(),
            Language::Rust,
            "benchmark.rs".to_string(),
        );
        processor.process_tree_zero_copy(tree.root_node()).unwrap();
        let zero_copy_time = start.elapsed();

        println!("Regular conversion: {:?}", regular_time);
        println!("Zero-copy conversion: {:?}", zero_copy_time);

        // Zero-copy should be at least as fast (or faster)
        assert!(
            zero_copy_time <= regular_time * 2,
            "Zero-copy should not be significantly slower"
        );
    }
}
