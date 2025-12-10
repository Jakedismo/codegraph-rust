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
    ];

    for needle in expectations {
        assert!(
            schema.contains(needle),
            "schema missing expected definition: {}",
            needle
        );
    }
}
