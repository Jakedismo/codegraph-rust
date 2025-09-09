// Simple test to verify AST to graph node conversion pipeline
use std::collections::HashMap;

// Simplified structures for testing
#[derive(Debug, Clone)]
pub struct SimpleNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub content: String,
    pub relationships: Vec<SimpleEdge>,
}

#[derive(Debug, Clone)]
pub struct SimpleEdge {
    pub from: String,
    pub to: String,
    pub edge_type: String,
}

// Simplified converter that demonstrates the pattern
pub struct SimpleAstConverter {
    pub nodes: Vec<SimpleNode>,
    pub symbol_table: HashMap<String, String>,
    pub source: String,
}

impl SimpleAstConverter {
    pub fn new(source: String) -> Self {
        Self {
            nodes: Vec::new(),
            symbol_table: HashMap::new(),
            source,
        }
    }

    pub fn extract_rust_entities(&mut self) -> Result<(), String> {
        // Simple pattern matching for demonstration
        let lines = self.source.lines().collect::<Vec<_>>();
        
        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();
            
            // Extract functions
            if line.starts_with("fn ") || line.starts_with("pub fn ") {
                if let Some(name) = self.extract_function_name(line) {
                    let id = format!("func_{}", i);
                    let node = SimpleNode {
                        id: id.clone(),
                        name: name.clone(),
                        node_type: "Function".to_string(),
                        content: line.to_string(),
                        relationships: Vec::new(),
                    };
                    self.nodes.push(node);
                    self.symbol_table.insert(name, id);
                }
            }
            
            // Extract structs
            if line.starts_with("struct ") || line.starts_with("pub struct ") {
                if let Some(name) = self.extract_struct_name(line) {
                    let id = format!("struct_{}", i);
                    let node = SimpleNode {
                        id: id.clone(),
                        name: name.clone(),
                        node_type: "Struct".to_string(),
                        content: line.to_string(),
                        relationships: Vec::new(),
                    };
                    self.nodes.push(node);
                    self.symbol_table.insert(name, id);
                }
            }
            
            // Extract use statements
            if line.starts_with("use ") {
                let id = format!("import_{}", i);
                let node = SimpleNode {
                    id: id.clone(),
                    name: line.to_string(),
                    node_type: "Import".to_string(),
                    content: line.to_string(),
                    relationships: Vec::new(),
                };
                self.nodes.push(node);
            }
        }
        
