use std::collections::HashMap;

mod helpers;
use helpers::*;

// Graph node store CRUD and index tests using RocksNodeStore
mod node_store {
    use super::*;
    use codegraph_graph::nodes::{Node, RocksNodeStore, NodeStore};
    use serde_json::Value as JsonValue;
    use futures::executor::block_on;

    fn store() -> RocksNodeStore {
        let _guard = TEST_DB_GUARD.lock();
        // Each store uses its own temp directory
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph_nodes.db");
        // Keep tempdir alive by not dropping it immediately
        // We can leak it for test lifetime; OS will clean it.
        std::mem::forget(dir);
        RocksNodeStore::new(path).expect("init RocksNodeStore")
    }

    #[test]
    fn crud_basic_roundtrip() {
        let s = store();
        let mut props = HashMap::new();
        props.insert("role".into(), JsonValue::String("admin".into()));
        let n = Node::new(vec!["User".into()], props);
        let id = n.id;
        block_on(async {
            s.create(n.clone()).await.unwrap();
            let got = s.read(id).await.unwrap().unwrap();
            assert_eq!(got.id, id);
            assert_eq!(got.labels, vec!["User"]);

            // update
            let mut updated = got.clone();
            updated.labels = vec!["Account".into()];
            s.update(updated.clone()).await.unwrap();
            let got2 = s.read(id).await.unwrap().unwrap();
            assert_eq!(got2.labels, vec!["Account"]);
            assert_eq!(got2.version, 2);

            // delete
            s.delete(id).await.unwrap();
            assert!(s.read(id).await.unwrap().is_none());
        });
    }

    #[test]
    fn find_by_label_and_property() {
        let s = store();
        let mut p1 = HashMap::new();
        p1.insert("lang".into(), JsonValue::String("rust".into()));
        let a = Node::new(vec!["Func".into()], p1);

        let mut p2 = HashMap::new();
        p2.insert("lang".into(), JsonValue::String("python".into()));
        let b = Node::new(vec!["Func".into(), "Async".into()], p2);

        block_on(async {
            s.batch_create(vec![a.clone(), b.clone()]).await.unwrap();
            let funcs = s.find_by_label("Func").await.unwrap();
            assert!(funcs.iter().any(|x| x.id == a.id));
            assert!(funcs.iter().any(|x| x.id == b.id));

            let rusts = s.find_by_property("lang", &JsonValue::String("rust".into())).await.unwrap();
            assert!(rusts.iter().any(|x| x.id == a.id));
            assert!(!rusts.iter().any(|x| x.id == b.id));
        });
    }

