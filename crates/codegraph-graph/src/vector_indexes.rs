// ABOUTME: Controls vector-index construction without dropping shared existing indexes.
// ABOUTME: Selected and deferred modes build only requested dimensions and await readiness.
use codegraph_core::{CodeGraphError, Result};
use std::time::{Duration, Instant};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorIndexMode {
    All,
    Selected,
    Deferred,
    Off,
}
impl VectorIndexMode {
    pub fn from_env() -> Result<Self> {
        match std::env::var("CODEGRAPH_VECTOR_INDEX_MODE")
            .unwrap_or_else(|_| "all".into())
            .as_str()
        {
            "all" => Ok(Self::All),
            "selected" => Ok(Self::Selected),
            "deferred" => Ok(Self::Deferred),
            "off" => Ok(Self::Off),
            _ => Err(CodeGraphError::Configuration(
                "CODEGRAPH_VECTOR_INDEX_MODE must be all, selected, deferred or off".into(),
            )),
        }
    }
    /// Only fresh stores use this filter. Shared existing indexes are preserved.
    pub fn initial_schema(self, schema: &str) -> String {
        schema
            .lines()
            .filter(|line| {
                self == Self::All
                    || !line.trim_start().starts_with("DEFINE INDEX")
                    || !line.contains(" HNSW ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
fn definitions(dimension: usize, tables: &[&str]) -> Result<Vec<(String, String, String)>> {
    if ![384, 768, 1024, 1536, 2048, 2560, 3072, 3584, 4096].contains(&dimension) {
        return Err(CodeGraphError::Configuration(format!(
            "Unsupported vector index dimension {dimension}"
        )));
    }
    let positive = |name: &str, default, minimum, maximum| -> Result<usize> {
        let value = std::env::var(name)
            .ok()
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|_| CodeGraphError::Configuration(format!("Invalid {name}")))?
            .unwrap_or(default);
        if !(minimum..=maximum).contains(&value) {
            return Err(CodeGraphError::Configuration(format!(
                "{name} must be {minimum}..={maximum}"
            )));
        }
        Ok(value)
    };
    let m = positive("CODEGRAPH_HNSW_M", 12, 2, 128)?;
    let efc = positive("CODEGRAPH_HNSW_EFC", 150, m, 4096)?;
    tables.iter().map(|&table| {
        let prefix = match table {"nodes"=>"idx_nodes_embedding", "chunks"=>"idx_chunks_embedding", "symbol_embeddings"=>"idx_symbol_embeddings_vector", _=>return Err(CodeGraphError::Configuration("Unsupported vector table".into()))};
        let name = format!("{prefix}_{dimension}");
        // Omit CONCURRENTLY: the checked acknowledgement is a build barrier.
        let sql = format!("DEFINE INDEX IF NOT EXISTS {name} ON {table} FIELDS embedding_{dimension} HNSW DIMENSION {dimension} DIST COSINE TYPE F32 EFC {efc} M {m};");
        Ok((table.into(),name,sql))
    }).collect()
}

pub async fn ensure_ready(db: &Surreal<Any>, dimension: usize, tables: &[&str]) -> Result<bool> {
    if VectorIndexMode::from_env()? == VectorIndexMode::Off || tables.is_empty() {
        return Ok(false);
    }
    let timeout = Duration::from_secs(
        std::env::var("CODEGRAPH_VECTOR_INDEX_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300),
    );
    for (table, name, sql) in definitions(dimension, tables)? {
        tokio::time::timeout(timeout, db.query(sql))
            .await
            .map_err(|_| CodeGraphError::Database("Vector index construction timed out".into()))?
            .map_err(|e| CodeGraphError::Database(e.to_string()))?
            .check()
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        let start = Instant::now();
        loop {
            let mut result = db
                .query(format!("INFO FOR INDEX {name} ON {table};"))
                .await
                .map_err(|e| CodeGraphError::Database(e.to_string()))?
                .check()
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let info: Option<serde_json::Value> = result
                .take(0)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let info = info.unwrap_or_default();
            let status = info
                .get("building")
                .and_then(|v| v.get("status"))
                .or_else(|| info.get("status"))
                .and_then(|v| v.as_str());
            match status {
                Some("ready")
                    if info
                        .get("building")
                        .and_then(|value| value.get("compacting"))
                        .and_then(|value| value.as_bool())
                        != Some(true) =>
                {
                    break;
                }
                Some("error" | "failed") => {
                    return Err(CodeGraphError::Database(format!(
                        "Vector index {name} failed: {info}"
                    )));
                }
                _ if start.elapsed() >= timeout => {
                    return Err(CodeGraphError::Database(format!(
                        "Vector index {name} not ready: {info}"
                    )));
                }
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_store_filter_preserves_non_vector_indexes_in_both_schemas() {
        for schema in [
            include_str!("../../../schema/codegraph.surql"),
            include_str!("../../../schema/codegraph_v2.surql"),
        ] {
            let filtered = VectorIndexMode::Deferred.initial_schema(schema);
            assert!(
                !filtered
                    .lines()
                    .any(|line| line.starts_with("DEFINE INDEX") && line.contains(" HNSW "))
            );
            assert!(filtered.contains("DEFINE INDEX"));
            assert_eq!(
                VectorIndexMode::All.initial_schema(schema).lines().count(),
                schema.lines().count()
            );
        }
    }
    #[tokio::test]
    async fn selected_build_reports_readiness_and_creates_only_one_dimension() {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test").use_db("indexes").await.unwrap();
        db.query("DEFINE TABLE chunks SCHEMALESS; CREATE chunks:a SET embedding_384 = array::repeat(0.1,384);").await.unwrap().check().unwrap();
        assert!(ensure_ready(&db, 384, &["chunks"]).await.unwrap());
        let mut result = db
            .query("INFO FOR TABLE chunks")
            .await
            .unwrap()
            .check()
            .unwrap();
        let info: Option<serde_json::Value> = result.take(0).unwrap();
        let info = info.unwrap();
        assert_eq!(info["indexes"].as_object().unwrap().len(), 1);
        assert!(definitions(7, &["chunks"]).is_err());
        assert!(definitions(384, &["unsafe table"]).is_err());
    }
}
