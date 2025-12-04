// ABOUTME: Exercises in-process graph tools against a live SurrealDB.
// ABOUTME: Skips automatically if required env vars are missing.

use codegraph_graph::{GraphFunctions, SurrealDbConfig, SurrealDbStorage};
use codegraph_vector::ollama_embedding_provider::{OllamaEmbeddingConfig, OllamaEmbeddingProvider};
use codegraph_vector::providers::EmbeddingProvider;
use serde_json::Value as JsonValue;

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

async fn setup_graph_functions() -> Option<GraphFunctions> {
    let url = env("CODEGRAPH_SURREALDB_URL", "");
    if url.is_empty() {
        eprintln!("[skip] CODEGRAPH_SURREALDB_URL not set");
        return None;
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
            return None;
        }
    };

    Some(GraphFunctions::new(storage.db()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn smoke_graph_tools() {
    let graph = match setup_graph_functions().await {
        Some(g) => g,
        None => return,
    };

    // 1) Semantic search with graph context
    let dim = std::env::var("CODEGRAPH_EMBEDDING_DIMENSION")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2048);
    let query_embedding = vec![0.0f32; dim];
    let query_text = "configuration loading";
    match graph
        .semantic_search_with_context(query_text, &query_embedding, dim, 3, 0.0, true)
        .await
    {
        Ok(res) => println!("semantic_search_with_context: {} results", res.len()),
        Err(e) => eprintln!("semantic_search_with_context error: {e}"),
    }

    // 2) Transitive deps (edge type Calls) from a placeholder node
    let node_id = "nodes:1"; // replace with a real node id in your DB if desired
    match graph.get_transitive_dependencies(node_id, "Calls", 2).await {
        Ok(res) => println!("get_transitive_dependencies: {} deps", res.len()),
        Err(e) => eprintln!("get_transitive_dependencies error: {e}"),
    }

    // 3) Reverse deps
    match graph.get_reverse_dependencies(node_id, "Calls", 2).await {
        Ok(res) => println!("get_reverse_dependencies: {} reverse deps", res.len()),
        Err(e) => eprintln!("get_reverse_dependencies error: {e}"),
    }
}

/// Test that semantic_code_search returns node_id field in results.
/// This is critical for agentic tools to use other graph functions.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_semantic_search_returns_node_ids() {
    let graph = match setup_graph_functions().await {
        Some(g) => g,
        None => return,
    };

    println!("Using project_id: {}", graph.project_id());

    // Create Ollama embedding provider with model from env var
    let mut ollama_config = OllamaEmbeddingConfig::default();
    if let Ok(model) = std::env::var("CODEGRAPH_EMBEDDING_MODEL") {
        println!("Using embedding model from env: {}", model);
        ollama_config.model_name = model;
    } else {
        println!(
            "Using default embedding model: {}",
            ollama_config.model_name
        );
    }
    let ollama_provider = OllamaEmbeddingProvider::new(ollama_config);

    // Verify Ollama is available
    match ollama_provider.check_availability().await {
        Ok(true) => println!("✅ Ollama embedding provider available"),
        Ok(false) => {
            eprintln!("[skip] Ollama not available");
            return;
        }
        Err(e) => {
            eprintln!("[skip] Failed to check Ollama availability: {e}");
            return;
        }
    }

    let dim = ollama_provider.embedding_dimension();
    println!("Using embedding dimension: {}", dim);

    // Generate REAL embedding from query text
    let query_text = "Where is pub fn read(&self) defined?";
    let query_embedding: Vec<f32> =
        match ollama_provider.generate_single_embedding(query_text).await {
            Ok(emb) => {
                println!("Generated embedding with {} dimensions", emb.len());
                println!("First 5 values: {:?}", &emb[..5.min(emb.len())]);
                emb
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to generate embedding: {e}");
                return;
            }
        };

    let results: Vec<JsonValue> = match graph
        .semantic_search_with_context(
            query_text,
            &query_embedding,
            dim,
            50,   // limit
            0.2,  // threshold
            true, // include_graph_context
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            eprintln!("semantic_search_with_context error: {e}");
            return;
        }
    };

    println!("\n{}", "=".repeat(80));
    println!("SEMANTIC SEARCH RESULTS - FULL DATA");
    println!("{}", "=".repeat(80));
    println!("Query: {}", query_text);
    println!("Total results: {}\n", results.len());

    if results.is_empty() {
        eprintln!("[warn] No results returned - check embedding model matches indexed data");
        return;
    }

    // Print full JSON for each result
    for (i, result) in results.iter().enumerate() {
        println!("--- Result {} ---", i + 1);
        println!(
            "{}",
            serde_json::to_string_pretty(result).unwrap_or_else(|_| format!("{:?}", result))
        );
        println!();
    }

    // Verification summary
    println!("{}", "=".repeat(80));
    println!("VERIFICATION SUMMARY");
    println!("{}", "=".repeat(80));

    let mut all_have_node_id = true;
    for (i, result) in results.iter().enumerate() {
        let has_node_id = result.get("node_id").is_some();
        let has_id = result.get("id").is_some();

        if has_node_id {
            println!(
                "Result {}: node_id = {}",
                i + 1,
                result.get("node_id").unwrap()
            );
        } else if has_id {
            println!("Result {}: id = {}", i + 1, result.get("id").unwrap());
        } else {
            eprintln!("Result {}: [ERROR] MISSING node_id AND id!", i + 1);
            all_have_node_id = false;
        }
    }

    println!();
    if all_have_node_id {
        println!("[PASS] All results have node_id or id field");
    } else {
        panic!("[FAIL] Some results missing node_id/id - agents cannot use other graph tools!");
    }
}

