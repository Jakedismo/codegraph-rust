// ABOUTME: Exercises the shipped CLI's public commands and project hook setup.
// ABOUTME: Checks output/exit contracts in isolated projects without real providers.

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: Option<&str>, project: &std::path::Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codegraph"))
        .args(args)
        .current_dir(project)
        .env("CODEGRAPH_CONFIG_PATH", project.join("missing-config.toml"))
        .env("CODEGRAPH_DEBUG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

#[test]
fn guidance_and_help_do_not_load_provider_configuration() {
    let project = tempfile::tempdir().unwrap();
    let output = run(&["agent", "instructions"], None, project.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let instructions = String::from_utf8(output.stdout).unwrap();
    for tool in ["context", "impact", "architecture", "quality"] {
        assert!(instructions.contains(&format!("codegraph agent {tool}")));
    }
    let output = run(&["agent", "--help"], None, project.path());
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(!help.contains("semantic_code_search"));
    assert!(!help.contains("graph-neighbors"));
    for tool in ["context", "impact", "architecture", "quality"] {
        let output = run(&["agent", tool, "--help"], None, project.path());
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("--timeout-secs"), "{help}");
        assert!(help.contains("[default: 600]"), "{help}");
    }
    for args in [
        &["agent", "context"][..],
        &["agent", "impact", "query", "--focus", "search"][..],
    ] {
        let output = run(args, None, project.path());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn hook_binary_emits_only_context_json_and_handles_invalid_events() {
    let project = tempfile::tempdir().unwrap();
    let input =
        json!({"hook_event_name": "SessionStart", "cwd": project.path(), "source": "compact"})
            .to_string();
    let output = run(&["hooks", "emit"], Some(&input), project.path());
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    if cfg!(feature = "ai-enhanced") {
        assert_eq!(
            response["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
        assert!(
            response["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains("codegraph agent context")
        );
    } else {
        assert_eq!(response, json!({}));
    }
    let output = run(&["hooks", "emit"], Some("invalid JSON"), project.path());
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({})
    );
}

#[test]
fn hook_install_is_opt_in_and_preserves_project_settings() {
    let project = tempfile::tempdir().unwrap();
    let output = run(&["hooks", "install", "--dry-run"], None, project.path());
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!project.path().join(".claude").exists());
    assert!(!project.path().join(".codex").exists());
    std::fs::create_dir(project.path().join(".claude")).unwrap();
    std::fs::write(
        project.path().join(".claude/settings.json"),
        r#"{"permissions":{"deny":["Bash(rm:*)"]}}"#,
    )
    .unwrap();
    let output = run(
        &["hooks", "install", "--harness", "both"],
        None,
        project.path(),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let settings: Value = serde_json::from_slice(
        &std::fs::read(project.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["permissions"]["deny"], json!(["Bash(rm:*)"]));
    let before = std::fs::read(project.path().join(".codex/hooks.json")).unwrap();
    assert!(
        run(&["hooks", "install"], None, project.path())
            .status
            .success()
    );
    assert_eq!(
        std::fs::read(project.path().join(".codex/hooks.json")).unwrap(),
        before
    );
}

#[test]
fn runtime_failure_has_json_stdout_and_nonzero_exit_status() {
    let project = tempfile::tempdir().unwrap();
    for tool in ["context", "impact", "architecture", "quality"] {
        let output = run(&["agent", tool, "question"], None, project.path());
        assert_eq!(output.status.code(), Some(1));
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["error"]["tool"], format!("agentic_{tool}"));
        let error = response["error"]["message"].as_str().unwrap();
        if cfg!(feature = "ai-enhanced") {
            assert!(error.contains("Failed to load config"), "{error}");
        } else {
            assert!(error.contains("--features ai-enhanced"), "{error}");
        }
        assert!(!output.stderr.is_empty());
    }
    let output = run(
        &["agent", "context", "question", "--format", "text"],
        None,
        project.path(),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[cfg(feature = "ai-enhanced")]
#[test]
fn explicit_config_file_and_query_file_are_used() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("question.txt"), "Question from a file").unwrap();
    std::fs::write(
        project.path().join("chosen.toml"),
        "[embedding]\nprovider = [\n",
    )
    .unwrap();
    let output = run(
        &[
            "--config",
            "chosen.toml",
            "agent",
            "context",
            "--query-file",
            "question.txt",
        ],
        None,
        project.path(),
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1));
    // An explicit config wins over the deliberately missing environment path.
    assert!(
        !response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing-config")
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Failed to parse config")
    );
    let output = run(
        &["agent", "context", "--query-file", "-"],
        Some("  \n"),
        project.path(),
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("blank")
    );
}

#[cfg(feature = "ai-enhanced")]
#[test]
fn project_configuration_is_loaded_after_switching_directories() {
    let invocation = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        invocation.path().join("question.txt"),
        "Question from invocation directory",
    )
    .unwrap();
    std::fs::write(
        project.path().join(".codegraph.toml"),
        "[embedding]\nprovider = [PROJECT_CONFIG_MARKER\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codegraph"))
        .args([
            "agent",
            "context",
            "--query-file",
            "question.txt",
            "--project",
        ])
        .arg(project.path())
        .current_dir(invocation.path())
        .env_remove("CODEGRAPH_CONFIG_PATH")
        .env("CODEGRAPH_DEBUG", "0")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("PROJECT_CONFIG_MARKER")
    );
}

#[cfg(feature = "ai-enhanced")]
#[tokio::test]
async fn deadline_cancels_stalled_database_setup_without_a_model_call() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("empty.toml");
    std::fs::write(&config, "").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stalled_database = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
        drop(socket);
    });
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_codegraph"))
            .args(["agent", "context", "question", "--timeout-secs", "1"])
            .current_dir(project.path())
            .env("CODEGRAPH_CONFIG_PATH", config)
            .env("CODEGRAPH_DEBUG", "0")
            .env("CODEGRAPH_LLM_PROVIDER", "ollama")
            .env("CODEGRAPH_MODEL", "test-model-not-invoked")
            .env("CODEGRAPH_EMBEDDING_PROVIDER", "ollama")
            .env("CODEGRAPH_SURREALDB_URL", format!("ws://{address}"))
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    stalled_database.abort();
    assert_eq!(output.status.code(), Some(1));
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timed out after 1 seconds"),
        "{response}"
    );
}

