// ABOUTME: Applies the shipped graph schemas to the current SurrealDB SDK.
// ABOUTME: Catches schema syntax and definition regressions without a remote database.
#![cfg(feature = "surrealdb")]

use surrealdb::{Surreal, engine::local::Mem};

#[tokio::test]
async fn shipped_schemas_apply_to_current_surrealdb() {
    for (name, schema) in [
        ("main", include_str!("../../../schema/codegraph.surql")),
        (
            "experimental",
            include_str!("../../../schema/codegraph_graph_experimental.surql"),
        ),
    ] {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("schema_test").use_db(name).await.unwrap();
        db.query(schema)
            .await
            .unwrap_or_else(|error| panic!("{name} schema parsing failed: {error}"))
            .check()
            .unwrap_or_else(|error| panic!("{name} schema application failed: {error}"));
    }
}
