use crate::{TreeSitterParser, LanguageRegistry};
use codegraph_core::{Language, NodeType};
use std::fs;
use tempfile::TempDir;
use tokio_test;

#[tokio::test]
async fn test_language_detection() {
    let registry = LanguageRegistry::new();
    
    assert_eq!(registry.detect_language("main.rs"), Some(Language::Rust));
    assert_eq!(registry.detect_language("component.tsx"), Some(Language::TypeScript));
    assert_eq!(registry.detect_language("script.js"), Some(Language::JavaScript));
    assert_eq!(registry.detect_language("module.py"), Some(Language::Python));
    assert_eq!(registry.detect_language("main.go"), Some(Language::Go));
    assert_eq!(registry.detect_language("Main.java"), Some(Language::Java));
    assert_eq!(registry.detect_language("program.cpp"), Some(Language::Cpp));
    assert_eq!(registry.detect_language("header.h"), Some(Language::Cpp));
    assert_eq!(registry.detect_language("unknown.xyz"), None);
}

#[tokio::test]
async fn test_rust_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");
    
    let rust_code = r#"
use std::collections::HashMap;

pub struct TestStruct {
    pub field: i32,
}

impl TestStruct {
    pub fn new(field: i32) -> Self {
        Self { field }
    }
    
    pub fn get_field(&self) -> i32 {
        self.field
    }
}

pub trait TestTrait {
    fn method(&self) -> i32;
}

impl TestTrait for TestStruct {
    fn method(&self) -> i32 {
        self.get_field()
    }
}

pub fn standalone_function() -> i32 {
    42
}

pub enum TestEnum {
    Variant1,
    Variant2(i32),
}
"#;
    
    fs::write(&file_path, rust_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let nodes = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    
    // Verify we extracted expected entities
    let struct_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Struct))
        .collect();
    assert!(!struct_nodes.is_empty(), "Should find struct definitions");
    
    let function_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Function))
        .collect();
    assert!(function_nodes.len() >= 3, "Should find multiple function definitions");
    
    let trait_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Trait))
        .collect();
    assert!(!trait_nodes.is_empty(), "Should find trait definitions");
    
    let import_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Import))
        .collect();
    assert!(!import_nodes.is_empty(), "Should find import statements");
}

#[tokio::test]
async fn test_typescript_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.ts");
    
    let ts_code = r#"
import { Component } from 'react';

interface TestInterface {
    property: number;
    method(): void;
}

class TestClass implements TestInterface {
    property: number;
    
    constructor(prop: number) {
        this.property = prop;
    }
    
    method(): void {
        console.log(this.property);
    }
}

function testFunction(param: number): number {
    return param * 2;
}

const arrowFunction = (x: number) => x + 1;

type TestType = {
    field: string;
};

enum TestEnum {
    Value1 = 1,
    Value2 = 'test'
}
"#;
    
    fs::write(&file_path, ts_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let nodes = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    
    let class_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Class))
        .collect();
    assert!(!class_nodes.is_empty(), "Should find class definitions");
    
    let interface_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Interface))
        .collect();
    assert!(!interface_nodes.is_empty(), "Should find interface definitions");
    
    let function_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Function))
        .collect();
    assert!(!function_nodes.is_empty(), "Should find function definitions");
}

#[tokio::test]
async fn test_python_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.py");
    
    let python_code = r#"
import os
from typing import Dict, List

class TestClass:
    def __init__(self, value: int):
        self.value = value
    
    def method(self) -> int:
        return self.value * 2
    
    @staticmethod
    def static_method() -> str:
        return "static"

def function(param: int) -> int:
    """Function documentation"""
    return param + 1

async def async_function(param: str) -> str:
    return f"async_{param}"

variable: int = 42
CONSTANT: str = "constant"
"#;
    
    fs::write(&file_path, python_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let nodes = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    
    let class_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Class))
        .collect();
    assert!(!class_nodes.is_empty(), "Should find class definitions");
    
    let function_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Function))
        .collect();
    assert!(function_nodes.len() >= 3, "Should find multiple function definitions");
    
    let import_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.node_type == Some(NodeType::Import))
        .collect();
    assert!(!import_nodes.is_empty(), "Should find import statements");
}

#[tokio::test]
async fn test_parallel_directory_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    
    // Create multiple files
    let rust_code = r#"
pub struct RustStruct {
    field: i32,
}

impl RustStruct {
    pub fn new() -> Self { Self { field: 0 } }
}
"#;
    
    let ts_code = r#"
interface TsInterface {
    prop: number;
}

class TsClass implements TsInterface {
    prop: number = 0;
}
"#;
    
    fs::write(src_dir.join("rust_file.rs"), rust_code).unwrap();
    fs::write(src_dir.join("ts_file.ts"), ts_code).unwrap();
    fs::write(src_dir.join("another.rs"), rust_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let (nodes, stats) = parser.parse_directory_parallel(&temp_dir.path().to_string_lossy()).await.unwrap();
    
    assert_eq!(stats.parsed_files, 3, "Should parse all 3 files");
    assert!(stats.total_lines > 0, "Should count lines");
    assert!(stats.parsing_duration.as_millis() > 0, "Should measure parsing time");
    assert!(!nodes.is_empty(), "Should extract nodes from all files");
    
    // Verify we have nodes from different languages
    let rust_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.language == Some(Language::Rust))
        .collect();
    let ts_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.language == Some(Language::TypeScript))
        .collect();
    
    assert!(!rust_nodes.is_empty(), "Should find Rust nodes");
    assert!(!ts_nodes.is_empty(), "Should find TypeScript nodes");
}

