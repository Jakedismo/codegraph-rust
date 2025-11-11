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

DEFINE ANALYZER code_analyzer TOKENIZERS BLANK,CLASS FILTERS LOWERCASE,SNOWBALL(ENGLISH);

-- =============================================================================
-- CORE TABLES
-- =============================================================================

-- -----------------------------------------------------------------------------
-- TABLE: nodes
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL
  COMMENT 'Code entities from AST parsing with semantic embeddings';

-- Core fields (preserved exactly)
DEFINE FIELD IF NOT EXISTS id          ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS name        ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS node_type   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language    ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content     ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS file_path   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS start_line  ON TABLE nodes TYPE option<int>;
DEFINE FIELD IF NOT EXISTS end_line    ON TABLE nodes TYPE option<int>;
DEFINE FIELD IF NOT EXISTS embedding_384   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 384;
DEFINE FIELD IF NOT EXISTS embedding_1024   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 1024;
DEFINE FIELD IF NOT EXISTS embedding_1536   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 1536;
DEFINE FIELD IF NOT EXISTS embedding_2048   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 2048;
DEFINE FIELD IF NOT EXISTS embedding_3072   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 3072;
DEFINE FIELD IF NOT EXISTS embedding_4096   ON TABLE nodes TYPE option<array<float>>
    ASSERT array::len($value) = 4096;
DEFINE FIELD IF NOT EXISTS embedding_model ON nodes TYPE option<string> DEFAULT 'jina-embeddings-v4';
DEFINE FIELD IF NOT EXISTS complexity  ON TABLE nodes TYPE option<float>;
DEFINE FIELD IF NOT EXISTS metadata    ON TABLE nodes TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at  ON TABLE nodes TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at  ON TABLE nodes TYPE datetime VALUE time::now();

-- New fields for unified schema (non-breaking additions)
DEFINE FIELD IF NOT EXISTS project_id       ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS organization_id  ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS repository_url   ON TABLE nodes TYPE option<string>;

-- Indexes (preserved exactly)
DEFINE INDEX IF NOT EXISTS idx_nodes_id         ON TABLE nodes COLUMNS id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_nodes_name       ON TABLE nodes COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_nodes_type       ON TABLE nodes COLUMNS node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_language   ON TABLE nodes COLUMNS language;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_path  ON TABLE nodes COLUMNS file_path;
DEFINE INDEX IF NOT EXISTS idx_nodes_project    ON TABLE nodes COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_384
    ON TABLE nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_1024
    ON TABLE nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_1536
    ON TABLE nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_2048
    ON TABLE nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_3072
    ON TABLE nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_4096
    ON TABLE nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 200 M 16;

-- Composite indexes for common query patterns (P0 Enhancement)
DEFINE INDEX IF NOT EXISTS idx_nodes_project_type ON TABLE nodes COLUMNS project_id, node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_type ON TABLE nodes COLUMNS file_path, node_type;

-- -----------------------------------------------------------------------------
-- TABLE: edges
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL
  COMMENT 'Code relationships (Calls, Imports, Uses, Extends, Implements, References)';

DEFINE FIELD IF NOT EXISTS id         ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS from       ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to         ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS edge_type  ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS weight     ON TABLE edges TYPE float DEFAULT 1.0
  ASSERT $value > 0.0;
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE edges TYPE option<object>;
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

DEFINE FIELD IF NOT EXISTS version    ON TABLE schema_versions TYPE int;
DEFINE FIELD IF NOT EXISTS name       ON TABLE schema_versions TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE schema_versions TYPE option<string>;
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS checksum   ON TABLE schema_versions TYPE option<string>;

DEFINE INDEX IF NOT EXISTS idx_schema_version ON TABLE schema_versions COLUMNS version UNIQUE;

-- -----------------------------------------------------------------------------
-- TABLE: metadata
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS key        ON TABLE metadata TYPE string;
DEFINE FIELD IF NOT EXISTS value      ON TABLE metadata TYPE option<string | number | bool | object | array>;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE metadata TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_metadata_key ON TABLE metadata COLUMNS key UNIQUE;

-- -----------------------------------------------------------------------------
-- TABLE: project_metadata
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS project_metadata SCHEMAFULL
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
DEFINE FIELD IF NOT EXISTS last_analyzed    ON TABLE project_metadata TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS codegraph_version ON TABLE project_metadata TYPE option<string>;