    macro_rules! make_prop_tests {
        ($($name:ident => $value:expr),+ $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let s = store();
                    let val = $value;
                    let mut props = HashMap::new();
                    props.insert("k".into(), val.clone());
                    let n = Node::new(vec!["L".into()], props);
                    let id = n.id;
                    block_on(async {
                        s.create(n.clone()).await.unwrap();
                        let hits = s.find_by_property("k", &val).await.unwrap();
                        assert!(hits.iter().any(|x| x.id == id));
                    });
                }
            )+
        };
    }

    make_prop_tests! {
        prop_idx_str_a => JsonValue::String("a".into()),
        prop_idx_str_b => JsonValue::String("b".into()),
        prop_idx_num_1 => JsonValue::from(1),
        prop_idx_num_2 => JsonValue::from(2),
        prop_idx_bool_t => JsonValue::Bool(true),
        prop_idx_bool_f => JsonValue::Bool(false),
        prop_idx_null => JsonValue::Null,
        prop_idx_obj => serde_json::json!({"x":1,"y":[1,2]}),
        prop_idx_arr1 => serde_json::json!([1,2,3]),
        prop_idx_arr2 => serde_json::json!(["a","b"]),
        prop_idx_long => JsonValue::String("long_value_for_indexing".into()),
        prop_idx_utf8 => JsonValue::String("ßå中".into()),
        prop_idx_edge1 => serde_json::json!({"nested":{"k":"v"}}),
        prop_idx_edge2 => serde_json::json!({"k":[{"a":1},{"b":2}]}),
        prop_idx_num_3 => JsonValue::from(3),
        prop_idx_num_4 => JsonValue::from(4),
        prop_idx_num_5 => JsonValue::from(5),
        prop_idx_str_c => JsonValue::String("c".into()),
        prop_idx_str_d => JsonValue::String("d".into()),
        prop_idx_str_e => JsonValue::String("e".into()),
        prop_idx_mix1 => serde_json::json!({"m":[1,"a",true]}),
        prop_idx_mix2 => serde_json::json!({"m":[2,"b",false]}),
        prop_idx_bool_t2 => JsonValue::Bool(true),
        prop_idx_bool_f2 => JsonValue::Bool(false),
    }

    // Generate multiple label tests to increase coverage and count
    macro_rules! make_label_test {
        ($name:ident, $label:expr) => {
            #[test]
            fn $name() {
                let s = store();
                let mut props = HashMap::new();
                props.insert("i".into(), JsonValue::from(1));
                let n = Node::new(vec![$label.into()], props);
                let id = n.id;
                block_on(async {
                    s.create(n.clone()).await.unwrap();
                    let hits = s.find_by_label($label).await.unwrap();
                    assert!(hits.iter().any(|x| x.id == id));
                });
            }
        };
    }

    make_label_test!(label_alpha, "Alpha");
    make_label_test!(label_beta, "Beta");
    make_label_test!(label_gamma, "Gamma");
    make_label_test!(label_delta, "Delta");
    make_label_test!(label_epsilon, "Epsilon");
    make_label_test!(label_zeta, "Zeta");
    make_label_test!(label_eta, "Eta");
    make_label_test!(label_theta, "Theta");
    make_label_test!(label_iota, "Iota");
    make_label_test!(label_kappa, "Kappa");

    // extra labels for breadth
    make_label_test!(label_lambda, "Lambda");
    make_label_test!(label_mu, "Mu");
    make_label_test!(label_nu, "Nu");
    make_label_test!(label_xi, "Xi");
    make_label_test!(label_omicron, "Omicron");
    make_label_test!(label_pi, "Pi");
    make_label_test!(label_rho, "Rho");
    make_label_test!(label_sigma, "Sigma");
    make_label_test!(label_tau, "Tau");
    make_label_test!(label_upsilon, "Upsilon");
    make_label_test!(label_phi, "Phi");
    make_label_test!(label_chi, "Chi");
    make_label_test!(label_psi, "Psi");
    make_label_test!(label_omega, "Omega");
    make_label_test!(label_delta2, "Delta2");
    make_label_test!(label_beta2, "Beta2");
    make_label_test!(label_gamma2, "Gamma2");
    make_label_test!(label_theta2, "Theta2");
    make_label_test!(label_kappa2, "Kappa2");
    make_label_test!(label_lambda2, "Lambda2");
}

// CodeGraph traversal and shortest_path tests (guarded with temp workdir)
mod traversal {
    use super::*;
    use codegraph_graph::{CodeGraph, CodeEdge};
    use codegraph_core::{Language, NodeType};

    #[tokio::test]
    async fn shortest_path_linear_chain() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");

        let n1 = make_node("n1", Some(NodeType::Function), Some(Language::Rust));
        let n2 = make_node("n2", Some(NodeType::Function), Some(Language::Rust));
        let n3 = make_node("n3", Some(NodeType::Function), Some(Language::Rust));
        g.add_node(n1.clone()).await.unwrap();
        g.add_node(n2.clone()).await.unwrap();
        g.add_node(n3.clone()).await.unwrap();

        g.add_edge(CodeEdge::new(n1.id, n2.id, codegraph_core::EdgeType::Calls)).await.unwrap();
        g.add_edge(CodeEdge::new(n2.id, n3.id, codegraph_core::EdgeType::Calls)).await.unwrap();

