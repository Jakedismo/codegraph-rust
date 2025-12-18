// ABOUTME: Derives local def-use and propagation edges from function bodies during indexing
// ABOUTME: Produces variable nodes plus `defines`/`uses`/`flows_to`/`returns`/`mutates` edges conservatively

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Language, Location, NodeId, NodeType};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DataflowStats {
    pub variable_nodes_added: usize,
    pub defines_edges_added: usize,
    pub uses_edges_added: usize,
    pub flows_to_edges_added: usize,
    pub returns_edges_added: usize,
    pub mutates_edges_added: usize,
}

pub fn enrich_rust_dataflow(
    _project_root: &Path,
    project_id: &str,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<DataflowStats> {
    let mut stats = DataflowStats::default();

    let let_re = Regex::new(r"(?m)^[ \t]*let(?:\s+mut)?\s+([A-Za-z_][A-Za-z0-9_]*)")?;
    let assign_re = Regex::new(r"(?m)\b([A-Za-z_][A-Za-z0-9_]*)\b\s*(?:=|\+=|-=|\*=|/=|%=)")?;
    let return_re = Regex::new(r"(?m)\breturn\b[^\n;]*\b([A-Za-z_][A-Za-z0-9_]*)\b")?;
    let flow_re = Regex::new(
        r"(?m)^[ \t]*let(?:\s+mut)?\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*;",
    )?;

    let mut created_variables: HashSet<String> = HashSet::new();

    let function_ids: Vec<NodeId> = nodes
        .iter()
        .filter(|n| n.language == Some(Language::Rust) && n.node_type == Some(NodeType::Function))
        .map(|n| n.id)
        .collect();

    for function_id in function_ids {
        let Some((file_path, start_line, function_qname, body)) =
            function_context(nodes, function_id)
        else {
            continue;
        };

        let mut var_by_name: HashMap<String, String> = HashMap::new();
        let mut var_id_by_qualified: HashMap<String, NodeId> = HashMap::new();

        for cap in let_re.captures_iter(&body) {
            let Some(m) = cap.get(1) else { continue };
            let var_name = m.as_str().to_string();
            let line_offset = body[..m.start()].bytes().filter(|b| *b == b'\n').count() as u32;
            let var_line = start_line.saturating_add(line_offset);
            let qualified = format!("{}::{}", function_qname, var_name);
            if !created_variables.insert(format!("{}:{}:{}", file_path, var_line, qualified)) {
                continue;
            }

            let mut var_node = CodeNode::new(
                var_name.clone(),
                Some(NodeType::Variable),
                Some(Language::Rust),
                Location {
                    file_path: file_path.clone(),
                    line: var_line,
                    column: 0,
                    end_line: Some(var_line),
                    end_column: Some(0),
                },
            )
            .with_deterministic_id(project_id);
            var_node
                .metadata
                .attributes
                .insert("analyzer".to_string(), "dataflow".to_string());
            var_node
                .metadata
                .attributes
                .insert("analyzer_confidence".to_string(), "0.7".to_string());
            var_node
                .metadata
                .attributes
                .insert("qualified_name".to_string(), qualified.clone());

            var_by_name.insert(var_name, qualified.clone());
            var_id_by_qualified.insert(qualified.clone(), var_node.id);
            nodes.push(var_node);
            stats.variable_nodes_added += 1;

            edges.push(EdgeRelationship {
                from: function_id,
                to: qualified,
                edge_type: EdgeType::Defines,
                metadata: std::collections::HashMap::from([
                    ("analyzer".to_string(), "dataflow".to_string()),
                    ("analyzer_confidence".to_string(), "0.7".to_string()),
                ]),
                span: None,
            });
            stats.defines_edges_added += 1;
        }

        if var_by_name.is_empty() {
            continue;
        }

        for (name, qualified) in var_by_name.iter() {
            let uses_re = Regex::new(&format!(r"(?m)\b{}\b", regex::escape(name)))?;
            let count = uses_re.find_iter(&body).count();
            if count > 1 {
                edges.push(EdgeRelationship {
                    from: function_id,
                    to: qualified.clone(),
                    edge_type: EdgeType::Uses,
                    metadata: std::collections::HashMap::from([
                        ("analyzer".to_string(), "dataflow".to_string()),
                        ("analyzer_confidence".to_string(), "0.6".to_string()),
                    ]),
                    span: None,
                });
                stats.uses_edges_added += 1;
            }
        }

        for cap in assign_re.captures_iter(&body) {
            let var = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(qualified) = var_by_name.get(var) {
                edges.push(EdgeRelationship {
                    from: function_id,
                    to: qualified.clone(),
                    edge_type: EdgeType::Other("mutates".to_string()),
                    metadata: std::collections::HashMap::from([
                        ("analyzer".to_string(), "dataflow".to_string()),
                        ("analyzer_confidence".to_string(), "0.6".to_string()),
                    ]),
                    span: None,
                });
                stats.mutates_edges_added += 1;
            }
        }

        for cap in return_re.captures_iter(&body) {
            let var = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(qualified) = var_by_name.get(var) {
                edges.push(EdgeRelationship {
                    from: function_id,
                    to: qualified.clone(),
                    edge_type: EdgeType::Other("returns".to_string()),
                    metadata: std::collections::HashMap::from([
                        ("analyzer".to_string(), "dataflow".to_string()),
                        ("analyzer_confidence".to_string(), "0.6".to_string()),
                    ]),
                    span: None,
                });
                stats.returns_edges_added += 1;
            }
        }

        for cap in flow_re.captures_iter(&body) {
            let dst = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let src = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let (Some(dst_q), Some(src_q)) = (var_by_name.get(dst), var_by_name.get(src)) else {
                continue;
            };
            let Some(from_id) = var_id_by_qualified.get(src_q) else {
                continue;
            };
            edges.push(EdgeRelationship {
                from: *from_id,
                to: dst_q.clone(),
                edge_type: EdgeType::Other("flows_to".to_string()),
                metadata: std::collections::HashMap::from([
                    ("analyzer".to_string(), "dataflow".to_string()),
                    ("analyzer_confidence".to_string(), "0.6".to_string()),
                ]),
                span: None,
            });
            stats.flows_to_edges_added += 1;
        }
    }

    Ok(stats)
}

fn function_context(
    nodes: &[CodeNode],
    function_id: NodeId,
) -> Option<(String, u32, String, String)> {
    let n = nodes.iter().find(|n| n.id == function_id)?;
    let body = n.content.as_ref()?.to_string();
    let qname = n
        .metadata
        .attributes
        .get("qualified_name")
        .cloned()
        .unwrap_or_else(|| n.name.to_string());
    Some((n.location.file_path.clone(), n.location.line, qname, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::Location;

    #[test]
    fn dataflow_enrichment_emits_def_use_and_propagation_edges() {
        let mut nodes = vec![{
            let mut n = CodeNode::new(
                "demo",
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: "src/lib.rs".to_string(),
                    line: 10,
                    column: 0,
                    end_line: Some(10),
                    end_column: Some(0),
                },
            )
            .with_content("fn demo() {\n  let a = 1;\n  let b = a;\n  a = 2;\n  return b;\n}\n");
            n.metadata
                .attributes
                .insert("qualified_name".to_string(), "crate::demo".to_string());
            n
        }];
        let mut edges = Vec::new();

        let stats =
            enrich_rust_dataflow(Path::new("."), "project", &mut nodes, &mut edges).unwrap();

        assert_eq!(stats.variable_nodes_added, 2);
        assert_eq!(stats.defines_edges_added, 2);
        assert!(edges.iter().any(|e| e.edge_type == EdgeType::Defines));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("flows_to".to_string())));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("mutates".to_string())));
        assert!(edges
            .iter()
            .any(|e| e.edge_type == EdgeType::Other("returns".to_string())));
    }
}
