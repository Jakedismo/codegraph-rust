// ABOUTME: Derives package-level dependency cycles and configured boundary violations during indexing
// ABOUTME: Produces `violates_boundary` edges and architecture metrics without querying storage

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, NodeId, NodeType};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ArchitectureStats {
    pub package_cycles_detected: usize,
    pub boundary_violations_added: usize,
}

#[derive(Debug, Default, Deserialize)]
struct BoundaryConfig {
    #[serde(default)]
    deny: Vec<BoundaryDenyRule>,
}

#[derive(Debug, Deserialize)]
struct BoundaryDenyRule {
    from: String,
    to: String,
    #[serde(default)]
    reason: Option<String>,
}

pub fn analyze_architecture(
    project_root: &Path,
    nodes: &[CodeNode],
    edges: &mut Vec<EdgeRelationship>,
) -> Result<ArchitectureStats> {
    let mut stats = ArchitectureStats::default();

    stats.package_cycles_detected = count_package_cycles(nodes, edges);

    let boundary = read_boundary_config(project_root).unwrap_or_default();
    if boundary.deny.is_empty() {
        return Ok(stats);
    }

    let mut packages_by_name: HashMap<String, NodeId> = HashMap::new();
    for n in nodes {
        if n.node_type == Some(NodeType::Other("package".to_string())) {
            packages_by_name.insert(n.name.to_string(), n.id);
        }
    }

    let mut depends: HashSet<(NodeId, String)> = HashSet::new();
    for e in edges.iter() {
        if e.edge_type == EdgeType::Other("depends_on".to_string()) {
            depends.insert((e.from, e.to.to_string()));
        }
    }

    for rule in boundary.deny {
        let Some(&from_id) = packages_by_name.get(&rule.from) else {
            continue;
        };
        if !depends.contains(&(from_id, rule.to.clone())) {
            continue;
        }

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("analyzer".to_string(), "architecture_boundary".to_string());
        metadata.insert("analyzer_confidence".to_string(), "1.0".to_string());
        if let Some(reason) = rule.reason {
            metadata.insert("boundary_reason".to_string(), reason);
        }
        metadata.insert(
            "boundary_rule".to_string(),
            format!("deny:{}->{}", rule.from, rule.to),
        );

        edges.push(EdgeRelationship {
            from: from_id,
            to: rule.to,
            edge_type: EdgeType::Other("violates_boundary".to_string()),
            metadata,
            span: None,
        });
        stats.boundary_violations_added += 1;
    }

    Ok(stats)
}

