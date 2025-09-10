use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use codegraph_core::{Language, Location, NodeType};
use codegraph_graph::CodeGraph;
use codegraph_parser::TreeSitterParser;
use codegraph_vector::{EmbeddingGenerator, FaissVectorStore, SemanticSearch};

#[derive(Debug, Clone, Serialize)]
struct PerfCheckResult {
    name: String,
    passed: bool,
    measured_value: f64,
    unit: &'static str,
    threshold: f64,
    notes: String,
}

#[derive(Debug, Clone, Serialize)]
struct PerfReport {
    reference_document: String,
    checks: Vec<PerfCheckResult>,
}

fn write_report(report: &PerfReport) {
    let out_dir = PathBuf::from("target/bench_reports");
    let _ = fs::create_dir_all(&out_dir);
    let path = out_dir.join("perf_report.json");
    if let Ok(json) = serde_json::to_string_pretty(report) {
        let _ = fs::write(&path, json);
    }
    // Regression indicator file
    if report.checks.iter().any(|c| !c.passed) {
        let _ = fs::write(out_dir.join("regression_detected"), b"1");
        eprintln!("PERF_REGRESSION: one or more checks failed");
    }
}

fn create_test_rust_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("use std::collections::HashMap;\nuse std::sync::Arc;\n\n");
    for i in 0..lines {
        match i % 10 {
            0 => code.push_str(&format!("pub struct S{i}{{x:i32}}\n\n")),
            1 => code.push_str(&format!("pub enum E{i}{{A(i32),B}}\n\n")),
            2 => code.push_str(&format!("pub trait T{i}{{fn m{i}(&self)->i32;}}\n\n")),
            3 => code.push_str(&format!("impl T{i} for S{i}{{fn m{i}(&self)->i32{{self.x}}}}\n\n")),
            4 => code.push_str(&format!("pub fn f{i}(p:i32)->i32{{p*{i}}}\n\n")),
            _ => code.push_str(&format!("// c{i}\nlet _v{i}= {i};\n")),
        }
    }
    code
}

fn create_test_typescript_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("import * as React from 'react';\n\n");
    for i in 0..lines {
        match i % 8 {
            0 => code.push_str(&format!("interface I{i}{{p{i}:number;}}\n\n")),
            1 => code.push_str(&format!("class C{i} implements I{i}{{p{i}:number={i};m(){}}}\n\n")),
            2 => code.push_str(&format!("function f{i}(x:number):number{{return x+{i}}}\n\n")),
            _ => code.push_str(&format!("// c{i}\nconst v{i}={i};\n")),
        }
    }
    code
}

fn create_test_python_code(lines: usize) -> String {
    let mut code = String::new();
    code.push_str("from typing import List\n\n");
    for i in 0..lines {
        match i % 6 {
            0 => code.push_str(&format!("class D{i}:\n    def __init__(self,v:int):\n        self.v=v\n\n")),
            1 => code.push_str(&format!("def f{i}(x:int)->int:\n    return x+{i}\n\n")),
            _ => code.push_str(&format!("# c{i}\nZ{i}={i}\n")),
        }
    }
    code
}

fn create_test_project(temp_dir: &TempDir, total_lines: usize) -> std::io::Result<()> {
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir)?;
    let rust_lines = total_lines / 3;
    let ts_lines = total_lines / 3;
    let py_lines = total_lines - rust_lines - ts_lines;
    fs::write(src_dir.join("main.rs"), create_test_rust_code(rust_lines))?;
    fs::write(src_dir.join("main.ts"), create_test_typescript_code(ts_lines))?;
    fs::write(src_dir.join("main.py"), create_test_python_code(py_lines))?;
    Ok(())
}

fn process_memory_mb() -> u64 {
    use sysinfo::{ProcessExt, System, SystemExt};
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = sysinfo::get_current_pid().unwrap_or(sysinfo::Pid::from_u32(0));
    if let Some(p) = sys.process(pid) {
        // sysinfo 0.30: memory() returns KiB; convert to MB
        (p.memory() / 1024) as u64
    } else {
        0
    }
}

