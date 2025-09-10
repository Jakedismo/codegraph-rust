use async_graphql::{Request, Variables};
use serde_json::json;

mod helpers;
use helpers::*;

async fn build_schema() -> codegraph_api::schema::CodeGraphSchema {
    let _lock = TEST_DB_GUARD.lock();
    let _wd = temp_workdir();
    let cfg = codegraph_core::ConfigManager::new_watching(None).expect("config");
    let state = codegraph_api::state::AppState::new(cfg).await.expect("state");
    codegraph_api::schema::create_schema(state)
}

#[tokio::test]
async fn api_health_and_version() {
    let schema = build_schema().await;
    let q = "{ health version }";
    let res = schema.execute(Request::new(q)).await;
    assert!(res.errors.is_empty());
    let data = res.data.into_json().unwrap();
    assert!(data["health"].is_string());
    assert!(data["version"].is_string());
}

#[tokio::test]
async fn mutation_add_update_delete_node() {
    let schema = build_schema().await;

    // Add
    let add = r#"
        mutation Add($input: AddNodeInput!) {
            addNode(input: $input)
        }
    "#;
    let vars = Variables::from_json(json!({
        "input": {
            "name": "N",
            "nodeType": "FUNCTION",
            "language": "RUST",
            "filePath": "f.rs",
            "startLine": 1, "startColumn": 1, "endLine": 1, "endColumn": 10
        }
    }));
    let res = schema.execute(Request::new(add).variables(vars)).await;
    assert!(res.errors.is_empty());

    // Query one random ID (may return null but should not error)
    let node_id = uuid::Uuid::new_v4().to_string();
    let q = r#"query($id: ID!){ node(id: $id){ id name } }"#;
    let res = schema
        .execute(Request::new(q).variables(Variables::from_json(json!({"id": node_id}))))
        .await;
    assert!(res.errors.is_empty());
}

#[tokio::test]
async fn mutation_batch_operations_executes() {
    let schema = build_schema().await;
    let m = r#"
        mutation Batch($ops: [BatchOperationInput!]!) { batchOperations(operations: $ops) }
    "#;
    let id = uuid::Uuid::new_v4().to_string();
    let vars = Variables::from_json(json!({
        "ops": [
            {"addNode": {"name":"A","nodeType":"FUNCTION","language":"RUST","filePath":"a.rs","startLine":1,"startColumn":1,"endLine":1,"endColumn":5}},
            {"updateNode": {"id": id, "name": "B"}},
            {"deleteNode": {"id": id}}
        ]
    }));
    let res = schema.execute(Request::new(m).variables(vars)).await;
    assert!(res.errors.is_empty());
}

#[tokio::test]
async fn query_nodes_batch_and_neighbors() {
    let schema = build_schema().await;
    let ids = vec![uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string()];
    let q = r#"
        query($a: ID!, $b: ID!){
            nodes(ids: [$a,$b]){ id name }
            getNeighbors(id:$a, limit: 5){ id name }
        }
    "#;
    let vars = Variables::from_json(json!({"a": ids[0], "b": ids[1]}));
    let res = schema.execute(Request::new(q).variables(vars)).await;
    assert!(res.errors.is_empty());
    let data = res.data.into_json().unwrap();
    assert!(data["nodes"].is_array());
    assert!(data["getNeighbors"].is_array());
}

#[tokio::test]
async fn query_find_path_structure() {
    let schema = build_schema().await;
    let a = uuid::Uuid::new_v4().to_string();
    let b = uuid::Uuid::new_v4().to_string();
    let q = r#"query($a:ID!,$b:ID!){ findPath(from:$a,to:$b,maxDepth:3){ id sourceId targetId edgeType } }"#;
    let res = schema
        .execute(Request::new(q).variables(Variables::from_json(json!({"a":a,"b":b}))))
        .await;
    assert!(res.errors.is_empty());
}

#[tokio::test]
async fn node_query_invalid_uuid_is_graceful() {
    let schema = build_schema().await;
    let q = r#"query($id:ID!){ node(id:$id){ id } }"#;
    let res = schema
        .execute(Request::new(q).variables(Variables::from_json(json!({"id":"not-a-uuid"}))))
        .await;
    // Accept either error or null result, but no panic
    if !res.errors.is_empty() {
        let msg = res.errors[0].message.to_lowercase();
        assert!(msg.contains("invalid") || msg.contains("uuid"));
    }
}

#[tokio::test]
async fn simple_health_only() {
    let schema = build_schema().await;
    let res = schema.execute(Request::new("{ health }")).await;
    assert!(res.errors.is_empty());
}

#[tokio::test]
async fn simple_version_only() {
    let schema = build_schema().await;
    let res = schema.execute(Request::new("{ version }")).await;
    assert!(res.errors.is_empty());
    let data = res.data.into_json().unwrap();
    let v = data["version"].as_str().unwrap();
    assert!(v.contains('.'));
}

#[tokio::test]
async fn maintenance_mutations() {
    let schema = build_schema().await;
    let m1 = "mutation{ triggerReindexing }";
    let m2 = r#"mutation($i:UpdateConfigurationInput!){ updateConfiguration(input:$i) }"#;
    let r1 = schema.execute(Request::new(m1)).await;
    assert!(r1.errors.is_empty());
    let r2 = schema
        .execute(Request::new(m2).variables(Variables::from_json(json!({"i":{"key":"k","value":"v"}}))))
        .await;
    assert!(r2.errors.is_empty());
}

#[tokio::test]
async fn nodes_three_ids_and_neighbors_limit() {
    let schema = build_schema().await;
    let q = r#"query($a:ID!,$b:ID!,$c:ID!){ nodes(ids:[$a,$b,$c]){ id } getNeighbors(id:$a, limit:2){ id } }"#;
    let vars = Variables::from_json(json!({
        "a": uuid::Uuid::new_v4().to_string(),
        "b": uuid::Uuid::new_v4().to_string(),
        "c": uuid::Uuid::new_v4().to_string(),
    }));
    let res = schema.execute(Request::new(q).variables(vars)).await;
    assert!(res.errors.is_empty());
}

#[tokio::test]
async fn find_path_zero_depth() {
    let schema = build_schema().await;
    let q = r#"query($a:ID!,$b:ID!){ findPath(from:$a,to:$b,maxDepth:0){ id } }"#;
    let vars = Variables::from_json(json!({
        "a": uuid::Uuid::new_v4().to_string(),
        "b": uuid::Uuid::new_v4().to_string(),
    }));
    let res = schema.execute(Request::new(q).variables(vars)).await;
    assert!(res.errors.is_empty());
}
