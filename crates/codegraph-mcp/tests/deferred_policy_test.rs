// ABOUTME: Verifies graph readiness and durable deferred jobs without provider startup.
// ABOUTME: An intentionally invalid provider must not be touched by deferred indexing.
#![cfg(feature = "embeddings")]
use anyhow::Result;
use codegraph_core::{
    artifact_cache::ArtifactCache,
    config_manager::{CodeGraphConfig, IndexingTier},
};
use codegraph_mcp::{
    IndexerConfig, ProjectIndexer,
    policy::{StagePolicy, completion_policies},
};
use indicatif::{MultiProgress, ProgressDrawTarget};

#[tokio::test]
async fn deferred_graph_persists_a_resumable_job_without_loading_models() -> Result<()> {
    if !test_env::run(
        concat!(
            module_path!(),
            "::deferred_graph_persists_a_resumable_job_without_loading_models"
        ),
        &[
            ("CODEGRAPH_SURREALDB_URL", Some("mem://")),
            ("CODEGRAPH_EMBEDDING_POLICY", Some("deferred")),
            ("CODEGRAPH_SEMANTIC_RESOLUTION", Some("off")),
            ("CODEGRAPH_ANALYZERS", Some("0")),
            ("CODEGRAPH_NO_PROGRESS", Some("1")),
            ("CODEGRAPH_PROJECT_ID", Some("deferred-test")),
        ],
    ) {
        return Ok(());
    }
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("a.rs"), "pub fn sample() {}\n")?;
    let mut config = CodeGraphConfig::default();
    config.embedding.provider = "invalid-provider-must-not-initialize".into();
    let indexer = ProjectIndexer::new(
        IndexerConfig {
            project_root: root.path().into(),
            indexing_tier: IndexingTier::Fast,
            ..Default::default()
        },
        &config,
        MultiProgress::with_draw_target(ProgressDrawTarget::hidden()),
    )
    .await?;
    indexer
        .surreal_storage()
        .await
        .lock()
        .await
        .db()
        .query(
            codegraph_graph::vector_indexes::VectorIndexMode::Off
                .initial_schema(include_str!("../../../schema/codegraph_v2.surql")),
        )
        .await?
        .check()?;
    let stats = indexer.index_project(root.path()).await?;
    assert!(stats.graph_complete);
    assert!(!stats.complete);
    assert!(!stats.vector_index_ready);
    assert_eq!(stats.embedding_status, "pending");
    assert_eq!(stats.inference_texts, 0);
    let completion = completion_policies(root.path(), "deferred-test")?;
    assert_eq!(completion.embeddings, StagePolicy::Sync);
    assert_eq!(completion.semantic, StagePolicy::Off);
    let warm = indexer.index_project(root.path()).await?;
    assert_eq!(warm.cached_files, 1);
    assert!(!warm.complete);

    // A completed-catalog fixture checks scheduling reuse without loading a model.
    // This does not simulate or assert actual provider inference.
    let cache = ArtifactCache::new(
        root.path().join(".codegraph/index-cache"),
        "project-catalog-v1",
    );
    let mut catalog: serde_json::Value = cache.get("deferred-test").unwrap();
    catalog["stats"]["embedding_status"] = serde_json::json!("ready");
    catalog["stats"]["complete"] = serde_json::json!(true);
    cache.put("deferred-test", &catalog)?;
    codegraph_mcp::policy::pending_cache(root.path()).remove("deferred-test")?;
    let completed = indexer.index_project(root.path()).await?;
    assert!(completed.complete);
    assert_eq!(completed.embedding_status, "ready");
    assert_eq!(completed.writer_rows_acked, 0);
    assert_eq!(completed.inference_texts, 0);
    assert!(completion_policies(root.path(), "deferred-test").is_err());
    Ok(())
}
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