-- Cross-project fields
DEFINE FIELD IF NOT EXISTS organization_id  ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD IF NOT EXISTS domain           ON TABLE project_metadata TYPE option<string>;

-- Metadata
DEFINE FIELD IF NOT EXISTS metadata         ON TABLE project_metadata TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at       ON TABLE project_metadata TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS updated_at       ON TABLE project_metadata TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_project_id     ON TABLE project_metadata COLUMNS project_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_project_name   ON TABLE project_metadata COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_project_org    ON TABLE project_metadata COLUMNS organization_id;
DEFINE INDEX IF NOT EXISTS idx_project_domain ON TABLE project_metadata COLUMNS domain;

-- -----------------------------------------------------------------------------
-- TABLE: symbol_embeddings (AI-assisted symbol resolution cache)
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS symbol_embeddings SCHEMAFULL
    COMMENT 'Cached embeddings for normalized symbols used during edge resolution'
    PERMISSIONS FULL;

DEFINE FIELD IF NOT EXISTS id                 ON symbol_embeddings TYPE string;
DEFINE FIELD IF NOT EXISTS symbol             ON symbol_embeddings TYPE string;
DEFINE FIELD IF NOT EXISTS normalized_symbol  ON symbol_embeddings TYPE string;
DEFINE FIELD IF NOT EXISTS project_id         ON symbol_embeddings TYPE option<string>;
DEFINE FIELD IF NOT EXISTS organization_id    ON symbol_embeddings TYPE option<string>;
DEFINE FIELD IF NOT EXISTS embedding_384   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 384;
DEFINE FIELD IF NOT EXISTS embedding_1024   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 1024;
DEFINE FIELD IF NOT EXISTS embedding_1536   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 1536;
DEFINE FIELD IF NOT EXISTS embedding_2048   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 2048;
DEFINE FIELD IF NOT EXISTS embedding_3072   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 3072;
DEFINE FIELD IF NOT EXISTS embedding_4096   ON TABLE symbol_embeddings TYPE option<array<float>>
    ASSERT array::len($value) = 4096;
DEFINE FIELD IF NOT EXISTS embedding_384[*]  ON symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_1024[*]  ON symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_2048[*]  ON symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_4096[*]  ON symbol_embeddings TYPE float;
DEFINE FIELD IF NOT EXISTS embedding_model    ON symbol_embeddings TYPE string DEFAULT 'jina-embeddings-v4';
DEFINE FIELD IF NOT EXISTS last_computed_at   ON symbol_embeddings TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD IF NOT EXISTS access_count       ON symbol_embeddings TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS metadata           ON symbol_embeddings TYPE option<object>;

DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_id
    ON symbol_embeddings COLUMNS id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_symbol
    ON symbol_embeddings COLUMNS normalized_symbol;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_project_symbol
    ON symbol_embeddings COLUMNS project_id, normalized_symbol;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_384
    ON symbol_embeddings FIELDS embedding_384
    HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_1024
    ON symbol_embeddings FIELDS embedding_1024
    HNSW DIMENSION 1024 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_1536
    ON symbol_embeddings FIELDS embedding_1536
    HNSW DIMENSION 1536 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_2048
    ON symbol_embeddings FIELDS embedding_2048
    HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_3072
    ON symbol_embeddings FIELDS embedding_3072
    HNSW DIMENSION 3072 DIST COSINE EFC 200 M 16;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_vector_4096
    ON symbol_embeddings FIELDS embedding_4096
    HNSW DIMENSION 4096 DIST COSINE EFC 200 M 16;

DEFINE FIELD IF NOT EXISTS node_id ON symbol_embeddings
  TYPE option<record<nodes>>;
DEFINE FIELD IF NOT EXISTS source_edge_id ON symbol_embeddings
  TYPE option<record<edges>>;

-- Indexes for relation fields (UNCHANGED)
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_node
    ON symbol_embeddings COLUMNS node_id;
DEFINE INDEX IF NOT EXISTS idx_symbol_embeddings_edge
    ON symbol_embeddings COLUMNS source_edge_id;



-- =============================================================================
-- FUNCTIONS
-- =============================================================================