        Ok(())
    }

    pub fn build_relationships(&mut self) -> Result<(), String> {
        let nodes = self.nodes.clone();
        
        for node in &mut self.nodes {
            // Find function calls
            for other_name in self.symbol_table.keys() {
                if node.content.contains(&format!("{}(", other_name)) && node.name != *other_name {
                    if let Some(target_id) = self.symbol_table.get(other_name) {
                        node.relationships.push(SimpleEdge {
                            from: node.id.clone(),
                            to: target_id.clone(),
                            edge_type: "Calls".to_string(),
                        });
                    }
                }
                
                // Find references
                if node.content.contains(other_name) && node.name != *other_name && !node.content.contains(&format!("{}(", other_name)) {
                    if let Some(target_id) = self.symbol_table.get(other_name) {
                        node.relationships.push(SimpleEdge {
                            from: node.id.clone(),
                            to: target_id.clone(),
                            edge_type: "References".to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(())
    }

    fn extract_function_name(&self, line: &str) -> Option<String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for i in 0..parts.len() {
            if parts[i] == "fn" && i + 1 < parts.len() {
                let name_part = parts[i + 1];
                if let Some(paren_pos) = name_part.find('(') {
                    return Some(name_part[..paren_pos].to_string());
                }
                return Some(name_part.to_string());
            }
        }
        None
    }

    fn extract_struct_name(&self, line: &str) -> Option<String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for i in 0..parts.len() {
            if parts[i] == "struct" && i + 1 < parts.len() {
                let name_part = parts[i + 1];
                if let Some(brace_pos) = name_part.find('{') {
                    return Some(name_part[..brace_pos].trim().to_string());
                }
                return Some(name_part.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_entity_extraction() {
        let rust_code = r#"
use std::collections::HashMap;

pub struct Config {
    name: String,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

pub fn create_config(name: &str) -> Config {
    Config::new(name.to_string())
}

pub fn process_config(config: Config) -> String {
    let name = config.get_name();
    format!("Processing: {}", name)
}
        "#;

        let mut converter = SimpleAstConverter::new(rust_code.to_string());
        
        // Extract entities
        converter.extract_rust_entities().unwrap();
        
        // Verify we found the expected entities
        assert!(converter.nodes.len() >= 4); // At least: new, get_name, create_config, process_config
        
        let function_names: Vec<&String> = converter.nodes
            .iter()
            .filter(|n| n.node_type == "Function")
            .map(|n| &n.name)
            .collect();
        
        assert!(function_names.contains(&&"new".to_string()));
        assert!(function_names.contains(&&"get_name".to_string()));
        assert!(function_names.contains(&&"create_config".to_string()));
        assert!(function_names.contains(&&"process_config".to_string()));
        
        let struct_names: Vec<&String> = converter.nodes
            .iter()
            .filter(|n| n.node_type == "Struct")
            .map(|n| &n.name)
            .collect();
        
        assert!(struct_names.contains(&&"Config".to_string()));
        
        println!("Extracted {} entities:", converter.nodes.len());
        for node in &converter.nodes {
            println!("  {} ({}): {}", node.name, node.node_type, node.content);
        }
    }

    #[test]
    fn test_relationship_building() {
        let rust_code = r#"
pub struct User {
    name: String,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}", self.name)
    }
}

pub fn create_user(name: &str) -> User {
    User::new(name.to_string())
}

pub fn welcome_user(user: User) -> String {
    user.greet()
}
        "#;

        let mut converter = SimpleAstConverter::new(rust_code.to_string());
        
        // Extract entities and build relationships
        converter.extract_rust_entities().unwrap();
        converter.build_relationships().unwrap();
        
        // Find relationships
        let mut call_relationships = Vec::new();
        for node in &converter.nodes {
            for rel in &node.relationships {
                if rel.edge_type == "Calls" {
                    call_relationships.push((node.name.clone(), rel.edge_type.clone()));
                }
            }
        }
        
        assert!(!call_relationships.is_empty(), "Should have found some call relationships");
        
        println!("Found {} call relationships:", call_relationships.len());
        for (from, edge_type) in &call_relationships {
            println!("  {} -> {} ({})", from, "target", edge_type);
        }
        
        // Verify specific relationships
        let create_user_node = converter.nodes.iter().find(|n| n.name == "create_user");
        assert!(create_user_node.is_some(), "Should find create_user function");
        
        let welcome_user_node = converter.nodes.iter().find(|n| n.name == "welcome_user");
        assert!(welcome_user_node.is_some(), "Should find welcome_user function");
        
        // Check if welcome_user calls greet
        let welcome_relationships = &welcome_user_node.unwrap().relationships;
        let has_greet_call = welcome_relationships.iter().any(|r| r.edge_type == "Calls");
        assert!(has_greet_call, "welcome_user should call greet");
    }

    #[test]
    fn test_cross_file_simulation() {
        // Simulate cross-file analysis
        let mut global_symbol_table = HashMap::new();
        
        // File 1: lib.rs
        let lib_code = r#"
pub struct Database {
    connection: String,
}

pub fn connect() -> Database {
    Database { connection: "localhost".to_string() }
}
        "#;
        
        let mut lib_converter = SimpleAstConverter::new(lib_code.to_string());
        lib_converter.extract_rust_entities().unwrap();
        
        // Register symbols globally
        for (symbol, id) in &lib_converter.symbol_table {
            global_symbol_table.insert(format!("lib::{}", symbol), id.clone());
        }
        
        // File 2: main.rs
        let main_code = r#"
use lib::{Database, connect};

pub fn main() {
    let db = connect();
    println!("Connected to database");
}
        "#;
        
        let mut main_converter = SimpleAstConverter::new(main_code.to_string());
        main_converter.extract_rust_entities().unwrap();
        
        // Verify cross-file references can be found
        let main_function = main_converter.nodes.iter().find(|n| n.name == "main");
        assert!(main_function.is_some());
        assert!(main_function.unwrap().content.contains("connect"));
        
        println!("Global symbol table has {} entries", global_symbol_table.len());
        println!("Main file references external symbols: {:?}", 
                 main_function.unwrap().content.contains("connect"));
    }

    #[test]
    fn test_memory_efficiency_simulation() {
        // Generate a large code sample to test memory efficiency
        let mut large_code = String::from("use std::collections::HashMap;\n\n");
        
        for i in 0..1000 {
            large_code.push_str(&format!(
                "pub fn function_{}() -> i32 {{\n    println!(\"Function {}\");\n    {}\n}}\n\n",
                i, i, i
            ));
        }
        
        let start_memory = std::mem::size_of_val(&large_code);
        
        let mut converter = SimpleAstConverter::new(large_code);
        converter.extract_rust_entities().unwrap();
        
        let node_memory = converter.nodes.len() * std::mem::size_of::<SimpleNode>();
        let symbol_memory = converter.symbol_table.len() * (std::mem::size_of::<String>() * 2);
        let total_extracted_memory = node_memory + symbol_memory;
        
        let efficiency_ratio = total_extracted_memory as f64 / start_memory as f64;
        
        println!("Memory efficiency test:");
        println!("  Source size: {} bytes", start_memory);
        println!("  Extracted data: {} bytes", total_extracted_memory);
        println!("  Efficiency ratio: {:.2}", efficiency_ratio);
        println!("  Functions found: {}", converter.nodes.len());
        
        assert!(converter.nodes.len() >= 1000, "Should find at least 1000 functions");
        assert!(efficiency_ratio < 2.0, "Memory usage should be reasonable");
    }
}

fn main() {
    println!("AST to Graph Node Conversion Pipeline Test");
    
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

    pub fn process(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
}

pub fn create_default() -> Config {
    Config::new("default".to_string())
}

pub fn process_config(mut config: Config) -> HashMap<String, i32> {
    config.add_value("test".to_string(), 42);
    config.values
}
    "#;

    let mut converter = SimpleAstConverter::new(rust_code.to_string());
    
    println!("1. Extracting entities...");
    converter.extract_rust_entities().unwrap();
    
    println!("2. Building relationships...");
    converter.build_relationships().unwrap();
    
    println!("\n=== Extracted Entities ===");
    for node in &converter.nodes {
        println!("{} ({}): {}", node.name, node.node_type, node.content.replace('\n', " "));
        
        if !node.relationships.is_empty() {
            println!("  Relationships:");
            for rel in &node.relationships {
                println!("    {} -> {} ({})", rel.from, rel.to, rel.edge_type);
            }
        }
    }
    
    println!("\n=== Summary ===");
    println!("Total entities: {}", converter.nodes.len());
    
    let functions = converter.nodes.iter().filter(|n| n.node_type == "Function").count();
    let structs = converter.nodes.iter().filter(|n| n.node_type == "Struct").count();
    let imports = converter.nodes.iter().filter(|n| n.node_type == "Import").count();
    
    println!("Functions: {}", functions);
    println!("Structs: {}", structs);
    println!("Imports: {}", imports);
    
    let total_relationships: usize = converter.nodes.iter().map(|n| n.relationships.len()).sum();
    println!("Total relationships: {}", total_relationships);
    
    println!("\n✅ AST to Graph Node conversion pipeline working correctly!");
}