fn bench_indexing_and_memory(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut report = PerfReport {
        reference_document: "docs/specifications/FEATURE_INVENTORY.md".to_string(),
        checks: Vec::new(),
    };

    let mut group = c.benchmark_group("performance_targets");
    group.measurement_time(Duration::from_secs(30));

    // 1) Indexing throughput: >20k LOC in 15s
    for &lines in &[5_000usize, 20_000usize] {
        group.bench_with_input(BenchmarkId::new("indexing_loc", lines), &lines, |b, &loc| {
            b.to_async(&rt).iter(|| async move {
                let temp_dir = TempDir::new().unwrap();
                create_test_project(&temp_dir, loc).unwrap();
                let parser = TreeSitterParser::new();
                let start = Instant::now();
                let (_nodes, stats) = parser
                    .parse_directory_parallel(&temp_dir.path().to_string_lossy())
                    .await
                    .unwrap();
                let duration = start.elapsed();
                if loc >= 20_000 {
                    let pass = duration.as_secs_f64() <= 15.0;
                    report.checks.push(PerfCheckResult {
                        name: "Indexing throughput (20k LOC)".into(),
                        passed: pass,
                        measured_value: duration.as_secs_f64(),
                        unit: "s",
                        threshold: 15.0,
                        notes: format!("parsed {} files, {} lines", stats.parsed_files, stats.total_lines),
                    });
                    if !pass {
                        eprintln!(
                            "⚠️ Indexing {} LOC took {:.2}s (>15s target)",
                            loc,
                            duration.as_secs_f64()
                        );
                    } else {
                        println!(
                            "✅ Indexing {} LOC took {:.2}s (<=15s)",
                            loc,
                            duration.as_secs_f64()
                        );
                    }
                }
            });
        });
    }

    // 2) Memory usage under load: <250MB for 100k LOC
    group.bench_function("memory_under_load_100k_loc", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let loc = 100_000usize;
            create_test_project(&temp_dir, loc).unwrap();

            let parser = TreeSitterParser::new();
            let (nodes, _stats) = parser
                .parse_directory_parallel(&temp_dir.path().to_string_lossy())
                .await
                .unwrap();

            // Build embeddings and vector index
            let embedding = EmbeddingGenerator::default();
            let mut enriched = Vec::with_capacity(nodes.len());
            for mut n in nodes {
                // Fill required fields if missing content (most nodes have content already)
                if n.content.is_none() {
                    n = n.with_content(" ");
                }
                let emb = embedding.generate_embedding(&n).await.unwrap();
                n = n.with_embedding(emb);
                enriched.push(n);
            }

            let mut store = FaissVectorStore::new(384).unwrap();
            store.store_embeddings(&enriched).await.unwrap();

            // Also add to graph (nodes only) to simulate pipeline
            let mut graph = CodeGraph::new_with_cache().unwrap();
            for n in &enriched {
                let _ = graph.add_node(n.clone()).await;
            }

            // Measure process memory
            let mem_mb = process_memory_mb();
            let pass = mem_mb <= 250;
            println!("Memory under load (100k LOC): {} MB", mem_mb);
            // Record result once (not per iteration)
            // Note: Criterion may run multiple iterations; keep side effects idempotent enough
            report.checks.push(PerfCheckResult {
                name: "Memory usage (100k LOC)".into(),
                passed: pass,
                measured_value: mem_mb as f64,
                unit: "MB",
                threshold: 250.0,
                notes: "Process RSS after parse+embed+graph".into(),
            });
            if !pass {
                eprintln!("⚠️ Memory usage {:.0}MB exceeds 250MB target", mem_mb);
            } else {
                println!("✅ Memory usage {:.0}MB (<=250MB)", mem_mb);
            }
        });
    });

    // 3) Simple and complex query performance validation (sanity)
    group.bench_function("query_performance_sanity", |b| {
        b.to_async(&rt).iter(|| async {
            let mut graph = CodeGraph::new_with_cache().unwrap();
            let nodes: Vec<_> = (0..5_000).map(|i| {
                let location = Location { file_path: format!("f{i}.rs"), line: 1, column: 1, end_line: Some(2), end_column: Some(1) };
                codegraph_core::CodeNode::new(format!("n{i}"), Some(NodeType::Function), Some(Language::Rust), location)
            }).collect();
            for n in &nodes { let _ = graph.add_node(n.clone()).await; }
            // Simple: cached get_node calls
            let start = Instant::now();
            for _ in 0..500 { let _ = graph.get_node(nodes[0].id).await; }
            let simple_ms = start.elapsed().as_millis() as f64;
            let simple_pass = simple_ms <= 50.0;
            report.checks.push(PerfCheckResult{ name: "Simple query latency".into(), passed: simple_pass, measured_value: simple_ms, unit: "ms", threshold: 50.0, notes: "500 cached get_node ops".into()});

            // Complex: shortest path batches on sparse graph (no edges -> None quickly)
            let start = Instant::now();
            for _ in 0..50 { let _ = graph.shortest_path(nodes[0].id, nodes[1].id).await; }
            let complex_ms = start.elapsed().as_millis() as f64;
            let complex_pass = complex_ms <= 200.0;
            report.checks.push(PerfCheckResult{ name: "Complex query latency".into(), passed: complex_pass, measured_value: complex_ms, unit: "ms", threshold: 200.0, notes: "50 shortest_path ops (sparse)".into()});
        });
    });

    group.finish();

    write_report(&report);
}

criterion_group!(benches, bench_indexing_and_memory);
criterion_main!(benches);

