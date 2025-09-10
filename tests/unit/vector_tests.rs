use codegraph_vector::embedding::{EmbeddingGenerator, ModelConfig};
use codegraph_vector::cache::{LfuCache, CacheConfig, QueryHash, SearchCacheManager, ContextHash};
use codegraph_core::{CodeNode, Language, NodeType, Location, NodeId};
use std::time::Duration;

fn sample_node(name: &str) -> CodeNode {
    CodeNode::new(
        name.into(),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location { file_path: "x.rs".into(), line: 1, column: 1, end_line: None, end_column: None },
    )
}

#[tokio::test]
async fn embedding_generator_dimension_and_norm() {
    let gen = EmbeddingGenerator::new(ModelConfig { dimension: 64, max_tokens: 128, model_name: "test".into() });
    let node = sample_node("f");
    let emb = gen.generate_embedding(&node).await.unwrap();
    assert_eq!(emb.len(), 64);
    let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-3);
}

#[tokio::test]
async fn embedding_generator_is_deterministic() {
    let gen = EmbeddingGenerator::default();
    let node = sample_node("same");
    let a = gen.generate_embedding(&node).await.unwrap();
    let b = gen.generate_embedding(&node).await.unwrap();
    assert_eq!(a, b);
}

#[tokio::test]
async fn lfu_cache_put_get_evict() {
    let cfg = CacheConfig { max_entries: 2, ttl: Duration::from_secs(60), cleanup_interval: Duration::from_secs(3600), enable_stats: true };
    let cache: LfuCache<u32, u32> = LfuCache::new(cfg);
    cache.put(1, 10);
    cache.put(2, 20);
    // Access key 1 to increase its freq
    assert_eq!(cache.get(&1), Some(10));
    // Inserting third should evict LFU (key 2)
    cache.put(3, 30);
    assert!(cache.get(&2).is_none());
    assert_eq!(cache.get(&1), Some(10));
    assert_eq!(cache.get(&3), Some(30));
}

#[tokio::test]
async fn lfu_cache_remove_and_clear() {
    let cfg = CacheConfig::default();
    let cache: LfuCache<&str, i32> = LfuCache::new(cfg);
    cache.put("a", 1);
    assert_eq!(cache.get(&"a"), Some(1));
    assert_eq!(cache.remove(&"a"), Some(1));
    assert!(cache.get(&"a").is_none());
    cache.put("b", 2);
    cache.clear();
    assert!(cache.is_empty());
}

#[tokio::test]
async fn search_cache_manager_roundtrip() {
    let mut cfg = CacheConfig::default();
    cfg.max_entries = 100;
    let mgr = SearchCacheManager::new(cfg.clone(), cfg.clone(), cfg);
    let emb = vec![0.1f32; 128];
    let qh = QueryHash::new(&emb, 10, "conf");
    mgr.cache_query_results(qh.clone(), vec![(NodeId::new_v4(), 0.9)]);
    assert!(mgr.get_query_results(&qh).is_some());

    let nid = NodeId::new_v4();
    mgr.cache_embedding(nid, emb.clone());
    assert!(mgr.get_embedding(&nid).is_some());

    let ch = ContextHash::new(vec![nid], "ctx".into());
    mgr.cache_context_score(ch.clone(), 0.5);
    assert_eq!(mgr.get_context_score(&ch), Some(0.5));
}

#[tokio::test]
async fn query_hash_changes_with_config_or_k() {
    let emb = vec![0.2f32; 64];
    let a = QueryHash::new(&emb, 10, "a");
    let b = QueryHash::new(&emb, 10, "b");
    let c = QueryHash::new(&emb, 20, "a");
    assert_ne!(format!("{:?}", a), format!("{:?}", b));
    assert_ne!(format!("{:?}", a), format!("{:?}", c));
}

macro_rules! eviction_pattern_test {
    ($name:ident, $order:expr) => {
        #[tokio::test]
        async fn $name() {
            let cfg = CacheConfig { max_entries: 3, ttl: Duration::from_secs(60), cleanup_interval: Duration::from_secs(3600), enable_stats: true };
            let cache: LfuCache<i32, i32> = LfuCache::new(cfg);
            let order: &[i32] = &$order;
            for &k in order { cache.put(k, k*10); let _ = cache.get(&k); }
            // insert more to trigger evictions
            for k in 100..103 { cache.put(k, k*10); }
            // cache should not grow beyond capacity
            assert!(cache.len() <= 3);
        }
    };
}

eviction_pattern_test!(evict_pattern_1, [1,2,3,1,2]);
eviction_pattern_test!(evict_pattern_2, [3,2,1,2,3,2]);
eviction_pattern_test!(evict_pattern_3, [5,6,7,5,5]);
eviction_pattern_test!(evict_pattern_4, [10,11,12,13]);
eviction_pattern_test!(evict_pattern_5, [1,1,2,2,3,3]);
eviction_pattern_test!(evict_pattern_6, [7,8,9,7,8,10]);
eviction_pattern_test!(evict_pattern_7, [0,1,0,2,0,3]);
eviction_pattern_test!(evict_pattern_8, [4,4,4,5,6]);
eviction_pattern_test!(evict_pattern_9, [20,21,22,23,24]);
eviction_pattern_test!(evict_pattern_10, [100,101,100,102,103,101]);
eviction_pattern_test!(evict_pattern_11, [42,43,44,42,43,45]);

#[tokio::test]
async fn embedding_multiple_configs() {
    for dim in [32usize, 48, 96, 128] {
        let gen = EmbeddingGenerator::new(ModelConfig { dimension: dim, max_tokens: 64, model_name: "m".into() });
        let e = gen.generate_embedding(&sample_node("x")).await.unwrap();
        assert_eq!(e.len(), dim);
    }
}

#[tokio::test]
async fn lfu_cache_stats_update() {
    let cfg = CacheConfig { max_entries: 4, ttl: Duration::from_secs(60), cleanup_interval: Duration::from_secs(3600), enable_stats: true };
    let cache: LfuCache<i32, i32> = LfuCache::new(cfg);
    // miss
    assert!(cache.get(&10).is_none());
    // hits
    cache.put(1, 1);
    assert_eq!(cache.get(&1), Some(1));
    let stats = cache.get_stats();
    assert!(stats.hits >= 1);
    assert!(stats.misses >= 1);
}

#[tokio::test]
async fn lfu_cache_contains_and_len() {
    let cfg = CacheConfig { max_entries: 2, ttl: Duration::from_secs(60), cleanup_interval: Duration::from_secs(3600), enable_stats: false };
    let cache: LfuCache<&str, &str> = LfuCache::new(cfg);
    assert_eq!(cache.len(), 0);
    cache.put("k","v");
    assert!(cache.contains_key(&"k"));
    assert_eq!(cache.len(), 1);
}
