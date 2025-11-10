// ABOUTME: Ensures repository estimation helpers expose correct calculations.
// ABOUTME: Validates symbol indexing and embedding ETA heuristics for the CLI.

use codegraph_core::{CodeNode, Language, Location, NodeType};
use codegraph_mcp::estimation::{build_symbol_index, EmbeddingThroughputConfig, TimeEstimates};

fn sample_location() -> Location {
    Location {
        file_path: "src/lib.rs".to_string(),
        line: 1,
        column: 0,
        end_line: Some(1),
        end_column: Some(10),
    }
}

fn make_node(name: &str, node_type: NodeType) -> CodeNode {
    CodeNode::new(
        name,
        Some(node_type),
        Some(Language::Rust),
        sample_location(),
    )
}

#[test]
fn embedding_time_estimator_respects_jina_batches() {
    let cfg = EmbeddingThroughputConfig {
        jina_batch_size: 2000,
        jina_batch_minutes: 9.0,
        local_embeddings_per_minute: Some(12000.0),
    };

    let estimates = TimeEstimates::from_node_count(4500, &cfg);

    assert_eq!(estimates.jina_batches, 3);
    assert!((estimates.jina_minutes - 27.0).abs() < f64::EPSILON);
    assert!((estimates.local_minutes.unwrap() - 0.375).abs() < f64::EPSILON);
}

#[test]
fn symbol_index_records_expected_aliases() {
    let mut import_node = make_node("chrono::Utc", NodeType::Import);
    import_node
        .metadata
        .attributes
        .insert("qualified_name".into(), "./foo.rs::chrono::Utc".into());
    import_node
        .metadata
        .attributes
        .insert("method_of".into(), "chrono".into());
    import_node
        .metadata
        .attributes
        .insert("implements_trait".into(), "TimeZone".into());

    let mut struct_node = make_node("MyStruct", NodeType::Struct);
    struct_node
        .metadata
        .attributes
        .insert("qualified_name".into(), "crate::MyStruct".into());

    let symbols = build_symbol_index(&[import_node, struct_node]);

    assert!(symbols.contains_key("chrono::Utc"));
    assert!(symbols.contains_key("./foo.rs::chrono::Utc"));
    assert!(symbols.contains_key("Import::chrono::Utc"));
    assert!(symbols.contains_key("src/lib.rs::chrono::Utc"));
    assert!(symbols.contains_key("Utc"));
    assert!(symbols.contains_key("chrono::chrono::Utc"));
    assert!(symbols.contains_key("TimeZone::chrono::Utc"));
    assert!(symbols.contains_key("MyStruct"));
    assert!(symbols.contains_key("Struct::MyStruct"));
}
