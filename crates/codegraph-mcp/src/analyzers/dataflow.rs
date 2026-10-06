// ABOUTME: Derives local def-use and propagation edges from function bodies during indexing
// ABOUTME: Produces variable nodes plus `defines`/`uses`/`flows_to`/`returns`/`mutates` edges conservatively

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Language, Location, NodeId, NodeType};
use std::collections::HashMap;
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
    project_root: &Path,
    project_id: &str,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<DataflowStats> {
    use codegraph_core::artifact_cache::{ArtifactCache, fingerprint};
    let cache = ArtifactCache::new(project_root.join(".codegraph/index-cache"), "dataflow-v3");
    let registry = codegraph_parser::LanguageRegistry::new();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&registry.get_config(&Language::Rust).unwrap().language)?;
    let mut stats = DataflowStats::default();
    let functions: Vec<_> = nodes
        .iter()
        .filter(|node| {
            node.language == Some(Language::Rust) && node.node_type == Some(NodeType::Function)
        })
        .collect();
    let mut additions = Vec::new();
    for function in functions {
        let Some(source) = &function.content else {
            continue;
        };
        let key = fingerprint(&(
            "dataflow-v3",
            project_id,
            function.id,
            source,
            &function.location,
            &function.span,
        ))?;
        let artifact: FlowArtifact = if let Some(artifact) = cache.get(&key) {
            artifact
        } else {
            let tree = parser
                .parse(source.as_bytes(), None)
                .ok_or_else(|| anyhow::anyhow!("Dataflow AST parse failed"))?;
            let mut context = FlowContext {
                source,
                function,
                project: project_id,
                scopes: vec![HashMap::new()],
                nodes: Vec::new(),
                edges: Vec::new(),
            };
            let root = tree.root_node();
            let mut cursor = root.walk();
            for item in root.named_children(&mut cursor) {
                if item.kind() == "function_item" {
                    if let Some(parameters) = item.child_by_field_name("parameters") {
                        context.parameters(parameters);
                    }
                    if let Some(body) = item.child_by_field_name("body") {
                        context.walk(body, false);
                    }
                }
            }
            let artifact = FlowArtifact {
                nodes: context.nodes,
                edges: context.edges,
            };
            if let Err(error) = cache.put(&key, &artifact) {
                tracing::debug!("Dataflow cache unavailable: {error}");
            }
            artifact
        };
        stats.variable_nodes_added += artifact.nodes.len();
        for edge in &artifact.edges {
            match edge.edge_type.to_string().as_str() {
                "defines" => stats.defines_edges_added += 1,
                "uses" => stats.uses_edges_added += 1,
                "flows_to" => stats.flows_to_edges_added += 1,
                "returns" => stats.returns_edges_added += 1,
                "mutates" => stats.mutates_edges_added += 1,
                _ => {}
            }
        }
        additions.extend(artifact.nodes);
        edges.extend(artifact.edges);
    }
    nodes.extend(additions);
    Ok(stats)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FlowArtifact {
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
}
#[derive(Clone)]
struct Binding {
    id: NodeId,
    qualified: String,
}
struct FlowContext<'a> {
    source: &'a str,
    function: &'a CodeNode,
    project: &'a str,
    scopes: Vec<HashMap<String, Binding>>,
    nodes: Vec<CodeNode>,
    edges: Vec<EdgeRelationship>,
}
impl FlowContext<'_> {
    fn text(&self, node: tree_sitter::Node<'_>) -> &str {
        &self.source[node.byte_range()]
    }
    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
    fn edge(
        &mut self,
        from: NodeId,
        binding: &Binding,
        kind: EdgeType,
        evidence: tree_sitter::Node<'_>,
    ) {
        let base = self
            .function
            .span
            .as_ref()
            .map_or(0, |span| span.start_byte);
        self.edges.push(EdgeRelationship {
            from,
            to: binding.qualified.clone(),
            edge_type: kind,
            metadata: HashMap::from([
                ("analyzer".into(), "dataflow".into()),
                ("analyzer_confidence".into(), "0.8".into()),
                ("target_node_id".into(), binding.id.to_string()),
                (
                    "source_file".into(),
                    self.function.location.file_path.clone(),
                ),
            ]),
            span: Some(codegraph_core::Span {
                start_byte: base + evidence.start_byte() as u32,
                end_byte: base + evidence.end_byte() as u32,
            }),
        });
    }
    fn bind(&mut self, pattern: tree_sitter::Node<'_>) -> Vec<Binding> {
        if pattern.kind() == "identifier" {
            let name = self.text(pattern).to_owned();
            let line = self.function.location.line + pattern.start_position().row as u32;
            let qualified = format!("{}::{}@{}", self.function.id, name, pattern.start_byte());
            let mut node = CodeNode::new(
                name.clone(),
                Some(NodeType::Variable),
                Some(Language::Rust),
                Location {
                    file_path: self.function.location.file_path.clone(),
                    line,
                    column: pattern.start_position().column as u32,
                    end_line: Some(line),
                    end_column: Some(pattern.end_position().column as u32),
                },
            );
            node.metadata.attributes.extend(HashMap::from([
                ("qualified_name".into(), qualified.clone()),
                ("analyzer".into(), "dataflow".into()),
                ("analyzer_confidence".into(), "0.8".into()),
            ]));
            node.id = codegraph_core::generate_node_id(
                self.project,
                &node.location.file_path,
                &qualified,
                "variable",
                line,
            );
            let binding = Binding {
                id: node.id,
                qualified,
            };
            self.edge(self.function.id, &binding, EdgeType::Defines, pattern);
            self.scopes
                .last_mut()
                .unwrap()
                .insert(name, binding.clone());
            self.nodes.push(node);
            vec![binding]
        } else {
            let mut bindings = Vec::new();
            let mut cursor = pattern.walk();
            for child in pattern.named_children(&mut cursor) {
                if !matches!(
                    child.kind(),
                    "type_identifier" | "scoped_identifier" | "field_identifier"
                ) {
                    bindings.extend(self.bind(child));
                }
            }
            bindings
        }
    }
    fn parameters(&mut self, parameters: tree_sitter::Node<'_>) {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if let Some(pattern) = parameter.child_by_field_name("pattern") {
                self.bind(pattern);
            }
        }
    }
    fn identifiers(&self, node: tree_sitter::Node<'_>, output: &mut Vec<Binding>) {
        if node.kind() == "identifier" {
            if let Some(binding) = self.lookup(self.text(node)) {
                output.push(binding);
            }
        } else {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                self.identifiers(child, output);
            }
        }
    }
    fn walk(&mut self, node: tree_sitter::Node<'_>, returning: bool) {
        match node.kind() {
            "function_item" | "string_literal" | "raw_string_literal" | "line_comment"
            | "block_comment" => return,
            "block" => {
                self.scopes.push(HashMap::new());
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.walk(child, returning);
                }
                self.scopes.pop();
                return;
            }
            "closure_expression" => {
                self.scopes.push(HashMap::new());
                if let Some(parameters) = node.child_by_field_name("parameters") {
                    self.bind(parameters);
                }
                if let Some(body) = node.child_by_field_name("body") {
                    self.walk(body, false);
                }
                self.scopes.pop();
                return;
            }
            "let_declaration" => {
                let mut sources = Vec::new();
                if let Some(value) = node.child_by_field_name("value") {
                    self.identifiers(value, &mut sources);
                    self.walk(value, false);
                }
                if let Some(pattern) = node.child_by_field_name("pattern") {
                    for destination in self.bind(pattern) {
                        for source in &sources {
                            self.edge(
                                source.id,
                                &destination,
                                EdgeType::Other("flows_to".into()),
                                node,
                            );
                        }
                    }
                }
                return;
            }
            "assignment_expression" | "compound_assignment_expr" => {
                if let Some(left) = node.child_by_field_name("left") {
                    let mut bindings = Vec::new();
                    self.identifiers(left, &mut bindings);
                    for binding in bindings {
                        self.edge(
                            self.function.id,
                            &binding,
                            EdgeType::Other("mutates".into()),
                            left,
                        );
                    }
                }
            }
            "return_expression" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.walk(child, true);
                }
                return;
            }
            "identifier" => {
                if let Some(binding) = self.lookup(self.text(node)) {
                    self.edge(self.function.id, &binding, EdgeType::Uses, node);
                    if returning {
                        self.edge(
                            self.function.id,
                            &binding,
                            EdgeType::Other("returns".into()),
                            node,
                        );
                    }
                }
                return;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.walk(child, returning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::Location;

    #[test]
    fn shadowed_bindings_are_distinct_and_comments_and_strings_are_not_uses() {
        let function = CodeNode::new(
            "f",
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: "a.rs".into(),
                line: 1,
                column: 0,
                end_line: None,
                end_column: None,
            },
        )
        .with_content(
            "fn f() { let x = 1; { let x = x; x = 3; } return x; let s = \"x\"; /* x */ }",
        );
        let function_id = function.id;
        let mut nodes = vec![function];
        let mut edges = Vec::new();
        let cache = tempfile::tempdir().unwrap();
        enrich_rust_dataflow(cache.path(), "project", &mut nodes, &mut edges).unwrap();
        let xs: Vec<_> = nodes
            .iter()
            .filter(|node| node.name.as_str() == "x")
            .collect();
        assert_eq!(xs.len(), 2);
        assert_ne!(xs[0].id, xs[1].id);
        let outer = xs[0].id.to_string();
        let inner = xs[1].id.to_string();
        assert!(
            edges
                .iter()
                .any(|edge| edge.edge_type == EdgeType::Other("returns".into())
                    && edge.metadata["target_node_id"] == outer)
        );
        assert!(
            edges
                .iter()
                .any(|edge| edge.edge_type == EdgeType::Other("mutates".into())
                    && edge.metadata["target_node_id"] == inner)
        );
        assert!(
            edges
                .iter()
                .any(|edge| edge.edge_type == EdgeType::Other("flows_to".into())
                    && edge.from == xs[0].id
                    && edge.metadata["target_node_id"] == inner)
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.from == function_id && edge.edge_type == EdgeType::Uses)
                .count(),
            3
        );
    }

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

        let stats = enrich_rust_dataflow(
            tempfile::tempdir().unwrap().path(),
            "project",
            &mut nodes,
            &mut edges,
        )
        .unwrap();

        assert_eq!(stats.variable_nodes_added, 2);
        assert_eq!(stats.defines_edges_added, 2);
        assert!(edges.iter().any(|e| e.edge_type == EdgeType::Defines));
        assert!(
            edges
                .iter()
                .any(|e| e.edge_type == EdgeType::Other("flows_to".to_string()))
        );
        assert!(
            edges
                .iter()
                .any(|e| e.edge_type == EdgeType::Other("mutates".to_string()))
        );
        assert!(
            edges
                .iter()
                .any(|e| e.edge_type == EdgeType::Other("returns".to_string()))
        );
    }
}
