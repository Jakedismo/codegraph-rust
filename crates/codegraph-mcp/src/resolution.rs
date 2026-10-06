// ABOUTME: Deterministic, context-aware symbol catalog used before semantic inference.
// ABOUTME: Preserves ambiguous aliases and indexes lexical candidates by character trigrams.

use codegraph_core::{CodeNode, EdgeRelationship, NodeId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone)]
struct Symbol {
    id: NodeId,
    file: String,
    qualified: String,
}

struct LexicalCandidate {
    alias: String,
    normalized: String,
    #[cfg(any(test, feature = "ai-enhanced"))]
    gram_count: usize,
    characters: usize,
}

pub(crate) struct SymbolCatalog {
    aliases: BTreeMap<String, Vec<NodeId>>,
    symbols: HashMap<NodeId, Symbol>,
    lexical: Vec<LexicalCandidate>,
    trigrams: HashMap<String, Vec<usize>>,
}

pub(crate) fn aliases(node: &CodeNode) -> BTreeSet<String> {
    let name = node.name.to_string();
    let mut names = BTreeSet::from([name.clone(), format!("{}::{name}", node.location.file_path)]);
    if let Some(qualified) = node.metadata.attributes.get("qualified_name") {
        names.insert(qualified.clone());
    }
    if let Some(kind) = &node.node_type {
        names.insert(format!("{kind:?}::{name}"));
    }
    if let Some(short) = name.rsplit("::").next() {
        names.insert(short.to_string());
    }
    for attribute in ["method_of", "implements_trait"] {
        if let Some(owner) = node.metadata.attributes.get(attribute) {
            names.insert(format!("{owner}::{name}"));
        }
    }
    names
}

fn trigrams(name: &str) -> BTreeSet<String> {
    let chars: Vec<_> = name.chars().collect();
    if chars.len() < 3 {
        return BTreeSet::from([name.to_string()]);
    }
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}

impl SymbolCatalog {
    pub(crate) fn new(nodes: &[CodeNode]) -> Self {
        let mut catalog = Self {
            aliases: BTreeMap::new(),
            symbols: HashMap::new(),
            lexical: Vec::new(),
            trigrams: HashMap::new(),
        };
        for node in nodes {
            catalog.symbols.insert(
                node.id,
                Symbol {
                    id: node.id,
                    file: node.location.file_path.clone(),
                    qualified: node
                        .metadata
                        .attributes
                        .get("qualified_name")
                        .cloned()
                        .unwrap_or_else(|| node.name.to_string()),
                },
            );
            for alias in aliases(node) {
                let ids = catalog.aliases.entry(alias).or_default();
                if !ids.contains(&node.id) {
                    ids.push(node.id);
                }
            }
        }
        for ids in catalog.aliases.values_mut() {
            ids.sort();
        }
        for alias in catalog.aliases.keys() {
            let normalized = alias.to_lowercase();
            let index = catalog.lexical.len();
            let grams = trigrams(&normalized);
            for gram in &grams {
                catalog
                    .trigrams
                    .entry(gram.clone())
                    .or_default()
                    .push(index);
            }
            catalog.lexical.push(LexicalCandidate {
                alias: alias.clone(),
                characters: normalized.chars().count(),
                normalized,
                #[cfg(any(test, feature = "ai-enhanced"))]
                gram_count: grams.len(),
            });
        }
        catalog
    }

    /// Legacy consumers receive only aliases with exactly one target.
    pub(crate) fn unique_aliases(&self) -> HashMap<String, NodeId> {
        self.aliases
            .iter()
            .filter(|(_, ids)| ids.len() == 1)
            .map(|(alias, ids)| (alias.clone(), ids[0]))
            .collect()
    }

    pub(crate) fn ambiguous(&self, alias: &str) -> bool {
        self.aliases.get(alias).is_some_and(|ids| ids.len() > 1)
    }

    #[cfg(any(test, feature = "ai-enhanced"))]
    pub(crate) fn semantic_candidates(&self, target: &str, limit: usize) -> Vec<String> {
        let target = target.to_lowercase();
        let grams = trigrams(&target);
        let target_len = target.chars().count().max(1) as f32;
        self.candidate_indices(&grams, limit)
            .into_iter()
            .filter_map(|(index, common)| {
                let candidate = &self.lexical[index];
                if self.aliases[&candidate.alias].len() != 1 {
                    return None;
                }
                let name_len = candidate.characters.max(1) as f32;
                if (target_len / name_len).min(name_len / target_len) < 0.5 {
                    return None;
                }
                let union = grams.len() + candidate.gram_count - common;
                (common as f32 / union.max(1) as f32 >= 0.2).then(|| candidate.alias.clone())
            })
            .collect()
    }

    fn choose(&self, ids: &[NodeId], from: NodeId) -> Option<NodeId> {
        if ids.len() == 1 {
            return ids.first().copied();
        }
        let source = self.symbols.get(&from)?;
        let local: Vec<_> = ids
            .iter()
            .filter_map(|id| self.symbols.get(id))
            .filter(|symbol| symbol.file == source.file)
            .collect();
        if local.len() == 1 {
            return Some(local[0].id);
        }
        let source_scope = source.qualified.rsplit_once("::").map(|(scope, _)| scope)?;
        let scoped: Vec<_> = local
            .iter()
            .filter(|symbol| {
                symbol.qualified.rsplit_once("::").map(|(scope, _)| scope) == Some(source_scope)
            })
            .collect();
        (scoped.len() == 1).then(|| scoped[0].id)
    }

