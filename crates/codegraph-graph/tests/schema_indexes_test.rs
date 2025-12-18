// ABOUTME: Verifies Surreal schema includes project-scoped indexes and fields.
// ABOUTME: Guards against regressions that reintroduce full-table scans.

use std::path::PathBuf;

#[test]
fn schema_defines_project_scoped_indexes() {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schema")
        .join("codegraph.surql");

    let schema = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", schema_path.display(), e));

    let expectations = [
        "DEFINE INDEX idx_nodes_project_file ON nodes FIELDS project_id, file_path",
        "DEFINE INDEX idx_chunks_project ON chunks FIELDS project_id",
        "DEFINE INDEX idx_symbol_embeddings_project ON symbol_embeddings FIELDS project_id",
        "DEFINE FIELD project_id ON edges",
        "DEFINE INDEX idx_edges_project ON edges FIELDS project_id",
        "DEFINE INDEX idx_edges_project_type ON edges FIELDS project_id, edge_type",
        "DEFINE FUNCTION fn::edge_types() { RETURN ['calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to', 'depends_on', 'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates', 'violates_boundary', 'documents', 'specifies']; }",
    ];

    for needle in expectations {
        assert!(
            schema.contains(needle),
            "schema missing expected definition: {}",
            needle
        );
    }
}

#[test]
fn experimental_schema_matches_storage_contracts() {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schema")
        .join("codegraph_graph_experimental.surql");

    let schema = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", schema_path.display(), e));

    let expectations = [
        "DEFINE FIELD edge_type    ON TABLE edges TYPE string",
        "DEFINE INDEX idx_edges_project_type ON TABLE edges FIELDS project_id, edge_type",
        "DEFINE FIELD node_type       ON TABLE nodes TYPE option<string>",
        "DEFINE FIELD project_id        ON TABLE project_metadata TYPE string",
        "DEFINE INDEX idx_project_id ON TABLE project_metadata FIELDS project_id UNIQUE",
        "DEFINE FIELD content_hash     ON TABLE file_metadata TYPE string",
        "DEFINE INDEX idx_file_metadata_project_path",
        "DEFINE INDEX idx_chunks_embedding_1536",
        "TYPE F32",
    ];

    for needle in expectations {
        assert!(
            schema.contains(needle),
            "experimental schema missing expected definition: {}",
            needle
        );
    }
}

#[test]
fn schemas_define_graph_tool_functions() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schema");

    let schemas = [
        ("main", base.join("codegraph.surql")),
        ("experimental", base.join("codegraph_graph_experimental.surql")),
    ];

    let required_functions = [
        "DEFINE FUNCTION fn::get_transitive_dependencies(",
        "DEFINE FUNCTION fn::detect_circular_dependencies(",
        "DEFINE FUNCTION fn::trace_call_chain(",
        "DEFINE FUNCTION fn::calculate_coupling_metrics(",
        "DEFINE FUNCTION fn::get_hub_nodes(",
        "DEFINE FUNCTION fn::get_reverse_dependencies(",
        "DEFINE FUNCTION fn::get_complexity_hotspots(",
        "DEFINE FUNCTION fn::find_nodes_by_name(",
        "DEFINE FUNCTION fn::semantic_search_nodes_via_chunks(",
        "DEFINE FUNCTION fn::semantic_search_chunks_with_context(",
    ];

    for (label, path) in schemas {
        let schema = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));

        for needle in required_functions {
            assert!(
                schema.contains(needle),
                "{} schema missing expected function definition: {}",
                label,
                needle
            );
        }

        let header_start = schema
            .find("DEFINE FUNCTION fn::semantic_search_chunks_with_context(")
            .unwrap_or_else(|| panic!("{} schema missing chunk search function", label));
        let header_tail = &schema[header_start..];
        let header_end = header_tail
            .find(") {")
            .unwrap_or_else(|| panic!("{} schema chunk search header missing ') {{'", label));
        let header = &header_tail[..header_end];

        assert!(
            header.contains("$query_text"),
            "{} schema chunk search signature must include $query_text",
            label
        );
        assert!(
            header.contains("$threshold"),
            "{} schema chunk search signature must include $threshold",
            label
        );
        assert!(
            header.contains("$include_graph_context"),
            "{} schema chunk search signature must include $include_graph_context",
            label
        );
    }
}