#[cfg(all(feature = "ai-enhanced", feature = "server-http"))]
#[tokio::test]
async fn successful_command_returns_shared_workflow_json_with_a_local_mock_model() {
    successful_mock_workflow("rig").await;
}

#[cfg(all(feature = "ai-enhanced", feature = "server-http"))]
#[tokio::test]
async fn lats_command_executes_graph_tools_and_reports_their_count() {
    successful_mock_workflow("lats").await;
}

#[cfg(all(feature = "ai-enhanced", feature = "server-http"))]
async fn successful_mock_workflow(architecture: &'static str) {
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = calls.clone();
    let app = Router::new()
        .route("/api/tags", get(|| async {
            Json(json!({"models": [{"name": "test-embedding-model"}]}))
        }))
        .route("/api/show", post(|| async {
            Json(json!({"model_info": {"mock.context_length": 512}, "capabilities": ["embedding"]}))
        }))
        .route("/api/chat", post(move |Json(request): Json<Value>| {
            calls_for_handler.fetch_add(1, Ordering::SeqCst);
            async move {
                let messages = request["messages"].as_array().unwrap();
                let critic = messages.iter().any(|message| message["role"] == "system"
                    && message["content"].as_str().unwrap_or_default().starts_with("You score one proposed step"));
                let observed = messages.iter().any(|message| message["role"] == "tool");
                let message = if architecture == "lats" && critic {
                    json!({"role": "assistant", "content": "95"})
                } else if architecture == "lats" && !observed {
                    json!({"role": "assistant", "content": "", "tool_calls": [{
                        "function": {"name": "get_hub_nodes", "arguments": {"min_degree": 1}}
                    }]})
                } else if architecture == "lats" {
                    json!({"role": "assistant", "content": "Final answer: No hub nodes found in this lookup."})
                } else {
                    json!({"role": "assistant", "content": "Mock agent answer with source location src/lib.rs:12."})
                };
                Json(json!({
                    "model": "test-model", "created_at": "2026-01-01T00:00:00Z",
                    "message": message,
                    "done": true
                }))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let model_server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let output = tokio::task::spawn_blocking(move || {
        let project = tempfile::tempdir().unwrap();
        let config = project.path().join("empty.toml");
        std::fs::write(&config, "").unwrap();
        // Prevent fallback to any user dotenv configuration.
        std::fs::write(project.path().join(".env"), "").unwrap();
        // Fresh disk stores receive the bundled graph functions; mem:// intentionally
        // does not. Keep the LATS fixture entirely inside this temporary project.
        let database = if architecture == "lats" {
            format!(
                "surrealkv://{}",
                project.path().join("fixture-db").display()
            )
        } else {
            "mem://".to_string()
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_codegraph"));
        command
            .args([
                "agent",
                "context",
                "Find request handling",
                "--focus",
                "search",
                "--timeout-secs",
                "10",
            ])
            .current_dir(project.path())
            .env("CODEGRAPH_CONFIG_PATH", config)
            .env("CODEGRAPH_DEBUG", "0")
            .env("CODEGRAPH_AGENT_ARCHITECTURE", architecture)
            .env("CODEGRAPH_LLM_PROVIDER", "ollama")
            .env("CODEGRAPH_MODEL", "test-model")
            .env("CODEGRAPH_LLM_MODEL", "test-model")
            .env("CODEGRAPH_EMBEDDING_PROVIDER", "ollama")
            .env("CODEGRAPH_EMBEDDING_MODEL", "test-embedding-model")
            .env(
                "CODEGRAPH_TOKENIZER_PATH",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../codegraph-vector/tokenizers/qwen2.5-coder.json"
                ),
            )
            .env("CODEGRAPH_OLLAMA_URL", format!("http://{address}"))
            .env("OLLAMA_API_BASE_URL", format!("http://{address}"))
            .env("CODEGRAPH_SURREALDB_URL", database)
            .env("CODEGRAPH_USE_GRAPH_SCHEMA", "false")
            .env("CODEGRAPH_PROJECT_ID", "mock-cli-project");
        for key in [
            "CODEGRAPH_SURREALDB_USERNAME",
            "CODEGRAPH_SURREALDB_PASSWORD",
            "SURREALDB_USERNAME",
            "SURREALDB_PASSWORD",
        ] {
            command.env_remove(key);
        }
        command.output().unwrap()
    })
    .await
    .unwrap();
    model_server.abort();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["query"], "Find request handling");
    assert_eq!(response["analysis_type"], "code_search");
    assert_eq!(response["framework"], "Rig");
    if architecture == "lats" {
        assert_eq!(response["answer"], "No hub nodes found in this lookup.");
        assert_eq!(response["tool_use_count"], 3);
        assert_eq!(calls.load(Ordering::SeqCst), 9);
    } else {
        assert_eq!(
            response["answer"],
            "Mock agent answer with source location src/lib.rs:12."
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    assert!(response.get("tool_use_count").is_some());
    assert!(response.get("error").is_none());
}
