-- ABOUTME: CodeGraph schema
-- ABOUTME: Combines AST-driven code structure analysis with embeddings and graph structure

-- =============================================================================
-- CodeGraph SurrealDB Schema
-- Version: 1.0.0-alpha.0
-- =============================================================================
--
-- This schema unifies:
-- 1. CodeGraph: AST-driven code structure analysis (nodes, edges, functions)
--
-- =============================================================================

-- Optional namespace/database selection
-- USE NS unified;
-- USE DB codegraph;

-- =============================================================================
-- ANALYZER
-- =============================================================================

DEFINE ANALYZER code_analyzer
  TOKENIZERS blank, class
  FILTERS lowercase, snowball(english);

-- =============================================================================
-- CORE TABLES
-- =============================================================================

-- -----------------------------------------------------------------------------
-- TABLE: nodes
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL PERMISSIONS FULL
  COMMENT 'Code entities from AST parsing with semantic embeddings';

-- Core fields (id is built-in record type, no need to define it)
DEFINE FIELD IF NOT EXISTS name        ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS node_type   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language    ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content     ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS file_path   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS start_line  ON TABLE nodes TYPE option<int>;
DEFINE FIELD IF NOT EXISTS end_line    ON TABLE nodes TYPE option<int>;

DEFINE FIELD IF NOT EXISTS embedding_384   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD IF NOT EXISTS embedding_768   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD IF NOT EXISTS embedding_1024  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD IF NOT EXISTS embedding_1536  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD IF NOT EXISTS embedding_2048  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD IF NOT EXISTS embedding_2560  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD IF NOT EXISTS embedding_3072  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD IF NOT EXISTS embedding_4096  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;

DEFINE FIELD IF NOT EXISTS embedding_model ON TABLE nodes TYPE option<string>
  DEFAULT 'jina-embeddings-v4';

DEFINE FIELD IF NOT EXISTS complexity  ON TABLE nodes TYPE option<float>;
DEFINE FIELD IF NOT EXISTS metadata    ON TABLE nodes FLEXIBLE TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at  ON TABLE nodes TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at  ON TABLE nodes TYPE datetime VALUE time::now();

-- New fields for unified schema (non-breaking additions)
DEFINE FIELD IF NOT EXISTS project_id       ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS organization_id  ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS repository_url   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS chunk_count      ON TABLE nodes TYPE option<int>;

-- Indexes
DEFINE INDEX IF NOT EXISTS idx_nodes_name       ON TABLE nodes COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_nodes_type       ON TABLE nodes COLUMNS node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_language   ON TABLE nodes COLUMNS language;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_path  ON TABLE nodes COLUMNS file_path;
DEFINE INDEX IF NOT EXISTS idx_nodes_project    ON TABLE nodes COLUMNS project_id;

-- Full-text search indexes for semantic_search_with_context function
DEFINE INDEX IF NOT EXISTS idx_nodes_name_search ON TABLE nodes FIELDS name SEARCH ANALYZER code_analyzer BM25;
DEFINE INDEX IF NOT EXISTS idx_nodes_content_search ON TABLE nodes FIELDS content SEARCH ANALYZER code_analyzer BM25;

DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_384
    ON TABLE nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_768
    ON TABLE nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_1024
    ON TABLE nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_1536
    ON TABLE nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_2048
    ON TABLE nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_2560
    ON TABLE nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_3072
    ON TABLE nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_4096
    ON TABLE nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 200 M 16;

-- Composite indexes for common query patterns (P0 Enhancement)
DEFINE INDEX IF NOT EXISTS idx_nodes_project_type
    ON TABLE nodes COLUMNS project_id, node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_type
    ON TABLE nodes COLUMNS file_path, node_type;

-- -----------------------------------------------------------------------------
-- TABLE: chunks (chunked embeddings)
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS chunks SCHEMAFULL PERMISSIONS FULL
  COMMENT 'Chunked embeddings for long nodes (tokenizer-aware chunking)';
DEFINE FIELD IF NOT EXISTS parent_node   ON TABLE chunks TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS chunk_index   ON TABLE chunks TYPE int;
DEFINE FIELD IF NOT EXISTS text          ON TABLE chunks TYPE string;
DEFINE FIELD IF NOT EXISTS project_id    ON TABLE chunks TYPE string;
DEFINE FIELD IF NOT EXISTS embedding_384   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD IF NOT EXISTS embedding_768   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD IF NOT EXISTS embedding_1024  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD IF NOT EXISTS embedding_1536  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD IF NOT EXISTS embedding_2048  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD IF NOT EXISTS embedding_2560  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD IF NOT EXISTS embedding_3072  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD IF NOT EXISTS embedding_4096  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;
DEFINE FIELD IF NOT EXISTS embedding_model ON TABLE chunks TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at    ON TABLE chunks TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at    ON TABLE chunks TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_chunks_parent   ON TABLE chunks COLUMNS parent_node;
DEFINE INDEX IF NOT EXISTS idx_chunks_order    ON TABLE chunks COLUMNS parent_node, chunk_index;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_384
    ON TABLE chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_768
    ON TABLE chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_1024
    ON TABLE chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_1536
    ON TABLE chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_2048
    ON TABLE chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_2560
    ON TABLE chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_3072
    ON TABLE chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_chunks_embedding_4096
    ON TABLE chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 200 M 16;

