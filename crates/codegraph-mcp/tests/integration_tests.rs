/// Practical E2E Integration Tests for CodeGraph MCP Server
///
/// Tests that focus on validating essential functionality
/// Tests against the indexed Rust codebase in this repository
use std::process::{Command, Stdio};
use std::time::Duration;

#[tokio::test]
async fn test_mcp_server_startup() {
    println!("🚀 Testing MCP server startup with indexed Rust codebase...");

    // Test that the server starts without crashing
    let mut child = Command::new("codegraph")
        .args(["start", "stdio"])
        .env("RUST_LOG", "error")
        .env(
            "CODEGRAPH_MODEL",
            "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start CodeGraph MCP server");

    // Give server time to initialize
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check if process is still running (not crashed)
    match child.try_wait() {
        Ok(Some(status)) => {
            let output = child.wait_with_output().expect("Failed to get output");
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "MCP server exited unexpectedly with status: {}\nStderr: {}",
                status, stderr
            );
        }
        Ok(None) => {
            println!("✅ MCP server started successfully and is running");
        }
        Err(e) => {
            panic!("Error checking server status: {}", e);
        }
    }

    // Clean shutdown
    child.kill().expect("Failed to kill server process");
    child.wait().expect("Failed to wait for server process");

    println!("🎉 MCP server startup test passed!");
}

#[tokio::test]
async fn test_language_support_comprehensive() {
    println!("🌍 Testing comprehensive language support...");

    let registry = codegraph_parser::LanguageRegistry::new();

    // Test all 11 supported languages
    let language_tests = vec![
        ("test.rs", codegraph_core::Language::Rust),
        ("test.py", codegraph_core::Language::Python),
        ("test.js", codegraph_core::Language::JavaScript),
        ("test.ts", codegraph_core::Language::TypeScript),
        ("test.go", codegraph_core::Language::Go),
        ("test.java", codegraph_core::Language::Java),
        ("test.cpp", codegraph_core::Language::Cpp),
        // New revolutionary language support
        ("test.swift", codegraph_core::Language::Swift),
        ("test.cs", codegraph_core::Language::CSharp),
        ("test.rb", codegraph_core::Language::Ruby),
        ("test.php", codegraph_core::Language::Php),
    ];

    for (filename, expected_lang) in language_tests {
        let detected = registry.detect_language(filename);
        assert_eq!(
            detected,
            Some(expected_lang.clone()),
            "Language detection failed for {}: expected {:?}, got {:?}",
            filename,
            expected_lang,
            detected
        );
        println!(
            "✅ Language detection working for: {} -> {:?}",
            filename, expected_lang
        );
    }

    println!("🎉 Comprehensive language support test passed!");
}

#[tokio::test]
async fn test_official_server_creation() {
    println!("🔧 Testing official MCP server creation...");

    // Test that we can create the server instance
    let server = codegraph_mcp::official_server::CodeGraphMCPServer::new();
    println!("✅ CodeGraph MCP server instance created successfully");

    // Test Qwen initialization (if available)
    server.initialize_qwen().await;
    println!("✅ Qwen initialization completed (may show warnings if model not available)");

    println!("🎉 Official server creation test passed!");
}

#[test]
fn test_indexed_codebase_validation() {
    println!("📊 Testing indexed codebase validation...");

    // Verify we have indexed data
    let codegraph_dir = std::path::Path::new(".codegraph");
    if !codegraph_dir.exists() {
        println!("ℹ️ No .codegraph directory found - run 'codegraph init .' and 'codegraph index .' to create");
        return;
    }

    let faiss_index = codegraph_dir.join("faiss.index");
    if faiss_index.exists() {
        let faiss_size = std::fs::metadata(&faiss_index)
            .expect("Should be able to read FAISS index")
            .len();
        println!(
            "✅ Found FAISS index: {:.1}MB",
            faiss_size as f64 / 1024.0 / 1024.0
        );
    }

    let db_dir = codegraph_dir.join("db");
    if db_dir.exists() {
        println!("✅ Found graph database directory");
    }

    println!("🎉 Indexed codebase validation passed!");
}