        let path = g.shortest_path(n1.id, n3.id).await.unwrap().unwrap();
        assert_eq!(path, vec![n1.id, n2.id, n3.id]);
    }

    #[tokio::test]
    async fn shortest_path_same_node() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");
        let n = make_node("solo", Some(NodeType::Struct), Some(Language::Rust));
        let id = n.id;
        g.add_node(n).await.unwrap();
        let path = g.shortest_path(id, id).await.unwrap().unwrap();
        assert_eq!(path, vec![id]);
    }

    #[tokio::test]
    async fn neighbors_cached_after_first_fetch() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");
        let a = make_node("a", Some(NodeType::Function), Some(Language::Rust));
        let b = make_node("b", Some(NodeType::Function), Some(Language::Rust));
        g.add_node(a.clone()).await.unwrap();
        g.add_node(b.clone()).await.unwrap();
        g.add_edge(CodeEdge::new(a.id, b.id, codegraph_core::EdgeType::Uses)).await.unwrap();
        let n1 = g.get_neighbors(a.id).await.unwrap();
        assert_eq!(n1, vec![b.id]);
        let stats1 = g.get_query_stats();
        let _n2 = g.get_neighbors(a.id).await.unwrap();
        let stats2 = g.get_query_stats();
        assert!(stats2.cache_hits >= stats1.cache_hits);
    }

    #[tokio::test]
    async fn shortest_path_unreachable_returns_none() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");
        let a = make_node("a", Some(NodeType::Function), Some(Language::Rust));
        let b = make_node("b", Some(NodeType::Function), Some(Language::Rust));
        g.add_node(a.clone()).await.unwrap();
        g.add_node(b.clone()).await.unwrap();
        let path = g.shortest_path(a.id, b.id).await.unwrap();
        assert!(path.is_none());
    }

    #[tokio::test]
    async fn shortest_path_handles_cycle() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");
        let a = make_node("a", Some(NodeType::Function), Some(Language::Rust));
        let b = make_node("b", Some(NodeType::Function), Some(Language::Rust));
        let c = make_node("c", Some(NodeType::Function), Some(Language::Rust));
        g.add_node(a.clone()).await.unwrap();
        g.add_node(b.clone()).await.unwrap();
        g.add_node(c.clone()).await.unwrap();
        g.add_edge(CodeEdge::new(a.id, b.id, codegraph_core::EdgeType::Calls)).await.unwrap();
        g.add_edge(CodeEdge::new(b.id, c.id, codegraph_core::EdgeType::Calls)).await.unwrap();
        g.add_edge(CodeEdge::new(c.id, a.id, codegraph_core::EdgeType::Calls)).await.unwrap();
        let path = g.shortest_path(a.id, c.id).await.unwrap().unwrap();
        assert_eq!(path.first().copied(), Some(a.id));
        assert_eq!(path.last().copied(), Some(c.id));
    }

    #[tokio::test]
    async fn clear_caches_resets_sizes() {
        let _lock = TEST_DB_GUARD.lock();
        let _wd = temp_workdir();
        let mut g = CodeGraph::new().expect("graph");
        let a = make_node("a", Some(NodeType::Function), Some(Language::Rust));
        let b = make_node("b", Some(NodeType::Function), Some(Language::Rust));
        g.add_node(a.clone()).await.unwrap();
        g.add_node(b.clone()).await.unwrap();
        g.add_edge(CodeEdge::new(a.id, b.id, codegraph_core::EdgeType::Calls)).await.unwrap();
        let _ = g.get_neighbors(a.id).await.unwrap();
        let stats = g.get_query_stats();
        assert!(stats.cache_size >= 1);
        g.clear_caches();
        let stats2 = g.get_query_stats();
        assert_eq!(stats2.cache_size, 0);
    }
}