-- -----------------------------------------------------------------------------
-- TABLE: edges
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL PERMISSIONS FULL
  COMMENT 'Code relationships (Calls, Imports, Uses, Extends, Implements, References)';

-- id is built-in record type, no need to define it
DEFINE FIELD IF NOT EXISTS from       ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to         ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS edge_type  ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS weight     ON TABLE edges TYPE float DEFAULT 1.0
  ASSERT $value > 0.0;
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE edges FLEXIBLE TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE edges TYPE datetime DEFAULT time::now() READONLY;

DEFINE INDEX IF NOT EXISTS idx_edges_from      ON TABLE edges COLUMNS from;
DEFINE INDEX IF NOT EXISTS idx_edges_to        ON TABLE edges COLUMNS to;
DEFINE INDEX IF NOT EXISTS idx_edges_type      ON TABLE edges COLUMNS edge_type;
DEFINE INDEX IF NOT EXISTS idx_edges_from_to   ON TABLE edges COLUMNS from, to;

-- Composite index for common query patterns (P0 Enhancement)
DEFINE INDEX IF NOT EXISTS idx_edges_type_from ON TABLE edges COLUMNS edge_type, from;

-- -----------------------------------------------------------------------------
-- TABLE: schema_versions
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS version     ON TABLE schema_versions TYPE int;
DEFINE FIELD IF NOT EXISTS name        ON TABLE schema_versions TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE schema_versions TYPE option<string>;
DEFINE FIELD IF NOT EXISTS applied_at  ON TABLE schema_versions TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS checksum    ON TABLE schema_versions TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_schema_version
    ON TABLE schema_versions COLUMNS version UNIQUE;

-- -----------------------------------------------------------------------------
-- TABLE: metadata
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS key        ON TABLE metadata TYPE string;
DEFINE FIELD IF NOT EXISTS value      ON TABLE metadata FLEXIBLE TYPE option<string | number | bool | object | array>;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE metadata TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_metadata_key
    ON TABLE metadata COLUMNS key UNIQUE;

-- -----------------------------------------------------------------------------
-- TABLE: project_metadata
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS project_metadata SCHEMAFULL
  PERMISSIONS FULL
  COMMENT 'Project registry with CodeGraph statistics';

-- Core fields
DEFINE FIELD IF NOT EXISTS project_id       ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS name             ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS root_path        ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS primary_language ON TABLE project_metadata TYPE option<string>;

-- CodeGraph statistics
DEFINE FIELD IF NOT EXISTS file_count       ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS node_count       ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS edge_count       ON TABLE project_metadata TYPE int DEFAULT 0;

-- Indexing metadata
DEFINE FIELD IF NOT EXISTS last_analyzed     ON TABLE project_metadata TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS codegraph_version ON TABLE project_metadata TYPE option<string>;

-- Cross-project fields
DEFINE FIELD IF NOT EXISTS organization_id  ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD IF NOT EXISTS domain           ON TABLE project_metadata TYPE option<string>;

-- Metadata
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE project_metadata FLEXIBLE TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE project_metadata TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE project_metadata TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_project_id
    ON TABLE project_metadata COLUMNS project_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_project_name
    ON TABLE project_metadata COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_project_org
    ON TABLE project_metadata COLUMNS organization_id;
DEFINE INDEX IF NOT EXISTS idx_project_domain
    ON TABLE project_metadata COLUMNS domain;

-- -----------------------------------------------------------------------------
-- TABLE: file_metadata (incremental indexing support)
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS file_metadata SCHEMAFULL
  COMMENT 'Tracks file state for incremental indexing and change detection';

-- Core identification
DEFINE FIELD IF NOT EXISTS file_path    ON TABLE file_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS project_id   ON TABLE file_metadata TYPE string;

-- Change detection
DEFINE FIELD IF NOT EXISTS content_hash ON TABLE file_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS modified_at  ON TABLE file_metadata TYPE datetime;
DEFINE FIELD IF NOT EXISTS file_size    ON TABLE file_metadata TYPE int;

-- Indexing metadata
DEFINE FIELD IF NOT EXISTS last_indexed_at ON TABLE file_metadata TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS node_count      ON TABLE file_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS edge_count      ON TABLE file_metadata TYPE int DEFAULT 0;

-- Additional metadata
DEFINE FIELD IF NOT EXISTS language     ON TABLE file_metadata TYPE option<string>;
DEFINE FIELD IF NOT EXISTS parse_errors ON TABLE file_metadata TYPE option<array<string>>;

-- Timestamps
DEFINE FIELD IF NOT EXISTS created_at   ON TABLE file_metadata TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at   ON TABLE file_metadata TYPE datetime VALUE time::now();

-- Indexes for efficient queries
DEFINE INDEX IF NOT EXISTS idx_file_metadata_composite
    ON TABLE file_metadata COLUMNS project_id, file_path UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_file_metadata_project
    ON TABLE file_metadata COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS idx_file_metadata_hash
    ON TABLE file_metadata COLUMNS content_hash;
DEFINE INDEX IF NOT EXISTS idx_file_metadata_modified
    ON TABLE file_metadata COLUMNS modified_at;

