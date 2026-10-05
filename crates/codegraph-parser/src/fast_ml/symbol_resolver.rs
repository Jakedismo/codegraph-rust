// ABOUTME: Deterministic lexical symbol resolution without global cross-project state.
// ABOUTME: Uses an inverted character index to shortlist similar names.

use codegraph_core::{EdgeRelationship, EdgeType, ExtractionResult};
use std::collections::HashMap;
use tracing::debug;

/// Deterministic, file-local lexical resolver with an inverted character index.
pub struct SymbolResolver {
    symbols: std::collections::BTreeSet<String>,
    candidates: HashMap<char, std::collections::BTreeSet<String>>,
    min_similarity_threshold: f32,
}

impl SymbolResolver {
    pub fn new() -> Self {
        Self {
            symbols: Default::default(),
            candidates: Default::default(),
            min_similarity_threshold: 0.7,
        }
    }

    pub fn index_symbols(&mut self, result: &ExtractionResult) {
        for node in &result.nodes {
            let name = node.name.to_string();
            if name.is_empty() || !self.symbols.insert(name.clone()) {
                continue;
            }
            for character in name.to_lowercase().chars() {
                self.candidates
                    .entry(character)
                    .or_default()
                    .insert(name.clone());
            }
        }
    }

    /// Resolve similar symbols for unmatched references (100-500μs per query)
    pub fn resolve_symbols(&self, mut result: ExtractionResult) -> ExtractionResult {
        // Skip resolver for very small files to avoid overhead
        if result.nodes.len() < 2 {
            return result;
        }
        let mut new_edges = Vec::new();
        let max_edges_per_file = 10usize;
        let mut added = 0usize;

        // Find edges pointing to symbols that don't exist in nodes
        let existing_symbols: HashMap<String, _> = result
            .nodes
            .iter()
            .map(|n| (n.name.to_string(), n.id))
            .collect();

        for edge in &result.edges {
            // Check if target symbol exists
            if !existing_symbols.contains_key(&edge.to) && !edge.to.is_empty() {
                // Try to find similar symbols using LSH
                if let Some(similar) = self.find_similar_symbol(&edge.to) {
                    // Create edge to similar symbol
                    let mut metadata = HashMap::new();
                    metadata.insert("original_target".to_string(), edge.to.clone());
                    metadata.insert("resolved_target".to_string(), similar.clone());
                    metadata.insert(
                        "fast_ml_enhancement".to_string(),
                        "lexical_resolution".to_string(),
                    );

                    new_edges.push(EdgeRelationship {
                        from: edge.from,
                        to: similar,
                        edge_type: EdgeType::Uses,
                        metadata,
                        span: None,
                    });
                    added += 1;
                    if added >= max_edges_per_file {
                        break;
                    }
                }
            }
        }

        let enhancement_count = new_edges.len();
        if enhancement_count > 0 {
            debug!(
                "⚡ SymbolResolver: Resolved {} symbols using LSH",
                enhancement_count
            );
            result.edges.extend(new_edges);
        }

        result
    }

    fn find_similar_symbol(&self, symbol: &str) -> Option<String> {
        let mut candidates = std::collections::BTreeSet::new();
        for character in symbol.to_lowercase().chars() {
            if let Some(names) = self.candidates.get(&character) {
                candidates.extend(names.iter());
            }
        }
        let mut best = None;
        let mut best_score = self.min_similarity_threshold;
        for name in candidates {
            let score = Self::string_similarity(symbol, name);
            if score >= best_score && (score > best_score || best.is_none()) {
                best_score = score;
                best = Some(name.clone());
            }
        }
        best
    }

    /// Calculate string similarity (simple but fast)
    fn string_similarity(s1: &str, s2: &str) -> f32 {
        if s1 == s2 {
            return 1.0;
        }

        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();

        if s1_lower == s2_lower {
            return 0.95;
        }

        // Check containment
        if s1_lower.contains(&s2_lower) || s2_lower.contains(&s1_lower) {
            return 0.8;
        }

        // Simple character overlap
        let chars1: std::collections::HashSet<char> = s1_lower.chars().collect();
        let chars2: std::collections::HashSet<char> = s2_lower.chars().collect();
        let intersection = chars1.intersection(&chars2).count();
        let union = chars1.union(&chars2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::CodeNode;

    #[test]
    fn test_symbol_similarity() {
        // Exact match
        let sim1 = SymbolResolver::string_similarity("HashMap", "HashMap");
        assert_eq!(sim1, 1.0);

        // Case-insensitive match returns 0.95
        let sim2 = SymbolResolver::string_similarity("HashMap", "hashmap");
        assert_eq!(sim2, 0.95);

        // Character overlap: {h,a,s} / {h,a,s,m,p,e,t} = 3/7 ≈ 0.43
        let sim3 = SymbolResolver::string_similarity("HashMap", "HashSet");
        assert!(sim3 > 0.4 && sim3 < 0.5);
    }

    #[test]
    fn test_symbol_resolution() {
        let mut resolver = SymbolResolver::new();

        // Create multiple symbols to ensure LSH has enough data for bucket distribution
        let symbols = ["HashMap", "HashSet", "BTreeMap", "BTreeSet", "LinkedList"];
        let nodes: Vec<_> = symbols
            .iter()
            .map(|name| {
                let mut node = CodeNode::new_test();
                node.name = (*name).into();
                node
            })
            .collect();

        let result = ExtractionResult {
            nodes,
            edges: vec![],
        };

        // Test that indexing doesn't crash
        resolver.index_symbols(&result);

        // Verify symbols were indexed
        assert_eq!(resolver.symbols.len(), 5, "All 5 symbols should be indexed");
        assert_eq!(
            resolver.find_similar_symbol("HashMap"),
            Some("HashMap".to_string())
        );
        resolver.index_symbols(&result);
        assert_eq!(
            resolver.symbols.len(),
            5,
            "Repeated indexing must not grow state"
        );

        // LSH lookup is probabilistic - just verify it doesn't crash
        let _ = resolver.find_similar_symbol("HashMap");
        let _ = resolver.find_similar_symbol("UnknownSymbol");
    }
}
