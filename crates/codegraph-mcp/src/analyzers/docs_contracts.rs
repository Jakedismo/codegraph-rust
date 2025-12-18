// ABOUTME: Links repository documentation and specifications to code symbols during indexing
// ABOUTME: Produces document nodes plus `documents`/`specifies` edges with provenance metadata

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Location, NodeType};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DocsContractsStats {
    pub document_nodes_added: usize,
    pub document_edges_added: usize,
    pub specification_edges_added: usize,
}

pub fn link_docs_and_contracts(
    project_root: &Path,
    project_id: &str,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<DocsContractsStats> {
    let mut stats = DocsContractsStats::default();

    let symbols = build_symbol_set(nodes);
    if symbols.is_empty() {
        return Ok(stats);
    }

    let doc_paths = collect_document_paths(project_root);
    if doc_paths.is_empty() {
        return Ok(stats);
    }

    let tick_re = Regex::new(r"`([^`]+)`")?;
    let mut seen_edges: HashSet<(codegraph_core::NodeId, String, String)> = HashSet::new();

    for doc_path in doc_paths {
        let rel = relative_display_path(project_root, &doc_path);
        let content = match std::fs::read_to_string(&doc_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut doc_node = CodeNode::new(
            rel.clone(),
            Some(NodeType::Other("document".to_string())),
            None,
            Location {
                file_path: rel.clone(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        )
        .with_deterministic_id(project_id);

        doc_node
            .metadata
            .attributes
            .insert("analyzer".to_string(), "docs_contracts".to_string());
        doc_node
            .metadata
            .attributes
            .insert("analyzer_confidence".to_string(), "0.9".to_string());
        doc_node
            .metadata
            .attributes
            .insert("qualified_name".to_string(), format!("doc::{}", rel));

        let is_spec = rel.ends_with(".spec.md") || rel.starts_with("docs/specifications/");
        let edge_type = if is_spec {
            EdgeType::Other("specifies".to_string())
        } else {
            EdgeType::Other("documents".to_string())
        };

        for cap in tick_re.captures_iter(&content) {
            let Some(m) = cap.get(1) else {
                continue;
            };
            let token = m.as_str().trim();
            if token.is_empty() {
                continue;
            }
            if !symbols.contains(token) {
                continue;
            }

            let line_1based = 1 + content[..m.start()].bytes().filter(|b| *b == b'\n').count();
            let mut metadata: HashMap<String, String> = HashMap::new();
            metadata.insert("analyzer".to_string(), "docs_contracts".to_string());
            metadata.insert("analyzer_confidence".to_string(), "0.7".to_string());
            metadata.insert(
                "analyzer_evidence".to_string(),
                format!("{}:{}", rel, line_1based),
            );

            let edge_key = (doc_node.id, token.to_string(), edge_type.to_string());
            if seen_edges.insert(edge_key) {
                edges.push(EdgeRelationship {
                    from: doc_node.id,
                    to: token.to_string(),
                    edge_type: edge_type.clone(),
                    metadata,
                    span: None,
                });
                if is_spec {
                    stats.specification_edges_added += 1;
                } else {
                    stats.document_edges_added += 1;
                }
            }
        }

        nodes.push(doc_node);
        stats.document_nodes_added += 1;
    }

    Ok(stats)
}

fn build_symbol_set(nodes: &[CodeNode]) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for node in nodes {
        out.insert(node.name.to_string());
        if let Some(q) = node.metadata.attributes.get("qualified_name") {
            out.insert(q.clone());
        }
    }
    out
}

fn collect_document_paths(project_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    let readme = project_root.join("README.md");
    if readme.is_file() {
        out.push(readme);
    }

    for dir in [project_root.join("docs"), project_root.join("schema")] {
        if !dir.is_dir() {
            continue;
        }
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "md" || ext == "surql" {
                    out.push(path.to_path_buf());
                }
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

fn relative_display_path(project_root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(project_root).unwrap_or(path);
    let mut s = rel.to_string_lossy().to_string();
    if std::path::MAIN_SEPARATOR != '/' {
        s = s.replace(std::path::MAIN_SEPARATOR, "/");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, Location, NodeType};

    #[test]
    fn doc_linking_creates_document_nodes_and_edges() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        std::fs::write(root.join("README.md"), "See `Foo` and `crate::Bar`.\n").unwrap();
        std::fs::create_dir_all(root.join("docs/specifications")).unwrap();
        std::fs::write(
            root.join("docs/specifications/example.spec.md"),
            "This specifies `crate::Bar`.\n",
        )
        .unwrap();

        let mut nodes = vec![
            CodeNode::new(
                "Foo",
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: "src/lib.rs".to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
            {
                let mut n = CodeNode::new(
                    "Bar",
                    Some(NodeType::Struct),
                    Some(Language::Rust),
                    Location {
                        file_path: "src/lib.rs".to_string(),
                        line: 10,
                        column: 0,
                        end_line: Some(10),
                        end_column: Some(0),
                    },
                );
                n.metadata
                    .attributes
                    .insert("qualified_name".to_string(), "crate::Bar".to_string());
                n
            },
        ];
        let mut edges = Vec::new();

        let stats = link_docs_and_contracts(root, "project", &mut nodes, &mut edges)
            .expect("link should succeed");

        assert_eq!(stats.document_nodes_added, 2);
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("documents".to_string())));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("specifies".to_string())));
    }
}
