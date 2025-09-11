use serde::{Deserialize, Deserializer, Serializer};

// Serde helpers for external types that don't implement Serialize/Deserialize

pub mod metric_type {
    use super::*;
    use faiss::MetricType;

    pub fn serialize<S>(mt: &MetricType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match mt {
            MetricType::InnerProduct => "inner_product",
            MetricType::L2 => "l2",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<MetricType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "ip" | "inner_product" | "innerproduct" => Ok(MetricType::InnerProduct),
            "l2" => Ok(MetricType::L2),
            other => Err(serde::de::Error::custom(format!(
                "invalid metric_type: {} (expected 'ip'/'inner_product' or 'l2')",
                other
            ))),
        }
    }
}
