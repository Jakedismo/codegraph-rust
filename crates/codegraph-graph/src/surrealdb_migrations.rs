use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use surrealdb::{engine::any::Any, Surreal};
use std::sync::Arc;
use tracing::{info, warn};

/// Migration record stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub version: u32,
    pub name: String,
    pub applied_at: String,
    pub checksum: String,
}

/// Migration definition
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub description: String,
    /// SQL statements to apply the migration
    pub up_sql: Vec<String>,
    /// SQL statements to rollback the migration (optional)
    pub down_sql: Option<Vec<String>>,
}

/// Migration runner
pub struct MigrationRunner {
    db: Arc<Surreal<Any>>,
    migrations: Vec<Migration>,
}

impl MigrationRunner {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self {
            db,
            migrations: Self::default_migrations(),
        }
    }

    /// Get default migrations
    fn default_migrations() -> Vec<Migration> {
        vec![
            // Migration 1: Initial schema
            Migration {
                version: 1,
                name: "initial_schema".to_string(),
                description: "Create initial database schema".to_string(),
                up_sql: vec![
                    include_str!("../migrations/001_initial_schema.sql").to_string(),
                ],
                down_sql: Some(vec![
                    "REMOVE TABLE nodes;".to_string(),
                    "REMOVE TABLE edges;".to_string(),
                    "REMOVE TABLE schema_versions;".to_string(),
                    "REMOVE TABLE metadata;".to_string(),
                ]),
            },
            // Migration 2: Add performance indexes
            Migration {
                version: 2,
                name: "add_performance_indexes".to_string(),
                description: "Add indexes for better query performance".to_string(),
                up_sql: vec![
                    r#"
                    DEFINE INDEX IF NOT EXISTS idx_nodes_created_at ON TABLE nodes COLUMNS created_at;
                    DEFINE INDEX IF NOT EXISTS idx_nodes_updated_at ON TABLE nodes COLUMNS updated_at;
                    DEFINE INDEX IF NOT EXISTS idx_edges_created_at ON TABLE edges COLUMNS created_at;
                    "#.to_string(),
                ],
                down_sql: Some(vec![
                    "REMOVE INDEX idx_nodes_created_at ON TABLE nodes;".to_string(),
                    "REMOVE INDEX idx_nodes_updated_at ON TABLE nodes;".to_string(),
                    "REMOVE INDEX idx_edges_created_at ON TABLE edges;".to_string(),
                ]),
            },
            // Migration 3: Add embedding dimensions tracking
            Migration {
                version: 3,
                name: "add_embedding_metadata".to_string(),
                description: "Add metadata for embedding dimensions and model".to_string(),
                up_sql: vec![
                    r#"
                    DEFINE FIELD IF NOT EXISTS embedding_model ON TABLE nodes TYPE option<string>;
                    DEFINE FIELD IF NOT EXISTS embedding_dim ON TABLE nodes TYPE option<number>;
                    "#.to_string(),
                ],
                down_sql: Some(vec![
                    "REMOVE FIELD embedding_model ON TABLE nodes;".to_string(),
                    "REMOVE FIELD embedding_dim ON TABLE nodes;".to_string(),
                ]),
            },
        ]
    }

    /// Add a custom migration
    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.push(migration);
        // Sort by version to ensure correct order
        self.migrations.sort_by_key(|m| m.version);
    }

    /// Get current schema version from database
    pub async fn get_current_version(&self) -> Result<u32> {
        let query = "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1";
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to get schema version: {}", e)))?;

        #[derive(Deserialize)]
        struct VersionRecord {
            version: u32,
        }

        let versions: Vec<VersionRecord> = result
            .take(0)
            .unwrap_or_default();

        Ok(versions.first().map(|v| v.version).unwrap_or(0))
    }

    /// Run all pending migrations
    pub async fn migrate(&self) -> Result<u32> {
        let current_version = self.get_current_version().await?;
        info!("Current schema version: {}", current_version);

        let mut applied_count = 0;

        for migration in &self.migrations {
            if migration.version > current_version {
                info!(
                    "Applying migration {}: {} - {}",
                    migration.version, migration.name, migration.description
                );

                self.apply_migration(migration).await?;
                applied_count += 1;
            }
        }

        if applied_count > 0 {
            info!("Applied {} migration(s)", applied_count);
        } else {
            info!("No pending migrations");
        }

        Ok(applied_count)
    }

    /// Apply a single migration
    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        // Execute all UP SQL statements
        for sql in &migration.up_sql {
            self.db
                .query(sql)
                .await
                .map_err(|e| {
                    CodeGraphError::Database(format!(
                        "Failed to apply migration {} ({}): {}",
                        migration.version, migration.name, e
                    ))
                })?;
        }

        // Record migration in database
        let checksum = Self::calculate_checksum(&migration.up_sql);
        let record = MigrationRecord {
            version: migration.version,
            name: migration.name.clone(),
            applied_at: chrono::Utc::now().to_rfc3339(),
            checksum,
        };

        let _: Option<MigrationRecord> = self.db
            .create(("schema_versions", format!("v{}", migration.version)))
            .content(record)
            .await
            .map_err(|e| {
                CodeGraphError::Database(format!(
                    "Failed to record migration {}: {}",
                    migration.version, e
                ))
            })?;

        Ok(())
    }

    /// Rollback to a specific version
    pub async fn rollback(&self, target_version: u32) -> Result<u32> {
        let current_version = self.get_current_version().await?;

        if target_version >= current_version {
            warn!("Target version {} is not lower than current version {}", target_version, current_version);
            return Ok(0);
        }

        info!("Rolling back from version {} to {}", current_version, target_version);

        let mut rollback_count = 0;

        // Rollback migrations in reverse order
        for migration in self.migrations.iter().rev() {
            if migration.version > target_version && migration.version <= current_version {
                info!(
                    "Rolling back migration {}: {}",
                    migration.version, migration.name
                );

                self.rollback_migration(migration).await?;
                rollback_count += 1;
            }
        }

        info!("Rolled back {} migration(s)", rollback_count);
        Ok(rollback_count)
    }

    /// Rollback a single migration
    async fn rollback_migration(&self, migration: &Migration) -> Result<()> {
        if let Some(down_sql) = &migration.down_sql {
            // Execute all DOWN SQL statements
            for sql in down_sql {
                self.db
                    .query(sql)
                    .await
                    .map_err(|e| {
                        CodeGraphError::Database(format!(
                            "Failed to rollback migration {} ({}): {}",
                            migration.version, migration.name, e
                        ))
                    })?;
            }

            // Remove migration record
            let _: Option<MigrationRecord> = self.db
                .delete(("schema_versions", format!("v{}", migration.version)))
                .await
                .map_err(|e| {
                    CodeGraphError::Database(format!(
                        "Failed to remove migration record {}: {}",
                        migration.version, e
                    ))
                })?;

            Ok(())
        } else {
            Err(CodeGraphError::Database(format!(
                "Migration {} has no rollback defined",
                migration.version
            )))
        }
    }

    /// Verify migration checksums
    pub async fn verify(&self) -> Result<bool> {
        info!("Verifying migration integrity");

        let query = "SELECT * FROM schema_versions ORDER BY version";
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to query migrations: {}", e)))?;

        let applied: Vec<MigrationRecord> = result
            .take(0)
            .unwrap_or_default();

        let mut all_valid = true;

        for record in applied {
            if let Some(migration) = self.migrations.iter().find(|m| m.version == record.version) {
                let expected_checksum = Self::calculate_checksum(&migration.up_sql);
                if record.checksum != expected_checksum {
                    warn!(
                        "Checksum mismatch for migration {}: expected {}, got {}",
                        record.version, expected_checksum, record.checksum
                    );
                    all_valid = false;
                }
            } else {
                warn!("Unknown migration version {} in database", record.version);
                all_valid = false;
            }
        }

        if all_valid {
            info!("All migrations verified successfully");
        } else {
            warn!("Some migrations failed verification");
        }

        Ok(all_valid)
    }

    /// List all migrations and their status
    pub async fn status(&self) -> Result<Vec<MigrationStatus>> {
        let current_version = self.get_current_version().await?;

        let query = "SELECT * FROM schema_versions ORDER BY version";
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| CodeGraphError::Database(format!("Failed to query migrations: {}", e)))?;

        let applied: Vec<MigrationRecord> = result
            .take(0)
            .unwrap_or_default();

        let mut statuses = Vec::new();

        for migration in &self.migrations {
            let applied_record = applied.iter().find(|r| r.version == migration.version);
            let status = if let Some(record) = applied_record {
                MigrationStatus {
                    version: migration.version,
                    name: migration.name.clone(),
                    description: migration.description.clone(),
                    applied: true,
                    applied_at: Some(record.applied_at.clone()),
                    has_rollback: migration.down_sql.is_some(),
                }
            } else {
                MigrationStatus {
                    version: migration.version,
                    name: migration.name.clone(),
                    description: migration.description.clone(),
                    applied: false,
                    applied_at: None,
                    has_rollback: migration.down_sql.is_some(),
                }
            };
            statuses.push(status);
        }

        Ok(statuses)
    }

    /// Calculate checksum for SQL statements
    fn calculate_checksum(sql_statements: &[String]) -> String {
        use sha2::{Digest, Sha256};

        let combined = sql_statements.join("\n");
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Create a new migration file template
    pub fn create_migration_template(version: u32, name: &str) -> String {
        format!(
            r#"-- Migration {}: {}
-- Created: {}

-- UP Migration
-- Add your schema changes here

-- Example:
-- DEFINE TABLE IF NOT EXISTS my_table SCHEMAFULL;
-- DEFINE FIELD IF NOT EXISTS my_field ON TABLE my_table TYPE string;

-- DOWN Migration (for rollback)
-- Add rollback statements here

-- Example:
-- REMOVE TABLE my_table;
"#,
            version,
            name,
            chrono::Utc::now().to_rfc3339()
        )
    }
}

/// Migration status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub applied: bool,
    pub applied_at: Option<String>,
    pub has_rollback: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_calculation() {
        let sql = vec!["CREATE TABLE test;".to_string()];
        let checksum1 = MigrationRunner::calculate_checksum(&sql);
        let checksum2 = MigrationRunner::calculate_checksum(&sql);
        assert_eq!(checksum1, checksum2);

        let sql2 = vec!["CREATE TABLE test2;".to_string()];
        let checksum3 = MigrationRunner::calculate_checksum(&sql2);
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_migration_template() {
        let template = MigrationRunner::create_migration_template(1, "test_migration");
        assert!(template.contains("Migration 1"));
        assert!(template.contains("test_migration"));
        assert!(template.contains("UP Migration"));
        assert!(template.contains("DOWN Migration"));
    }
}
