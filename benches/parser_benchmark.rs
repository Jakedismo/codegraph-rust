use codegraph_parser::{TreeSitterParser, ParsingStatistics};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

const SMALL_FILE_SIZE: usize = 100;   // ~100 lines
const MEDIUM_FILE_SIZE: usize = 1000; // ~1000 lines  
const LARGE_FILE_SIZE: usize = 5000;  // ~5000 lines

fn create_test_rust_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use std::sync::Arc;\n\n");
    
    for i in 0..lines {
        match i % 10 {
            0 => code.push_str(&format!("pub struct TestStruct{} {{\n    pub field{}: i32,\n}}\n\n", i, i)),
            1 => code.push_str(&format!("pub enum TestEnum{} {{\n    Variant{}(i32),\n    OtherVariant{},\n}}\n\n", i, i, i)),
            2 => code.push_str(&format!("pub trait TestTrait{} {{\n    fn method{}(&self) -> i32;\n}}\n\n", i, i)),
            3 => code.push_str(&format!("impl TestTrait{} for TestStruct{} {{\n    fn method{}(&self) -> i32 {{ self.field{} }}\n}}\n\n", i, i, i, i)),
            4 => code.push_str(&format!("pub fn function{}(param: i32) -> i32 {{\n    let result = param * {};\n    result\n}}\n\n", i, i)),
            5 => code.push_str(&format!("const CONSTANT{}: i32 = {};\n", i, i)),
            6 => code.push_str(&format!("static STATIC{}: i32 = {};\n", i, i)),
            7 => code.push_str(&format!("type TypeAlias{} = HashMap<String, i32>;\n", i)),
            8 => code.push_str(&format!("mod module{} {{\n    pub fn inner_function() -> i32 {{ {} }}\n}}\n\n", i, i)),
            _ => code.push_str(&format!("// Comment line {}\nlet variable{} = {};\n", i, i, i)),
        }
    }
    
    code
}

fn create_test_typescript_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("import { Component } from 'react';\nimport * as React from 'react';\n\n");
    
    for i in 0..lines {
        match i % 8 {
            0 => code.push_str(&format!("interface TestInterface{} {{\n  property{}: number;\n  method{}(): void;\n}}\n\n", i, i, i)),
            1 => code.push_str(&format!("class TestClass{} implements TestInterface{} {{\n  property{}: number = {};\n  method{}() {{ console.log(this.property{}); }}\n}}\n\n", i, i, i, i, i, i)),
            2 => code.push_str(&format!("function testFunction{}(param: number): number {{\n  return param * {};\n}}\n\n", i, i)),
            3 => code.push_str(&format!("const arrowFunction{} = (x: number) => x + {};\n", i, i)),
            4 => code.push_str(&format!("type TestType{} = {{\n  field{}: string;\n}};\n\n", i, i)),
            5 => code.push_str(&format!("enum TestEnum{} {{\n  Value{} = {},\n  OtherValue{} = '{}'\n}}\n\n", i, i, i, i, format!("value{}", i))),
            6 => code.push_str(&format!("namespace TestNamespace{} {{\n  export const value = {};\n}}\n\n", i, i)),
            _ => code.push_str(&format!("// Comment {}\nconst variable{}: number = {};\n", i, i, i)),
        }
    }
    
    code
}

fn create_test_python_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("import os\nimport sys\nfrom typing import Dict, List\n\n");
    
    for i in 0..lines {
        match i % 6 {
            0 => code.push_str(&format!("class TestClass{}:\n    def __init__(self, value: int):\n        self.value = value\n\n    def method{}(self) -> int:\n        return self.value * {}\n\n", i, i, i)),
            1 => code.push_str(&format!("def function{}(param: int) -> int:\n    \"\"\"Function documentation for {}\"\"\"\n    result = param + {}\n    return result\n\n", i, i, i)),
            2 => code.push_str(&format!("async def async_function{}(param: str) -> str:\n    return f\"{{param}}_{{{}}\"\n\n", i, i)),
            3 => code.push_str(&format!("variable{}: int = {}\n", i, i)),
            4 => code.push_str(&format!("CONSTANT{}: str = \"value{}\"\n\n", i, i)),
            _ => code.push_str(&format!("# Comment {}\nif True:\n    temp{} = {}\n\n", i, i, i)),
        }
    }
    
    code
}

fn create_test_project(temp_dir: &TempDir, total_lines: usize) -> std::io::Result<()> {
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir)?;
    
    // Distribute lines across different files and languages
    let rust_lines = total_lines / 3;
    let ts_lines = total_lines / 3;
    let py_lines = total_lines - rust_lines - ts_lines;
    
    // Create Rust files
    let rust_content = create_test_rust_code(rust_lines);
    fs::write(src_dir.join("main.rs"), rust_content)?;
    
    // Create TypeScript files
    let ts_content = create_test_typescript_code(ts_lines);
    fs::write(src_dir.join("main.ts"), ts_content)?;
    
    // Create Python files
    let py_content = create_test_python_code(py_lines);
    fs::write(src_dir.join("main.py"), py_content)?;
    
    Ok(())
}

