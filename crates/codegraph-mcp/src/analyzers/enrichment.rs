// ABOUTME: Enriches indexed nodes and edges with API surface, docs, and architecture signals
// ABOUTME: Adds lightweight derived edges and metadata without requiring database queries

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Language, NodeId, NodeType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EnrichmentStats {
    pub docs_attached: usize,
    pub api_marked: usize,
    pub export_edges_added: usize,
    pub uses_edges_derived: usize,
    pub package_cycles_detected: usize,
}

pub fn apply_basic_enrichment(
    project_root: &Path,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<EnrichmentStats> {
    let mut stats = EnrichmentStats::default();

    let package_roots = package_roots(nodes);
    let mut file_cache: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for node in nodes.iter_mut() {
        if node.language != Some(Language::Rust) {
            continue;
        }

        let lines = file_cache
            .entry(node.location.file_path.clone())
            .or_insert_with(|| read_lines(project_root, &node.location.file_path));

        if let Some(doc) = rust_doc_comment_block(lines, node.location.line) {
            node.metadata.attributes.insert("doc".to_string(), doc);
            node.metadata
                .attributes
                .insert("analyzer".to_string(), "docs_contracts".to_string());
            node.metadata
                .attributes
                .insert("analyzer_confidence".to_string(), "0.8".to_string());
            stats.docs_attached += 1;
        }

        if let Some(visibility) = rust_visibility(lines, node.location.line) {
            node.metadata
                .attributes
                .insert("api_visibility".to_string(), visibility.to_string());
            stats.api_marked += 1;
        }
    }

    let mut export_edges: Vec<EdgeRelationship> = Vec::new();
    for node in nodes.iter() {
        if node.language != Some(Language::Rust) {
            continue;
        }
        if node
            .metadata
            .attributes
            .get("api_visibility")
            .map(|v| v.as_str())
            != Some("public")
        {
            continue;
        }

        let Some((package_id, _root)) = package_for_file(&package_roots, &node.location.file_path)
        else {
            continue;
        };

        export_edges.push(EdgeRelationship {
            from: package_id,
            to: node
                .metadata
                .attributes
                .get("qualified_name")
                .cloned()
                .unwrap_or_else(|| node.name.to_string()),
            edge_type: EdgeType::Other("exports".to_string()),
            metadata: std::collections::HashMap::from([
                ("analyzer".to_string(), "api_surface".to_string()),
                ("analyzer_confidence".to_string(), "0.9".to_string()),
            ]),
            span: None,
        });
    }
    stats.export_edges_added = export_edges.len();
    edges.extend(export_edges);

    for edge in edges.iter_mut() {
        if edge
            .metadata
            .get("analyzer")
            .map(|v| v.as_str())
            != Some("lsp_definition")
        {
            continue;
        }
        if edge.edge_type == EdgeType::References {
            edge.edge_type = EdgeType::Uses;
            stats.uses_edges_derived += 1;
        }
    }

    stats.package_cycles_detected = count_package_cycles(nodes, edges);

    Ok(stats)
}

fn read_lines(project_root: &Path, file_path: &str) -> Vec<String> {
    let p = PathBuf::from(file_path);
    let full = if p.is_absolute() {
        p
    } else {
        project_root.join(p)
    };
    std::fs::read_to_string(full)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

fn rust_doc_comment_block(lines: &[String], line_1based: u32) -> Option<String> {
    let line_idx = (line_1based as usize).saturating_sub(1);
    if line_idx == 0 || line_idx > lines.len() {
        return None;
    }

    let mut collected: Vec<String> = Vec::new();
    let mut idx = line_idx.saturating_sub(1);
    loop {
        let l = lines.get(idx)?;
        let trimmed = l.trim_start();
        if let Some(rest) = trimmed.strip_prefix("///") {
            collected.push(rest.trim_start().to_string());
        } else if trimmed.is_empty() && !collected.is_empty() {
            break;
        } else {
            break;
        }

        if idx == 0 {
            break;
        }
        idx -= 1;
    }

    if collected.is_empty() {
        return None;
    }

    collected.reverse();
    Some(collected.join("\n"))
}

fn rust_visibility(lines: &[String], line_1based: u32) -> Option<&'static str> {
    let idx = (line_1based as usize).saturating_sub(1);
    let line = lines.get(idx)?;
    let trimmed = line.trim_start();

    if trimmed.starts_with("pub ") || trimmed.starts_with("pub(") {
        return Some("public");
    }
    Some("private")
}

fn package_roots(nodes: &[CodeNode]) -> Vec<(PathBuf, NodeId)> {
    let mut out: Vec<(PathBuf, NodeId)> = Vec::new();
    for node in nodes {
        if node.node_type != Some(NodeType::Other("package".to_string())) {
            continue;
        }
        if !node.location.file_path.ends_with("Cargo.toml") {
            continue;
        }
        let manifest = PathBuf::from(&node.location.file_path);
        if let Some(dir) = manifest.parent() {
            out.push((dir.to_path_buf(), node.id));
        }
    }
    out.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));
    out
}

