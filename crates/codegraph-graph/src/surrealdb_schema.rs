use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{engine::any::Any, Surreal};
use tracing::{info, warn};

/// Schema definition for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub fields: Vec<FieldDefinition>,
    pub indexes: Vec<IndexDefinition>,
}

/// Field definition with type and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub optional: bool,
    pub default: Option<String>,
}

/// SurrealDB field types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Number,
    Float,
    Bool,
    DateTime,
    Array(Box<FieldType>),
    Object,
    Record(String), // Reference to another table
    Option(Box<FieldType>),
}

impl FieldType {
    pub fn to_surreal_type(&self) -> String {
        match self {
            FieldType::String => "string".to_string(),
            FieldType::Number => "number".to_string(),
            FieldType::Float => "float".to_string(),
            FieldType::Bool => "bool".to_string(),
            FieldType::DateTime => "datetime".to_string(),
            FieldType::Array(inner) => format!("array<{}>", inner.to_surreal_type()),
            FieldType::Object => "object".to_string(),
            FieldType::Record(table) => format!("record({})", table),
            FieldType::Option(inner) => format!("option<{}>", inner.to_surreal_type()),
        }
    }
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Migration definition
pub struct Migration {
    pub version: u32,
    pub description: String,
    pub up: Box<
        dyn Fn(
                &Surreal<Any>,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
            + Send
            + Sync,
    >,
    pub down: Option<
        Box<
            dyn Fn(
                    &Surreal<Any>,
                )
                    -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
                + Send
                + Sync,
        >,
    >,
}

/// Schema manager for flexible schema evolution
pub struct SchemaManager {
    db: Surreal<Any>,
    tables: HashMap<String, TableSchema>,
    migrations: Vec<Migration>,
}

impl SchemaManager {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db,
            tables: HashMap::new(),
            migrations: Vec::new(),
        }
    }

    /// Define a new table schema
    pub fn define_table(&mut self, schema: TableSchema) {
        self.tables.insert(schema.name.clone(), schema);
    }