-- -----------------------------------------------------------------------------
-- TABLE: symbol_embeddings (AI-assisted symbol resolution cache)
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS symbol_embeddings SCHEMAFULL
    PERMISSIONS FULL
    COMMENT 'Cached embeddings for normalized symbols used during edge resolution';

-- id is built-in record type, no need to define it
DEFINE FIELD IF NOT EXISTS symbol            ON TABLE symbol_embeddings TYPE string;
DEFINE FIELD IF NOT EXISTS normalized_symbol ON TABLE symbol_embeddings TYPE string;
DEFINE FIELD IF NOT EXISTS project_id        ON TABLE symbol_embeddings TYPE option<string>;
DEFINE FIELD IF NOT EXISTS organization_id   ON TABLE symbol_embeddings TYPE option<string>;

DEFINE FIELD IF NOT EXISTS embedding_384   ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD IF NOT EXISTS embedding_768   ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD IF NOT EXISTS embedding_1024  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD IF NOT EXISTS embedding_1536  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD IF NOT EXISTS embedding_2048  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD IF NOT EXISTS embedding_2560  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD IF NOT EXISTS embedding_3072  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD IF NOT EXISTS embedding_4096  ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;

-- Per-element constraints (optional but kept)
DEFINE FIELD IF NOT EXISTS embedding_384[*]   ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_768[*]   ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_1024[*]  ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_1536[*]  ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_2048[*]  ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_2560[*]  ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_3072[*]  ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_4096[*]  ON TABLE symbol_embeddings TYPE float;

DEFINE FIELD IF NOT EXISTS embedding_model   ON TABLE symbol_embeddings TYPE string
  DEFAULT 'jina-embeddings-v4';
DEFINE FIELD IF NOT EXISTS last_computed_at  ON TABLE symbol_embeddings TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS access_count      ON TABLE symbol_embeddings TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS metadata          ON TABLE symbol_embeddings FLEXIBLE TYPE option<object>;

DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_symbol
    ON TABLE symbol_embeddings COLUMNS normalized_symbol;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_project_symbol
    ON TABLE symbol_embeddings COLUMNS project_id, normalized_symbol;

DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_384
    ON TABLE symbol_embeddings FIELDS embedding_384
    HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_768
    ON TABLE symbol_embeddings FIELDS embedding_768
    HNSW DIMENSION 768 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_1024
    ON TABLE symbol_embeddings FIELDS embedding_1024
    HNSW DIMENSION 1024 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_1536
    ON TABLE symbol_embeddings FIELDS embedding_1536
    HNSW DIMENSION 1536 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_2048
    ON TABLE symbol_embeddings FIELDS embedding_2048
    HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_2560
    ON TABLE symbol_embeddings FIELDS embedding_2560
    HNSW DIMENSION 2560 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_3072
    ON TABLE symbol_embeddings FIELDS embedding_3072
    HNSW DIMENSION 3072 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_4096
    ON TABLE symbol_embeddings FIELDS embedding_4096
    HNSW DIMENSION 4096 DIST COSINE EFC 200 M 16;

DEFINE FIELD IF NOT EXISTS node_id        ON TABLE symbol_embeddings TYPE option<record<nodes>>;
DEFINE FIELD IF NOT EXISTS source_edge_id ON TABLE symbol_embeddings TYPE option<record<edges>>;

-- Indexes for relation fields
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_node
    ON TABLE symbol_embeddings COLUMNS node_id;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_edge
    ON TABLE symbol_embeddings COLUMNS source_edge_id;

#=============================================================================
-- FUNCTIONS
-- =============================================================================

