-- CodeGraph SurrealDB Schema Definition
-- ABOUTME: This file defines the complete database schema for CodeGraph's code analysis graph storage.
-- ABOUTME: It includes tables for nodes (code entities), edges (relationships), and supporting metadata tables.

-- =============================================================================
-- Database Configuration
-- =============================================================================
-- Note: Uncomment and modify the following lines to specify namespace and database
-- USE NS your_namespace;
-- USE DB codegraph;

-- =============================================================================
-- NODES TABLE - Code Entity Storage
-- =============================================================================
-- Stores individual code entities (functions, classes, variables, etc.)
-- with their metadata, embeddings, and location information

DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL;

-- Primary identifier (unique string ID)
DEFINE FIELD IF NOT EXISTS id ON TABLE nodes TYPE string;

-- Entity name (function name, class name, variable name, etc.)
DEFINE FIELD IF NOT EXISTS name ON TABLE nodes TYPE string;

-- Type of code entity (Function, Class, Method, Variable, etc.)
DEFINE FIELD IF NOT EXISTS node_type ON TABLE nodes TYPE option<string>;

-- Programming language of the entity
DEFINE FIELD IF NOT EXISTS language ON TABLE nodes TYPE option<string>;

-- Source code content of the entity
DEFINE FIELD IF NOT EXISTS content ON TABLE nodes TYPE option<string>;

-- File path where the entity is located
DEFINE FIELD IF NOT EXISTS file_path ON TABLE nodes TYPE option<string>;

-- Starting line number in the source file
DEFINE FIELD IF NOT EXISTS start_line ON TABLE nodes TYPE option<int>;

-- Ending line number in the source file
DEFINE FIELD IF NOT EXISTS end_line ON TABLE nodes TYPE option<int>;

-- Vector embedding for semantic similarity search
DEFINE FIELD IF NOT EXISTS embedding ON TABLE nodes TYPE option<array<float>>;

-- Complexity score for the code entity
DEFINE FIELD IF NOT EXISTS complexity ON TABLE nodes TYPE option<float>;

-- Additional metadata as flexible JSON object
DEFINE FIELD IF NOT EXISTS metadata ON TABLE nodes TYPE option<object>;

-- Timestamp when the record was created
DEFINE FIELD IF NOT EXISTS created_at ON TABLE nodes TYPE datetime DEFAULT time::now();

-- Timestamp when the record was last updated
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE nodes TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX IF NOT EXISTS idx_nodes_id ON TABLE nodes COLUMNS id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_nodes_name ON TABLE nodes COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_nodes_type ON TABLE nodes COLUMNS node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_language ON TABLE nodes COLUMNS language;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_path ON TABLE nodes COLUMNS file_path;

-- Vector similarity search index using HNSW
-- Note: Dimension must match your embedding provider
-- - Jina embeddings-v4: 2048 dimensions
-- - Ollama nomic-embed-code: 768 dimensions
-- - OpenAI text-embedding-3-small: 1536 dimensions
-- Update DIMENSION parameter based on your CODEGRAPH_EMBEDDING_PROVIDER
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_hnsw ON TABLE nodes FIELDS embedding HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;

-- =============================================================================
-- EDGES TABLE - Relationship Storage
-- =============================================================================
-- Stores relationships between code entities (calls, imports, inherits, etc.)
-- with metadata about the relationship type and weight

DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL;

-- Primary identifier for the edge
DEFINE FIELD IF NOT EXISTS id ON TABLE edges TYPE string;

-- Source node reference
DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record<nodes>;

-- Target node reference
DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record<nodes>;

-- Type of relationship (calls, imports, inherits, contains, etc.)
DEFINE FIELD IF NOT EXISTS edge_type ON TABLE edges TYPE string;

-- Weight/strength of the relationship (default: 1.0)
DEFINE FIELD IF NOT EXISTS weight ON TABLE edges TYPE float DEFAULT 1.0;

-- Additional relationship metadata
DEFINE FIELD IF NOT EXISTS metadata ON TABLE edges TYPE option<object>;

-- Timestamp when the relationship was created
DEFINE FIELD IF NOT EXISTS created_at ON TABLE edges TYPE datetime DEFAULT time::now();

-- Indexes for efficient relationship traversal
DEFINE INDEX IF NOT EXISTS idx_edges_from ON TABLE edges COLUMNS from;
DEFINE INDEX IF NOT EXISTS idx_edges_to ON TABLE edges COLUMNS to;
DEFINE INDEX IF NOT EXISTS idx_edges_type ON TABLE edges COLUMNS edge_type;

-- Composite index for efficient bidirectional edge queries
DEFINE INDEX IF NOT EXISTS idx_edges_from_to ON TABLE edges COLUMNS from, to;

-- =============================================================================
-- SCHEMA_VERSIONS TABLE - Migration Tracking
-- =============================================================================
-- Tracks applied schema migrations and their versions

DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;

-- Schema version number
DEFINE FIELD IF NOT EXISTS version ON TABLE schema_versions TYPE int;

-- Description of what this version includes
DEFINE FIELD IF NOT EXISTS description ON TABLE schema_versions TYPE string;

-- Timestamp when the migration was applied
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now();

-- Index for version lookups
DEFINE INDEX IF NOT EXISTS idx_schema_version ON TABLE schema_versions COLUMNS version UNIQUE;

