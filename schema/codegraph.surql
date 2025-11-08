-- ABOUTME: Defines CodeGraph's SurrealDB schema, indexes, metadata, and graph helpers.
-- ABOUTME: Serves as the single migration file for all CodeGraph database objects.

-- =============================================================================
-- CodeGraph SurrealDB Schema Definition
-- =============================================================================

-- Optional namespace/database selection
-- USE NS codegraph;
-- USE DB codegraph;

-- =============================================================================
-- NODES TABLE - Code Entity Storage
-- =============================================================================
DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id          ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS name        ON TABLE nodes TYPE string;
DEFINE FIELD IF NOT EXISTS node_type   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language    ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content     ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS file_path   ON TABLE nodes TYPE option<string>;
DEFINE FIELD IF NOT EXISTS start_line  ON TABLE nodes TYPE option<int>;
DEFINE FIELD IF NOT EXISTS end_line    ON TABLE nodes TYPE option<int>;
DEFINE FIELD IF NOT EXISTS embedding   ON TABLE nodes TYPE option<array<float>>;
DEFINE FIELD IF NOT EXISTS complexity  ON TABLE nodes TYPE option<float>;
DEFINE FIELD IF NOT EXISTS metadata    ON TABLE nodes TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at  ON TABLE nodes TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at  ON TABLE nodes TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_nodes_id         ON TABLE nodes COLUMNS id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_nodes_name       ON TABLE nodes COLUMNS name;
DEFINE INDEX IF NOT EXISTS idx_nodes_type       ON TABLE nodes COLUMNS node_type;
DEFINE INDEX IF NOT EXISTS idx_nodes_language   ON TABLE nodes COLUMNS language;
DEFINE INDEX IF NOT EXISTS idx_nodes_file_path  ON TABLE nodes COLUMNS file_path;
DEFINE INDEX IF NOT EXISTS idx_nodes_embedding_hnsw
    ON TABLE nodes FIELDS embedding HNSW DIMENSION 2048 DIST COSINE EFC 200 M 16;

-- =============================================================================
-- EDGES TABLE - Relationship Storage
-- =============================================================================
DEFINE TABLE IF NOT EXISTS edges SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS id         ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS from       ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to         ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS edge_type  ON TABLE edges TYPE string;
DEFINE FIELD IF NOT EXISTS weight     ON TABLE edges TYPE float DEFAULT 1.0;
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE edges TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE edges TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_edges_from    ON TABLE edges COLUMNS from;
DEFINE INDEX IF NOT EXISTS idx_edges_to      ON TABLE edges COLUMNS to;
DEFINE INDEX IF NOT EXISTS idx_edges_type    ON TABLE edges COLUMNS edge_type;
DEFINE INDEX IF NOT EXISTS idx_edges_from_to ON TABLE edges COLUMNS from, to;

-- =============================================================================
-- SCHEMA_VERSIONS TABLE - Migration Tracking
-- =============================================================================
DEFINE TABLE IF NOT EXISTS schema_versions SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS version    ON TABLE schema_versions TYPE int;
DEFINE FIELD IF NOT EXISTS name       ON TABLE schema_versions TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE schema_versions TYPE option<string>;
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS checksum   ON TABLE schema_versions TYPE option<string>;
DEFINE INDEX IF NOT EXISTS idx_schema_version ON TABLE schema_versions COLUMNS version UNIQUE;

-- =============================================================================
-- METADATA TABLE - System Metadata
-- =============================================================================
DEFINE TABLE IF NOT EXISTS metadata SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS key        ON TABLE metadata TYPE string;
DEFINE FIELD IF NOT EXISTS value      ON TABLE metadata TYPE option<string | number | bool | object | array>;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE metadata TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_metadata_key ON TABLE metadata COLUMNS key UNIQUE;

-- =============================================================================
-- PROJECT METADATA TABLE - Project Summary
-- =============================================================================
DEFINE TABLE IF NOT EXISTS project_metadata SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS project_id       ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS name             ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS root_path        ON TABLE project_metadata TYPE string;
DEFINE FIELD IF NOT EXISTS primary_language ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_analyzed    ON TABLE project_metadata TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS file_count       ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS node_count       ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS edge_count       ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS metadata         ON TABLE project_metadata TYPE option<object>;
DEFINE FIELD IF NOT EXISTS created_at       ON TABLE project_metadata TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at       ON TABLE project_metadata TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_project_id   ON TABLE project_metadata COLUMNS project_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_project_name ON TABLE project_metadata COLUMNS name;

