// ABOUTME: Offline LATS regressions for graph grounding, branch history and answer selection

use super::*;
use rig::message::{CallId, ToolName};
use serde_json::json;

#[test]
fn test_parse_score() {
    assert_eq!(parse_score("85"), Some(0.85));
    assert_eq!(parse_score(" 0\n"), Some(0.0));
    assert_eq!(parse_score("Score: 85/100"), Some(0.85));
    assert_eq!(parse_score("no score"), None);
    assert_eq!(parse_score(""), None);
    assert_eq!(parse_score("250"), None);
}

#[test]
fn grounding_requires_successful_tool_observations_not_prose_or_budget_notes() {
    let observation = |value: Value| {
        Message::tool_result(
            CallId::from_wire("test-call"),
            ToolName::new("get_hub_nodes").unwrap(),
            value.to_string(),
        )
    };
    assert!(!has_graph_observation(&[Message::assistant(
        "Final answer: get_hub_nodes returned src/fiction.rs:42"
    )]));
    assert!(!has_graph_observation(&[observation(json!({
        "tool": "get_hub_nodes", "error": "database unavailable"
    }))]));
    assert!(!has_graph_observation(&[observation(json!({
        "tool": "get_hub_nodes", "result": [], "_budget": {"exhausted": true}
    }))]));
    assert!(has_graph_observation(&[observation(json!({
        "tool": "get_hub_nodes", "result": []
    }))]));
}

