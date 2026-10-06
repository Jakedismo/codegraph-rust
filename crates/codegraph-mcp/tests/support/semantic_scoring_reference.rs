// ABOUTME: Frozen scalar semantic resolver used for score-equivalence tests and timing comparisons.
// ABOUTME: Keeps the pre-optimization arithmetic, thresholds and tie logic independent of production.

use codegraph_core::NodeId;
use std::collections::HashMap;

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn resolve_serial(
    candidates_by_target: &HashMap<String, Vec<String>>,
    candidate_symbols: &HashMap<String, NodeId>,
    known: &HashMap<String, Vec<f32>>,
    unknown: &HashMap<String, Vec<f32>>,
) -> (HashMap<String, (NodeId, f32)>, usize) {
    let mut targets = HashMap::new();
    let mut comparisons = 0;
    for (target, candidates) in candidates_by_target {
        let Some(query) = unknown.get(target) else {
            continue;
        };
        let mut best = 0.75f32;
        let mut winner = None;
        let mut tied = false;
        for alias in candidates {
            let Some(vector) = known.get(alias) else {
                continue;
            };
            comparisons += 1;
            let score = cosine(query, vector);
            let id = candidate_symbols[alias];
            if score > best + 1e-6 {
                best = score;
                winner = Some(id);
                tied = false;
            } else if (score - best).abs() <= 1e-6 && winner.is_some_and(|winner| winner != id) {
                tied = true;
            }
        }
        if !tied && let Some(id) = winner {
            targets.insert(target.clone(), (id, best));
        }
    }
    (targets, comparisons)
}