#[tokio::test]
async fn test_error_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("malformed.rs");
    
    // Malformed Rust code
    let malformed_code = r#"
pub struct ValidStruct {
    field: i32,
}

pub fn incomplete_function( // Missing closing paren and body

pub fn valid_function() -> i32 {
    42
}

impl ValidStruct {
    pub fn method(&self) -> i32 {
        self.field + // Missing operand
    }
}
"#;
    
    fs::write(&file_path, malformed_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let result = parser.parse_file(&file_path.to_string_lossy()).await;
    
    // Should not panic and should recover some nodes
    assert!(result.is_ok(), "Parser should handle malformed code gracefully");
    
    let nodes = result.unwrap();
    
    // Should still extract some valid structures
    let valid_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.name.contains("valid") || n.name.contains("Valid"))
        .collect();
    assert!(!valid_nodes.is_empty(), "Should recover some valid nodes");
}

#[tokio::test]
async fn test_incremental_parsing() {
    let original_code = r#"
pub struct TestStruct {
    field: i32,
}

impl TestStruct {
    pub fn method(&self) -> i32 {
        self.field
    }
}
"#;
    
    let modified_code = r#"
pub struct TestStruct {
    field: i32,
    new_field: String,
}

impl TestStruct {
    pub fn method(&self) -> i32 {
        self.field
    }
    
    pub fn new_method(&self) -> &str {
        &self.new_field
    }
}
"#;
    
    let parser = TreeSitterParser::new();
    let result = parser.incremental_update("test.rs", original_code, modified_code).await;
    
    assert!(result.is_ok(), "Incremental parsing should succeed");
    
    let nodes = result.unwrap();
    
    // Should detect the new method
    let new_method_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.name.contains("new_method"))
        .collect();
    assert!(!new_method_nodes.is_empty(), "Should detect new method in incremental update");
}

#[tokio::test]
async fn test_parser_caching() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");
    
    let rust_code = r#"
pub struct TestStruct {
    field: i32,
}
"#;
    
    fs::write(&file_path, rust_code).unwrap();
    
    let parser = TreeSitterParser::new();
    
    // First parse
    let start = std::time::Instant::now();
    let nodes1 = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    let first_duration = start.elapsed();
    
    // Second parse (should use cache)
    let start = std::time::Instant::now();
    let nodes2 = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    let second_duration = start.elapsed();
    
    assert_eq!(nodes1.len(), nodes2.len(), "Cached result should be identical");
    
    // Cache should make second parse faster (though this might not always be true in tests)
    let (cache_size, _) = parser.cache_stats();
    assert!(cache_size > 0, "Cache should have entries");
}

#[tokio::test]
async fn test_large_file_performance() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("large.rs");
    
    // Generate a large Rust file
    let mut large_code = String::new();
    large_code.push_str("use std::collections::HashMap;\n\n");
    
    for i in 0..1000 {
        large_code.push_str(&format!(
            r#"
pub struct TestStruct{i} {{
    field{i}: i32,
}}

impl TestStruct{i} {{
    pub fn new() -> Self {{
        Self {{ field{i}: {i} }}
    }}
    
    pub fn method{i}(&self) -> i32 {{
        self.field{i} * 2
    }}
}}

pub fn function{i}(param: i32) -> i32 {{
    param + {i}
}}
"#,
            i = i
        ));
    }
    
    fs::write(&file_path, large_code).unwrap();
    
    let parser = TreeSitterParser::new();
    let start = std::time::Instant::now();
    let nodes = parser.parse_file(&file_path.to_string_lossy()).await.unwrap();
    let duration = start.elapsed();
    
    assert!(nodes.len() > 2000, "Should extract many nodes from large file");
    assert!(duration.as_secs() < 5, "Should parse large file reasonably quickly");
    
    println!("Parsed {} nodes in {:.2}s", nodes.len(), duration.as_secs_f64());
}

#[tokio::test]
async fn test_concurrency_scaling() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    
    // Create many small files
    for i in 0..20 {
        let code = format!(
            r#"
pub struct TestStruct{} {{
    field: i32,
}}

pub fn function{}() -> i32 {{
    {}
}}
"#,
            i, i, i
        );
        fs::write(src_dir.join(format!("file_{}.rs", i)), code).unwrap();
    }
    
    // Test different concurrency levels
    for concurrency in &[1, 4, 8] {
        let parser = TreeSitterParser::new().with_concurrency(*concurrency);
        
        let start = std::time::Instant::now();
        let (nodes, stats) = parser.parse_directory_parallel(&temp_dir.path().to_string_lossy()).await.unwrap();
        let duration = start.elapsed();
        
        assert_eq!(stats.parsed_files, 20, "Should parse all files");
        assert!(!nodes.is_empty(), "Should extract nodes");
        
        println!(
            "Concurrency {}: parsed {} files in {:.2}s ({:.1} files/s)",
            concurrency, stats.parsed_files, duration.as_secs_f64(),
            stats.files_per_second
        );
    }
}