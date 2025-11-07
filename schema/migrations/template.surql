-- Migration Template
-- Version: [VERSION_NUMBER]
-- Description: [SHORT_DESCRIPTION]
-- Date: [DATE]

-- =============================================================================
-- Migration: [MIGRATION_NAME]
-- =============================================================================

-- Add your schema changes here
-- Examples:

-- Add a new field
-- DEFINE FIELD IF NOT EXISTS new_field ON TABLE nodes TYPE string;

-- Add a new index
-- DEFINE INDEX IF NOT EXISTS idx_new_field ON TABLE nodes COLUMNS new_field;

-- Add a new table
-- DEFINE TABLE IF NOT EXISTS new_table SCHEMAFULL;
-- DEFINE FIELD IF NOT EXISTS id ON TABLE new_table TYPE string;

-- Modify existing data (use with caution)
-- UPDATE nodes SET new_field = "default_value" WHERE new_field IS NONE;

-- =============================================================================
-- Update Schema Version
-- =============================================================================

INSERT INTO schema_versions (version, description, applied_at)
VALUES ([VERSION_NUMBER], '[SHORT_DESCRIPTION]', time::now())
ON DUPLICATE KEY UPDATE description = '[SHORT_DESCRIPTION]';
