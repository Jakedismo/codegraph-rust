use crate::AstToGraphConverter;
use codegraph_core::{EdgeType, Language, NodeType};

use tree_sitter::Parser;

// Additional test modules can be added here as needed

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tree_sitter_python;
    use tree_sitter_rust;
    use tree_sitter_typescript;

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

            // Verify usage of imported entities
            assert!(edges.iter().any(|e| matches!(e.edge_type, EdgeType::Uses)
                || matches!(e.edge_type, EdgeType::References)));

            println!(
                "Python: Extracted {} nodes and {} edges",
                nodes.len(),
                edges.len()
            );
        }
    }
}
