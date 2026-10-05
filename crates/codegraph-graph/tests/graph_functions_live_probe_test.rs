// ABOUTME: Opt-in timing probe: runs every fn:: against a project's embedded store (PROBE_ROOT=<project>).
// ABOUTME: Prints row counts and latencies; does nothing unless PROBE_ROOT is set.
#![cfg(feature = "surrealdb")]
use serde_json::Value;
#[tokio::test]
async fn live_probe() {
    let Ok(root) = std::env::var("PROBE_ROOT") else {
        return;
    };
    let cfg = codegraph_graph::SurrealDbConfig::embedded(std::path::Path::new(&root));
    let storage = codegraph_graph::SurrealDbStorage::new(cfg).await.unwrap();
    let db = storage.db();
    let project = std::fs::canonicalize(&root).unwrap().display().to_string();
    for (label, q) in [
        ("hubs", "RETURN { r: fn::get_hub_nodes($p, 20) }"),
        (
            "hotspots",
            "RETURN { r: fn::get_complexity_hotspots($p, 10.0, 5) }",
        ),
        (
            "cycles",
            "RETURN { r: fn::detect_circular_dependencies($p, 'calls') }",
        ),
        (
            "find",
            "RETURN { r: fn::find_nodes_by_name($p, 'index_project', 3) }",
        ),
        ("counts", "RETURN { r: fn::count_nodes_for_project($p) }"),
        (
            "snippets",
            "RETURN { r: fn::search_snippets($p, 'embedding generator', 3) }",
        ),
    ] {
        let t = std::time::Instant::now();
        let mut r = db.query(q).bind(("p", project.clone())).await.unwrap();
        let v: Option<Value> = match r.take(0) {
            Ok(v) => v,
            Err(e) => {
                println!("PROBE {label}: ERROR {e}");
                continue;
            }
        };
        let v = v.unwrap();
        let n = v["r"].as_array().map(|a| a.len());
        println!(
            "PROBE {label}: {:?} rows in {:?}; first={}",
            n,
            t.elapsed(),
            v["r"]
                .as_array()
                .and_then(|a| a.first())
                .map(|f| f.to_string().chars().take(220).collect::<String>())
                .unwrap_or(v["r"].to_string())
        );
    }
    let mut r = db
        .query(
            "SELECT VALUE id FROM nodes WHERE project_id = $p AND name = 'index_project' LIMIT 1",
        )
        .bind(("p", project.clone()))
        .await
        .unwrap();
    let ids: Vec<Value> = r.take(0).unwrap();
    let id = ids[0].as_str().unwrap().to_string();
    for (label, q) in [
        (
            "deps",
            "RETURN { r: fn::get_transitive_dependencies($p, $n, 'calls', 3) }",
        ),
        (
            "rdeps",
            "RETURN { r: fn::get_reverse_dependencies($p, $n, 'calls', 3) }",
        ),
        ("chain", "RETURN { r: fn::trace_call_chain($p, $n, 4) }"),
        (
            "coupling",
            "RETURN { r: fn::calculate_coupling_metrics($p, $n) }",
        ),
    ] {
        let t = std::time::Instant::now();
        let mut r = db
            .query(q)
            .bind(("p", project.clone()))
            .bind(("n", id.clone()))
            .await
            .unwrap();
        let v: Option<Value> = r.take(0).unwrap();
        let v = v.unwrap();
        println!(
            "PROBE {label}: {:?} rows in {:?}; first={}",
            v["r"].as_array().map(|a| a.len()),
            t.elapsed(),
            v["r"]
                .as_array()
                .and_then(|a| a.first())
                .map(|f| f.to_string().chars().take(200).collect::<String>())
                .unwrap_or(v["r"].to_string().chars().take(300).collect())
        );
    }
}