-- -----------------------------------------------------------------------------
-- FUNCTION: node_info (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::node_info($node_id: record) {
    LET $res = SELECT
        id,
        name,
        node_type AS kind,
        language,
        content,
        metadata,
        {
            file_path: file_path,
            start_line: start_line,
            end_line: end_line
        } AS location
    FROM ONLY $node_id;

    RETURN $res;
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: node_reference (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::node_reference($node_id: string) {
    LET $record = type::thing('nodes', $node_id);
    LET $info = fn::node_info($record);
    IF $info = NONE {
        RETURN NONE;
    };
    RETURN {
        id: $info.id,
        name: $info.name,
        kind: $info.kind,
        location: $info.location
    };
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: find_nodes_by_name (PROJECT SCOPED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::find_nodes_by_name($project_id: string, $needle: string, $limit: int) {
    LET $max = IF $limit != NONE AND $limit > 0 THEN $limit ELSE 10 END;
    RETURN SELECT
        id,
        name,
        node_type AS kind,
        language,
        metadata,
        {
            file_path: file_path,
            start_line: start_line,
            end_line: end_line
        } AS location
    FROM nodes
    WHERE project_id = $project_id
      AND (
          string::lowercase(name) CONTAINS string::lowercase($needle)
          OR file_path CONTAINS $needle
      )
    ORDER BY name
    LIMIT $max;
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: edge_types (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::edge_types() {
    RETURN ['calls', 'imports', 'uses', 'extends', 'implements', 'references'];
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: get_transitive_dependencies (FIXED - uses JOIN instead of arrows)
-- Arrow syntax only works with RELATE-created edges, not regular tables
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::get_transitive_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
    LET $edge_name = string::lowercase($edge_type ?? 'calls');

    -- Normalize node_id to record type
    LET $record = IF type::is::record($node_id) THEN $node_id ELSE (
        IF string::starts_with(<string>$node_id, "nodes:")
        THEN type::thing(<string>$node_id)
        ELSE type::thing('nodes', <string>$node_id)
        END
    ) END;

    -- Verify node belongs to project (allow NULL for backwards compatibility)
    LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
    IF $rec_proj != NONE AND $rec_proj != $project_id {
        RETURN [];
    };

    -- Level 1: Direct dependencies via JOIN (not arrow syntax)
    LET $lvl1 = (
        SELECT VALUE to FROM edges
        WHERE from = $record
          AND edge_type = $edge_name
          AND (to.project_id = $project_id OR to.project_id = NONE)
    );

    -- Level 2: Dependencies of level 1
    LET $lvl2 = IF $safe_depth >= 2 AND array::len($lvl1) > 0 THEN (
        SELECT VALUE to FROM edges
        WHERE from INSIDE $lvl1
          AND edge_type = $edge_name
          AND (to.project_id = $project_id OR to.project_id = NONE)
          AND to NOTINSIDE $lvl1
          AND to != $record
    ) ELSE [] END;

    -- Level 3: Dependencies of level 2
    LET $lvl3 = IF $safe_depth >= 3 AND array::len($lvl2) > 0 THEN (
        SELECT VALUE to FROM edges
        WHERE from INSIDE $lvl2
          AND edge_type = $edge_name
          AND (to.project_id = $project_id OR to.project_id = NONE)
          AND to NOTINSIDE array::concat($lvl1, $lvl2)
          AND to != $record
    ) ELSE [] END;

    -- Level 4: Dependencies of level 3
    LET $lvl4 = IF $safe_depth >= 4 AND array::len($lvl3) > 0 THEN (
        SELECT VALUE to FROM edges
        WHERE from INSIDE $lvl3
          AND edge_type = $edge_name
          AND (to.project_id = $project_id OR to.project_id = NONE)
          AND to NOTINSIDE array::concat($lvl1, $lvl2, $lvl3)
          AND to != $record
    ) ELSE [] END;

    -- Level 5: Dependencies of level 4
    LET $lvl5 = IF $safe_depth >= 5 AND array::len($lvl4) > 0 THEN (
        SELECT VALUE to FROM edges
        WHERE from INSIDE $lvl4
          AND edge_type = $edge_name
          AND (to.project_id = $project_id OR to.project_id = NONE)
          AND to NOTINSIDE array::concat($lvl1, $lvl2, $lvl3, $lvl4)
          AND to != $record
    ) ELSE [] END;

    -- Build results with depth tracking
    LET $pairs = array::concat(
        array::map($lvl1, |$n| { id: $n, depth: 1 }),
        array::map($lvl2, |$n| { id: $n, depth: 2 }),
        array::map($lvl3, |$n| { id: $n, depth: 3 }),
        array::map($lvl4, |$n| { id: $n, depth: 4 }),
        array::map($lvl5, |$n| { id: $n, depth: 5 })
    );

    -- Deduplicate keeping minimum depth
    LET $min_depths = (SELECT id, math::min(depth) AS dependency_depth FROM $pairs GROUP BY id);

    -- Enrich with node info
    LET $raw = (SELECT fn::node_info(id) AS node, dependency_depth FROM $min_depths);

    RETURN (
        SELECT
            node.id AS id,
            node.name AS name,
            node.kind AS kind,
            node.location AS location,
            node.language AS language,
            node.content AS content,
            node.metadata AS metadata,
            dependency_depth,
            $safe_depth AS requested_depth
        FROM $raw
        WHERE node != NONE
    );
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: detect_circular_dependencies (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
    LET $edge_name = string::lowercase($edge_type ?? 'Calls');

    LET $pairs = (
        SELECT
            from AS node1_id,
            to   AS node2_id
        FROM edges
        WHERE edge_type = $edge_name
          AND from != to
          AND (SELECT VALUE project_id FROM from) = $project_id
          AND (SELECT VALUE project_id FROM to) = $project_id
    );

    LET $cycles = (
        SELECT
            node1_id,
            node2_id
        FROM $pairs
        WHERE node1_id < node2_id
          AND (
              SELECT VALUE count()
              FROM edges
              WHERE edge_type = $edge_name
                AND from = node2_id
                AND to   = node1_id
                AND (SELECT VALUE project_id FROM from) = $project_id
                AND (SELECT VALUE project_id FROM to) = $project_id
          ) > 0
        GROUP BY node1_id, node2_id
    );

    LET $raw = (
        SELECT
            node1_id,
            node2_id,
            fn::node_info(node1_id) AS node1,
            fn::node_info(node2_id) AS node2
        FROM $cycles
    );

    RETURN SELECT
        node1_id,
        node2_id,
        $edge_name AS dependency_type,
        node1,
        node2
    FROM $raw
    WHERE node1 != NONE AND node2 != NONE;
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: trace_call_chain (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::trace_call_chain($project_id: string, $from_node: any, $max_depth: int) {
    LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;
    LET $record = IF type::is::record($from_node) THEN $from_node ELSE (
        IF string::starts_with($from_node, "nodes:") THEN type::thing($from_node) ELSE type::thing('nodes', $from_node) END
    ) END;

    LET $rec_proj = SELECT VALUE project_id FROM ONLY $record;
    IF $rec_proj != NONE AND $rec_proj != $project_id {
        RETURN [];
    };

    LET $raw = (
        SELECT
            fn::node_info(id) AS node,
            array::distinct((
                SELECT fn::node_reference(from) AS caller
                FROM edges
                WHERE to = id
                  AND edge_type = 'Calls'
                  AND (SELECT VALUE project_id FROM from) = $project_id
                  AND (SELECT VALUE project_id FROM to) = $project_id
            ).caller) AS called_by
        FROM (
            SELECT ->edges[WHERE edge_type = 'Calls' AND (to.project_id ?? $project_id) = $project_id]
            FROM ONLY $record
        )->to
    );

    RETURN SELECT
        node.id       AS id,
        node.name     AS name,
        node.kind     AS kind,
        node.location AS location,
        node.language AS language,
        node.content  AS content,
        node.metadata AS metadata,
        1 AS call_depth,
        called_by,
        $safe_depth AS requested_depth
    FROM $raw
    WHERE node != NONE;
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: calculate_coupling_metrics (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
    LET $edge_list = array::map(fn::edge_types(), |$v| string::lowercase($v));
    LET $record = IF type::is::record($node_id) THEN $node_id ELSE (
        IF string::starts_with($node_id, "nodes:") THEN type::thing($node_id) ELSE type::thing('nodes', $node_id) END
    ) END;

    LET $rec_proj = SELECT VALUE project_id FROM ONLY $record;
    IF $rec_proj != NONE AND $rec_proj != $project_id {
        RETURN NONE;
    };

    LET $dependents = array::distinct(
        SELECT VALUE id
        FROM ONLY $record
            <-edges[WHERE edge_type INSIDE $edge_list AND (from.project_id ?? $project_id) = $project_id AND (to.project_id ?? $project_id) = $project_id]
            <-from
    );

    LET $dependencies = array::distinct(
        SELECT VALUE id
        FROM ONLY $record
            ->edges[WHERE edge_type INSIDE $edge_list AND (to.project_id ?? $project_id) = $project_id]
            ->to
    );

    LET $dependents_info = (
        SELECT VALUE fn::node_reference(value)
        FROM $dependents
        WHERE fn::node_reference(value) != NONE
    );

    LET $dependencies_info = (
        SELECT VALUE fn::node_reference(value)
        FROM $dependencies
        WHERE fn::node_reference(value) != NONE
    );

    LET $afferent = array::len($dependents_info);
    LET $efferent = array::len($dependencies_info);
    LET $total = $afferent + $efferent;
    LET $instability = IF $total > 0 THEN math::round($efferent / $total, 6) ELSE 0 END;

    RETURN {
        node: fn::node_info($record),
        metrics: {
            afferent_coupling: $afferent,
            efferent_coupling: $efferent,
            total_coupling: $total,
            instability: $instability,
            stability: 1.0 - $instability,
            is_stable: $instability < 0.3,
            is_unstable: $instability > 0.7,
            coupling_category: IF $instability < 0.3 THEN 'stable'
                               ELSE IF $instability > 0.7 THEN 'unstable'
                               ELSE 'balanced' END
        },
        dependents: $dependents_info,
        dependencies: $dependencies_info
    };
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: get_hub_nodes (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::get_hub_nodes($project_id: string, $min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
    -- Filter out NONE values and safely lowercase edge types
    LET $raw_types = fn::edge_types();
    LET $edge_list = array::map(array::filter($raw_types, |$v| $v != NONE), |$v| string::lowercase($v));

    LET $project_edges = (
        SELECT from, to, edge_type
        FROM edges
        WHERE edge_type INSIDE $edge_list
          AND (SELECT VALUE project_id FROM from) = $project_id
          AND (SELECT VALUE project_id FROM to) = $project_id
    );

    LET $incoming_by_type = (
        SELECT
            to AS node_id,
            edge_type,
            count() AS count
        FROM $project_edges
        GROUP BY to, edge_type
    );

    LET $outgoing_by_type = (
        SELECT
            from AS node_id,
            edge_type,
            count() AS count
        FROM $project_edges
        GROUP BY from, edge_type
    );

    LET $incoming_totals = (
        SELECT
            to AS node_id,
            count() AS total
        FROM $project_edges
        GROUP BY to
    );

    LET $outgoing_totals = (
        SELECT
            from AS node_id,
            count() AS total
        FROM $project_edges
        GROUP BY from
    );

    LET $candidates = array::distinct(
      array::concat(
        (SELECT VALUE node_id FROM $incoming_totals),
        (SELECT VALUE node_id FROM $outgoing_totals)
      )
    );

    LET $raw = (
        SELECT
            candidate_id,
            fn::node_info(candidate_id) AS node,
            (array::first(SELECT VALUE total FROM $incoming_totals WHERE node_id = candidate_id LIMIT 1) ?? 0) AS afferent_degree,
            (array::first(SELECT VALUE total FROM $outgoing_totals WHERE node_id = candidate_id LIMIT 1) ?? 0) AS efferent_degree,
            (
                SELECT edge_type, count
                FROM $incoming_by_type
                WHERE node_id = candidate_id
            ) AS incoming_by_type,
            (
                SELECT edge_type, count
                FROM $outgoing_by_type
                WHERE node_id = candidate_id
            ) AS outgoing_by_type
        FROM (
            SELECT node_id AS candidate_id FROM $candidates
        )
    );

    RETURN SELECT
        candidate_id AS node_id,
        node,
        afferent_degree,
        efferent_degree,
        afferent_degree + efferent_degree AS total_degree,
        incoming_by_type,
        outgoing_by_type
    FROM $raw
    WHERE node != NONE
      AND (afferent_degree + efferent_degree) >= $threshold
    ORDER BY total_degree DESC;
}PERMISSIONS FULL;
-- -----------------------------------------------------------------------------
-- FUNCTION: get_reverse_dependencies (FIXED - uses JOIN instead of arrows)
-- Finds nodes that depend ON the given node (who calls this?)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION  fn::get_reverse_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
    LET $edge_name = string::lowercase($edge_type ?? 'calls');

    -- Normalize node_id to record type
    LET $record = IF type::is::record($node_id) THEN $node_id ELSE (
        IF string::starts_with(<string>$node_id, "nodes:")
        THEN type::thing(<string>$node_id)
        ELSE type::thing('nodes', <string>$node_id)
        END
    ) END;

    -- Verify node belongs to project (allow NULL for backwards compatibility)
    LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
    IF $rec_proj != NONE AND $rec_proj != $project_id {
        RETURN [];
    };

    -- Level 1: Direct dependents via JOIN (who depends on this node?)
    LET $lvl1 = (
        SELECT VALUE from FROM edges
        WHERE to = $record
          AND edge_type = $edge_name
          AND (from.project_id = $project_id OR from.project_id = NONE)
    );

    -- Level 2: Dependents of level 1
    LET $lvl2 = IF $safe_depth >= 2 AND array::len($lvl1) > 0 THEN (
        SELECT VALUE from FROM edges
        WHERE to INSIDE $lvl1
          AND edge_type = $edge_name
          AND (from.project_id = $project_id OR from.project_id = NONE)
          AND from NOTINSIDE $lvl1
          AND from != $record
    ) ELSE [] END;

    -- Level 3: Dependents of level 2
    LET $lvl3 = IF $safe_depth >= 3 AND array::len($lvl2) > 0 THEN (
        SELECT VALUE from FROM edges
        WHERE to INSIDE $lvl2
          AND edge_type = $edge_name
          AND (from.project_id = $project_id OR from.project_id = NONE)
          AND from NOTINSIDE array::concat($lvl1, $lvl2)
          AND from != $record
    ) ELSE [] END;

    -- Level 4: Dependents of level 3
    LET $lvl4 = IF $safe_depth >= 4 AND array::len($lvl3) > 0 THEN (
        SELECT VALUE from FROM edges
        WHERE to INSIDE $lvl3
          AND edge_type = $edge_name
          AND (from.project_id = $project_id OR from.project_id = NONE)
          AND from NOTINSIDE array::concat($lvl1, $lvl2, $lvl3)
          AND from != $record
    ) ELSE [] END;

    -- Level 5: Dependents of level 4
    LET $lvl5 = IF $safe_depth >= 5 AND array::len($lvl4) > 0 THEN (
        SELECT VALUE from FROM edges
        WHERE to INSIDE $lvl4
          AND edge_type = $edge_name
          AND (from.project_id = $project_id OR from.project_id = NONE)
          AND from NOTINSIDE array::concat($lvl1, $lvl2, $lvl3, $lvl4)
          AND from != $record
    ) ELSE [] END;

    -- Build results with depth tracking
    LET $pairs = array::concat(
        array::map($lvl1, |$n| { id: $n, depth: 1 }),
        array::map($lvl2, |$n| { id: $n, depth: 2 }),
        array::map($lvl3, |$n| { id: $n, depth: 3 }),
        array::map($lvl4, |$n| { id: $n, depth: 4 }),
        array::map($lvl5, |$n| { id: $n, depth: 5 })
    );

    -- Deduplicate keeping minimum depth
    LET $min_depths = (SELECT id, math::min(depth) AS dependent_depth FROM $pairs GROUP BY id);

    -- Enrich with node info
    LET $raw = (SELECT fn::node_info(id) AS node, dependent_depth FROM $min_depths);

    RETURN (
        SELECT
            node.id AS id,
            node.name AS name,
            node.kind AS kind,
            node.location AS location,
            node.language AS language,
            node.content AS content,
            node.metadata AS metadata,
            dependent_depth,
            $safe_depth AS requested_depth
        FROM $raw
        WHERE node != NONE
    );
} PERMISSIONS FULL;
-- =============================================================================
-- ABOUTME: Comprehensive semantic search function for CodeGraph agentic tools
-- ABOUTME: Combines HNSW vector search, full-text analyzer search, and graph enrichment

-- =============================================================================
-- FUNCTION: semantic_search_with_context
-- =============================================================================
--
-- Performs comprehensive code search combining:
-- 1. HNSW vector similarity search (semantic)
-- 2. Full-text search using code_analyzer (exact+fuzzy)
-- 3. Graph enrichment with dependencies and context
-- 4. Returns candidates for optional reranking in Rust layer
--
-- Parameters:
--   $project_id: string          - Project scope
--   $query_embedding: array<float> - Query embedding vector
--   $query_text: string          - Original query text for full-text search
--   $dimension: int              - Embedding dimension (384,768,1024,1536,2048,2560,3072,4096)
--   $limit: int                  - Maximum results to return
--   $threshold: float            - Minimum similarity score (0.0-1.0, default 0.7)
--   $include_graph_context: bool - Whether to enrich with graph data
--
-- Returns: Array of nodes with:
--   - Node information (id, name, kind, location, content)
--   - Search scores (vector_score, text_score, combined_score)
--   - Graph context (dependencies, dependents, file_siblings) if enabled
--
-- Example:
--   fn::semantic_search_with_context(
--     "my-project",
--     [0.1, 0.2, ...],  -- 2048-dim embedding
--     "JWT authentication",
--     2048,
--     10,
--     0.7,
--     true
--   )
-- =============================================================================
DEFINE FUNCTION fn::semantic_search_with_context(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $include_graph_context: bool
) {
    --------------------------------------------------------------------------
    -- PARAM SAFETY
    --------------------------------------------------------------------------
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold =
        IF $threshold >= 0.0 AND $threshold <= 1.0
        THEN $threshold
        ELSE 0.7
        END;

    --------------------------------------------------------------------------
    -- STAGE 1: HNSW VECTOR SEARCH (semantic similarity)
    --------------------------------------------------------------------------
    LET $vector_candidates = IF $dimension = 384 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_384 IS NOT NONE
          AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 768 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_768 IS NOT NONE
          AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 1024 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_1024 IS NOT NONE
          AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 1536 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_1536 IS NOT NONE
          AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 2048 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_2048 IS NOT NONE
          AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 2560 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_2560 IS NOT NONE
          AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 3072 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_3072 IS NOT NONE
          AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 4096 THEN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
        FROM nodes
        WHERE project_id = $project_id
          AND embedding_4096 IS NOT NONE
          AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    ) ELSE [] END;

    --------------------------------------------------------------------------
    -- STAGE 2: FULL-TEXT SEARCH
    --------------------------------------------------------------------------
    LET $text_candidates = SELECT
        id,
        name,
        node_type AS kind,
        language,
        content,
        file_path,
        start_line,
        end_line,
        metadata,
        0.0 AS vector_score,
        search::score(1) AS text_score
    FROM nodes
    WHERE project_id = $project_id
      AND (
          name    @1@ $query_text
          OR content @2@ $query_text
      )
    ORDER BY text_score DESC
    LIMIT $safe_limit;

    --------------------------------------------------------------------------
    -- STAGE 3: HYBRID MERGE (vector + text)
    --------------------------------------------------------------------------
    LET $all_candidates_scored = array::concat(
        (
            SELECT
                *,
                (vector_score * 0.7) + (0.0 * 0.3) AS combined_score
            FROM $vector_candidates
        ),
        (
            SELECT
                *,
                (0.0 * 0.7) + (text_score * 0.3) AS combined_score
            FROM $text_candidates
        )
    );

    LET $sorted = (
        SELECT * FROM $all_candidates_scored
        ORDER BY combined_score DESC
    );

    LET $merged = array::slice($sorted, 0, $safe_limit);

    --------------------------------------------------------------------------
    -- STAGE 4: OPTIONAL GRAPH ENRICHMENT (using $parent, no FOR / AS)
    --------------------------------------------------------------------------
    -- Cast Thing IDs to strings for JSON serialization (handle NONE values)
    LET $final_results = IF $include_graph_context THEN (
        SELECT
            IF $parent.id IS NOT NONE THEN <string>$parent.id ELSE NONE END AS id,
            $parent.name      AS name,
            $parent.kind      AS kind,
            $parent.language  AS language,
            $parent.content   AS content,
            {
                file_path:  $parent.file_path,
                start_line: $parent.start_line,
                end_line:   $parent.end_line
            }                 AS location,
            $parent.metadata  AS metadata,
            $parent.vector_score   AS vector_score,
            $parent.text_score     AS text_score,
            $parent.combined_score AS combined_score,

            -- Direct dependencies (what this node calls / uses)
            (
                SELECT VALUE fn::node_reference(IF out IS NOT NONE THEN <string>out ELSE NONE END)
                FROM ($parent.id)->calls, ($parent.id)->imports, ($parent.id)->uses
                WHERE out.project_id = $project_id
                LIMIT 5
            ) AS direct_dependencies,

            -- Direct dependents (what calls / uses this node)
            (
                SELECT VALUE fn::node_reference(IF in IS NOT NONE THEN <string>in ELSE NONE END)
                FROM ($parent.id)<-calls, ($parent.id)<-imports, ($parent.id)<-uses
                WHERE in.project_id = $project_id
                LIMIT 5
            ) AS direct_dependents,

            -- File siblings (other nodes in the same file)
            (
                SELECT
                    IF id IS NOT NONE THEN <string>id ELSE NONE END AS id,
                    name,
                    node_type,
                    start_line
                FROM nodes
                WHERE project_id = $project_id
                  AND file_path = $parent.file_path
                  AND id != $parent.id
                ORDER BY start_line
                LIMIT 5
            ) AS file_siblings
        FROM (
            SELECT * FROM $merged
        )
    ) ELSE (
        SELECT
            IF id IS NOT NONE THEN <string>id ELSE NONE END AS id,
            name,
            kind,
            language,
            content,
            {
                file_path:  file_path,
                start_line: start_line,
                end_line:   end_line
            } AS location,
            metadata,
            vector_score,
            text_score,
            combined_score
        FROM $merged
    ) END;

    RETURN $final_results;
}
PERMISSIONS FULL;

DEFINE FUNCTION fn::semantic_search_chunks_with_context(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $include_graph_context: bool
) {
    -- Safety guards
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold =
        IF $threshold >= 0.0 AND $threshold <= 1.0
        THEN $threshold
        ELSE 0.7
        END;

    -- Dimension-dependent vector search with FETCH to resolve parent_node record links
    LET $vector_candidates = IF $dimension = 384 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_384 IS NOT NONE
            AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 768 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_768 IS NOT NONE
            AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 1024 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_1024 IS NOT NONE
            AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 1536 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_1536 IS NOT NONE
            AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 2048 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_2048 IS NOT NONE
            AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 2560 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_2560 IS NOT NONE
            AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE IF $dimension = 3072 THEN (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_3072 IS NOT NONE
            AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) ELSE (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_4096 IS NOT NONE
            AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
        FETCH parent_node
    ) END;

    LET $vector_results = array::flatten([$vector_candidates]);

    -- Extract node context from fetched parent_node objects
    LET $results = (
        SELECT
            IF id IS NOT NONE THEN <string>id ELSE NONE END AS id,
            chunk_index,
            text,
            vector_score,

            -- Node context from fetched parent_node (resolved by FETCH in vector search)
            IF parent_node IS NOT NONE THEN <string>parent_node.id ELSE NONE END AS node_id,
            parent_node.name AS name,
            parent_node.node_type AS kind,
            parent_node.language AS language,
            parent_node.file_path AS file_path,
            parent_node.start_line AS start_line,
            parent_node.end_line AS end_line,
            parent_node.metadata AS metadata
        FROM $vector_results
    );

    RETURN $results;
}
PERMISSIONS FULL;

-- =============================================================================
-- fn::semantic_search_nodes_via_chunks
-- =============================================================================
-- Context-engineering optimized semantic search that returns full node records.
-- Uses chunk-level semantic search for precision, then deduplicates by parent
-- node and returns complete code units (functions, classes, etc.) with full content.
--
-- Returns: Array of node records with content, deduplicated by node_id
-- =============================================================================
DEFINE FUNCTION IF NOT EXISTS fn::semantic_search_nodes_via_chunks(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float
) {
    -- Safety guards
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold =
        IF $threshold >= 0.0 AND $threshold <= 1.0
        THEN $threshold
        ELSE 0.7
        END;

    -- Search more chunks than limit to ensure enough unique nodes after deduplication
    LET $chunk_limit = $safe_limit * 3;

    -- Dimension-dependent vector search on chunks with FETCH parent_node
    LET $chunk_results = IF $dimension = 384 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_384 IS NOT NONE
            AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 768 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_768 IS NOT NONE
            AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 1024 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_1024 IS NOT NONE
            AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 1536 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_1536 IS NOT NONE
            AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 2048 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_2048 IS NOT NONE
            AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 2560 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_2560 IS NOT NONE
            AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dimension = 3072 THEN (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_3072 IS NOT NONE
            AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE (
        SELECT
            parent_node,
            vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
            AND embedding_4096 IS NOT NONE
            AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) END;

    LET $flattened = array::flatten([$chunk_results]);

    -- Deduplicate by parent_node, keeping the highest score per node
    LET $deduplicated = (
        SELECT
            math::max(vector_score) AS vector_score,
            parent_node.id AS node_id,
            parent_node.name AS name,
            parent_node.node_type AS kind,
            parent_node.language AS language,
            parent_node.file_path AS file_path,
            parent_node.start_line AS start_line,
            parent_node.end_line AS end_line,
            parent_node.content AS content,
            parent_node.metadata AS metadata
        FROM $flattened
        WHERE parent_node IS NOT NONE
        GROUP BY parent_node.id
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    );

    -- Enrich each result with FULL graph context (ALL edge types)
    -- NOTE: node_id here is still a record<nodes> Thing, convert to string only for output
    LET $results = (
        SELECT
            IF node_id IS NOT NONE THEN <string>node_id ELSE NONE END AS node_id,
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            content,
            metadata,
            vector_score,

            -- GRAPH CONTEXT: ALL outgoing edges (what this node connects TO)
            -- Use $parent.node_id to access the record reference before string conversion
            (SELECT
                <string>to.id AS node_id,
                to.name AS name,
                to.node_type AS kind,
                to.file_path AS file_path,
                edge_type AS relationship
             FROM edges
             WHERE from = $parent.node_id
             LIMIT 20
             FETCH to
            ) AS outgoing_edges,

            -- GRAPH CONTEXT: ALL incoming edges (what connects TO this node)
            (SELECT
                <string>from.id AS node_id,
                from.name AS name,
                from.node_type AS kind,
                from.file_path AS file_path,
                edge_type AS relationship
             FROM edges
             WHERE to = $parent.node_id
             LIMIT 20
             FETCH from
            ) AS incoming_edges

        FROM $deduplicated
    );

    RETURN $results;
}
PERMISSIONS FULL;
