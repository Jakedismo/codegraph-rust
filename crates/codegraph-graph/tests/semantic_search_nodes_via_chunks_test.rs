// ABOUTME: Integration test for hybrid semantic search over nodes via chunks.
// ABOUTME: Verifies vector scores surface and match sources are populated.
#![cfg(feature = "surrealdb")]

use std::path::PathBuf;
use std::{env, str::FromStr};

use anyhow::{Context, Result};
use serde_json::Value;
use surrealdb::{engine::local::Mem, Surreal};

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .ok()
        .and_then(|v| {
            bool::from_str(v.trim()).ok().or_else(|| match v.trim() {
                "1" => Some(true),
                "0" => Some(false),
                _ => None,
            })
        })
        .unwrap_or(false)
}

fn extract_function(schema: &str, name: &str) -> Result<String> {
    let marker = format!("DEFINE FUNCTION {}(", name);
    let start = schema
        .find(&marker)
        .with_context(|| format!("could not find marker for {}", name))?;
    let tail = &schema[start..];
    let end = tail
        .find("PERMISSIONS FULL;")
        .with_context(|| format!("could not find terminator for {}", name))?;
    Ok(tail[..end + "PERMISSIONS FULL;".len()].to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn vector_scores_surface_in_hybrid_search() -> Result<()> {
    if !env_flag_enabled("CODEGRAPH_RUN_SEMANTIC_SEARCH_NODES_VIA_CHUNKS_TEST") {
        return Ok(());
    }

    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("test").await?;

    // Minimal schema to satisfy function dependencies and indexes used by HNSW and BM25.
    db.query(
        r#"
        DEFINE ANALYZER code_analyzer TOKENIZERS BLANK,CLASS FILTERS LOWERCASE,SNOWBALL(ENGLISH);
        DEFINE TABLE nodes SCHEMALESS PERMISSIONS FULL;
        DEFINE FIELD project_id ON nodes TYPE string PERMISSIONS FULL;
        DEFINE FIELD name ON nodes TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD node_type ON nodes TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD language ON nodes TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD content ON nodes TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD file_path ON nodes TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD start_line ON nodes TYPE option<int> PERMISSIONS FULL;
        DEFINE FIELD end_line ON nodes TYPE option<int> PERMISSIONS FULL;
        DEFINE FIELD metadata ON nodes TYPE option<object> PERMISSIONS FULL;
        DEFINE INDEX idx_nodes_content ON nodes FIELDS content SEARCH ANALYZER code_analyzer;
        DEFINE INDEX idx_nodes_name ON nodes FIELDS name SEARCH ANALYZER code_analyzer;

        DEFINE TABLE chunks SCHEMALESS PERMISSIONS FULL;
        DEFINE FIELD project_id ON chunks TYPE string PERMISSIONS FULL;
        DEFINE FIELD parent_node ON chunks TYPE option<record<nodes>> PERMISSIONS FULL;
        DEFINE FIELD embedding_384 ON chunks TYPE option<array<float>> PERMISSIONS FULL;
        DEFINE INDEX idx_chunks_vector_384 ON chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12;

        DEFINE TABLE symbol_embeddings SCHEMALESS PERMISSIONS FULL;
        DEFINE FIELD project_id ON symbol_embeddings TYPE string PERMISSIONS FULL;
        DEFINE FIELD source_edge_id ON symbol_embeddings TYPE option<record<edges>> PERMISSIONS FULL;
        DEFINE FIELD symbol ON symbol_embeddings TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD embedding_384 ON symbol_embeddings TYPE option<array<float>> PERMISSIONS FULL;
        DEFINE INDEX idx_symbol_vector_384 ON symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12;

        DEFINE TABLE edges SCHEMALESS PERMISSIONS FULL;
        DEFINE FIELD edge_type ON edges TYPE option<string> PERMISSIONS FULL;
        DEFINE FIELD from ON edges TYPE option<record<nodes>> PERMISSIONS FULL;
        DEFINE FIELD to ON edges TYPE option<record<nodes>> PERMISSIONS FULL;
        DEFINE FIELD metadata ON edges TYPE option<object> PERMISSIONS FULL;
        "#,
    )
    .await?;

    // Minimal helper functions used by semantic_search_nodes_via_chunks.
    db.query(
        r#"
        DEFINE FUNCTION fn::parse_record_id($table: string, $input: any) {
            IF type::is::record($input) { RETURN $input; };
            LET $str = <string>$input;
            LET $after_prefix = IF string::starts_with($str, $table + ':') THEN string::slice($str, string::len($table) + 1) ELSE $str END;
            LET $clean_id = IF string::starts_with($after_prefix, '⟨') AND string::ends_with($after_prefix, '⟩') THEN string::slice($after_prefix, 1, string::len($after_prefix) - 2) ELSE $after_prefix END;
            RETURN type::thing($table, $clean_id);
        } PERMISSIONS FULL;

        DEFINE FUNCTION fn::node_info($node_id: any) {
            IF $node_id = NONE OR !type::is::record($node_id) { RETURN NONE; };
            LET $res = (SELECT <string>id AS id, name, node_type AS kind, language, content, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location, file_path, start_line, end_line FROM ONLY $node_id);
            RETURN $res;
        } PERMISSIONS FULL;

        DEFINE FUNCTION fn::edge_context($node_ref: any) {
            IF $node_ref = NONE { RETURN { outgoing: [], incoming: [] }; };
            RETURN {
                outgoing: (SELECT
                    <string> `to` AS node_id,
                    (SELECT VALUE name FROM nodes WHERE id = `to` LIMIT 1)[0] AS name,
                    (SELECT VALUE node_type FROM nodes WHERE id = `to` LIMIT 1)[0] AS kind,
                    (SELECT VALUE file_path FROM nodes WHERE id = `to` LIMIT 1)[0] AS file_path,
                    edge_type AS relationship
                FROM edges
                WHERE `from` = $node_ref
                LIMIT 5),
                incoming: (SELECT
                    <string> `from` AS node_id,
                    (SELECT VALUE name FROM nodes WHERE id = `from` LIMIT 1)[0] AS name,
                    (SELECT VALUE node_type FROM nodes WHERE id = `from` LIMIT 1)[0] AS kind,
                    (SELECT VALUE file_path FROM nodes WHERE id = `from` LIMIT 1)[0] AS file_path,
                    edge_type AS relationship
                FROM edges
                WHERE `to` = $node_ref
                LIMIT 5)
            };
        } PERMISSIONS FULL;
        "#,
    )
    .await?;

    // Load and install only the function under test to avoid running the entire schema.
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schema")
        .join("codegraph.surql");
    let schema = std::fs::read_to_string(&schema_path)
        .with_context(|| format!("failed to read {}", schema_path.display()))?;
    let function_sql = extract_function(&schema, "fn::semantic_search_nodes_via_chunks")
        .context("missing function")?;
    db.query(function_sql).await?;

    let project_id = "proj";
    let embedding: Vec<f32> = vec![1.0; 384];

    db.query(
        r#"
        CREATE nodes:node1 SET
            project_id = $project_id,
            name = 'foo function',
            node_type = 'Function',
            language = 'rust',
            content = 'fn foo() {}',
            file_path = 'src/lib.rs',
            start_line = 1,
            end_line = 3;

        CREATE nodes:node2 SET
            project_id = $project_id,
            name = 'bar helper',
            node_type = 'Function',
            language = 'rust',
            content = 'fn bar() {}',
            file_path = 'src/lib.rs',
            start_line = 5,
            end_line = 8;

        CREATE edges:edge1 SET
            edge_type = 'calls',
            from = nodes:node1,
            to = nodes:node2;

        CREATE edges:edge2 SET
            edge_type = 'returns',
            from = nodes:node2,
            to = nodes:node1;

        CREATE chunks:chunk1 SET
            project_id = $project_id,
            parent_node = nodes:node1,
            embedding_384 = $embedding;
        "#,
    )
    .bind(("project_id", project_id))
    .bind(("embedding", embedding.clone()))
    .await?;

    let mut response = db
        .query(
            "RETURN fn::semantic_search_nodes_via_chunks($project_id, $query_text, $dimension, $limit, $threshold, $query_embedding);",
        )
        .bind(("project_id", project_id))
        .bind(("query_text", "foo"))
        .bind(("dimension", 384_i64))
        .bind(("limit", 5_i64))
        .bind(("threshold", 0.01_f64))
        .bind(("query_embedding", embedding.clone()))
        .await?;

    let results: Vec<Value> = response.take(0).context("no result set returned")?;
    assert!(
        !results.is_empty(),
        "expected at least one result from semantic search"
    );

    let first = results.first().context("missing first result")?;
    let vector_score = first
        .get("vector_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    assert!(
        vector_score > 0.01,
        "vector_score should be > 0.01, got {}",
        vector_score
    );

    let match_sources = first
        .get("match_sources")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        match_sources.iter().any(|v| v == "chunk"),
        "match_sources should include 'chunk', got {:?}",
        match_sources
    );

    let outgoing = first
        .get("outgoing_edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        outgoing
            .iter()
            .any(|e| e.get("relationship").and_then(|r| r.as_str()) == Some("calls")),
        "outgoing_edges should include a 'calls' relationship, got {:?}",
        outgoing
    );

    let incoming = first
        .get("incoming_edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        incoming
            .iter()
            .any(|e| e.get("relationship").and_then(|r| r.as_str()) == Some("returns")),
        "incoming_edges should include a 'returns' relationship, got {:?}",
        incoming
    );

    Ok(())
}
