// ABOUTME: Extracts build-system context (packages, features, dependencies) for indexing
// ABOUTME: Produces package-level nodes and edges to improve correctness and navigation

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Language, Location, NodeType};
use serde_json::Value as JsonValue;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Default)]
pub struct BuildContextOutput {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<EdgeRelationship>,
}

pub fn analyze_cargo_workspace(project_root: &Path, project_id: &str) -> Result<BuildContextOutput> {
    if !project_root.join("Cargo.toml").is_file() {
        return Ok(BuildContextOutput::default());
    }

    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_cargo_metadata_json(&stdout, project_id)
}

pub fn parse_cargo_metadata_json(json: &str, project_id: &str) -> Result<BuildContextOutput> {
    let root: JsonValue = serde_json::from_str(json)?;
    let packages = root
        .get("packages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = BuildContextOutput::default();

    let mut package_ids: std::collections::HashMap<String, codegraph_core::NodeId> =
        std::collections::HashMap::new();

    for pkg in &packages {
        let Some(name) = pkg.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut node = CodeNode::new(
            name,
            Some(NodeType::Other("package".to_string())),
            Some(Language::Rust),
            Location {
                file_path: manifest_path.to_string(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        )
        .with_deterministic_id(project_id);

        node.metadata
            .attributes
            .insert("analyzer".to_string(), "build_context".to_string());
        node.metadata.attributes.insert(
            "analyzer_confidence".to_string(),
            "1.0".to_string(),
        );
        node.metadata.attributes.insert(
            "qualified_name".to_string(),
            format!("package::{}", name),
        );

        package_ids.insert(name.to_string(), node.id);
        out.nodes.push(node);
    }

    for pkg in &packages {
        let Some(name) = pkg.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(&from_id) = package_ids.get(name) else {
            continue;
        };

        let deps = pkg
            .get("dependencies")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for dep in deps {
            let Some(dep_name) = dep.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            out.edges.push(EdgeRelationship {
                from: from_id,
                to: dep_name.to_string(),
                edge_type: EdgeType::Other("depends_on".to_string()),
                metadata: std::collections::HashMap::from([
                    ("analyzer".to_string(), "build_context".to_string()),
                    ("analyzer_confidence".to_string(), "1.0".to_string()),
                ]),
                span: None,
            });
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{EdgeType, Language, NodeType};

    #[test]
    fn cargo_metadata_produces_package_nodes_and_dependency_edges() {
        let json = r#"
        {
          "packages": [
            {
              "name": "app",
              "manifest_path": "/repo/app/Cargo.toml",
              "dependencies": [{"name": "lib"}],
              "features": {"default": ["lib/default"]}
            },
            {
              "name": "lib",
              "manifest_path": "/repo/lib/Cargo.toml",
              "dependencies": [],
              "features": {"default": []}
            }
          ]
        }"#;

        let out = parse_cargo_metadata_json(json, "project").expect("parse should succeed");

        let packages: Vec<_> = out
            .nodes
            .iter()
            .filter(|n| n.node_type == Some(NodeType::Other("package".to_string())))
            .collect();
        assert_eq!(packages.len(), 2, "expected two package nodes");

        assert!(
            out.edges.iter().any(|e| {
                e.edge_type == EdgeType::Other("depends_on".to_string()) && e.to == "lib"
            }),
            "expected a depends_on edge from app to lib"
        );
    }

    #[test]
    fn build_context_nodes_are_project_scoped_and_language_tagged() {
        let json = r#"
        { "packages": [{"name":"app","manifest_path":"/repo/app/Cargo.toml","dependencies":[],"features":{}}] }
        "#;
        let out = parse_cargo_metadata_json(json, "project").expect("parse should succeed");
        let node = out.nodes.first().expect("expected a node");
        assert_eq!(node.language, Some(Language::Rust));
        assert!(node.location.file_path.ends_with("Cargo.toml"));
        assert_eq!(node.location.line, 1);
        assert_eq!(node.location.column, 0);
        assert!(node.metadata.attributes.contains_key("analyzer"));
    }
}
