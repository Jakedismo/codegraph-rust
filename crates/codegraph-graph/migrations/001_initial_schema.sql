-- Initial schema migration for CodeGraph
-- This is a stub file to satisfy the build system
-- The actual schema is maintained in schema/codegraph.surql

-- Core tables
DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL;
DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL;
DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;
DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;

-- Note: Full schema definitions are in schema/codegraph.surql
-- This migration file is kept minimal to avoid duplication
