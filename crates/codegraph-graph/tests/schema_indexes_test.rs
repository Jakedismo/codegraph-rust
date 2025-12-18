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
