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
    pub reexport_edges_added: usize,
    pub feature_enables_edges_added: usize,
    pub uses_edges_derived: usize,
}

pub fn apply_basic_enrichment(
    project_root: &Path,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<EnrichmentStats> {
    let mut stats = EnrichmentStats::default();

    let package_roots = package_roots(project_root, nodes);
    let feature_ids = feature_ids_by_package(nodes);
    let package_name_by_id: std::collections::HashMap<NodeId, String> = nodes
        .iter()
        .filter(|n| n.node_type == Some(NodeType::Other("package".to_string())))
        .map(|n| (n.id, n.name.to_string()))
        .collect();
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

        if let Some(feature) = cfg_feature(lines, node.location.line) {
            node.metadata
                .attributes
                .insert("cfg_feature".to_string(), feature);
        }
    }

    let mut export_edges: Vec<EdgeRelationship> = Vec::new();
    let mut feature_edges: Vec<EdgeRelationship> = Vec::new();
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

        let Some((package_id, _root)) =
            package_for_file(project_root, &package_roots, &node.location.file_path)
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

        if let Some(feature) = node.metadata.attributes.get("cfg_feature") {
            let package_name = package_name_by_id
                .get(&package_id)
                .cloned()
                .unwrap_or_default();
            if let Some(&feature_id) = feature_ids.get(&(package_name, feature.to_string())) {
                feature_edges.push(EdgeRelationship {
                    from: feature_id,
                    to: node
                        .metadata
                        .attributes
                        .get("qualified_name")
                        .cloned()
                        .unwrap_or_else(|| node.name.to_string()),
                    edge_type: EdgeType::Other("enables".to_string()),
                    metadata: std::collections::HashMap::from([
                        ("analyzer".to_string(), "api_surface".to_string()),
                        ("analyzer_confidence".to_string(), "0.8".to_string()),
                    ]),
                    span: None,
                });
            }
        }
    }
    stats.export_edges_added = export_edges.len();
    edges.extend(export_edges);
    stats.feature_enables_edges_added = feature_edges.len();
    edges.extend(feature_edges);

    let mut reexport_edges: Vec<EdgeRelationship> = Vec::new();
    let mut seen_reexport: std::collections::HashSet<(NodeId, String)> =
        std::collections::HashSet::new();
    for (file_path, lines) in file_cache.iter() {
        let Some((package_id, _root)) = package_for_file(project_root, &package_roots, file_path)
        else {
            continue;
        };
        for target in pub_use_reexports(lines) {
            if seen_reexport.insert((package_id, target.clone())) {
                reexport_edges.push(EdgeRelationship {
                    from: package_id,
                    to: target,
                    edge_type: EdgeType::Other("reexports".to_string()),
                    metadata: std::collections::HashMap::from([
                        ("analyzer".to_string(), "api_surface".to_string()),
                        ("analyzer_confidence".to_string(), "0.7".to_string()),
                    ]),
                    span: None,
                });
            }
        }
    }
    stats.reexport_edges_added = reexport_edges.len();
    edges.extend(reexport_edges);

    for edge in edges.iter_mut() {
        if edge.metadata.get("analyzer").map(|v| v.as_str()) != Some("lsp_definition") {
            continue;
        }
        
        // Count all LSP-resolved edges in the metric
        stats.uses_edges_derived += 1;

        // Promote generic references to concrete uses
        if edge.edge_type == EdgeType::References {
            edge.edge_type = EdgeType::Uses;
        }
    }

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

fn cfg_feature(lines: &[String], line_1based: u32) -> Option<String> {
    let idx = (line_1based as usize).saturating_sub(1);
    let start = idx.saturating_sub(3);
    let end = idx.min(lines.len());

    for l in &lines[start..end] {
        let trimmed = l.trim();
        if !trimmed.starts_with("#[cfg") {
            continue;
        }
        if let Some(feature_idx) = trimmed.find("feature") {
            let after = &trimmed[feature_idx..];
            if let Some(start_quote) = after.find('"') {
                let rest = &after[start_quote + 1..];
                if let Some(end_quote) = rest.find('"') {
                    return Some(rest[..end_quote].to_string());
                }
            }
        }
    }
    None
}

fn pub_use_reexports(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        let trimmed = l.trim_start();
        if !trimmed.starts_with("pub use ") {
            continue;
        }
        let rest = trimmed.trim_start_matches("pub use ").trim();
        let rest = rest.trim_end_matches(';').trim();
        if !rest.is_empty() {
            out.push(rest.to_string());
        }
    }
    out
}

