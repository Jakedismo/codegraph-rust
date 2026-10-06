// ABOUTME: Compares the production semantic scorer with its frozen scalar predecessor offline.
// ABOUTME: Checks every winning node and score bit while timing cached norms and parallel scoring.

#[path = "../tests/support/semantic_scoring_reference.rs"]
mod reference;
#[path = "../src/semantic_scoring.rs"]
mod semantic_scoring;

use anyhow::{Result, ensure};
use clap::Parser;
use codegraph_core::NodeId;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    hint::black_box,
    time::Instant,
};

#[derive(Parser)]
struct Options {
    #[arg(long, default_value_t = 1024)]
    queries: usize,
    #[arg(long, default_value_t = 4096)]
    aliases: usize,
    #[arg(long, default_value_t = 128)]
    candidates: usize,
    #[arg(long, default_value_t = 1024)]
    dimension: usize,
    #[arg(long, default_value_t = 10)]
    workers: usize,
    #[arg(long, default_value_t = 5)]
    repeats: usize,
}

fn bits(matches: &HashMap<String, (NodeId, f32)>) -> BTreeMap<&str, (NodeId, u32)> {
    matches
        .iter()
        .map(|(target, (id, score))| (target.as_str(), (*id, score.to_bits())))
        .collect()
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    ensure!(
        options.queries > 0
            && options.aliases > 0
            && options.candidates > 0
            && options.dimension > 0
            && options.workers > 0
            && options.repeats > 0
            && options.candidates <= options.aliases,
        "Sizes/workers/repeats must be positive; candidates cannot exceed aliases"
    );
    let seed = 0x1234_5678_9abc_def0u64;
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 40) as f32 / (1u32 << 24) as f32 - 0.5
    };
    let known: HashMap<_, _> = (0..options.aliases)
        .map(|index| {
            (
                format!("symbol_{index}"),
                (0..options.dimension).map(|_| next()).collect::<Vec<_>>(),
            )
        })
        .collect();
    let ids: HashMap<_, _> = (0..options.aliases)
        .map(|index| {
            (
                format!("symbol_{index}"),
                NodeId::from_u128(index as u128 + 1),
            )
        })
        .collect();
    let mut candidates = HashMap::new();
    let mut unknown = HashMap::new();
    for index in 0..options.queries {
        let start = index.wrapping_mul(17) % options.aliases;
        let aliases: Vec<_> = (0..options.candidates)
            .map(|offset| format!("symbol_{}", (start + offset) % options.aliases))
            .collect();
        let target = format!("unresolved_{index}");
        let mut query = known[&aliases[options.candidates / 2]].clone();
        for value in &mut query {
            *value += next() * 0.001;
        }
        unknown.insert(target.clone(), query);
        candidates.insert(target, aliases);
    }
    let pool_one = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let pool_many = rayon::ThreadPoolBuilder::new()
        .num_threads(options.workers)
        .build()?;
    let (expected, comparisons) = reference::resolve_serial(&candidates, &ids, &known, &unknown);
    let expected_bits = bits(&expected);
    for pool in [&pool_one, &pool_many] {
        let warm =
            pool.install(|| semantic_scoring::resolve_targets(&candidates, &ids, &known, &unknown));
        ensure!(
            bits(&warm.targets) == expected_bits && warm.comparisons == comparisons,
            "Warm score/identity mismatch"
        );
    }
    let mut samples = [Vec::new(), Vec::new(), Vec::new()];
    let mut vector_norms = 0;
    for repeat in 0..options.repeats {
        // Rotate method order so the optimized path does not always get warmer caches.
        for offset in 0..3 {
            let method = (repeat + offset) % 3;
            let started = Instant::now();
            let (targets, count) = if method == 0 {
                black_box(reference::resolve_serial(
                    black_box(&candidates),
                    &ids,
                    &known,
                    &unknown,
                ))
            } else {
                let pool = if method == 1 { &pool_one } else { &pool_many };
                let result = black_box(pool.install(|| {
                    semantic_scoring::resolve_targets(
                        black_box(&candidates),
                        &ids,
                        &known,
                        &unknown,
                    )
                }));
                vector_norms = result.vector_norms;
                (result.targets, result.comparisons)
            };
            samples[method].push(started.elapsed().as_secs_f64() * 1000.0);
            ensure!(
                count == comparisons && bits(&targets) == expected_bits,
                "Score/identity mismatch at method {method}, repeat {repeat}"
            );
        }
    }
    let medians: Vec<_> = samples.iter().map(|sample| median(sample)).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fixture": {"seed":seed, "queries":options.queries, "aliases":options.aliases,
                "candidates_per_query":options.candidates, "dimension":options.dimension},
            "workers": options.workers, "debug_assertions": cfg!(debug_assertions),
            "platform": {"os":std::env::consts::OS, "arch":std::env::consts::ARCH},
            "comparisons":comparisons, "cached_vector_norms":vector_norms,
            "matches":expected.len(), "bit_identical":true,
            "samples_ms": {"serial_reference":samples[0], "cached_norms_one_worker":samples[1], "cached_norms_parallel":samples[2]},
            "median_ms": {"serial_reference":medians[0], "cached_norms_one_worker":medians[1], "cached_norms_parallel":medians[2]},
            "median_speedup": {"cached_norms_one_worker":medians[0]/medians[1], "cached_norms_parallel":medians[0]/medians[2]},
            "result_fingerprint":codegraph_core::artifact_cache::fingerprint(&expected_bits)?,
            "scope":"Synthetic CPU scoring only; excludes inference, candidate selection, database and indexing startup"
        }))?
    );
    Ok(())
}