fn read_boundary_config(project_root: &Path) -> Option<BoundaryConfig> {
    let path = project_root.join("codegraph.boundaries.toml");
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

fn count_package_cycles(nodes: &[CodeNode], edges: &[EdgeRelationship]) -> usize {
    let mut package_names: HashMap<NodeId, String> = HashMap::new();
    for n in nodes {
        if n.node_type == Some(NodeType::Other("package".to_string())) {
            package_names.insert(n.id, n.name.to_string());
        }
    }

    let mut package_id_by_name: HashMap<String, NodeId> = HashMap::new();
    for (id, name) in package_names.iter() {
        package_id_by_name.insert(name.clone(), *id);
    }

    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for e in edges {
        if e.edge_type != EdgeType::Other("depends_on".to_string()) {
            continue;
        }
        if !package_names.contains_key(&e.from) {
            continue;
        }
        let Some(&to_id) = package_id_by_name.get(e.to.as_str()) else {
            continue;
        };
        adj.entry(e.from).or_default().push(to_id);
    }

    let mut index: usize = 0;
    let mut stack: Vec<NodeId> = Vec::new();
    let mut on_stack: HashSet<NodeId> = HashSet::new();
    let mut indices: HashMap<NodeId, usize> = HashMap::new();
    let mut lowlink: HashMap<NodeId, usize> = HashMap::new();
    let mut cycles = 0usize;

    fn strongconnect(
        v: NodeId,
        index: &mut usize,
        indices: &mut HashMap<NodeId, usize>,
        lowlink: &mut HashMap<NodeId, usize>,
        stack: &mut Vec<NodeId>,
        on_stack: &mut HashSet<NodeId>,
        adj: &HashMap<NodeId, Vec<NodeId>>,
        cycles: &mut usize,
    ) {
        indices.insert(v, *index);
        lowlink.insert(v, *index);
        *index += 1;
        stack.push(v);
        on_stack.insert(v);

        for w in adj.get(&v).cloned().unwrap_or_default() {
            if !indices.contains_key(&w) {
                strongconnect(w, index, indices, lowlink, stack, on_stack, adj, cycles);
                let lw = *lowlink.get(&w).unwrap();
                let lv = lowlink.get_mut(&v).unwrap();
                *lv = (*lv).min(lw);
            } else if on_stack.contains(&w) {
                let iw = *indices.get(&w).unwrap();
                let lv = lowlink.get_mut(&v).unwrap();
                *lv = (*lv).min(iw);
            }
        }

        if indices.get(&v) == lowlink.get(&v) {
            let mut scc: Vec<NodeId> = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                scc.push(w);
                if w == v {
                    break;
                }
            }
            if scc.len() > 1 {
                *cycles += 1;
            }
        }
    }

    for v in package_names.keys().cloned().collect::<Vec<_>>() {
        if !indices.contains_key(&v) {
            strongconnect(
                v,
                &mut index,
                &mut indices,
                &mut lowlink,
                &mut stack,
                &mut on_stack,
                &adj,
                &mut cycles,
            );
        }
    }

    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, Location};
    use tempfile::tempdir;

    #[test]
    fn boundary_rules_produce_violation_edges() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("codegraph.boundaries.toml"),
            r#"
                [[deny]]
                from = "app"
                to = "lib"
                reason = "app must not depend on lib"
            "#,
        )
        .unwrap();

        let nodes = vec![
            CodeNode::new(
                "app",
                Some(NodeType::Other("package".to_string())),
                Some(Language::Rust),
                Location {
                    file_path: "Cargo.toml".to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
            CodeNode::new(
                "lib",
                Some(NodeType::Other("package".to_string())),
                Some(Language::Rust),
                Location {
                    file_path: "Cargo.toml".to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
        ];

        let app_id = nodes[0].id;
        let mut edges = vec![EdgeRelationship {
            from: app_id,
            to: "lib".to_string(),
            edge_type: EdgeType::Other("depends_on".to_string()),
            metadata: HashMap::new(),
            span: None,
        }];

        let stats = analyze_architecture(dir.path(), &nodes, &mut edges).unwrap();
        assert_eq!(stats.boundary_violations_added, 1);
        assert!(edges.iter().any(|e| {
            e.edge_type == EdgeType::Other("violates_boundary".to_string()) && e.to == "lib"
        }));
    }

    #[test]
    fn cycle_detection_counts_sccs() {
        let dir = tempdir().expect("tempdir");
        let nodes = vec![CodeNode::new_test(), CodeNode::new_test()];
        let a = nodes[0].id;
        let b = nodes[1].id;

        let nodes = vec![
            CodeNode {
                name: "a".into(),
                node_type: Some(NodeType::Other("package".to_string())),
                ..nodes[0].clone()
            },
            CodeNode {
                name: "b".into(),
                node_type: Some(NodeType::Other("package".to_string())),
                ..nodes[1].clone()
            },
        ];

        let mut edges = vec![
            EdgeRelationship {
                from: a,
                to: "b".to_string(),
                edge_type: EdgeType::Other("depends_on".to_string()),
                metadata: HashMap::new(),
                span: None,
            },
            EdgeRelationship {
                from: b,
                to: "a".to_string(),
                edge_type: EdgeType::Other("depends_on".to_string()),
                metadata: HashMap::new(),
                span: None,
            },
        ];

        let stats = analyze_architecture(dir.path(), &nodes, &mut edges).unwrap();
        assert_eq!(stats.package_cycles_detected, 1);
    }
}
