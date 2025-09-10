use std::{sync::Arc, path::PathBuf};
use axum::Router;
use axum_test::TestServer;
use codegraph_api::{create_router, AppState};
use codegraph_core::ConfigManager;
use tempfile::TempDir;
use anyhow::Result;
use serial_test::serial;

pub struct TestContext {
    pub server: TestServer,
    pub tmpdir: TempDir,
}

impl TestContext {
    pub fn base_url(&self) -> &str { self.server.base_url() }
}

/// Create an axum TestServer with isolated working directory and clean RocksDB path.
#[allow(dead_code)]
pub async fn setup_test_server() -> Result<TestContext> {
    // Isolate filesystem side-effects per test
    let tmpdir = tempfile::tempdir()?;
    // Move CWD so that graph storage at ./data/graph.db stays under tmpdir
    let original_cwd = std::env::current_dir()?;
    std::env::set_current_dir(tmpdir.path())?;

    // Ensure data dir is clean
    let data_dir = PathBuf::from("data");
    if data_dir.exists() {
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    // Minimal config
    let config = ConfigManager::new_watching(Some("test".to_string()))
        .expect("Failed to init ConfigManager");
    let state = AppState::new(config).await.expect("Failed to create AppState");
    let app: Router = create_router(state);
    let server = TestServer::new(app).expect("Failed to create TestServer");

    // Restore CWD after server init so relative paths in tests are stable
    std::env::set_current_dir(original_cwd)?;

    Ok(TestContext { server, tmpdir })
}

/// Helper to write a small source tree to a temp directory and return its path.
#[allow(dead_code)]
pub fn write_sample_repo(tmpdir: &TempDir) -> std::path::PathBuf {
    let root = tmpdir.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();

    // Rust file
    std::fs::write(
        root.join("lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 { a + b }
pub struct Foo { pub x: i32 }
"#,
    )
    .unwrap();

    // Python file
    let py_dir = root.join("py");
    std::fs::create_dir_all(&py_dir).unwrap();
    std::fs::write(
        py_dir.join("util.py"),
        r#"def greet(name):
    return f"Hello, {name}!"
"#,
    )
    .unwrap();

    // JavaScript file
    let js_dir = root.join("js");
    std::fs::create_dir_all(&js_dir).unwrap();
    std::fs::write(
        js_dir.join("index.js"),
        r#"export function mul(a, b) { return a * b }
export const NAME = 'cg';
"#,
    )
    .unwrap();

    root
}