-- =============================================================================
-- MIGRATION BOOKKEEPING
-- =============================================================================
INSERT INTO schema_versions (version, name, description, applied_at, checksum)
VALUES (1, 'initial_schema', 'Nodes, edges, metadata, and project tables', time::now(), 'schema_v1')
ON DUPLICATE KEY UPDATE description = 'Nodes, edges, metadata, and project tables';

INSERT INTO schema_versions (version, name, description, applied_at, checksum)
VALUES (2, 'graph_functions', 'SurrealDB functions for graph analysis', time::now(), 'graph_functions_v1')
ON DUPLICATE KEY UPDATE description = 'SurrealDB functions for graph analysis';

-- =============================================================================
-- Helper Functions
-- =============================================================================
DEFINE FUNCTION fn::node_info($node_id: string) {
    RETURN (
        SELECT
            id,
            name,
            node_type AS kind,
            language,
            content,
            metadata,
            {
                file_path,
                start_line,
                end_line
            } AS location
        FROM ONLY $node_id
        LIMIT 1
    )[0];
};

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

DEFINE FUNCTION fn::edge_types() {
    RETURN ['Calls', 'Imports', 'Uses', 'Extends', 'Implements', 'References'];
};

-- =============================================================================
-- GRAPH ANALYSIS FUNCTIONS
-- =============================================================================

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

DEFINE FUNCTION fn::detect_circular_dependencies($edge_type: string) {
    LET $edge_name = $edge_type ?? 'Calls';

    LET $pairs = (
        SELECT from AS node1_id, to AS node2_id
        FROM edges
        WHERE edge_type = $edge_name
    );

    LET $cycles = (
        SELECT DISTINCT node1_id, node2_id
        FROM $pairs
        WHERE node1_id != node2_id
          AND node1_id < node2_id
          AND (
            SELECT count() FROM edges
            WHERE from = node2_id AND to = node1_id AND edge_type = $edge_name
        ) > 0
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

DEFINE FUNCTION fn::get_hub_nodes($min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
    LET $edge_list = fn::edge_types();

    LET $incoming = (
        SELECT to AS node_id, edge_type, count() AS count
        FROM edges
        WHERE edge_type INSIDE $edge_list
        GROUP BY to, edge_type
    );

    LET $outgoing = (
        SELECT from AS node_id, edge_type, count() AS count
        FROM edges
        WHERE edge_type INSIDE $edge_list
        GROUP BY from, edge_type
    );

    LET $incoming_totals = (
        SELECT node_id, math::sum(count) AS total
        FROM $incoming
        GROUP BY node_id
    );

    LET $outgoing_totals = (
        SELECT node_id, math::sum(count) AS total
        FROM $outgoing
        GROUP BY node_id
    );

    LET $candidates = (
        SELECT DISTINCT node_id
        FROM (
            SELECT node_id FROM $incoming
            UNION
            SELECT node_id FROM $outgoing
        )
    );

    LET $raw = (
        SELECT
            candidate_id,
            fn::node_info(candidate_id) AS node,
            ((SELECT VALUE total FROM $incoming_totals WHERE node_id = candidate_id LIMIT 1)[0] ?? 0) AS afferent_degree,
            ((SELECT VALUE total FROM $outgoing_totals WHERE node_id = candidate_id LIMIT 1)[0] ?? 0) AS efferent_degree,
            (
                SELECT edge_type, count
                FROM $incoming
                WHERE node_id = candidate_id
            ) AS incoming_by_type,
            (
                SELECT edge_type, count
                FROM $outgoing
                WHERE node_id = candidate_id
            ) AS outgoing_by_type
        FROM (
            SELECT node_id AS candidate_id
            FROM $candidates
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
    WHERE (afferent_degree + efferent_degree) >= $threshold
    ORDER BY total_degree DESC;
};

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