-- =============================================================================
-- PROJECT_METADATA TABLE - Project Information
-- =============================================================================
-- Stores metadata about analyzed projects

DEFINE TABLE IF NOT EXISTS project_metadata SCHEMAFULL;

-- Project identifier
DEFINE FIELD IF NOT EXISTS project_id ON TABLE project_metadata TYPE string;

-- Project name
DEFINE FIELD IF NOT EXISTS name ON TABLE project_metadata TYPE string;

-- Project root path
DEFINE FIELD IF NOT EXISTS root_path ON TABLE project_metadata TYPE string;

-- Primary programming language
DEFINE FIELD IF NOT EXISTS primary_language ON TABLE project_metadata TYPE option<string>;

-- Last analysis timestamp
DEFINE FIELD IF NOT EXISTS last_analyzed ON TABLE project_metadata TYPE datetime;

-- Total number of files analyzed
DEFINE FIELD IF NOT EXISTS file_count ON TABLE project_metadata TYPE int DEFAULT 0;

-- Total number of nodes in the graph
DEFINE FIELD IF NOT EXISTS node_count ON TABLE project_metadata TYPE int DEFAULT 0;

-- Total number of edges in the graph
DEFINE FIELD IF NOT EXISTS edge_count ON TABLE project_metadata TYPE int DEFAULT 0;

-- Project-specific configuration and metadata
DEFINE FIELD IF NOT EXISTS metadata ON TABLE project_metadata TYPE option<object>;

-- Timestamp when the project was first indexed
DEFINE FIELD IF NOT EXISTS created_at ON TABLE project_metadata TYPE datetime DEFAULT time::now();

-- Timestamp when the project metadata was last updated
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE project_metadata TYPE datetime DEFAULT time::now();

-- Indexes for project lookups
DEFINE INDEX IF NOT EXISTS idx_project_id ON TABLE project_metadata COLUMNS project_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_project_name ON TABLE project_metadata COLUMNS name;

-- =============================================================================
-- Initial Schema Version
-- =============================================================================
-- Record the initial schema version

INSERT INTO schema_versions (version, description, applied_at)
VALUES (1, 'Initial CodeGraph schema with nodes, edges, and metadata tables', time::now())
ON DUPLICATE KEY UPDATE description = 'Initial CodeGraph schema with nodes, edges, and metadata tables';

-- =============================================================================
-- Example Queries
-- =============================================================================
-- Uncomment these to test the schema after applying it

-- Find all functions in a specific file
-- SELECT * FROM nodes WHERE file_path = '/path/to/file.rs' AND node_type = 'Function';

-- Find all calls from a specific function
-- SELECT * FROM edges WHERE edge_type = 'calls' AND from = nodes:some_function_id;

-- Find all nodes that import a specific module
-- SELECT ->from FROM edges WHERE edge_type = 'imports' AND to = nodes:module_id;

-- Get project statistics
-- SELECT * FROM project_metadata;

-- =============================================================================
-- Vector Similarity Search Examples (using HNSW index)
-- =============================================================================

-- Pure vector similarity search (K-nearest neighbors)
-- Find the 10 most similar nodes to a query embedding
-- LET $query_embedding = [0.1, 0.2, ...]; -- Your query vector (2048 dimensions for Jina)
-- SELECT id, name, file_path, node_type, vector::distance::knn() AS similarity_score
-- FROM nodes
-- WHERE embedding <|10,100|> $query_embedding  -- K=10 results, EF=100 candidate list size
-- ORDER BY similarity_score ASC;

-- Hybrid search: Vector search + keyword/metadata filters
-- Find similar functions in a specific directory
-- LET $query_embedding = [0.1, 0.2, ...];
-- SELECT id, name, file_path, vector::distance::knn() AS score
-- FROM nodes
-- WHERE
--     node_type = 'Function'
--     AND file_path ~ '/src/'
--     AND embedding <|5,80|> $query_embedding
-- ORDER BY score ASC;

-- Hybrid search: Vector + language filter
-- Find similar code entities in Rust files only
-- LET $query_embedding = [0.1, 0.2, ...];
-- SELECT id, name, file_path, language, vector::distance::knn() AS score
-- FROM nodes
-- WHERE
--     language = 'Rust'
--     AND embedding <|10,100|> $query_embedding
-- ORDER BY score ASC;

-- Full-text search combined with vector search (requires full-text index)
-- First, define a full-text index on the content field:
-- DEFINE ANALYZER code_analyzer TOKENIZERS class,punct FILTERS lowercase,ascii;
-- DEFINE INDEX IF NOT EXISTS idx_nodes_content_fulltext ON TABLE nodes FIELDS content SEARCH ANALYZER code_analyzer BM25;
--
-- Then perform hybrid search with fusion:
-- LET $query_embedding = [0.1, 0.2, ...];
-- LET $vector_results = SELECT id FROM nodes WHERE embedding <|20,100|> $query_embedding;
-- LET $text_results = SELECT id, search::score(1) AS score FROM nodes WHERE content @1@ 'authentication' ORDER BY score DESC LIMIT 20;
-- search::linear([$vector_results, $text_results], [2, 1], 10, 'zscore');  -- Fuse results with weights [2x vector, 1x text], return top 10
