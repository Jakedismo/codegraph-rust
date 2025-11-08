-- Migration 001: Initial Schema
-- This migration creates the core tables for CodeGraph with SurrealDB

-- Nodes table: Stores code entities (functions, classes, variables, etc.)
DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS name ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS node_type ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS file_path ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS start_line ON TABLE nodes TYPE option<number>;
DEFINE FIELD IF NOT EXISTS end_line ON TABLE nodes TYPE option<number>;
DEFINE FIELD IF NOT EXISTS embedding ON TABLE nodes TYPE option<array<float>>;
DEFINE FIELD IF NOT EXISTS complexity ON TABLE nodes TYPE option<float>;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE nodes TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE nodes TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE nodes TYPE datetime DEFAULT time::now();

-- Indexes for efficient queries on nodes
DEFINE INDEX IF NOT EXISTS idx_nodes_id ON TABLE nodes COLUMNS id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_nodes_name ON TABLE nodes COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_nodes_type ON TABLE nodes COLUMNS node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_language ON TABLE nodes COLUMNS language;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_path ON TABLE nodes COLUMNS file_path;

-- Edges table: Stores relationships between nodes
DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS edge_type ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS weight ON TABLE edges TYPE float DEFAULT 1.0;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE edges TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE edges TYPE datetime DEFAULT time::now();

-- Indexes for graph traversal on edges
DEFINE INDEX IF NOT EXISTS idx_edges_from ON TABLE edges COLUMNS from;
DEFINE INDEX IF NOT EXISTS idx_edges_to ON TABLE edges COLUMNS to;
DEFINE INDEX IF NOT EXISTS idx_edges_type ON TABLE edges COLUMNS edge_type;

-- Schema versions table: Tracks applied migrations
DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS version ON TABLE schema_versions TYPE number;
DEFINE FIELD IF NOT EXISTS name ON TABLE schema_versions TYPE string;
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS checksum ON TABLE schema_versions TYPE string;

DEFINE INDEX IF NOT EXISTS idx_schema_version ON TABLE schema_versions COLUMNS version UNIQUE;

-- Metadata table: Stores system-level metadata
DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS key ON TABLE metadata TYPE string;
DEFINE FIELD IF NOT EXISTS value ON TABLE metadata TYPE option<string | number | bool | object | array>;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE metadata TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_metadata_key ON TABLE metadata COLUMNS key UNIQUE;
