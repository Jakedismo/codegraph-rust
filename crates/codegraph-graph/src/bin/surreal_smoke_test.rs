use anyhow::Context;
use serde_json::json;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root, Surreal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_filename(".env").or_else(|_| dotenvy::dotenv()).context("Failed to load .env")?;

    let url = std::env::var("CODEGRAPH_SURREALDB_URL")?;
    let namespace = std::env::var("CODEGRAPH_SURREALDB_NAMESPACE")?;
    let database = std::env::var("CODEGRAPH_SURREALDB_DATABASE")?;
    let username = std::env::var("CODEGRAPH_SURREALDB_USERNAME")?;
    let password = std::env::var("CODEGRAPH_SURREALDB_PASSWORD")?;

    let endpoint = url
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .to_string();
    let db = Surreal::new::<Ws>(&endpoint).await?;
    db.signin(Root {
        username: &username,
        password: &password,
    })
    .await?;
    db.use_ns(&namespace).use_db(&database).await?;

    let node_one = json!({
        "id": "smoke-node-alpha",
        "name": "Alpha",
        "node_type": "Struct",
        "language": "Rust",
        "content": "pub struct Alpha;",
        "file_path": "smoke/alpha.rs",
        "start_line": 1,
        "end_line": 1,
        "metadata": {"sample": true},
        "project_id": "smoke-project",
    });

    let node_two = json!({
        "id": "smoke-node-beta",
        "name": "Beta",
        "node_type": "Function",
        "language": "Rust",
        "content": "fn beta() {}",
        "file_path": "smoke/beta.rs",
        "start_line": 3,
        "end_line": 4,
        "metadata": {"sample": true},
        "project_id": "smoke-project",
    });

    db.query("UPSERT type::thing('nodes', $doc.id) CONTENT $doc;")
        .bind(("doc", node_one))
        .await?;

    db.query("UPSERT type::thing('nodes', $doc.id) CONTENT $doc;")
        .bind(("doc", node_two))
        .await?;

    let edge_payload = json!({
        "id": "smoke-edge-alpha-beta",
        "from": "smoke-node-alpha",
        "to": "smoke-node-beta",
        "edge_type": "Calls",
        "weight": 1.0,
        "metadata": {"sample": true},
    });

    db.query(
        "UPSERT type::thing('edges', $doc.id) CONTENT {
            id: $doc.id,
            from: type::thing('nodes', $doc.from),
            to: type::thing('nodes', $doc.to),
            edge_type: $doc.edge_type,
            weight: $doc.weight,
            metadata: $doc.metadata,
            created_at: time::now()
        };",
    )
    .bind(("doc", edge_payload))
    .await?;

    let zero_embedding: Vec<f64> = vec![0.0; 2048];
    let symbol_doc = json!({
        "id": "smoke-symbol-alpha",
        "symbol": "Alpha::beta",
        "normalized_symbol": "alpha::beta",
        "project_id": "smoke-project",
        "embedding_2048": zero_embedding,
        "embedding_model": "dummy",
        "access_count": 0,
    });

    db.query("UPSERT type::thing('symbol_embeddings', $doc.id) CONTENT $doc;")
        .bind(("doc", symbol_doc))
        .await?;

    println!("✅ Smoke records inserted. Verify via Surreal shell.");
    Ok(())
}