fn bench_single_file_parsing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("single_file_parsing");
    group.measurement_time(Duration::from_secs(30));
    
    for &size in &[SMALL_FILE_SIZE, MEDIUM_FILE_SIZE, LARGE_FILE_SIZE] {
        group.bench_with_input(
            BenchmarkId::new("rust", size),
            &size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let file_path = temp_dir.path().join("test.rs");
                let content = create_test_rust_code(size);
                fs::write(&file_path, content).unwrap();
                
                let parser = TreeSitterParser::new();
                
                b.to_async(&rt).iter(|| async {
                    black_box(parser.parse_file(&file_path.to_string_lossy()).await.unwrap())
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("typescript", size),
            &size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let file_path = temp_dir.path().join("test.ts");
                let content = create_test_typescript_code(size);
                fs::write(&file_path, content).unwrap();
                
                let parser = TreeSitterParser::new();
                
                b.to_async(&rt).iter(|| async {
                    black_box(parser.parse_file(&file_path.to_string_lossy()).await.unwrap())
                });
            },
        );
    }
    
    group.finish();
}

fn bench_parallel_directory_parsing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("parallel_directory_parsing");
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(10);
    
    // Test with different total line counts to hit our 10,000+ LOC target
    for &total_lines in &[5_000, 10_000, 15_000, 25_000] {
        group.bench_with_input(
            BenchmarkId::new("mixed_languages", total_lines),
            &total_lines,
            |b, &total_lines| {
                let temp_dir = TempDir::new().unwrap();
                create_test_project(&temp_dir, total_lines).unwrap();
                
                let parser = TreeSitterParser::new();
                
                b.to_async(&rt).iter(|| async {
                    let (nodes, stats) = black_box(
                        parser.parse_directory_parallel(&temp_dir.path().to_string_lossy()).await.unwrap()
                    );
                    
                    // Verify we're meeting our performance targets
                    println!(
                        "Parsed {} files, {} lines in {:.2}s ({:.1} files/s, {:.0} lines/s)",
                        stats.parsed_files,
                        stats.total_lines,
                        stats.parsing_duration.as_secs_f64(),
                        stats.files_per_second,
                        stats.lines_per_second
                    );
                    
                    // Assert performance requirements
                    if stats.total_lines >= 10_000 {
                        assert!(stats.parsing_duration.as_secs() < 30, 
                                "Failed to parse {}+ lines in under 30 seconds: took {:.2}s", 
                                stats.total_lines, stats.parsing_duration.as_secs_f64());
                    }
                    
                    (nodes, stats)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_concurrent_parsing_scalability(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrency_scalability");
    group.measurement_time(Duration::from_secs(45));
    
    let total_lines = 15_000;
    
    for &concurrency in &[1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_files", concurrency),
            &concurrency,
            |b, &concurrency| {
                let temp_dir = TempDir::new().unwrap();
                create_test_project(&temp_dir, total_lines).unwrap();
                
                let parser = TreeSitterParser::new().with_concurrency(concurrency);
                
                b.to_async(&rt).iter(|| async {
                    let (nodes, stats) = black_box(
                        parser.parse_directory_parallel(&temp_dir.path().to_string_lossy()).await.unwrap()
                    );
                    
                    println!("Concurrency {}: {:.2}s for {} lines ({:.0} lines/s)", 
                            concurrency, stats.parsing_duration.as_secs_f64(), 
                            stats.total_lines, stats.lines_per_second);
                    
                    (nodes, stats)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_incremental_parsing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("incremental_parsing");
    group.measurement_time(Duration::from_secs(30));
    
    group.bench_function("rust_code_modification", |b| {
        let original = create_test_rust_code(1000);
        let modified = format!("{}\n\npub fn new_function() -> i32 {{ 42 }}\n", original);
        
        let parser = TreeSitterParser::new();
        
        b.to_async(&rt).iter(|| async {
            black_box(
                parser.incremental_update("test.rs", &original, &modified).await.unwrap()
            )
        });
    });
    
    group.finish();
}

fn bench_error_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("error_recovery");
    group.measurement_time(Duration::from_secs(20));
    
    group.bench_function("malformed_rust_code", |b| {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("malformed.rs");
        
        // Create malformed Rust code that will trigger error recovery
        let malformed_code = r#"
            use std::collections::HashMap;
            
            pub struct Test {
                field: i32,
            }
            
            impl Test {
                pub fn method(&self) -> i32 {
                    self.field + // Missing right operand
                }
                
                pub fn another_method(&self {  // Missing closing paren
                    println!("test");
                }
            }
            
            fn function_with_error( // Missing parameters and body
            
            pub fn valid_function() -> i32 {
                42
            }
        "#;
        
        fs::write(&file_path, malformed_code).unwrap();
        let parser = TreeSitterParser::new();
        
        b.to_async(&rt).iter(|| async {
            // This should not panic and should recover some valid nodes
            let result = parser.parse_file(&file_path.to_string_lossy()).await;
            black_box(result.unwrap_or_else(|_| Vec::new()))
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_file_parsing,
    bench_parallel_directory_parsing,
    bench_concurrent_parsing_scalability,
    bench_incremental_parsing,
    bench_error_recovery
);
criterion_main!(benches);