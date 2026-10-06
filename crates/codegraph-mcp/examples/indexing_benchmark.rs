// ABOUTME: Repeatable offline indexing speed and graph-equivalence benchmark.
// ABOUTME: Uses temporary corpora and reports feature/analyzer policies with every sample.
use anyhow::{Result, ensure};
use codegraph_core::config_manager::{CodeGraphConfig, IndexingTier};
use codegraph_mcp::{IndexStats, IndexerConfig, ProjectIndexer};
use indicatif::{MultiProgress, ProgressDrawTarget};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

async fn graph(indexer: &ProjectIndexer) -> Result<Value> {
    let storage = indexer.surreal_storage().await;
    let mut response = storage.lock().await.db().query("SELECT id, name, node_type, file_path, language, metadata.attributes FROM nodes ORDER BY id; SELECT id, from, to, edge_type, metadata FROM edges ORDER BY id").await?.check()?;
    Ok(json!({"nodes":response.take::<Vec<Value>>(0)?,"edges":response.take::<Vec<Value>>(1)?}))
}
async fn sample(
    indexer: &ProjectIndexer,
    path: &Path,
    name: &str,
    repeat: usize,
    output: &mut Vec<Value>,
) -> Result<IndexStats> {
    let start = Instant::now();
    let stats = indexer.reconcile_project(path, false).await?;
    ensure!(
        stats.complete && stats.errors == 0,
        "Incomplete benchmark run"
    );
    output.push(json!({"scenario":name,"repeat":repeat,"wall_us":start.elapsed().as_micros(),"stats":stats,"graph_fingerprint":codegraph_core::artifact_cache::fingerprint(&graph(indexer).await?)?}));
    Ok(stats)
}
#[tokio::main]
async fn main() -> Result<()> {
    ensure!(
        std::env::var("CODEGRAPH_SURREALDB_URL").as_deref() == Ok("mem://"),
        "Set CODEGRAPH_SURREALDB_URL=mem:// to isolate this benchmark"
    );
    let repeats = std::env::var("CODEGRAPH_BENCH_REPEATS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let files = std::env::var("CODEGRAPH_BENCH_FILES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100)
        .max(2);
    let mut samples = Vec::new();
    for repeat in 0..repeats {
        let directory = tempfile::tempdir()?;
        let root = std::fs::canonicalize(directory.path())?;
        std::fs::write(root.join("target.rs"), "pub fn target() {}\n")?;
        for index in 0..files - 1 {
            std::fs::write(
                root.join(format!("file_{index}.rs")),
                format!("pub fn caller_{index}() {{ target(); }}\n"),
            )?;
        }
        let startup = Instant::now();
        let make = |tier| IndexerConfig {
            project_root: root.clone(),
            indexing_tier: tier,
            ..Default::default()
        };
        let mut indexer = ProjectIndexer::new(
            make(IndexingTier::Fast),
            &CodeGraphConfig::default(),
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
        let startup_us = startup.elapsed().as_micros();
        sample(&indexer, &root, "cold", repeat, &mut samples).await?;
        let warm = sample(&indexer, &root, "no_change", repeat, &mut samples).await?;
        ensure!(
            warm.cached_files == files && warm.writer_rows_acked == 0 && warm.inference_texts == 0,
            "No-change run performed unexpected work"
        );
        let start = Instant::now();
        let stats = indexer.reconcile_project(&root, true).await?;
        ensure!(
            stats.cached_files == files,
            "Warm forced run missed AST cache"
        );
        samples.push(json!({"scenario":"warm_full","repeat":repeat,"wall_us":start.elapsed().as_micros(),"stats":stats}));
        std::fs::write(
            root.join("file_0.rs"),
            "pub fn caller_0() { target(); target(); }\n",
        )?;
        let edit = sample(&indexer, &root, "single_file", repeat, &mut samples).await?;
        ensure!(
            edit.cached_files == files - 1,
            "Single-file edit reparsed unrelated files"
        );
        std::fs::rename(root.join("target.rs"), root.join("renamed.rs"))?;
        sample(&indexer, &root, "cross_file_rename", repeat, &mut samples).await?;
        let incremental = graph(&indexer).await?;
        indexer.reconcile_project(&root, true).await?;
        ensure!(
            graph(&indexer).await? == incremental,
            "Incremental rename differs from full indexing"
        );
        std::fs::remove_file(root.join("renamed.rs"))?;
        sample(&indexer, &root, "delete", repeat, &mut samples).await?;
        ensure!(
            !graph(&indexer).await?["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["name"] == "target"),
            "Deleted definition survived"
        );
        std::fs::write(root.join("README.md"), "`caller_0` is public\n")?;
        sample(&indexer, &root, "docs_change", repeat, &mut samples).await?;
        std::fs::write(root.join("package.json"), "{\"name\":\"fixture\"}")?;
        sample(&indexer, &root, "manifest_change", repeat, &mut samples).await?;
        for (name, tier) in [
            ("balanced_upgrade", IndexingTier::Balanced),
            ("full_upgrade", IndexingTier::Full),
        ] {
            indexer.set_indexing_tier(tier);
            sample(&indexer, &root, name, repeat, &mut samples).await?;
        }
        samples.push(json!({"scenario":"startup","repeat":repeat,"wall_us":startup_us}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"format":1,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"files":files,"repeats":repeats,"features":{"embeddings":cfg!(feature="embeddings"),"ai_enhanced":cfg!(feature="ai-enhanced")},"analyzers_env":std::env::var("CODEGRAPH_ANALYZERS").unwrap_or_default(),"samples":samples})
        )?
    );
    Ok(())
}
