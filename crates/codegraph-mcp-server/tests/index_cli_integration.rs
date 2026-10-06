// ABOUTME: Verifies directory traversal, analyzer failures and embedding batch overrides.
// ABOUTME: Exercises the shipped CLI in isolated embedded stores with offline mock providers.

use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Stdio},
};

fn offline_index_command(root: &Path, config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codegraph"));
    command
        .current_dir(root)
        .env("CODEGRAPH_CONFIG_PATH", config)
        .env(
            "CODEGRAPH_SURREALDB_URL",
            format!("surrealkv://{}", root.join(".codegraph/db").display()),
        )
        .env("CODEGRAPH_SURREALDB_USERNAME", "")
        .env("CODEGRAPH_SURREALDB_PASSWORD", "")
        .env("CODEGRAPH_EMBEDDING_POLICY", "off")
        .env("CODEGRAPH_SEMANTIC_RESOLUTION", "off")
        .env("CODEGRAPH_VECTOR_INDEX_MODE", "off")
        .env("CODEGRAPH_PROJECT_ID", "cli-traversal")
        .env("CODEGRAPH_USE_GRAPH_SCHEMA", "false")
        .env("CODEGRAPH_SCHEMA", "v2")
        .env("CODEGRAPH_DEBUG", "0")
        .env("CODEGRAPH_NO_PROGRESS", "1")
        .stdin(Stdio::null());
    command
}

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
    let mut command = offline_index_command(root, &config);
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
        .env("CODEGRAPH_ANALYZERS", "0")
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

#[cfg(unix)]
#[test]
fn broken_rustup_shim_is_reported_before_parsing_even_for_another_project() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let invocation_root = directory.path();
    let root = invocation_root.join("project");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.join("project.marker"), "").unwrap();
    let config = root.join("providers.toml");
    codegraph_core::config_manager::ConfigManager::create_default_config(&config).unwrap();
    std::fs::create_dir(invocation_root.join("bin")).unwrap();
    let shim = invocation_root.join("bin/rust-analyzer");
    std::fs::write(&shim, "#!/bin/sh\n[ -f project.marker ] || { printf 'wrong project directory' >&2; exit 2; }\nprintf \"Unknown binary 'rust-analyzer' in official toolchain 'stable'\" >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = offline_index_command(&root, &config)
        .current_dir(invocation_root)
        .args([
            "index",
            "--languages",
            "Rust",
            "--verbose",
            "--index-tier",
            "balanced",
            "--stats-json",
            "stats.json",
            "project",
        ])
        .env("PATH", "bin")
        .env("CODEGRAPH_ANALYZERS", "1")
        .env("CODEGRAPH_ANALYZERS_REQUIRE_TOOLS", "1")
        .env_remove("CODEGRAPH_SCIP_INDEX")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let logs = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), stderr);
    assert!(stderr.contains("Unknown binary 'rust-analyzer'"), "{logs}");
    assert!(
        stderr.contains("rustup component add rust-analyzer"),
        "{logs}"
    );
    assert!(!logs.contains("wrong project directory"), "{logs}");
    assert!(!logs.contains("UNIFIED AST EXTRACTION COMPLETE"), "{logs}");
    assert!(
        !logs.contains("Language-server analysis starting"),
        "{logs}"
    );
    assert!(!invocation_root.join("stats.json").exists());
}