    /// Add a migration
    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.push(migration);
    }

    /// Apply all table schemas
    pub async fn apply_schemas(&self) -> Result<()> {
        for (_, schema) in &self.tables {
            self.apply_table_schema(schema).await?;
        }
        Ok(())
    }

    /// Apply a single table schema
    async fn apply_table_schema(&self, schema: &TableSchema) -> Result<()> {
        info!("Applying schema for table: {}", schema.name);

        // Define table
        let mut ddl = format!("DEFINE TABLE IF NOT EXISTS {} SCHEMAFULL;", schema.name);

        // Define fields
        for field in &schema.fields {
            ddl.push_str(&format!(
                "\nDEFINE FIELD IF NOT EXISTS {} ON TABLE {} TYPE {};",
                field.name,
                schema.name,
                if field.optional {
                    format!("option<{}>", field.field_type.to_surreal_type())
                } else {
                    field.field_type.to_surreal_type()
                }
            ));

            if let Some(default) = &field.default {
                ddl.push_str(&format!(" DEFAULT {}", default));
            }
        }

        // Define indexes
        for index in &schema.indexes {
            let columns = index.columns.join(", ");
            let unique = if index.unique { " UNIQUE" } else { "" };
            ddl.push_str(&format!(
                "\nDEFINE INDEX IF NOT EXISTS {} ON TABLE {} COLUMNS {}{};\n",
                index.name, schema.name, columns, unique
            ));
        }

        // Execute DDL
        self.db.query(&ddl).await.map_err(|e| {
            CodeGraphError::Database(format!(
                "Failed to apply schema for table {}: {}",
                schema.name, e
            ))
        })?;

        Ok(())
    }

    /// Run migrations
    pub async fn migrate(&self, from_version: u32, to_version: u32) -> Result<()> {
        info!(
            "Running migrations from version {} to {}",
            from_version, to_version
        );

        for migration in &self.migrations {
            if migration.version > from_version && migration.version <= to_version {
                info!(
                    "Applying migration {}: {}",
                    migration.version, migration.description
                );

                (migration.up)(&self.db).await?;
            }
        }

        Ok(())
    }

    /// Rollback migrations
    pub async fn rollback(&self, from_version: u32, to_version: u32) -> Result<()> {
        warn!(
            "Rolling back migrations from version {} to {}",
            from_version, to_version
        );

        for migration in self.migrations.iter().rev() {
            if migration.version <= from_version && migration.version > to_version {
                if let Some(down) = &migration.down {
                    info!(
                        "Rolling back migration {}: {}",
                        migration.version, migration.description
                    );

                    down(&self.db).await?;
                } else {
                    warn!("No rollback defined for migration {}", migration.version);
                }
            }
        }

        Ok(())
    }

    /// Add a field to an existing table
    pub async fn add_field(&self, table: &str, field: FieldDefinition) -> Result<()> {
        info!("Adding field {} to table {}", field.name, table);

        let ddl = format!(
            "DEFINE FIELD IF NOT EXISTS {} ON TABLE {} TYPE {}{}",
            field.name,
            table,
            if field.optional {
                format!("option<{}>", field.field_type.to_surreal_type())
            } else {
                field.field_type.to_surreal_type()
            },
            field
                .default
                .as_ref()
                .map(|d| format!(" DEFAULT {}", d))
                .unwrap_or_default()
        );

        self.db
            .query(&ddl)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to add field: {}", e)))?;

        Ok(())
    }

    /// Add an index to an existing table
    pub async fn add_index(&self, table: &str, index: IndexDefinition) -> Result<()> {
        info!("Adding index {} to table {}", index.name, table);

        let columns = index.columns.join(", ");
        let unique = if index.unique { " UNIQUE" } else { "" };

        let ddl = format!(
            "DEFINE INDEX IF NOT EXISTS {} ON TABLE {} COLUMNS {}{}",
            index.name, table, columns, unique
        );

        self.db
            .query(&ddl)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to add index: {}", e)))?;

        Ok(())
    }

    /// Remove a field from a table (requires data migration)
    pub async fn remove_field(&self, table: &str, field: &str) -> Result<()> {
        warn!("Removing field {} from table {}", field, table);

        let ddl = format!("REMOVE FIELD {} ON TABLE {}", field, table);

        self.db
            .query(&ddl)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to remove field: {}", e)))?;

        Ok(())
    }

    /// Remove an index from a table
    pub async fn remove_index(&self, table: &str, index: &str) -> Result<()> {
        info!("Removing index {} from table {}", index, table);

        let ddl = format!("REMOVE INDEX {} ON TABLE {}", index, table);

        self.db
            .query(&ddl)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to remove index: {}", e)))?;

        Ok(())
    }

    /// Get current schema version
    pub async fn get_schema_version(&self) -> Result<u32> {
        #[derive(Deserialize)]
        struct VersionRecord {
            version: u32,
        }

        let query = "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1";
        let mut result = self.db.query(query).await.map_err(|e| {
            CodeGraphError::Database(format!("Failed to get schema version: {}", e))
        })?;

        let versions: Vec<VersionRecord> = result
            .take(0)
            .map_err(|e| CodeGraphError::Database(format!("Failed to extract version: {}", e)))?;

        Ok(versions.first().map(|v| v.version).unwrap_or(0))
    }

    /// Export current schema as JSON
    pub async fn export_schema(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.tables)?)
    }

    /// Import schema from JSON
    pub fn import_schema(&mut self, json: &str) -> Result<()> {
        let tables: HashMap<String, TableSchema> = serde_json::from_str(json)
            .map_err(|e| CodeGraphError::Parse(format!("Failed to import schema: {}", e)))?;

        self.tables = tables;
        Ok(())
    }
}

