// Keep existing Serde field names, defaults and content decompression when using
// SurrealDB 3's native conversion contract.
#[cfg(feature = "surrealdb")]
macro_rules! impl_surreal_serde {
    ($($ty:ty),+ $(,)?) => { $(
        impl surrealdb::types::SurrealValue for $ty {
            fn kind_of() -> surrealdb::types::Kind {
                surrealdb::types::Kind::Any
            }
            fn into_value(self) -> surrealdb::types::Value {
                surrealdb::types::SurrealValue::into_value(surrealdb::types::SerdeWrapper(self))
            }
            fn from_value(value: surrealdb::types::Value) -> std::result::Result<Self, surrealdb::types::Error> {
                <surrealdb::types::SerdeWrapper<Self> as surrealdb::types::SurrealValue>::from_value(crate::serde_compatible_value(value))
                    .map(|wrapped| wrapped.0)
            }
        }
    )+ };
}
#[cfg(feature = "surrealdb")]
pub(crate) use impl_surreal_serde;

/// Our Serde DTOs represent database scalar types as strings. SDK 3's native
/// deserializer requires those conversions explicitly, including nested metadata.
#[cfg(feature = "surrealdb")]
fn serde_compatible_value(value: surrealdb::types::Value) -> surrealdb::types::Value {
    use surrealdb::types::{ToSql, Value};
    match value {
        Value::RecordId(id) => Value::String(id.to_sql()),
        Value::Datetime(time) => Value::String(time.to_string()),
        Value::Uuid(id) => Value::String(id.to_string()),
        Value::Duration(duration) => Value::String(duration.to_string()),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(serde_compatible_value).collect())
        }
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, serde_compatible_value(value)))
                .collect(),
        ),
        value => value,
    }
}

pub mod edge;

#[cfg(feature = "surrealdb")]
pub mod graph_functions;
#[cfg(feature = "surrealdb")]
pub mod surrealdb_migrations;
#[cfg(feature = "surrealdb")]
pub mod surrealdb_storage;
#[cfg(feature = "surrealdb")]
pub mod vector_indexes;

pub use edge::*;

#[cfg(feature = "surrealdb")]
pub use graph_functions::*;
#[cfg(feature = "surrealdb")]
pub use surrealdb_migrations::*;
#[cfg(feature = "surrealdb")]
pub use surrealdb_storage::*;