#[cfg(all(feature = "embeddings-ollama", feature = "server-http"))]
#[tokio::test]
async fn embedding_requests_honor_cli_env_and_config_batch_precedence() {
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use std::sync::{Arc, Mutex};

    let requests = Arc::new(Mutex::new(Vec::<usize>::new()));
    let handler_requests = requests.clone();
    let app =
        Router::new()
            .route(
                "/api/tags",
                get(|| async {
                    Json(serde_json::json!({"models": [{"name": "mock-batch-model"}]}))
                }),
            )
            .route(
                "/api/show",
                post(|| async { Json(serde_json::json!({"model_info": {"mock.context_length": 512}, "capabilities": ["embedding"]})) }),
            )
            .route(
                "/api/embed",
                post(move |Json(body): Json<Value>| {
                    let count = body["input"].as_array().unwrap().len();
                    handler_requests.lock().unwrap().push(count);
                    async move {
                        Json(serde_json::json!({"embeddings": vec![vec![1.0_f32; 384]; count]}))
                    }
                }),
            );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // Conflicting dotenv/config values must not override explicit CLI values,
    // including the old magic default (100) and batches above the old provider cap (256).
    for (cli_size, dotenv, expected) in [
        (Some(512), "CODEGRAPH_EMBEDDINGS_BATCH_SIZE=64\n", 512),
        (Some(100), "CODEGRAPH_EMBEDDINGS_BATCH_SIZE=64\n", 100),
        (None, "CODEGRAPH_EMBEDDINGS_BATCH_SIZE=64\n", 64),
        (None, "", 96),
        (None, "", 32),
    ] {
        requests.lock().unwrap().clear();
        let url = url.clone();
        let output = tokio::task::spawn_blocking(move || {
            let project = tempfile::tempdir().unwrap();
            let root = project.path();
            let config = root.join("providers.toml");
            std::fs::write(&config, format!(
                "[embedding]\nprovider = \"ollama\"\nmodel = \"mock-batch-model\"\ndimension = 384\nbatch_size = 32\nollama_url = {url:?}\n"
            )).unwrap();
            std::fs::write(root.join(".env"), dotenv).unwrap();
            std::fs::create_dir(root.join("src")).unwrap();
            let source: String = (0..520)
                .map(|index| format!("pub fn fixture_{index:04}() {{}}\n"))
                .collect();
            std::fs::write(root.join("src/lib.rs"), source).unwrap();
            let mut command = offline_index_command(root, &config);
            command
                .args(["index", "--languages", "Rust", "--index-tier", "fast"])
                .env("CODEGRAPH_ANALYZERS", "0")
                .env("CODEGRAPH_EMBEDDING_POLICY", "sync")
                .env("CODEGRAPH_EMBEDDING_PROVIDER", "ollama")
                .env("CODEGRAPH_EMBEDDING_MODEL", "mock-batch-model")
                .env("CODEGRAPH_EMBEDDING_DIMENSION", "384")
                .env("CODEGRAPH_OLLAMA_URL", url)
                .env_remove("CODEGRAPH_EMBEDDINGS_BATCH_SIZE")
                .env_remove("CODEGRAPH_EMBEDDING_BATCH_SIZE")
                .env("CODEGRAPH_EMBEDDING_BATCH_TOKENS", "1000000")
                .env("CODEGRAPH_EMBEDDING_BATCH_BYTES", "8388608")
                .env("CODEGRAPH_CHUNK_DB_BATCH_SIZE", "32");
            command.env("CODEGRAPH_TOKENIZER_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../codegraph-vector/tokenizers/qwen2.5-coder.json"));
            if expected != 32 {
                command.env("CODEGRAPH_EMBEDDING_BATCH_SIZE", "96");
            }
            if let Some(size) = cli_size {
                command.args(["--batch-size", &size.to_string()]);
            }
            command.arg(".").output().unwrap()
        }).await.unwrap();
        let logs = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{logs}");
        let sizes = requests.lock().unwrap().clone();
        assert_eq!(
            sizes.iter().max().copied(),
            Some(expected),
            "requests: {sizes:?}\n{logs}"
        );
        assert!(sizes.iter().all(|size| *size > 0 && *size <= expected));
        assert!(
            logs.contains(&format!("Embedding batch row limit: {expected}")),
            "{logs}"
        );
        assert!(logs.contains("Chunk DB write row limit: 32"), "{logs}");
        assert!(
            logs.contains(&format!("Embedding inference limits: {expected} rows")),
            "{logs}"
        );
        assert!(
            logs.contains(&format!(
                "Embedding batch row limit: {expected} | Languages:"
            )),
            "{logs}"
        );
    }
    server.abort();
}

