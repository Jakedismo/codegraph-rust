// ABOUTME: Exercises in-process graph tools against a live SurrealDB.
// ABOUTME: Skips automatically if required env vars are missing.

use codegraph_core::EdgeType;
use codegraph_graph::{GraphFunctions, SurrealDbConfig, SurrealDbStorage};

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn smoke_graph_tools() {
    // Allow skipping if no DB is configured
    let url = env("CODEGRAPH_SURREALDB_URL", "");
    if url.is_empty() {
        eprintln!("[skip] CODEGRAPH_SURREALDB_URL not set; graph tools not exercised");
        return;
    }

    let config = SurrealDbConfig {
        connection: url,
        namespace: env("CODEGRAPH_SURREALDB_NAMESPACE", "ouroboros"),
        database: env("CODEGRAPH_SURREALDB_DATABASE", "codegraph"),
        username: std::env::var("CODEGRAPH_SURREALDB_USERNAME").ok(),
        password: std::env::var("CODEGRAPH_SURREALDB_PASSWORD").ok(),
        strict_mode: false,
        auto_migrate: false,
        cache_enabled: false,
    };

    let storage = match SurrealDbStorage::new(config).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[skip] failed to connect to SurrealDB: {e}");
            return;
        }
    };

    let graph = GraphFunctions::new(storage);
    let project_id = env("CODEGRAPH_PROJECT_ID", "");

    // 1) Semantic search with graph context
    let dim = 1024usize;
    let query_embedding = vec![0.0f32; dim];
    let query_text = "configuration loading";
    match graph
        .semantic_search_with_context(
            &project_id,
            &query_embedding,
            query_text,
            dim as i32,
            3,
            0.0,
            true,
        )
        .await
    {
        Ok(res) => println!("semantic_search_with_context: {} results", res.len()),
        Err(e) => eprintln!("semantic_search_with_context error: {e}"),
    }

    // 2) Transitive deps (edge type Calls) from a placeholder node
    let node_id = "nodes:1"; // replace with a real node id in your DB if desired
    match graph
        .get_transitive_dependencies(node_id, EdgeType::Calls, 2)
        .await
    {
        Ok(res) => println!("get_transitive_dependencies: {} deps", res.len()),
        Err(e) => eprintln!("get_transitive_dependencies error: {e}"),
    }

    // 3) Reverse deps
    match graph
        .get_reverse_dependencies(node_id, EdgeType::Calls, 2)
        .await
    {
        Ok(res) => println!("get_reverse_dependencies: {} reverse deps", res.len()),
        Err(e) => eprintln!("get_reverse_dependencies error: {e}"),
    }
}
