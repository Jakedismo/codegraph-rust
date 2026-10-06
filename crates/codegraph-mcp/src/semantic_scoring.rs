// ABOUTME: Scores semantic relationship candidates with cached norms and bounded parallel work.
// ABOUTME: Preserves scalar cosine arithmetic, candidate order, thresholds and ambiguous ties.

use codegraph_core::NodeId;
use rayon::prelude::*;
use std::collections::HashMap;

pub(crate) struct ScoringResult {
    pub targets: HashMap<String, (NodeId, f32)>,
    pub comparisons: usize,
    pub vector_norms: usize,
}

struct PreparedVector<'a> {
    values: &'a [f32],
    norm: f32,
}

impl<'a> PreparedVector<'a> {
    fn new(values: &'a [f32]) -> Self {
        Self {
            values,
            norm: values.iter().map(|value| value * value).sum::<f32>().sqrt(),
        }
    }

    fn cosine(&self, other: &Self) -> f32 {
        if self.values.len() != other.values.len() || self.norm == 0.0 || other.norm == 0.0 {
            return 0.0;
        }
        // Keep the original summation and division order. Normalizing each element
        // or reducing the dot product in parallel can change threshold/tie decisions.
        let dot: f32 = self
            .values
            .iter()
            .zip(other.values)
            .map(|(a, b)| a * b)
            .sum();
        dot / (self.norm * other.norm)
    }
}

/// Call within the indexer's configured Rayon pool; only independent targets run in parallel.
pub(crate) fn resolve_targets(
    candidates_by_target: &HashMap<String, Vec<String>>,
    candidate_symbols: &HashMap<String, NodeId>,
    known: &HashMap<String, Vec<f32>>,
    unknown: &HashMap<String, Vec<f32>>,
) -> ScoringResult {
    let prepared: HashMap<_, _> = known
        .par_iter()
        .map(|(alias, vector)| (alias.as_str(), PreparedVector::new(vector)))
        .collect();
    let results: Vec<_> = candidates_by_target
        .par_iter()
        .filter_map(|(target, candidates)| {
            let query = PreparedVector::new(unknown.get(target)?);
            let mut comparisons = 0;
            let mut best = 0.75f32;
            let mut winner = None;
            let mut tied = false;
            // The catalog's candidate order is significant for the existing epsilon rule.
            for alias in candidates {
                let Some(vector) = prepared.get(alias.as_str()) else {
                    continue;
                };
                comparisons += 1;
                let score = query.cosine(vector);
                let id = candidate_symbols[alias];
                if score > best + 1e-6 {
                    best = score;
                    winner = Some(id);
                    tied = false;
                } else if (score - best).abs() <= 1e-6 && winner.is_some_and(|winner| winner != id)
                {
                    tied = true;
                }
            }
            let result = if tied {
                None
            } else {
                winner.map(|id| (id, best))
            };
            Some((target, result, comparisons))
        })
        .collect();
    let comparisons = results.iter().map(|(_, _, count)| count).sum();
    let vector_norms = prepared.len() + results.len();
    let targets = results
        .into_iter()
        .filter_map(|(target, result, _)| result.map(|value| (target.clone(), value)))
        .collect();
    ScoringResult {
        targets,
        comparisons,
        vector_norms,
    }
}

#[cfg(test)]
#[path = "../tests/support/semantic_scoring_reference.rs"]
mod reference;

#[cfg(test)]
mod tests {
    use super::*;

    fn vector_with_cosine(score: f32) -> Vec<f32> {
        vec![score, (1.0 - score * score).sqrt()]
    }

    #[test]
    fn matches_serial_scores_and_ambiguity_at_threshold_and_tie_boundaries() {
        let known: HashMap<String, Vec<f32>> = [
            ("exact", vec![1.0, 0.0]),
            ("same_node", vec![1.0, 0.0]),
            ("different_node", vec![1.0, 0.0]),
            ("below", vector_with_cosine(0.749999)),
            ("threshold", vector_with_cosine(0.75)),
            ("near_threshold", vector_with_cosine(0.7500005)),
            ("above", vector_with_cosine(0.750002)),
            ("tie_a", vector_with_cosine(0.9)),
            ("tie_b", vector_with_cosine(0.9000005)),
            ("clear_winner", vector_with_cosine(0.900002)),
            ("zero", vec![0.0, 0.0]),
            ("wrong_dimension", vec![1.0]),
        ]
        .into_iter()
        .map(|(name, vector)| (name.to_owned(), vector))
        .collect();
        let mut ids: HashMap<_, _> = known
            .keys()
            .enumerate()
            .map(|(index, alias)| (alias.clone(), NodeId::from_u128(index as u128 + 1)))
            .collect();
        ids.insert("same_node".into(), ids["exact"]);
        let lists = [
            ("winner", vec!["below", "exact"]),
            ("same_node_aliases", vec!["exact", "same_node"]),
            ("ambiguous", vec!["exact", "different_node"]),
            ("below", vec!["below"]),
            ("threshold", vec!["threshold"]),
            ("epsilon_threshold", vec!["near_threshold"]),
            ("above", vec!["above"]),
            ("epsilon_tie", vec!["tie_a", "tie_b"]),
            (
                "tie_then_clear_winner",
                vec!["tie_a", "tie_b", "clear_winner"],
            ),
            ("reversed_near_tie", vec!["tie_b", "tie_a"]),
            (
                "invalid_vectors",
                vec!["zero", "wrong_dimension", "missing"],
            ),
            ("missing_query", vec!["exact"]),
            ("empty", vec![]),
            ("zero_query", vec!["exact"]),
        ];
        let candidates: HashMap<String, Vec<String>> = lists
            .into_iter()
            .map(|(target, aliases)| {
                (
                    target.into(),
                    aliases.into_iter().map(str::to_owned).collect(),
                )
            })
            .collect();
        let mut unknown: HashMap<_, _> = candidates
            .keys()
            .map(|target| (target.clone(), vec![1.0, 0.0]))
            .collect();
        unknown.remove("missing_query");
        unknown.insert("zero_query".into(), vec![0.0, 0.0]);
        let (expected, comparisons) =
            reference::resolve_serial(&candidates, &ids, &known, &unknown);
        assert_eq!(expected.len(), 4);
        assert!(expected.contains_key("winner"));
        assert!(expected.contains_key("same_node_aliases"));
        assert!(expected.contains_key("above"));
        assert!(expected.contains_key("tie_then_clear_winner"));
        for workers in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let actual = pool.install(|| resolve_targets(&candidates, &ids, &known, &unknown));
            assert_eq!(actual.comparisons, comparisons);
            assert_eq!(actual.vector_norms, known.len() + unknown.len());
            assert_eq!(actual.targets.len(), expected.len());
            for (target, (id, score)) in &expected {
                let result = actual.targets[target];
                assert_eq!(result.0, *id, "{target}: {workers} workers");
                assert_eq!(
                    result.1.to_bits(),
                    score.to_bits(),
                    "{target}: {workers} workers"
                );
            }
        }
    }

    #[test]
    fn cached_norms_preserve_scalar_cosines_for_provider_sized_vectors() {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 40) as f32 / (1u32 << 24) as f32 - 0.5
        };
        for dimension in [384, 768, 1024, 1536] {
            for _ in 0..32 {
                let query: Vec<_> = (0..dimension).map(|_| next()).collect();
                let candidate: Vec<_> = (0..dimension).map(|_| next()).collect();
                let expected = reference::cosine(&query, &candidate);
                let actual = PreparedVector::new(&query).cosine(&PreparedVector::new(&candidate));
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
    }
}
