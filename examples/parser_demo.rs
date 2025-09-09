use codegraph_parser::{TreeSitterParser, ParsingStatistics};
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

fn create_large_rust_project(temp_dir: &TempDir, total_lines: usize) -> std::io::Result<()> {
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir)?;
    
    // Create main.rs
    let mut main_content = String::new();
    main_content.push_str("use std::collections::HashMap;\nuse std::sync::Arc;\n\n");
    
    let lines_per_file = 500;
    let num_files = (total_lines / lines_per_file).max(1);
    
    for file_idx in 0..num_files {
        let mut file_content = String::new();
        file_content.push_str("use std::collections::HashMap;\n");
        file_content.push_str("use std::sync::Arc;\n\n");
        
        let file_lines = if file_idx == num_files - 1 {
            total_lines - (file_idx * lines_per_file)
        } else {
            lines_per_file
        };
        
        for i in 0..file_lines {
            let global_i = file_idx * lines_per_file + i;
            match global_i % 10 {
                0 => file_content.push_str(&format!(
                    "pub struct TestStruct{} {{\n    pub field{}: i32,\n    pub other_field{}: String,\n}}\n\n",
                    global_i, global_i, global_i
                )),
                1 => file_content.push_str(&format!(
                    "pub enum TestEnum{} {{\n    Variant{}(i32),\n    OtherVariant{}(String),\n    UnitVariant{},\n}}\n\n",
                    global_i, global_i, global_i, global_i
                )),
                2 => file_content.push_str(&format!(
                    "pub trait TestTrait{} {{\n    fn method{}(&self) -> i32;\n    fn default_method{}(&self) -> String {{ \"default\".to_string() }}\n}}\n\n",
                    global_i, global_i, global_i
                )),
                3 => file_content.push_str(&format!(
                    "impl TestTrait{} for TestStruct{} {{\n    fn method{}(&self) -> i32 {{ self.field{} }}\n}}\n\n",
                    global_i, global_i, global_i, global_i
                )),
                4 => file_content.push_str(&format!(
                    "pub fn function{}(param: i32, other: &str) -> Result<i32, String> {{\n    if param > 0 {{\n        Ok(param * {})\n    }} else {{\n        Err(\"Invalid parameter\".to_string())\n    }}\n}}\n\n",
                    global_i, global_i
                )),
                5 => file_content.push_str(&format!(
                    "pub async fn async_function{}(data: Vec<i32>) -> Vec<i32> {{\n    data.iter().map(|x| x * {}).collect()\n}}\n\n",
                    global_i, global_i
                )),
                6 => file_content.push_str(&format!(
                    "pub const CONSTANT{}: i32 = {};\npub static STATIC{}: &str = \"value{}\";\n\n",
                    global_i, global_i, global_i, global_i
                )),
                7 => file_content.push_str(&format!(
                    "type TypeAlias{} = HashMap<String, TestStruct{}>;\ntype ResultType{} = Result<TypeAlias{}, String>;\n\n",
                    global_i, global_i, global_i, global_i
                )),
                8 => file_content.push_str(&format!(
                    "mod module{} {{\n    use super::*;\n    pub fn inner_function{}() -> i32 {{ {} }}\n    pub struct InnerStruct{} {{ value: i32 }}\n}}\n\n",
                    global_i, global_i, global_i, global_i
                )),
                _ => file_content.push_str(&format!(
                    "// Complex comment block {}\nlet variable{}: HashMap<String, i32> = HashMap::new();\nlet closure{} = |x: i32| -> i32 {{ x + {} }};\n\n",
                    global_i, global_i, global_i, global_i
                )),
            }
        }
        
        let file_name = if file_idx == 0 { 
            "main.rs".to_string() 
        } else { 
            format!("module_{}.rs", file_idx) 
        };
        
        fs::write(src_dir.join(file_name), file_content)?;
    }
    
    // Create some TypeScript files too
    let ts_content = r#"
import { Component } from 'react';

interface UserInterface {
    id: number;
    name: string;
    email: string;
}

class UserService {
    private users: UserInterface[] = [];
    
    addUser(user: UserInterface): void {
        this.users.push(user);
    }
    
    findUser(id: number): UserInterface | undefined {
        return this.users.find(u => u.id === id);
    }
}

function processUsers(users: UserInterface[]): string[] {
    return users.map(user => `${user.name} (${user.email})`);
}

const userValidator = (user: UserInterface): boolean => {
    return user.id > 0 && user.name.length > 0 && user.email.includes('@');
};
"#;
    
    fs::write(src_dir.join("user.ts"), ts_content)?;
    
    // Create a Python file
    let py_content = r#"
from typing import List, Dict, Optional
import asyncio

class DataProcessor:
    def __init__(self, name: str):
        self.name = name
        self.data: List[Dict[str, any]] = []
    
    def add_data(self, item: Dict[str, any]) -> None:
        self.data.append(item)
    
    def process_data(self) -> List[str]:
        return [str(item) for item in self.data]
    
    async def async_process(self, items: List[int]) -> List[int]:
        await asyncio.sleep(0.1)
        return [item * 2 for item in items]

def calculate_average(numbers: List[float]) -> float:
    return sum(numbers) / len(numbers) if numbers else 0.0

async def main():
    processor = DataProcessor("main")
    result = await processor.async_process([1, 2, 3, 4, 5])
    print(f"Result: {result}")

if __name__ == "__main__":
    asyncio.run(main())
"#;
    
    fs::write(src_dir.join("data_processor.py"), py_content)?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CodeGraph Tree-sitter Parser Performance Demo");
    println!("==============================================");
    
    // Test different project sizes
    let test_cases = vec![
        (2_000, "Small project"),
        (5_000, "Medium project"),
        (10_000, "Large project"),
        (15_000, "Very large project"),
        (25_000, "Massive project"),
    ];
    
    for (total_lines, description) in test_cases {
        println!("\n{} (~{} lines):", description, total_lines);
        println!("{}", "-".repeat(50));
        
        let temp_dir = TempDir::new()?;
        create_large_rust_project(&temp_dir, total_lines)?;
        
        // Test with different concurrency levels
        for concurrency in &[1, 4, 8] {
            let parser = TreeSitterParser::new().with_concurrency(*concurrency);
            
            let start = Instant::now();
            let (nodes, stats) = parser
                .parse_directory_parallel(&temp_dir.path().to_string_lossy())
                .await?;
            let duration = start.elapsed();
            
            println!(
                "  Concurrency {}: {} files, {} lines, {} nodes",
                concurrency, stats.parsed_files, stats.total_lines, nodes.len()
            );
            println!(
                "    Time: {:.2}s ({:.1} files/s, {:.0} lines/s)",
                duration.as_secs_f64(),
                stats.files_per_second,
                stats.lines_per_second
            );
            
            // Check if we meet the 10,000+ LOC in <30s requirement
            if stats.total_lines >= 10_000 {
                let meets_requirement = duration.as_secs() < 30;
                println!(
                    "    Performance target (10k+ lines <30s): {} ({}s)", 
                    if meets_requirement { "✅ PASSED" } else { "❌ FAILED" },
                    duration.as_secs_f64()
                );
            }
            
            // Show cache statistics
            let (cache_size, cache_memory) = parser.cache_stats();
            println!(
                "    Cache: {} entries (~{} KB)",
                cache_size, cache_memory / 1024
            );
        }
    }
    
    println!("\n==============================================");
    println!("Demo completed successfully!");
    
    Ok(())
}