/// Helper function to create standard node schema
pub fn create_nodes_schema() -> TableSchema {
    TableSchema {
        name: "nodes".to_string(),
        fields: vec![
            FieldDefinition {
                name: "id".to_string(),
                field_type: FieldType::String,
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "name".to_string(),
                field_type: FieldType::String,
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "node_type".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::String)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "language".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::String)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "content".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::String)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "file_path".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::String)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "start_line".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Number)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "end_line".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Number)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "embedding".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Array(Box::new(
                    FieldType::Float,
                )))),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "complexity".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Float)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "metadata".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Object)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "created_at".to_string(),
                field_type: FieldType::DateTime,
                optional: false,
                default: Some("time::now()".to_string()),
            },
            FieldDefinition {
                name: "updated_at".to_string(),
                field_type: FieldType::DateTime,
                optional: false,
                default: Some("time::now()".to_string()),
            },
        ],
        indexes: vec![
            IndexDefinition {
                name: "idx_nodes_id".to_string(),
                columns: vec!["id".to_string()],
                unique: true,
            },
            IndexDefinition {
                name: "idx_nodes_name".to_string(),
                columns: vec!["name".to_string()],
                unique: false,
            },
            IndexDefinition {
                name: "idx_nodes_type".to_string(),
                columns: vec!["node_type".to_string()],
                unique: false,
            },
            IndexDefinition {
                name: "idx_nodes_language".to_string(),
                columns: vec!["language".to_string()],
                unique: false,
            },
            IndexDefinition {
                name: "idx_nodes_file_path".to_string(),
                columns: vec!["file_path".to_string()],
                unique: false,
            },
        ],
    }
}

/// Helper function to create edges schema
pub fn create_edges_schema() -> TableSchema {
    TableSchema {
        name: "edges".to_string(),
        fields: vec![
            FieldDefinition {
                name: "id".to_string(),
                field_type: FieldType::String,
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "from".to_string(),
                field_type: FieldType::Record("nodes".to_string()),
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "to".to_string(),
                field_type: FieldType::Record("nodes".to_string()),
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "edge_type".to_string(),
                field_type: FieldType::String,
                optional: false,
                default: None,
            },
            FieldDefinition {
                name: "weight".to_string(),
                field_type: FieldType::Float,
                optional: false,
                default: Some("1.0".to_string()),
            },
            FieldDefinition {
                name: "metadata".to_string(),
                field_type: FieldType::Option(Box::new(FieldType::Object)),
                optional: true,
                default: None,
            },
            FieldDefinition {
                name: "created_at".to_string(),
                field_type: FieldType::DateTime,
                optional: false,
                default: Some("time::now()".to_string()),
            },
        ],
        indexes: vec![
            IndexDefinition {
                name: "idx_edges_from".to_string(),
                columns: vec!["from".to_string()],
                unique: false,
            },
            IndexDefinition {
                name: "idx_edges_to".to_string(),
                columns: vec!["to".to_string()],
                unique: false,
            },
            IndexDefinition {
                name: "idx_edges_type".to_string(),
                columns: vec!["edge_type".to_string()],
                unique: false,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_conversion() {
        assert_eq!(FieldType::String.to_surreal_type(), "string");
        assert_eq!(FieldType::Number.to_surreal_type(), "number");
        assert_eq!(
            FieldType::Array(Box::new(FieldType::Float)).to_surreal_type(),
            "array<float>"
        );
        assert_eq!(
            FieldType::Option(Box::new(FieldType::String)).to_surreal_type(),
            "option<string>"
        );
    }

    #[test]
    fn test_schema_creation() {
        let schema = create_nodes_schema();
        assert_eq!(schema.name, "nodes");
        assert!(!schema.fields.is_empty());
        assert!(!schema.indexes.is_empty());
    }

    #[test]
    fn test_schema_export() {
        let schema = create_nodes_schema();
        let mut tables = HashMap::new();
        tables.insert(schema.name.clone(), schema);

        let json = serde_json::to_string_pretty(&tables).unwrap();
        assert!(json.contains("nodes"));
    }
}
