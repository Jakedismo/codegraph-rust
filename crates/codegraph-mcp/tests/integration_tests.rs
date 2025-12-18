// ABOUTME: Integration tests for starting a CodeGraph MCP stdio server process.
// ABOUTME: Exercises basic server lifecycle and tool discovery via rmcp.
/// Practical E2E Integration Tests for CodeGraph MCP Server
///
/// Tests that focus on validating essential functionality
/// Tests against the indexed Rust codebase in this repository
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;
use std::{path::PathBuf, sync::OnceLock};

use rmcp::{transport::TokioChildProcess, ServiceExt};
use tokio::process::Command;

static CODEGRAPH_BIN: OnceLock<PathBuf> = OnceLock::new();

fn workspace_root_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("Expected CARGO_MANIFEST_DIR to be under <workspace>/crates/<crate>")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root_dir().join("target"))
}

fn codegraph_bin_path() -> PathBuf {
    let exe = if cfg!(windows) {
        "codegraph.exe"
    } else {
        "codegraph"
    };
    target_dir().join("debug").join(exe)
}

fn ensure_codegraph_bin() -> PathBuf {
    CODEGRAPH_BIN
        .get_or_init(|| {
            let workspace_root = workspace_root_dir();
            let output = StdCommand::new("cargo")
                .current_dir(&workspace_root)
                .args([
                    "build",
                    "-q",
                    "-p",
                    "codegraph-mcp-server",
                    "--bin",
                    "codegraph",
                ])
                .stdin(Stdio::null())
                .output()
                .expect("Failed to invoke cargo build for codegraph-mcp-server");

            if !output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!(
                    "Failed to build codegraph-mcp-server binary.\nstdout:\n{}\nstderr:\n{}",
                    stdout, stderr
                );
            }

            let bin = codegraph_bin_path();
            assert!(
                bin.exists(),
                "Expected built binary at '{}', but it does not exist",
                bin.display()
            );
            bin
        })
        .clone()
}

#[tokio::test]
async fn test_mcp_server_startup() {
    println!("🚀 Testing MCP server startup with indexed Rust codebase...");

    // Test that the server starts without crashing
    let mut child = StdCommand::new(ensure_codegraph_bin())
        .args(["start", "stdio"])
        .env("RUST_LOG", "error")
        .env("CODEGRAPH_DEBUG", "0")
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
    println!("🔧 Testing MCP server handshake via stdio...");

    let mut cmd = Command::new(ensure_codegraph_bin());
    cmd.args(["start", "stdio"])
        .env("RUST_LOG", "error")
        .env("CODEGRAPH_DEBUG", "0");

    let (transport, _stderr) = TokioChildProcess::builder(cmd)
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start MCP server process");

    let service = ().serve(transport).await.expect("Failed to initialize MCP client");

    let tools = service
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Server should expose tools");

    service
        .cancel()
        .await
        .expect("Failed to cancel MCP client session");
}