fn feature_ids_by_package(
    nodes: &[CodeNode],
) -> std::collections::HashMap<(String, String), NodeId> {
    let mut out = std::collections::HashMap::new();
    for n in nodes {
        if n.node_type != Some(NodeType::Other("feature".to_string())) {
            continue;
        }
        let Some(q) = n.metadata.attributes.get("qualified_name") else {
            continue;
        };
        let mut parts = q.split("::");
        if parts.next() != Some("feature") {
            continue;
        }
        let Some(pkg) = parts.next() else {
            continue;
        };
        let Some(feature) = parts.next() else {
            continue;
        };
        out.insert((pkg.to_string(), feature.to_string()), n.id);
    }
    out
}

fn package_roots(project_root: &Path, nodes: &[CodeNode]) -> Vec<(PathBuf, NodeId)> {
    let mut out: Vec<(PathBuf, NodeId)> = Vec::new();
    for node in nodes {
        if node.node_type != Some(NodeType::Other("package".to_string())) {
            continue;
        }
        if !node.location.file_path.ends_with("Cargo.toml") {
            continue;
        }
        let manifest = normalize_project_path(project_root, Path::new(&node.location.file_path));
        if let Some(dir) = manifest.parent() {
            out.push((dir.to_path_buf(), node.id));
        }
    }
    out.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));
    out
}

fn package_for_file(
    project_root: &Path,
    package_roots: &[(PathBuf, NodeId)],
    file_path: &str,
) -> Option<(NodeId, PathBuf)> {
    let p = normalize_project_path(project_root, Path::new(file_path));
    for (root, id) in package_roots {
        if p.starts_with(root) {
            return Some((*id, root.clone()));
        }
    }
    None
}

fn normalize_project_path(project_root: &Path, path: &Path) -> PathBuf {
    let combined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    let mut out = PathBuf::new();
    for component in combined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
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
        std::fs::write(&file, "/// Hello\n/// world\npub fn foo() {}\n").expect("write");

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

    #[test]
    fn api_surface_enrichment_emits_reexports_and_feature_edges() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let lib_rs = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(lib_rs.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &lib_rs,
            r#"
                #[cfg(feature = "foo")]
                pub fn public_fn() {}

                pub use crate::other::Thing;
            "#,
        )
        .expect("write");

        let cargo = temp.path().join("Cargo.toml");
        std::fs::write(&cargo, "[package]\nname = \"app\"\n").expect("write cargo");

        let package = CodeNode::new(
            "app",
            Some(NodeType::Other("package".to_string())),
            Some(Language::Rust),
            Location {
                file_path: cargo.to_string_lossy().to_string(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        );

        let mut feature = CodeNode::new(
            "app::foo",
            Some(NodeType::Other("feature".to_string())),
            Some(Language::Rust),
            Location {
                file_path: cargo.to_string_lossy().to_string(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        );
        feature.metadata.attributes.insert(
            "qualified_name".to_string(),
            "feature::app::foo".to_string(),
        );

        let mut public_fn = CodeNode::new(
            "public_fn",
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: lib_rs.to_string_lossy().to_string(),
                line: 3,
                column: 0,
                end_line: Some(3),
                end_column: Some(0),
            },
        );
        public_fn
            .metadata
            .attributes
            .insert("qualified_name".to_string(), "crate::public_fn".to_string());

        let mut nodes = vec![package, feature, public_fn];
        let mut edges = Vec::new();
        let stats = apply_basic_enrichment(temp.path(), &mut nodes, &mut edges).expect("enrich");

        assert!(stats.reexport_edges_added > 0);
        assert!(stats.feature_enables_edges_added > 0);
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("reexports".to_string())));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("enables".to_string())));
    }

    #[test]
    fn api_surface_enrichment_matches_packages_for_relative_file_paths() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        let lib_rs = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(lib_rs.parent().unwrap()).expect("mkdir");
        std::fs::write(&lib_rs, "pub fn api() {}\npub use crate::api;\n").expect("write lib.rs");

        let package = CodeNode::new(
            "x",
            Some(NodeType::Other("package".to_string())),
            Some(Language::Rust),
            Location {
                file_path: temp.path().join("Cargo.toml").to_string_lossy().to_string(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        )
        .with_deterministic_id("p");

        let api_fn = CodeNode::new(
            "api",
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: "./src/lib.rs".to_string(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        );

        let mut nodes = vec![package, api_fn];
        let mut edges = Vec::new();
        let stats = apply_basic_enrichment(temp.path(), &mut nodes, &mut edges).expect("enrich");

        assert!(stats.api_marked > 0);
        assert!(
            stats.export_edges_added > 0,
            "expected export edges for public API items"
        );
        assert!(
            stats.reexport_edges_added > 0,
            "expected reexport edges for pub use"
        );
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("exports".to_string())));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("reexports".to_string())));
    }
}