fn package_for_file(
    package_roots: &[(PathBuf, NodeId)],
    file_path: &str,
) -> Option<(NodeId, PathBuf)> {
    let p = PathBuf::from(file_path);
    for (root, id) in package_roots {
        if p.starts_with(root) {
            return Some((*id, root.clone()));
        }
    }
    None
}

fn count_package_cycles(nodes: &[CodeNode], edges: &[EdgeRelationship]) -> usize {
    let mut package_names: std::collections::HashMap<NodeId, String> = std::collections::HashMap::new();
    for n in nodes {
        if n.node_type == Some(NodeType::Other("package".to_string())) {
            package_names.insert(n.id, n.name.to_string());
        }
    }

    let mut adj: std::collections::HashMap<NodeId, Vec<NodeId>> = std::collections::HashMap::new();
    for e in edges {
        if e.edge_type != EdgeType::Other("depends_on".to_string()) {
            continue;
        }
        if !package_names.contains_key(&e.from) {
            continue;
        }
        let Some((&to_id, _)) = package_names
            .iter()
            .find(|(_, name)| name.as_str() == e.to.as_str())
        else {
            continue;
        };
        adj.entry(e.from).or_default().push(to_id);
    }

    let mut index: usize = 0;
    let mut stack: Vec<NodeId> = Vec::new();
    let mut on_stack: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    let mut indices: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
    let mut lowlink: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
    let mut scc_count = 0usize;

    fn strongconnect(
        v: NodeId,
        index: &mut usize,
        indices: &mut std::collections::HashMap<NodeId, usize>,
        lowlink: &mut std::collections::HashMap<NodeId, usize>,
        stack: &mut Vec<NodeId>,
        on_stack: &mut std::collections::HashSet<NodeId>,
        adj: &std::collections::HashMap<NodeId, Vec<NodeId>>,
        scc_count: &mut usize,
    ) {
        indices.insert(v, *index);
        lowlink.insert(v, *index);
        *index += 1;
        stack.push(v);
        on_stack.insert(v);

        for w in adj.get(&v).cloned().unwrap_or_default() {
            if !indices.contains_key(&w) {
                strongconnect(
                    w, index, indices, lowlink, stack, on_stack, adj, scc_count,
                );
                let lw = *lowlink.get(&w).unwrap();
                let lv = lowlink.get_mut(&v).unwrap();
                *lv = (*lv).min(lw);
            } else if on_stack.contains(&w) {
                let iw = *indices.get(&w).unwrap();
                let lv = lowlink.get_mut(&v).unwrap();
                *lv = (*lv).min(iw);
            }
        }

        if lowlink.get(&v) == indices.get(&v) {
            let mut members = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack.remove(&w);
                members.push(w);
                if w == v {
                    break;
                }
            }
            if members.len() > 1 {
                *scc_count += 1;
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
                &mut scc_count,
            );
        }
    }

    scc_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::Location;

    #[test]
    fn rust_doc_comments_attach_to_nodes() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let file = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(file.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &file,
            "/// Hello\n/// world\npub fn foo() {}\n",
        )
        .expect("write");

        let mut nodes = vec![CodeNode::new(
            "foo",
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: file.to_string_lossy().to_string(),
                line: 3,
                column: 0,
                end_line: Some(3),
                end_column: Some(0),
            },
        )];
        let mut edges = Vec::new();
        let stats = apply_basic_enrichment(temp.path(), &mut nodes, &mut edges).expect("enrich");
        assert_eq!(stats.docs_attached, 1);
        assert_eq!(
            nodes[0].metadata.attributes.get("doc").map(|s| s.as_str()),
            Some("Hello\nworld")
        );
        assert_eq!(
            nodes[0]
                .metadata
                .attributes
                .get("api_visibility")
                .map(|s| s.as_str()),
            Some("public")
        );
    }
}