    pub(crate) fn exact(&self, edge: &EdgeRelationship, variants: &[String]) -> Option<NodeId> {
        if let Some(id) = edge
            .metadata
            .get("target_node_id")
            .and_then(|id| NodeId::parse_str(id).ok())
            && self.symbols.contains_key(&id)
        {
            return Some(id);
        }
        for alias in std::iter::once(edge.to.as_str()).chain(variants.iter().map(String::as_str)) {
            if let Some(ids) = self.aliases.get(alias)
                && let Some(id) = self.choose(ids, edge.from)
            {
                return Some(id);
            }
        }
        None
    }

    /// A deterministic shortlist; a zero limit retains every overlapping candidate.
    fn candidate_indices(&self, grams: &BTreeSet<String>, limit: usize) -> Vec<(usize, usize)> {
        let mut counts = HashMap::<usize, usize>::new();
        for gram in grams {
            if let Some(indices) = self.trigrams.get(gram) {
                for index in indices {
                    *counts.entry(*index).or_default() += 1;
                }
            }
        }
        let mut ranked: Vec<_> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
            .into_iter()
            .take(if limit == 0 { usize::MAX } else { limit })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn candidates(&self, target: &str, limit: usize) -> Vec<String> {
        self.candidate_indices(&trigrams(&target.to_lowercase()), limit)
            .into_iter()
            .map(|(index, _)| self.lexical[index].alias.clone())
            .collect()
    }

    pub(crate) fn fuzzy(&self, edge: &EdgeRelationship) -> Option<(NodeId, f64)> {
        let target = edge.to.to_lowercase();
        let target_length = target.chars().count();
        let scorer = rapidfuzz::distance::levenshtein::BatchComparator::new(target.chars());
        let mut best = 0.85f64;
        let mut matches = HashSet::new();
        for (index, _) in self.candidate_indices(&trigrams(&target), 0) {
            let candidate = &self.lexical[index];
            // Edit-distance similarity cannot exceed the ratio of the shorter length.
            if (target_length.min(candidate.characters) as f64)
                / (target_length.max(candidate.characters).max(1) as f64)
                + f64::EPSILON
                < best
            {
                continue;
            }
            let Some(score) = scorer.normalized_similarity_with_args(
                candidate.normalized.chars(),
                &rapidfuzz::distance::levenshtein::Args::default().score_cutoff(best),
            ) else {
                continue;
            };
            if score < best {
                continue;
            }
            let Some(id) = self.choose(&self.aliases[&candidate.alias], edge.from) else {
                continue;
            };
            if score > best {
                best = score;
                matches.clear();
            }
            matches.insert(id);
        }
        (matches.len() == 1).then(|| (*matches.iter().next().unwrap(), best))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{EdgeType, Language, Location, NodeType};

    fn node(file: &str, name: &str) -> CodeNode {
        let mut node = CodeNode::new(
            name.to_string(),
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: file.into(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
        );
        node.set_deterministic_id("test");
        node
    }
    fn edge(from: NodeId, to: &str) -> EdgeRelationship {
        EdgeRelationship {
            from,
            to: to.into(),
            edge_type: EdgeType::Calls,
            metadata: Default::default(),
            span: None,
        }
    }

    #[test]
    fn ambiguous_names_are_scoped_and_direct_definitions_remain_authoritative() {
        let a = node("a.rs", "duplicate");
        let b = node("b.rs", "duplicate");
        let caller = node("b.rs", "caller");
        for nodes in [
            vec![a.clone(), b.clone(), caller.clone()],
            vec![caller.clone(), b.clone(), a.clone()],
        ] {
            let catalog = SymbolCatalog::new(&nodes);
            assert!(!catalog.unique_aliases().contains_key("duplicate"));
            assert_eq!(
                catalog.exact(&edge(caller.id, "duplicate"), &[]),
                Some(b.id)
            );
            assert_eq!(
                catalog.exact(&edge(NodeId::new_v4(), "duplicate"), &[]),
                None
            );
            let mut direct = edge(caller.id, "duplicate");
            direct
                .metadata
                .insert("target_node_id".into(), a.id.to_string());
            assert_eq!(catalog.exact(&direct, &[]), Some(a.id));
        }
    }

    #[test]
    fn lexical_candidates_are_deterministic_and_unicode_safe() {
        let target = node("a.rs", "calculate_result");
        let catalog = SymbolCatalog::new(&[target.clone(), node("b.rs", "unrelated")]);
        assert_eq!(
            catalog
                .fuzzy(&edge(target.id, "calculate_reslt"))
                .map(|(id, _)| id),
            Some(target.id)
        );
        assert!(!catalog.candidates("calculate", 2).is_empty());
        assert!(!catalog.semantic_candidates("calculate_reslt", 0).is_empty());
        assert!(catalog.candidates("🦀", 0).is_empty());
    }
}