#[cfg(all(feature = "embeddings-ollama", feature = "server-http"))]
#[tokio::test]
async fn model_contexts_prefixes_and_strict_truncation_reach_actual_requests() {
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use std::sync::{Arc, Mutex};
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = requests.clone();
    let app = Router::new()
        .route(
            "/api/tags",
            get(|| async {
                Json(serde_json::json!({"models": [
                    {"name": "qwen3-embedding:0.6b"}, {"name": "nomic-embed-text-v2-moe:latest"}
                ]}))
            }),
        )
        .route(
            "/api/show",
            post(|Json(body): Json<Value>| async move {
                let qwen = body["model"].as_str().unwrap().contains("qwen");
                Json(
                    serde_json::json!({"capabilities": ["embedding"], "model_info": {
                        "mock.context_length": if qwen {32768} else {512},
                        "tokenizer.ggml.add_bos_token": false, "tokenizer.ggml.add_eos_token": true
                    }}),
                )
            }),
        )
        .route(
            "/api/ps",
            get(|| async { Json(serde_json::json!({"models": []})) }),
        )
        .route(
            "/api/embed",
            post(move |Json(body): Json<Value>| {
                let count = body["input"].as_array().unwrap().len();
                let dimension = if body["model"].as_str().unwrap().contains("qwen") {
                    1024
                } else {
                    768
                };
                recorded.lock().unwrap().push(body);
                async move {
                    Json(serde_json::json!({"embeddings": vec![vec![1.0_f32; dimension]; count]}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for (model, dimension, serving) in [
        ("qwen3-embedding:0.6b", 1024, 32768),
        ("nomic-embed-text-v2-moe:latest", 768, 512),
        ("qwen3-embedding:0.6b", 1024, 256),
    ] {
        requests.lock().unwrap().clear();
        let url = url.clone();
        let output = tokio::task::spawn_blocking(move || {
            let project = tempfile::tempdir().unwrap();
            let root = project.path();
            let config = root.join("providers.toml");
            std::fs::write(&config, format!("[embedding]\nprovider = \"ollama\"\nmodel = {model:?}\ndimension = {dimension}\nollama_url = {url:?}\n")).unwrap();
            std::fs::write(root.join(".env"), "").unwrap();
            std::fs::create_dir(root.join("src")).unwrap();
            let mut source = String::from("pub fn substantial_unit() {\n");
            for index in 0..180 { source.push_str(&format!("    let value_{index} = \"café 🚀\";\n")); }
            source.push_str("}\n");
            std::fs::write(root.join("src/lib.rs"), source).unwrap();
            offline_index_command(root, &config)
                .args(["index", "--languages", "Rust", "--index-tier", "fast", "."])
                .env("CODEGRAPH_ANALYZERS", "0")
                .env("CODEGRAPH_EMBEDDING_POLICY", "sync")
                .env("CODEGRAPH_EMBEDDING_PROVIDER", "ollama")
                .env("CODEGRAPH_EMBEDDING_MODEL", model)
                .env("CODEGRAPH_EMBEDDING_DIMENSION", dimension.to_string())
                .env("CODEGRAPH_OLLAMA_URL", url)
                .env("CODEGRAPH_OLLAMA_NUM_CTX", serving.to_string())
                .env("CODEGRAPH_TOKENIZER_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../codegraph-vector/tokenizers/qwen2.5-coder.json"))
                .env("CODEGRAPH_CHUNK_OVERLAP_TOKENS", "0")
                .env("CODEGRAPH_CHUNK_SMART_SPLIT", "0")
                .env_remove("CODEGRAPH_CHUNK_MAX_TOKENS")
                .env_remove("CODEGRAPH_MAX_CHUNK_TOKENS")
                .output().unwrap()
        }).await.unwrap();
        let logs = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{logs}");
        let bodies = requests.lock().unwrap().clone();
        assert!(!bodies.is_empty());
        let tokenizer = tokenizers::Tokenizer::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../codegraph-vector/tokenizers/qwen2.5-coder.json"
        ))
        .unwrap();
        let inputs: Vec<&str> = bodies
            .iter()
            .flat_map(|body| {
                body["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|text| text.as_str().unwrap())
            })
            .collect();
        for body in &bodies {
            assert_eq!(body["truncate"], false);
            assert_eq!(body["options"]["num_ctx"], serving);
        }
        for input in &inputs {
            assert!(tokenizer.encode(*input, true).unwrap().len() + 1 <= serving);
            if model.contains("nomic") {
                assert!(input.starts_with("search_document: "));
            }
        }
        if serving == 32768 {
            assert!(
                inputs
                    .iter()
                    .any(|input| tokenizer.encode(*input, false).unwrap().len() > 512)
            );
        }
        assert!(
            logs.contains(&format!("serving_context={serving}")),
            "{logs}"
        );
    }
    server.abort();
}