#[cfg(feature = "ollama")]
mod mock_model {
    use super::*;
    use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
    use codegraph_core::CodeGraphConfig;
    use codegraph_graph::{GraphFunctions, SurrealDbConfig, SurrealDbStorage};
    use codegraph_mcp_tools::GraphToolExecutor;
    use codegraph_vector::EmbeddingGenerator;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Copy)]
    enum Mode {
        Final,
        Deeper,
        Synthesize,
        NoTools,
        ModelError,
        ToolError,
    }

    struct Fixture {
        agent: LatsAgent,
        requests: Arc<Mutex<Vec<Value>>>,
        server: tokio::task::JoinHandle<()>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    fn is_critic(request: &Value) -> bool {
        request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| {
                message["role"] == "system"
                    && message["content"]
                        .as_str()
                        .unwrap_or_default()
                        .contains(EVALUATION_SYSTEM_PROMPT)
            })
    }

    fn observations(request: &Value) -> Vec<Value> {
        request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["role"] == "tool")
            .filter_map(|message| serde_json::from_str(message["content"].as_str()?).ok())
            .collect()
    }

    async fn fixture(mode: Mode, max_turns: usize) -> Fixture {
        let storage = SurrealDbStorage::new(SurrealDbConfig {
            connection: "mem://".to_string(),
            username: None,
            password: None,
            ..Default::default()
        })
        .await
        .unwrap();
        let body = if matches!(mode, Mode::ToolError) {
            "THROW 'offline fixture database failure';"
        } else {
            "LET $name = IF $degree = 2 { 'deep_hub' } ELSE { 'root_hub' }; \
             RETURN [{ node_id: 'nodes:fixture', node: {id: 'nodes:fixture', \
             name: $name, kind: 'function', location: {file_path: 'src/grounded.rs', \
             start_line: 17, end_line: 20}}, afferent_degree: 3, efferent_degree: 2, \
             total_degree: 5, incoming_by_type: [], outgoing_by_type: [] }];"
        };
        storage
            .db()
            .query(format!(
                "DEFINE FUNCTION fn::get_hub_nodes($project: string, $degree: int) {{ {body} }};"
            ))
            .await
            .unwrap()
            .check()
            .unwrap();
        let functions = Arc::new(GraphFunctions::new_with_project_id(
            storage.db(),
            "lats-test",
        ));
        let factory = GraphToolFactory::new(Arc::new(GraphToolExecutor::new(
            functions,
            Arc::new(CodeGraphConfig::default()),
            Arc::new(EmbeddingGenerator::default()),
        )));
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let recorded = requests.clone();
        let app = Router::new().route("/api/chat", post(move |Json(request): Json<Value>| {
            recorded.lock().unwrap().push(request.clone());
            async move {
                if matches!(mode, Mode::ModelError) {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": "offline model failure"}))).into_response();
                }
                let observations = observations(&request);
                let messages = request["messages"].as_array().unwrap();
                let deeper_branch = messages.iter().filter(|message| message["role"] == "user").count() > 1;
                let synthesizing = messages.iter().any(|message| {
                    message["role"] == "user" && message["content"].as_str()
                        .unwrap_or_default().starts_with("Answer the original question now")
                });
                let content = if is_critic(&request) {
                    "95"
                } else if matches!(mode, Mode::NoTools) {
                    "Final answer: Invented answer without graph observations"
                } else if synthesizing {
                    "Synthesized answer: root_hub at src/grounded.rs:17; further evidence is missing."
                } else if observations.is_empty() || (matches!(mode, Mode::Deeper) && deeper_branch && observations.len() < 2) {
                    let degree = if observations.is_empty() { 1 } else { 2 };
                    return Json(json!({
                        "model": "mock-model", "created_at": "2026-01-01T00:00:00Z", "done": true,
                        "message": {"role": "assistant", "content": "", "tool_calls": [{
                            "function": {"name": "get_hub_nodes", "arguments": {"min_degree": degree}}
                        }]}
                    })).into_response();
                } else if matches!(mode, Mode::Synthesize) || (matches!(mode, Mode::Deeper) && observations.len() == 1) {
                    "First graph step established root_hub at src/grounded.rs:17. Investigate the next step."
                } else if matches!(mode, Mode::Deeper) {
                    "Final answer: deep_hub is the selected answer at src/grounded.rs:17."
                } else {
                    "Final answer: root_hub is grounded at src/grounded.rs:17."
                };
                Json(json!({
                    "model": "mock-model", "created_at": "2026-01-01T00:00:00Z", "done": true,
                    "message": {"role": "assistant", "content": content}
                })).into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = rig::providers::ollama::OllamaConfig::new()
            .with_base_url(format!("http://{address}"))
            .client();
        Fixture {
            agent: LatsAgent::new(
                client.completion("mock-model").into(),
                factory,
                max_turns,
                ContextTier::from_context_window(128_000),
                1234,
                "Answer the question using project-scoped graph evidence.".to_string(),
            ),
            requests,
            server,
        }
    }

    async fn execute(fixture: &Fixture) -> Result<String> {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            fixture.agent.execute("Find hub nodes"),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn candidates_use_all_graph_tools_and_critics_receive_actual_observations() {
        let fixture = fixture(Mode::Final, 2).await;
        assert_eq!(
            execute(&fixture).await.unwrap(),
            "root_hub is grounded at src/grounded.rs:17."
        );
        assert_eq!(fixture.agent.take_tool_call_count(), 3);
        let traces = fixture.agent.take_tool_traces();
        assert_eq!(traces.len(), 3);
        for trace in traces {
            assert_eq!(trace.tool_name, "get_hub_nodes");
            assert!(trace.error.is_none());
            assert_eq!(
                trace.result.unwrap()["result"][0]["node"]["location"]["file_path"],
                "src/grounded.rs"
            );
        }
        let requests = fixture.requests.lock().unwrap();
        assert_eq!(requests.len(), 9);
        for request in requests.iter().filter(|request| !is_critic(request)) {
            let mut tools: Vec<_> = request["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
                .collect();
            tools.sort();
            let mut expected = GraphToolExecutor::get_tool_names();
            expected.sort();
            assert_eq!(tools, expected);
            assert_eq!(request["options"]["num_predict"], 1234);
        }
        for request in requests.iter().filter(|request| is_critic(request)) {
            let text = request["messages"].to_string();
            assert!(
                text.contains("root_hub") && text.contains("src/grounded.rs"),
                "{text}"
            );
            assert!(text.contains("toolresult"), "{text}");
        }
    }

    #[tokio::test]
    async fn deeper_answer_preserves_ancestor_observations_without_sibling_histories() {
        let fixture = fixture(Mode::Deeper, 2).await;
        assert_eq!(
            execute(&fixture).await.unwrap(),
            "deep_hub is the selected answer at src/grounded.rs:17."
        );
        assert_eq!(fixture.agent.take_tool_call_count(), 6);
        let requests = fixture.requests.lock().unwrap();
        let deeper: Vec<_> = requests
            .iter()
            .filter(|request| !is_critic(request) && observations(request).len() == 2)
            .collect();
        assert_eq!(deeper.len(), 3);
        for request in deeper {
            let observed = observations(request);
            assert_eq!(observed[0]["result"][0]["node"]["name"], "root_hub");
            assert_eq!(observed[1]["result"][0]["node"]["name"], "deep_hub");
            let prompts: Vec<_> = request["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|message| message["role"] == "user")
                .collect();
            assert_eq!(prompts.len(), 2);
            assert!(
                prompts[0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("candidate next step #1")
            );
        }
    }

    #[tokio::test]
    async fn unfinished_search_synthesizes_an_answer_from_the_selected_branch() {
        let fixture = fixture(Mode::Synthesize, 2).await;
        let answer = execute(&fixture).await.unwrap();
        assert!(
            answer.starts_with("Synthesized answer: root_hub"),
            "{answer}"
        );
        let requests = fixture.requests.lock().unwrap();
        let request = requests.last().unwrap();
        assert_eq!(observations(request).len(), 1);
        assert_eq!(request["tools"].as_array().unwrap().len(), 8);
    }

    #[tokio::test]
    async fn final_sounding_prose_without_graph_evidence_is_rejected() {
        let fixture = fixture(Mode::NoTools, 1).await;
        let error = execute(&fixture).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("successful graph-tool observation"),
            "{error}"
        );
        assert_eq!(fixture.agent.take_tool_call_count(), 0);
        assert!(fixture.agent.take_tool_traces().is_empty());
        assert_eq!(fixture.requests.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn candidate_model_failures_are_propagated() {
        let fixture = fixture(Mode::ModelError, 2).await;
        let error = execute(&fixture).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("All LATS candidate tool loops failed"),
            "{error}"
        );
        assert!(
            format!("{error:#}").contains("offline model failure"),
            "{error:#}"
        );
    }

    #[tokio::test]
    async fn zero_turn_budget_fails_before_any_model_or_tool_call() {
        let fixture = fixture(Mode::NoTools, 0).await;
        assert!(
            execute(&fixture)
                .await
                .unwrap_err()
                .to_string()
                .contains("positive search and tool-round budget")
        );
        assert!(fixture.requests.lock().unwrap().is_empty());
        assert_eq!(fixture.agent.take_tool_call_count(), 0);
    }

    #[tokio::test]
    async fn failed_tool_calls_do_not_ground_an_answer() {
        let fixture = fixture(Mode::ToolError, 2).await;
        assert!(execute(&fixture).await.is_err());
        let traces = fixture.agent.take_tool_traces();
        assert!(!traces.is_empty());
        assert!(
            traces
                .iter()
                .all(|trace| trace.result.is_none() && trace.error.is_some())
        );
    }
}