/// Test semantic_search_nodes_via_chunks returns full node records with content.
/// This is the context-engineering optimized search that returns deduplicated nodes.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_semantic_search_nodes_via_chunks() {
    let graph = match setup_graph_functions().await {
        Some(g) => g,
        None => return,
    };

    println!("Using project_id: {}", graph.project_id());

    // Create Ollama embedding provider with model from env var
    let mut ollama_config = OllamaEmbeddingConfig::default();
    if let Ok(model) = std::env::var("CODEGRAPH_EMBEDDING_MODEL") {
        println!("Using embedding model from env: {}", model);
        ollama_config.model_name = model;
    } else {
        println!(
            "Using default embedding model: {}",
            ollama_config.model_name
        );
    }
    let ollama_provider = OllamaEmbeddingProvider::new(ollama_config);

    // Verify Ollama is available
    match ollama_provider.check_availability().await {
        Ok(true) => println!("✅ Ollama embedding provider available"),
        Ok(false) => {
            eprintln!("[skip] Ollama not available");
            return;
        }
        Err(e) => {
            eprintln!("[skip] Failed to check Ollama availability: {e}");
            return;
        }
    }

    let dim = ollama_provider.embedding_dimension();
    println!("Using embedding dimension: {}", dim);

    // Generate REAL embedding from query text
    let query_text = "index_project function implementation";
    let query_embedding: Vec<f32> =
        match ollama_provider.generate_single_embedding(query_text).await {
            Ok(emb) => {
                println!("Generated embedding with {} dimensions", emb.len());
                emb
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to generate embedding: {e}");
                return;
            }
        };

    let results: Vec<JsonValue> = match graph
        .semantic_search_nodes_via_chunks(
            query_text,
            &query_embedding,
            dim,
            10,  // limit
            0.2, // threshold
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            eprintln!("semantic_search_nodes_via_chunks error: {e}");
            return;
        }
    };

    println!("\n{}", "=".repeat(80));
    println!("SEMANTIC SEARCH NODES VIA CHUNKS - FULL NODE RECORDS");
    println!("{}", "=".repeat(80));
    println!("Query: {}", query_text);
    println!("Total results: {}\n", results.len());

    if results.is_empty() {
        eprintln!("[warn] No results returned - check embedding model matches indexed data");
        return;
    }

    // Print results showing full content AND graph context
    for (i, result) in results.iter().enumerate() {
        println!("--- Result {} ---", i + 1);
        println!("  node_id: {:?}", result.get("node_id"));
        println!("  name: {:?}", result.get("name"));
        println!("  kind: {:?}", result.get("kind"));
        println!("  file_path: {:?}", result.get("file_path"));
        println!("  start_line: {:?}", result.get("start_line"));
        println!("  end_line: {:?}", result.get("end_line"));
        println!("  vector_score: {:?}", result.get("vector_score"));

        // Show content preview (first 200 chars)
        if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
            let preview: String = content.chars().take(200).collect();
            println!("  content (preview): {}...", preview.replace('\n', "\\n"));
        } else {
            println!("  content: [MISSING or NULL]");
        }

        // Show GRAPH CONTEXT - ALL edge types
        println!("  --- Graph Context (ALL relationships) ---");
        if let Some(outgoing) = result.get("outgoing_edges").and_then(|c| c.as_array()) {
            println!("  outgoing_edges ({}):", outgoing.len());
            for edge in outgoing.iter().take(5) {
                println!(
                    "    --[{}]--> {} ({})",
                    edge.get("relationship")
                        .and_then(|r| r.as_str())
                        .unwrap_or("?"),
                    edge.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                    edge.get("file_path")
                        .and_then(|f| f.as_str())
                        .unwrap_or("?")
                );
            }
        } else {
            println!("  outgoing_edges: []");
        }

        if let Some(incoming) = result.get("incoming_edges").and_then(|c| c.as_array()) {
            println!("  incoming_edges ({}):", incoming.len());
            for edge in incoming.iter().take(5) {
                println!(
                    "    <--[{}]-- {} ({})",
                    edge.get("relationship")
                        .and_then(|r| r.as_str())
                        .unwrap_or("?"),
                    edge.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                    edge.get("file_path")
                        .and_then(|f| f.as_str())
                        .unwrap_or("?")
                );
            }
        } else {
            println!("  incoming_edges: []");
        }
        println!();
    }

    // Verification
    println!("{}", "=".repeat(80));
    println!("VERIFICATION SUMMARY");
    println!("{}", "=".repeat(80));

    let mut all_have_content = true;
    let mut all_have_graph_context = true;
    let mut unique_node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, result) in results.iter().enumerate() {
        let has_content = result.get("content").map_or(false, |c| !c.is_null());
        let has_node_id = result.get("node_id").map_or(false, |n| !n.is_null());
        let has_outgoing = result.get("outgoing_edges").is_some();
        let has_incoming = result.get("incoming_edges").is_some();

        if !has_content {
            eprintln!("Result {}: [WARN] Missing content field", i + 1);
            all_have_content = false;
        }

        if !has_outgoing || !has_incoming {
            eprintln!("Result {}: [WARN] Missing graph context fields", i + 1);
            all_have_graph_context = false;
        }

        if let Some(node_id) = result.get("node_id").and_then(|n| n.as_str()) {
            if !unique_node_ids.insert(node_id.to_string()) {
                eprintln!("Result {}: [ERROR] Duplicate node_id: {}", i + 1, node_id);
            }
        } else if !has_node_id {
            eprintln!("Result {}: [ERROR] Missing node_id", i + 1);
        }
    }

    println!("Unique nodes returned: {}", unique_node_ids.len());
    println!("All have graph context: {}", all_have_graph_context);
    println!();

    if all_have_content && all_have_graph_context && unique_node_ids.len() == results.len() {
        println!("[PASS] Full GraphRAG: content + graph context + unique nodes");
    } else if !all_have_content {
        panic!("[FAIL] Some results missing content!");
    } else if !all_have_graph_context {
        panic!("[FAIL] Some results missing graph context fields!");
    } else {
        panic!("[FAIL] Duplicate nodes found!");
    }
}
