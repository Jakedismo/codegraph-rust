#![cfg(feature = "jina")]

use codegraph_vector::jina_provider::{
    JinaConfig, JinaEmbeddingProvider, MAX_REL_TEXTS_HARD_LIMIT,
};

#[test]
fn relationship_chunk_size_respects_relationship_limits() {
    let mut config = JinaConfig::default();
    config.api_key = "test-key".to_string();
    config.relationship_batch_size = 48;
    config.relationship_max_texts_per_request = 24;
    config.max_texts_per_request = 200;

    let provider = JinaEmbeddingProvider::new(config).expect("provider init");
    assert_eq!(provider.relationship_chunk_size(), 24);
}

#[test]
fn relationship_chunk_size_respects_global_limit() {
    let mut config = JinaConfig::default();
    config.api_key = "test-key".to_string();
    config.relationship_batch_size = 500;
    config.relationship_max_texts_per_request = 40;
    config.max_texts_per_request = 20;

    let provider = JinaEmbeddingProvider::new(config).expect("provider init");
    assert_eq!(provider.relationship_chunk_size(), 20);
}

#[test]
fn relationship_chunk_size_enforces_hard_cap() {
    let mut config = JinaConfig::default();
    config.api_key = "test-key".to_string();
    config.relationship_batch_size = 128;
    config.relationship_max_texts_per_request = 128;
    config.max_texts_per_request = 128;

    let provider = JinaEmbeddingProvider::new(config).expect("provider init");
    assert_eq!(provider.relationship_chunk_size(), MAX_REL_TEXTS_HARD_LIMIT);
}

#[test]
fn batch_size_setter_is_clamped() {
    let mut config = JinaConfig::default();
    config.api_key = "test-key".to_string();
    config.max_texts_per_request = 8;

    let mut provider = JinaEmbeddingProvider::new(config).expect("provider init");
    provider.set_batch_size(512);
    assert_eq!(provider.batch_size(), 8);
}
