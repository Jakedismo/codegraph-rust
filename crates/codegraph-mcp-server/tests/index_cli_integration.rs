// ABOUTME: Verifies directory indexing finds nested Rust sources without requiring --recursive.
// ABOUTME: Exercises shipped CLI traversal flags in isolated embedded stores without live services.

use serde_json::Value;
use std::process::{Command, Stdio};

fn index_files(traversal: Option<&str>, root_source: bool) -> usize {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    let config = root.join("providers.toml");
    codegraph_core::config_manager::ConfigManager::create_default_config(&config).unwrap();
    for (name, contents) in [
        ("src/main.rs", "fn main() {}\n"),
        ("crates/example/src/lib.rs", "pub fn nested() {}\n"),
        ("target/generated.rs", "pub fn ignored_build_output() {}\n"),
        ("src/other.py", "def ignored_language(): pass\n"),
    ] {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    if root_source {
        std::fs::write(root.join("root.rs"), "pub fn at_root() {}\n").unwrap();
    }
    let stats = root.join("stats.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_codegraph"));
    command.args([
        "index",
        "--languages",
        "Rust",
        "--verbose",
        "--index-tier",
        "balanced",
        "--stats-json",
        "stats.json",
    ]);
    if let Some(traversal) = traversal {
        command.arg(traversal);
    }
    let output = command
        .arg(".")
        .current_dir(root)
        .env("CODEGRAPH_CONFIG_PATH", &config)
        .env(
            "CODEGRAPH_SURREALDB_URL",
            format!("surrealkv://{}", root.join(".codegraph/db").display()),
        )
        .env("CODEGRAPH_SURREALDB_USERNAME", "")
        .env("CODEGRAPH_SURREALDB_PASSWORD", "")
        .env("CODEGRAPH_EMBEDDING_POLICY", "off")
        .env("CODEGRAPH_SEMANTIC_RESOLUTION", "off")
        .env("CODEGRAPH_ANALYZERS", "0")
        .env("CODEGRAPH_VECTOR_INDEX_MODE", "off")
        .env("CODEGRAPH_PROJECT_ID", "cli-traversal")
        .env("CODEGRAPH_USE_GRAPH_SCHEMA", "false")
        .env("CODEGRAPH_SCHEMA", "v2")
        .env("CODEGRAPH_DEBUG", "0")
        .env("CODEGRAPH_NO_PROGRESS", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stats: Value = serde_json::from_slice(&std::fs::read(stats).unwrap()).unwrap();
    assert_eq!(stats["complete"], true);
    assert_eq!(stats["inference_texts"], 0);
    assert!(stats["nodes"].as_u64().unwrap() > 0);
    stats["files"].as_u64().unwrap().try_into().unwrap()
}

#[test]
fn balanced_index_finds_nested_rust_without_a_recursive_flag() {
    // Reproduce the reported command in a project containing no root-level Rust files.
    assert_eq!(index_files(None, false), 2);
    for flag in ["-r", "--recursive"] {
        assert_eq!(index_files(Some(flag), false), 2);
    }
}

#[test]
fn root_only_index_requires_explicit_no_recursive_flag() {
    assert_eq!(index_files(None, true), 3);
    assert_eq!(index_files(Some("--no-recursive"), true), 1);
}
