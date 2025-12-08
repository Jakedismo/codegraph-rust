// ABOUTME: End-to-end graph function diagnostics using live SurrealDB
// ABOUTME: Validates semantic search via direct cosine selects (no UDFs) and hybrid merge

#[cfg(all(test, feature = "surrealdb"))]
mod semantic_search_direct_hybrid {
    use super::GraphFunctions;
    use super::SurrealDbConfig;
    use super::SurrealDbStorage;
    use serde_json::Value as JsonValue;
    use surrealdb::sql::Value as SqlValue;

    fn env(name: &str, default: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| default.to_string())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn semantic_search_direct_hybrid_smoke() {
        let url = env("CODEGRAPH_SURREALDB_URL", "");
        if url.is_empty() {
            eprintln!("[skip] CODEGRAPH_SURREALDB_URL not set");
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
        let gf = GraphFunctions::new(storage.db());
        let project_id = gf.project_id().to_string();
        println!("Project id: {}", project_id);

        // Build a real query embedding from the stored embeddings (reusing vector scores)
        let query_text = "GraphFunctions struct";
        // For smoke purposes, pull one 1024-d embedding from chunks as the query embedding surrogate
        // to bypass external providers in CI environments.
        let surrogate_sql = r#"
            SELECT embedding_1024
            FROM chunks
            WHERE project_id = $project_id AND embedding_1024 != NONE
            LIMIT 1;
        "#;
        let mut surrogate_resp = match gf
            .db
            .query(surrogate_sql)
            .bind(("project_id", project_id.clone()))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[skip] failed to fetch surrogate embedding: {e}");
                return;
            }
        };
        let surrogate: Option<SqlValue> = surrogate_resp.take(0).ok();
        let Some(SqlValue::Object(obj)) = surrogate else {
            eprintln!("[skip] no embedding_1024 available");
            return;
        };
        let Some(SqlValue::Array(embed_val)) = obj.get("embedding_1024") else {
            eprintln!("[skip] embedding shape unexpected");
            return;
        };
        let query_embedding = embed_val.clone();

        // Direct chunk cosine
        let chunk_sql = r#"
            SELECT <string>parent_node AS node_id,
                   vector::similarity::cosine(embedding_1024, $query_embedding) AS score,
                   'chunk' AS source
            FROM chunks
            WHERE project_id = $project_id
              AND embedding_1024 != NONE
            ORDER BY score DESC
            LIMIT 10;
        "#;
        let mut chunk_resp = gf
            .db
            .query(chunk_sql)
            .bind(("project_id", project_id.clone()))
            .bind(("query_embedding", query_embedding.clone()))
            .await
            .expect("chunk query failed");
        let chunk_hits: Vec<SqlValue> = chunk_resp.take::<Vec<SqlValue>>(0).unwrap_or_default();
        println!("Chunk hits: {:?}", chunk_hits);
        assert!(!chunk_hits.is_empty(), "chunk hits empty");

        // Direct symbol cosine
        let symbol_sql = r#"
            SELECT <string>source_edge_id.from AS node_id,
                   vector::similarity::cosine(embedding_1024, $query_embedding) AS score,
                   'symbol_reference' AS source,
                   symbol
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_1024 != NONE
            ORDER BY score DESC
            LIMIT 10;
        "#;
        let mut sym_resp = gf
            .db
            .query(symbol_sql)
            .bind(("project_id", project_id.clone()))
            .bind(("query_embedding", query_embedding.clone()))
            .await
            .expect("symbol query failed");
        let sym_hits: Vec<SqlValue> = sym_resp.take::<Vec<SqlValue>>(0).unwrap_or_default();
        println!("Symbol hits: {:?}", sym_hits);

        // Hybrid merge in Rust: take top vector hits, attach simple combined_score
        fn extract(v: &SqlValue, key: &str) -> Option<f64> {
            match v {
                SqlValue::Object(map) => map.get(key).and_then(|x| x.as_number()).and_then(|n| n.as_f64()),
                _ => None,
            }
        }
        fn node_id(v: &SqlValue) -> Option<String> {
            match v {
                SqlValue::Object(map) => map.get("node_id").and_then(|x| x.as_str()).map(|s| s.to_string()),
                _ => None,
            }
        }
        let mut combined: Vec<(String, f64, String)> = Vec::new();
        for h in chunk_hits.iter().chain(sym_hits.iter()) {
            if let Some(id) = node_id(h) {
                let score = extract(h, "score").unwrap_or(0.0);
                let source = match h {
                    SqlValue::Object(map) => map
                        .get("source")
                        .and_then(|x| x.as_str())
                        .unwrap_or("vector")
                        .to_string(),
                    _ => "vector".to_string(),
                };
                combined.push((id, score, source));
            }
        }
        combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        combined.truncate(5);
        println!("Hybrid combined top: {:?}", combined);
        assert!(!combined.is_empty(), "combined vector hits empty");
    }
}