-- -----------------------------------------------------------------------------
-- FUNCTION: node_info (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::node_info($node_id: string) {
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
    FROM ONLY $node_id
    LIMIT 1;

    RETURN $res[0];
};

-- -----------------------------------------------------------------------------
-- FUNCTION: node_reference (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::node_reference($node_id: string) {
    LET $info = fn::node_info($node_id);
    IF $info = NONE {
        RETURN NONE;
    };
    RETURN {
        id: $info.id,
        name: $info.name,
        kind: $info.kind,
        location: $info.location
    };
};

-- -----------------------------------------------------------------------------
-- FUNCTION: edge_types (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::edge_types() {
    RETURN ['Calls', 'Imports', 'Uses', 'Extends', 'Implements', 'References'];
};

-- -----------------------------------------------------------------------------
-- FUNCTION: get_transitive_dependencies (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::get_transitive_dependencies($node_id: string, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;
    LET $edge_name  = $edge_type ?? 'Calls';

    LET $lvl1 = IF $safe_depth >= 1 THEN
        SELECT VALUE id
        FROM ONLY $node_id
            ->edges[WHERE edge_type = $edge_name]
            ->to
    ELSE [] END;

    LET $lvl2 = IF $safe_depth >= 2 THEN
        SELECT VALUE id FROM $lvl1 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl3 = IF $safe_depth >= 3 THEN
        SELECT VALUE id FROM $lvl2 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl4 = IF $safe_depth >= 4 THEN
        SELECT VALUE id FROM $lvl3 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl5 = IF $safe_depth >= 5 THEN
        SELECT VALUE id FROM $lvl4 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl6 = IF $safe_depth >= 6 THEN
        SELECT VALUE id FROM $lvl5 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl7 = IF $safe_depth >= 7 THEN
        SELECT VALUE id FROM $lvl6 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl8 = IF $safe_depth >= 8 THEN
        SELECT VALUE id FROM $lvl7 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl9 = IF $safe_depth >= 9 THEN
        SELECT VALUE id FROM $lvl8 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $lvl10 = IF $safe_depth >= 10 THEN
        SELECT VALUE id FROM $lvl9 ->edges[WHERE edge_type = $edge_name] ->to
    ELSE [] END;

    LET $pairs = array::concat(
        (SELECT VALUE { id: id, depth: 1 }  FROM $lvl1),
        (SELECT VALUE { id: id, depth: 2 }  FROM $lvl2),
        (SELECT VALUE { id: id, depth: 3 }  FROM $lvl3),
        (SELECT VALUE { id: id, depth: 4 }  FROM $lvl4),
        (SELECT VALUE { id: id, depth: 5 }  FROM $lvl5),
        (SELECT VALUE { id: id, depth: 6 }  FROM $lvl6),
        (SELECT VALUE { id: id, depth: 7 }  FROM $lvl7),
        (SELECT VALUE { id: id, depth: 8 }  FROM $lvl8),
        (SELECT VALUE { id: id, depth: 9 }  FROM $lvl9),
        (SELECT VALUE { id: id, depth: 10 } FROM $lvl10)
    );

    LET $min_depths = SELECT id, math::min(depth) AS dependency_depth FROM $pairs GROUP BY id;

    LET $raw = SELECT fn::node_info(id) AS node, dependency_depth FROM $min_depths;

    RETURN SELECT
        node.id       AS id,
        node.name     AS name,
        node.kind     AS kind,
        node.location AS location,
        node.language AS language,
        node.content  AS content,
        node.metadata AS metadata,
        dependency_depth,
        $safe_depth   AS requested_depth
    FROM $raw
    WHERE node != NONE;
};

-- -----------------------------------------------------------------------------
-- FUNCTION: detect_circular_dependencies (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::detect_circular_dependencies($edge_type: string) {
    LET $edge_name = $edge_type ?? 'Calls';

    LET $pairs = (
        SELECT
            from AS node1_id,
            to   AS node2_id
        FROM edges
        WHERE edge_type = $edge_name
          AND from != to
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
};

-- -----------------------------------------------------------------------------
-- FUNCTION: trace_call_chain (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::trace_call_chain($from_node: string, $max_depth: int) {
    LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;

    LET $raw = (
        SELECT
            fn::node_info(id) AS node,
            array::distinct((
                SELECT fn::node_reference(from) AS caller
                FROM edges
                WHERE to = id AND edge_type = 'Calls'
            ).caller) AS called_by
        FROM (
            SELECT ->edges[WHERE edge_type = 'Calls']
            FROM ONLY $from_node
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
};

-- -----------------------------------------------------------------------------
-- FUNCTION: calculate_coupling_metrics (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::calculate_coupling_metrics($node_id: string) {
    LET $edge_list = fn::edge_types();

    LET $dependents = array::distinct(
        SELECT VALUE id
        FROM ONLY $node_id
            <-edges[WHERE edge_type INSIDE $edge_list]
            <-from
    );

    LET $dependencies = array::distinct(
        SELECT VALUE id
        FROM ONLY $node_id
            ->edges[WHERE edge_type INSIDE $edge_list]
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
        node: fn::node_info($node_id),
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
};

-- -----------------------------------------------------------------------------
-- FUNCTION: get_hub_nodes (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::get_hub_nodes($min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
    LET $edge_list = fn::edge_types();

    LET $incoming_by_type = (
        SELECT
            to AS node_id,
            edge_type,
            count() AS count
        FROM edges
        WHERE edge_type INSIDE $edge_list
        GROUP BY to, edge_type
    );

    LET $outgoing_by_type = (
        SELECT
            from AS node_id,
            edge_type,
            count() AS count
        FROM edges
        WHERE edge_type INSIDE $edge_list
        GROUP BY from, edge_type
    );

    LET $incoming_totals = (
        SELECT
            to AS node_id,
            count() AS total
        FROM edges
        WHERE edge_type INSIDE $edge_list
        GROUP BY to
    );

    LET $outgoing_totals = (
        SELECT
            from AS node_id,
            count() AS total
        FROM edges
        WHERE edge_type INSIDE $edge_list
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
};

-- -----------------------------------------------------------------------------
-- FUNCTION: get_reverse_dependencies (UNCHANGED)
-- -----------------------------------------------------------------------------
DEFINE FUNCTION fn::get_reverse_dependencies($node_id: string, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;
    LET $edge_name  = $edge_type ?? 'Calls';

    LET $lvl1 = IF $safe_depth >= 1 THEN
        SELECT VALUE id
        FROM ONLY $node_id
            <-edges[WHERE edge_type = $edge_name]
            <-from
    ELSE [] END;

    LET $lvl2 = IF $safe_depth >= 2 THEN
        SELECT VALUE id FROM $lvl1 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl3 = IF $safe_depth >= 3 THEN
        SELECT VALUE id FROM $lvl2 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl4 = IF $safe_depth >= 4 THEN
        SELECT VALUE id FROM $lvl3 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl5 = IF $safe_depth >= 5 THEN
        SELECT VALUE id FROM $lvl4 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl6 = IF $safe_depth >= 6 THEN
        SELECT VALUE id FROM $lvl5 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl7 = IF $safe_depth >= 7 THEN
        SELECT VALUE id FROM $lvl6 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl8 = IF $safe_depth >= 8 THEN
        SELECT VALUE id FROM $lvl7 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl9 = IF $safe_depth >= 9 THEN
        SELECT VALUE id FROM $lvl8 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $lvl10 = IF $safe_depth >= 10 THEN
        SELECT VALUE id FROM $lvl9 <-edges[WHERE edge_type = $edge_name] <-from
    ELSE [] END;

    LET $pairs = array::concat(
        (SELECT VALUE { id: id, depth: 1 }  FROM $lvl1),
        (SELECT VALUE { id: id, depth: 2 }  FROM $lvl2),
        (SELECT VALUE { id: id, depth: 3 }  FROM $lvl3),
        (SELECT VALUE { id: id, depth: 4 }  FROM $lvl4),
        (SELECT VALUE { id: id, depth: 5 }  FROM $lvl5),
        (SELECT VALUE { id: id, depth: 6 }  FROM $lvl6),
        (SELECT VALUE { id: id, depth: 7 }  FROM $lvl7),
        (SELECT VALUE { id: id, depth: 8 }  FROM $lvl8),
        (SELECT VALUE { id: id, depth: 9 }  FROM $lvl9),
        (SELECT VALUE { id: id, depth: 10 } FROM $lvl10)
    );

    LET $min_depths = SELECT id, math::min(depth) AS dependent_depth FROM $pairs GROUP BY id;

    LET $raw = SELECT fn::node_info(id) AS node, dependent_depth FROM $min_depths;

    RETURN SELECT
        node.id       AS id,
        node.name     AS name,
        node.kind     AS kind,
        node.location AS location,
        node.language AS language,
        node.content  AS content,
        node.metadata AS metadata,
        dependent_depth,
        $safe_depth   AS requested_depth
    FROM $raw
    WHERE node != NONE;
};
-- =============================================================================