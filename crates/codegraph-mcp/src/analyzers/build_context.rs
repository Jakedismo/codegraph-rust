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

pub fn analyze_cargo_workspace(
    project_root: &Path,
    project_id: &str,
) -> Result<BuildContextOutput> {
    if !project_root.join("Cargo.toml").is_file() {
        return Ok(BuildContextOutput::default());
    }

    use codegraph_core::artifact_cache::{ArtifactCache, fingerprint};
    let cache = ArtifactCache::new(
        project_root.join(".codegraph/index-cache"),
        "cargo-metadata-v1",
    );
    let manifests: std::collections::BTreeMap<_, _> =
        crate::reconciliation::support_fingerprints(project_root)?
            .into_iter()
            .filter(|(path, _)| {
                !matches!(
                    Path::new(path).extension().and_then(|s| s.to_str()),
                    Some("md" | "surql")
                )
            })
            .collect();
    let key = fingerprint(&(
        project_root,
        manifests,
        ["RUSTFLAGS", "CARGO_BUILD_TARGET", "RUSTUP_TOOLCHAIN"]
            .map(|name| std::env::var(name).unwrap_or_default()),
    ))?;
    if let Some(artifact) = cache.get::<MetadataArtifact>(&key)
        && artifact
            .external
            .iter()
            .all(|(path, hash)| file_hash(Path::new(path)).as_ref() == Some(hash))
    {
        cache.put("external-inputs", &artifact.external)?;
        return parse_cargo_metadata_json(&artifact.json, project_id);
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
    let root: JsonValue = serde_json::from_str(&stdout)?;
    let external: std::collections::BTreeMap<String, String> = root["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| package["manifest_path"].as_str())
        .filter(|path| !Path::new(path).starts_with(project_root))
        .filter_map(|path| file_hash(Path::new(path)).map(|hash| (path.to_owned(), hash)))
        .collect();
    cache.put("external-inputs", &external)?;
    if let Err(error) = cache.put(
        &key,
        &MetadataArtifact {
            json: stdout.to_string(),
            external,
        },
    ) {
        tracing::debug!("Cargo metadata cache unavailable: {error}");
    }
    parse_cargo_metadata_json(&stdout, project_id)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MetadataArtifact {
    json: String,
    external: std::collections::BTreeMap<String, String>,
}
fn file_hash(path: &Path) -> Option<String> {
    crate::reconciliation::file_fingerprint(path).ok()
}

pub(crate) fn external_input_fingerprints(
    root: &Path,
) -> Result<std::collections::BTreeMap<String, String>> {
    let cache = codegraph_core::artifact_cache::ArtifactCache::new(
        root.join(".codegraph/index-cache"),
        "cargo-metadata-v1",
    );
    let previous: std::collections::BTreeMap<String, String> =
        cache.get("external-inputs").unwrap_or_default();
    previous
        .keys()
        .map(|path| {
            Ok((
                path.clone(),
                match crate::reconciliation::file_fingerprint(Path::new(path)) {
                    Ok(hash) => hash,
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        "<missing>".into()
                    }
                    Err(error) => return Err(error),
                },
            ))
        })
        .collect()
}

pub fn parse_cargo_metadata_json(json: &str, project_id: &str) -> Result<BuildContextOutput> {
    let root: JsonValue = serde_json::from_str(json)?;
    let packages = root
        .get("packages")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let mut out = BuildContextOutput::default();

    let mut package_ids: std::collections::HashMap<String, codegraph_core::NodeId> =
        std::collections::HashMap::new();

    for pkg in packages {
        let Some(name) = pkg.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let package_identity = pkg.get("id").and_then(|v| v.as_str()).unwrap_or(name);
        let package_qualified = format!("package::{}", package_identity);

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
        node.metadata
            .attributes
            .insert("analyzer_confidence".to_string(), "1.0".to_string());
        node.metadata
            .attributes
            .insert("qualified_name".to_string(), package_qualified.clone());

        node.id = codegraph_core::generate_node_id(
            project_id,
            manifest_path,
            package_identity,
            "package",
            1,
        );
        package_ids.insert(package_identity.to_string(), node.id);
        out.nodes.push(node);

        let features = pkg
            .get("features")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        for (feature_name, _) in features {
            let mut feature_node = CodeNode::new(
                format!("{}::{}", name, feature_name),
                Some(NodeType::Other("feature".to_string())),
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

            feature_node
                .metadata
                .attributes
                .insert("analyzer".to_string(), "build_context".to_string());
            feature_node
                .metadata
                .attributes
                .insert("analyzer_confidence".to_string(), "1.0".to_string());
            feature_node.metadata.attributes.insert(
                "qualified_name".to_string(),
                format!("feature::{}::{}", name, feature_name),
            );
            out.edges.push(EdgeRelationship {
                from: feature_node.id,
                to: package_qualified.clone(),
                edge_type: EdgeType::Other("enables".to_string()),
                metadata: std::collections::HashMap::from([
                    ("analyzer".to_string(), "build_context".to_string()),
                    ("analyzer_confidence".to_string(), "1.0".to_string()),
                ]),
                span: None,
            });

            out.nodes.push(feature_node);
        }
    }

    for pkg in packages {
        let Some(name) = pkg.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let identity = pkg.get("id").and_then(|v| v.as_str()).unwrap_or(name);
        let Some(&from_id) = package_ids.get(identity) else {
            continue;
        };

        if let Some(resolve) = root
            .get("resolve")
            .and_then(|v| v.get("nodes"))
            .and_then(|v| v.as_array())
            .and_then(|rows| rows.iter().find(|row| row["id"].as_str() == Some(identity)))
        {
            for dep in resolve["deps"].as_array().into_iter().flatten() {
                let Some(target_identity) = dep["pkg"].as_str() else {
                    continue;
                };
                if let Some(target) = package_ids.get(target_identity) {
                    out.edges.push(EdgeRelationship {
                        from: from_id,
                        to: format!("package::{target_identity}"),
                        edge_type: EdgeType::Other("depends_on".into()),
                        metadata: std::collections::HashMap::from([
                            ("analyzer".into(), "build_context".into()),
                            ("target_node_id".into(), target.to_string()),
                            ("analyzer_confidence".into(), "1.0".into()),
                        ]),
                        span: None,
                    });
                }
            }
            continue;
        }
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

        let features: Vec<_> = out
            .nodes
            .iter()
            .filter(|n| n.node_type == Some(NodeType::Other("feature".to_string())))
            .collect();
        assert!(
            !features.is_empty(),
            "expected at least one feature node from cargo metadata"
        );

        assert!(
            out.edges.iter().any(|e| {
                e.edge_type == EdgeType::Other("enables".to_string()) && e.to == "package::app"
            }),
            "expected an enables edge from a feature to the owning package"
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
