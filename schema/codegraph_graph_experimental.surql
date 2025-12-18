-- =====================================================================
-- Codegraph experimental DB
-- =====================================================================

USE DB codegraph_experimental;

-- =====================================================================
-- ANALYZER FOR CODE / TEXT SEARCH
-- =====================================================================

DEFINE ANALYZER code_analyzer
  TOKENIZERS BLANK,CLASS
  FILTERS LOWERCASE,SNOWBALL(ENGLISH);

-- =====================================================================
-- CORE TABLES
-- =====================================================================

-- -----------------------------
-- nodes: code entities / symbols
-- -----------------------------
DEFINE TABLE nodes SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id      ON TABLE nodes TYPE option<string>;
DEFINE FIELD language        ON TABLE nodes TYPE option<string>;
DEFINE FIELD node_type       ON TABLE nodes TYPE option<string>;      -- e.g. function, struct, class
DEFINE FIELD name            ON TABLE nodes TYPE string;              -- symbol name
DEFINE FIELD file_path       ON TABLE nodes TYPE option<string>;      -- file path
DEFINE FIELD content         ON TABLE nodes TYPE option<string>;      -- optional full body / doc
DEFINE FIELD start_line      ON TABLE nodes TYPE option<int>;
DEFINE FIELD end_line        ON TABLE nodes TYPE option<int>;
DEFINE FIELD metadata        ON TABLE nodes TYPE option<object>;      -- arbitrary metadata
DEFINE FIELD range           ON TABLE nodes TYPE option<object>;      -- {start:{line}, end:{line}}
DEFINE FIELD chunk_count     ON TABLE nodes TYPE option<int> DEFAULT 0;
DEFINE FIELD complexity      ON TABLE nodes TYPE option<float>;
DEFINE FIELD organization_id ON TABLE nodes TYPE option<string>;
DEFINE FIELD repository_url  ON TABLE nodes TYPE option<string>;

DEFINE FIELD embedding_384   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_384[*]   ON TABLE nodes TYPE float;
DEFINE FIELD embedding_768   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_768[*]   ON TABLE nodes TYPE float;
DEFINE FIELD embedding_1024  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1024[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_1536  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_1536[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_2048  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2048[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_2560  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_2560[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_3072  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_3072[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_3584  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3584;
DEFINE FIELD embedding_3584[*]  ON TABLE nodes TYPE float;
DEFINE FIELD embedding_4096  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;
DEFINE FIELD embedding_4096[*]  ON TABLE nodes TYPE float;

DEFINE FIELD embedding_model ON TABLE nodes TYPE option<string> DEFAULT 'jina-embeddings-v4';
DEFINE FIELD created_at      ON TABLE nodes TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at      ON TABLE nodes TYPE datetime VALUE time::now();

-- -----------------------------
-- edges: typed relationships between nodes
-- -----------------------------
DEFINE TABLE edges SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id   ON TABLE edges TYPE option<string>;
DEFINE FIELD edge_type    ON TABLE edges TYPE string;             -- calls, imports, uses, ...
DEFINE FIELD from         ON TABLE edges TYPE record<nodes>;
DEFINE FIELD to           ON TABLE edges TYPE record<nodes>;
DEFINE FIELD weight       ON TABLE edges TYPE float DEFAULT 1f ASSERT $value > 0f;
DEFINE FIELD metadata     ON TABLE edges TYPE option<object>;
DEFINE FIELD created_at   ON TABLE edges TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at   ON TABLE edges TYPE datetime VALUE time::now();

-- -----------------------------
-- chunks: text chunks linked to parent node
-- -----------------------------
DEFINE TABLE chunks SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id      ON TABLE chunks TYPE string;
DEFINE FIELD parent_node     ON TABLE chunks TYPE record<nodes>;
DEFINE FIELD chunk_index     ON TABLE chunks TYPE int;
DEFINE FIELD text            ON TABLE chunks TYPE string;

DEFINE FIELD embedding_384   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_384[*]   ON TABLE chunks TYPE float;
DEFINE FIELD embedding_768   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_768[*]   ON TABLE chunks TYPE float;
DEFINE FIELD embedding_1024  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1024[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_1536  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_1536[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_2048  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2048[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_2560  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_2560[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_3072  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_3072[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_3584  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3584;
DEFINE FIELD embedding_3584[*]  ON TABLE chunks TYPE float;
DEFINE FIELD embedding_4096  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;
DEFINE FIELD embedding_4096[*]  ON TABLE chunks TYPE float;

DEFINE FIELD embedding_model ON TABLE chunks TYPE option<string>;
DEFINE FIELD created_at      ON TABLE chunks TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at      ON TABLE chunks TYPE datetime VALUE time::now();

-- -----------------------------
-- file_metadata: per-file aggregation
-- -----------------------------
DEFINE TABLE file_metadata SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id       ON TABLE file_metadata TYPE string;
DEFINE FIELD file_path        ON TABLE file_metadata TYPE string;
DEFINE FIELD language         ON TABLE file_metadata TYPE option<string>;
DEFINE FIELD content_hash     ON TABLE file_metadata TYPE string;
DEFINE FIELD file_size        ON TABLE file_metadata TYPE int;
DEFINE FIELD modified_at      ON TABLE file_metadata TYPE datetime;
DEFINE FIELD node_count       ON TABLE file_metadata TYPE int DEFAULT 0;
DEFINE FIELD edge_count       ON TABLE file_metadata TYPE int DEFAULT 0;
DEFINE FIELD parse_errors     ON TABLE file_metadata TYPE option<array<string>>;
DEFINE FIELD parse_errors[*]  ON TABLE file_metadata TYPE string;
DEFINE FIELD last_indexed_at  ON TABLE file_metadata TYPE datetime DEFAULT time::now();
DEFINE FIELD created_at       ON TABLE file_metadata TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at       ON TABLE file_metadata TYPE datetime VALUE time::now();

-- -----------------------------
-- project_metadata: project-level stats
-- -----------------------------
DEFINE TABLE project_metadata SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id        ON TABLE project_metadata TYPE string;
DEFINE FIELD name              ON TABLE project_metadata TYPE string;
DEFINE FIELD root_path         ON TABLE project_metadata TYPE string;
DEFINE FIELD primary_language  ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD file_count        ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD node_count        ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD edge_count        ON TABLE project_metadata TYPE int DEFAULT 0;
DEFINE FIELD last_analyzed     ON TABLE project_metadata TYPE option<datetime>;
DEFINE FIELD organization_id   ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD domain            ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD codegraph_version ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD metadata          ON TABLE project_metadata TYPE option<object>;
DEFINE FIELD created_at        ON TABLE project_metadata TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at        ON TABLE project_metadata TYPE datetime VALUE time::now();

-- -----------------------------
-- symbol_embeddings: identifier-level embeddings per node
-- -----------------------------
DEFINE TABLE symbol_embeddings SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD access_count       ON TABLE symbol_embeddings TYPE int DEFAULT 0;
DEFINE FIELD embedding_model    ON TABLE symbol_embeddings TYPE string DEFAULT 'jina-embeddings-v4';
DEFINE FIELD last_computed_at   ON TABLE symbol_embeddings TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD metadata           ON TABLE symbol_embeddings TYPE option<object>;
DEFINE FIELD node_id            ON TABLE symbol_embeddings TYPE option<record<nodes>>;
DEFINE FIELD normalized_symbol  ON TABLE symbol_embeddings TYPE string;
DEFINE FIELD organization_id    ON TABLE symbol_embeddings TYPE option<string>;
DEFINE FIELD project_id         ON TABLE symbol_embeddings TYPE option<string>;
DEFINE FIELD source_edge_id     ON TABLE symbol_embeddings TYPE option<record<edges>>;
DEFINE FIELD symbol             ON TABLE symbol_embeddings TYPE string;

DEFINE FIELD embedding_384      ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_384[*]      ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_768      ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_768[*]      ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_1024     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1024[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_1536     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_1536[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_2048     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2048[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_2560     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_2560[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_3072     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_3072[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_3584     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3584;
DEFINE FIELD embedding_3584[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD embedding_4096     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;
DEFINE FIELD embedding_4096[*]     ON TABLE symbol_embeddings TYPE float;
DEFINE FIELD created_at         ON TABLE symbol_embeddings TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD updated_at         ON TABLE symbol_embeddings TYPE datetime VALUE time::now();

-- =====================================================================
-- INDEXES
-- =====================================================================

-- nodes: scalar indexes (project-scoped)
DEFINE INDEX idx_nodes_name       ON TABLE nodes FIELDS name CONCURRENTLY;
DEFINE INDEX idx_nodes_type       ON TABLE nodes FIELDS node_type CONCURRENTLY;
DEFINE INDEX idx_nodes_language   ON TABLE nodes FIELDS language CONCURRENTLY;
DEFINE INDEX idx_nodes_file_path  ON TABLE nodes FIELDS file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_project    ON TABLE nodes FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_nodes_project_file ON TABLE nodes FIELDS project_id, file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_project_type ON TABLE nodes FIELDS project_id, node_type CONCURRENTLY;
-- nodes: full-text indexes for hybrid search
DEFINE INDEX idx_nodes_name_search
  ON TABLE nodes FIELDS name
  SEARCH ANALYZER code_analyzer BM25 CONCURRENTLY;

DEFINE INDEX idx_nodes_content_search
  ON TABLE nodes FIELDS content
  SEARCH ANALYZER code_analyzer BM25 CONCURRENTLY;

-- nodes: HNSW vector indexes (COSINE metric)
DEFINE INDEX idx_nodes_embedding_384
  ON TABLE nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_768
  ON TABLE nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1024
  ON TABLE nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1536
  ON TABLE nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2048
  ON TABLE nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2560
  ON TABLE nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3072
  ON TABLE nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3584
  ON TABLE nodes FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_4096
  ON TABLE nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- edges: scalar indexes (project-scoped)
DEFINE INDEX idx_edges_project      ON TABLE edges FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_edges_from         ON TABLE edges FIELDS from CONCURRENTLY;
DEFINE INDEX idx_edges_to           ON TABLE edges FIELDS to CONCURRENTLY;
DEFINE INDEX idx_edges_type         ON TABLE edges FIELDS edge_type CONCURRENTLY;
DEFINE INDEX idx_edges_from_to      ON TABLE edges FIELDS from, to CONCURRENTLY;
DEFINE INDEX idx_edges_type_from    ON TABLE edges FIELDS edge_type, from CONCURRENTLY;
DEFINE INDEX idx_edges_project_type ON TABLE edges FIELDS project_id, edge_type CONCURRENTLY;

-- chunks: scalar + vector indexes
DEFINE INDEX idx_chunks_parent
  ON TABLE chunks FIELDS parent_node CONCURRENTLY;
DEFINE INDEX idx_chunks_order
  ON TABLE chunks FIELDS parent_node, chunk_index CONCURRENTLY;
DEFINE INDEX idx_chunks_project
  ON TABLE chunks FIELDS project_id CONCURRENTLY;

DEFINE INDEX idx_chunks_embedding_384
  ON TABLE chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_768
  ON TABLE chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1024
  ON TABLE chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1536
  ON TABLE chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2048
  ON TABLE chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2560
  ON TABLE chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3072
  ON TABLE chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3584
  ON TABLE chunks FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_4096
  ON TABLE chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- file_metadata indexes
DEFINE INDEX idx_file_metadata_project_path
  ON TABLE file_metadata FIELDS project_id, file_path UNIQUE CONCURRENTLY;

-- project_metadata indexes
DEFINE INDEX idx_project_id ON TABLE project_metadata FIELDS project_id UNIQUE CONCURRENTLY;
DEFINE INDEX idx_project_name ON TABLE project_metadata FIELDS name CONCURRENTLY;
DEFINE INDEX idx_project_org ON TABLE project_metadata FIELDS organization_id CONCURRENTLY;
DEFINE INDEX idx_project_domain ON TABLE project_metadata FIELDS domain CONCURRENTLY;

-- symbol_embeddings indexes
DEFINE INDEX idx_symbol_embeddings_symbol
  ON TABLE symbol_embeddings FIELDS normalized_symbol CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_project_symbol
  ON TABLE symbol_embeddings FIELDS project_id, normalized_symbol CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_project
  ON TABLE symbol_embeddings FIELDS project_id CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_node
  ON TABLE symbol_embeddings FIELDS node_id CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_edge
  ON TABLE symbol_embeddings FIELDS source_edge_id CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_vector_384
  ON TABLE symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_768
  ON TABLE symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1024
  ON TABLE symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1536
  ON TABLE symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2048
  ON TABLE symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2560
  ON TABLE symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3072
  ON TABLE symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_4096
  ON TABLE symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3584
  ON TABLE symbol_embeddings FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- =====================================================================
-- FUNCTIONS
-- =====================================================================

-- Helper function to parse record IDs from various formats
-- Handles: record IDs, "nodes:⟨uuid⟩", "nodes:id", "⟨uuid⟩", "id"
DEFINE FUNCTION fn::parse_record_id($table: string, $input: any) {
IF type::is::record($input) { RETURN $input; };
LET $str = <string>$input;
-- Strip table prefix if present (e.g., "nodes:" or "chunks:")
LET $after_prefix = IF string::starts_with($str, $table + ':') THEN string::slice($str, string::len($table) + 1) ELSE $str END;
-- Strip angle brackets ⟨⟩ if present (SurrealDB's escaping for complex IDs)
LET $clean_id = IF string::starts_with($after_prefix, '⟨') AND string::ends_with($after_prefix, '⟩') THEN string::slice($after_prefix, 1, string::len($after_prefix) - 2) ELSE $after_prefix END;
RETURN type::thing($table, $clean_id);
} PERMISSIONS FULL;

-- Calculate coupling metrics (afferent/efferent coupling, instability)
DEFINE FUNCTION fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
    LET $edge_list = ['calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'depends_on', 'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates'];

    -- Resolve chunk IDs to parent nodes
    LET $resolved_id = IF string::starts_with($node_id, 'chunks:') THEN
        (SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $node_id) LIMIT 1)[0]
    ELSE
        $node_id
    END;

    IF $resolved_id = NONE { RETURN NONE; };

    LET $record = fn::parse_record_id('nodes', $resolved_id);
    LET $node_check = (SELECT project_id FROM $record);
    IF array::len($node_check) = 0 { RETURN NONE; };

    LET $rec_proj = $node_check[0].project_id;
    LET $effective_project = IF $rec_proj != NONE THEN $rec_proj ELSE $project_id END;

    ------------------------------------------------------------------------
    -- Query edges to find dependents (incoming) and dependencies (outgoing)
    ------------------------------------------------------------------------
    LET $dependents = (
        SELECT VALUE `from`
        FROM edges
        WHERE `to` = $record
          AND edge_type INSIDE $edge_list
          AND (`from`.project_id = $effective_project OR `from`.project_id = NONE)
    );

    LET $dependencies = (
        SELECT VALUE `to`
        FROM edges
        WHERE `from` = $record
          AND edge_type INSIDE $edge_list
          AND (`to`.project_id = $effective_project OR `to`.project_id = NONE)
    );

    ------------------------------------------------------------------------
    -- Convert to node references using map and filter
    ------------------------------------------------------------------------
    LET $dependents_info = $dependents
        .map(|$d| fn::node_reference(d))
        .filter(|$r| r != NONE);

    LET $dependencies_info = $dependencies
        .map(|$d| fn::node_reference(d))
        .filter(|$r| r != NONE);

    ------------------------------------------------------------------------
    -- Calculate metrics
    ------------------------------------------------------------------------
    LET $afferent = array::len($dependents_info);
    LET $efferent = array::len($dependencies_info);
    LET $total = $afferent + $efferent;
    LET $instability = IF $total > 0 THEN
        math::round(($efferent / $total) * 1000000f) / 1000000f
    ELSE
        0f
    END;

    RETURN {
        dependencies: $dependencies_info,
        dependents: $dependents_info,
        metrics: {
            afferent_coupling: $afferent,
            efferent_coupling: $efferent,
            total_coupling: $total,
            instability: $instability,
            stability: 1f - $instability,
            is_stable: $instability < 0.3f,
            is_unstable: $instability > 0.7f,
            coupling_category: IF $instability < 0.3f THEN 'stable'
                              ELSE IF $instability > 0.7f THEN 'unstable'
                              ELSE 'balanced' END
        },
        node: fn::node_info($record)
    };
} PERMISSIONS FULL;

-- Detect circular dependencies (A→B and B→A both exist)
DEFINE FUNCTION fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
    LET $edge_name = string::lowercase($edge_type ?? 'calls');

    LET $all_edges = (
        SELECT `from`, `to`
        FROM edges
        WHERE edge_type = $edge_name
          AND `from` != `to`
          AND (`from`.project_id = $project_id OR `from`.project_id = NONE)
          AND (`to`.project_id = $project_id OR `to`.project_id = NONE)
    );

    LET $edge_keys = (SELECT VALUE (<string>`from` + ":" + <string>`to`) FROM $all_edges);

    LET $cycles = (
        SELECT `from` AS node1_id, `to` AS node2_id
        FROM $all_edges
        WHERE `from` < `to`
          AND (<string>`to` + ":" + <string>`from`) INSIDE $edge_keys
    );

    LET $raw = (
        SELECT
            node1_id,
            node2_id,
            fn::node_info(node1_id) AS node1,
            fn::node_info(node2_id) AS node2
        FROM $cycles
    );

    RETURN (
        SELECT
            <string>node1_id AS node1_id,
            <string>node2_id AS node2_id,
            $edge_name AS dependency_type,
            node1,
            node2
        FROM $raw
        WHERE node1 != NONE AND node2 != NONE
    );
} PERMISSIONS FULL;

DEFINE FUNCTION fn::find_nodes_by_name($project_id: string, $needle: string, $limit: int) {
LET $max = IF $limit != NONE AND $limit > 0 THEN $limit ELSE 10 END;
RETURN (SELECT id, name, node_type AS kind, language, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM nodes WHERE project_id = $project_id AND (string::lowercase(name) CONTAINS string::lowercase($needle) OR file_path CONTAINS $needle) ORDER BY name
 LIMIT $max);
} PERMISSIONS FULL;

-- Find hub nodes (highly connected nodes in the graph)
DEFINE FUNCTION fn::get_hub_nodes($project_id: string, $min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
    LET $edge_list = ['calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to', 'depends_on', 'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates'];

    LET $project_edges = (
        SELECT `from`, `to`, edge_type
        FROM edges
        WHERE edge_type INSIDE $edge_list
          AND (`from`.project_id = $project_id OR `from`.project_id = NONE)
          AND (`to`.project_id = $project_id OR `to`.project_id = NONE)
    );

    LET $incoming_totals = (SELECT `to` AS node_id, count() AS total FROM $project_edges GROUP BY `to`);
    LET $outgoing_totals = (SELECT `from` AS node_id, count() AS total FROM $project_edges GROUP BY `from`);

    LET $incoming_map = (SELECT node_id, total AS afferent, 0 AS efferent FROM $incoming_totals);
    LET $outgoing_map = (SELECT node_id, 0 AS afferent, total AS efferent FROM $outgoing_totals);
    LET $all_nodes = array::concat($incoming_map, $outgoing_map);

    LET $node_degrees = (
        SELECT node_id, math::sum(afferent) AS afferent_degree, math::sum(efferent) AS efferent_degree
        FROM $all_nodes
        WHERE node_id != NONE
        GROUP BY node_id
    );

    LET $hub_candidates = (
        SELECT node_id, afferent_degree, efferent_degree, afferent_degree + efferent_degree AS total_degree
        FROM $node_degrees
        WHERE (afferent_degree + efferent_degree) >= $threshold
        ORDER BY total_degree DESC
        LIMIT 50
    );

    LET $incoming_by_type = (SELECT `to` AS node_id, edge_type, count() AS count FROM $project_edges GROUP BY `to`, edge_type);
    LET $outgoing_by_type = (SELECT `from` AS node_id, edge_type, count() AS count FROM $project_edges GROUP BY `from`, edge_type);

    RETURN (
        SELECT
            node_id,
            fn::node_info(node_id) AS node,
            afferent_degree,
            efferent_degree,
            total_degree,
            (SELECT edge_type, count FROM $incoming_by_type WHERE node_id = parent.node_id) AS incoming_by_type,
            (SELECT edge_type, count FROM $outgoing_by_type WHERE node_id = parent.node_id) AS outgoing_by_type
        FROM $hub_candidates
        WHERE fn::node_info(node_id) != NONE
    );
} PERMISSIONS FULL;

DEFINE FUNCTION fn::get_reverse_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
LET $edge_name = string::lowercase($edge_type ?? 'calls');
LET $resolved_id = IF string::starts_with(<string>$node_id, 'chunks:') THEN (
  SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $node_id) LIMIT 1
)[0] ELSE $node_id END;
IF $resolved_id = NONE { RETURN []; };
LET $record = fn::parse_record_id('nodes', $resolved_id);
LET $node_check = SELECT project_id FROM $record;
IF array::len($node_check) = 0 { RETURN []; };
LET $rec_proj = $node_check[0].project_id;
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN []; };
LET $lvl1 = (SELECT VALUE from FROM edges WHERE to = $record AND edge_type = $edge_name AND (from.project_id = $project_id OR from.project_id = NONE));
LET $lvl2 = IF $safe_depth >= 2 AND array::len($lvl1) > 0 THEN (SELECT VALUE from FROM edges WHERE to INSIDE $lvl1 AND edge_type = $edge_name AND (from.project_id = $project_id OR from.project_id = NONE) AND from NOTINSIDE $lvl1 AND from != $record) ELSE [] END;
LET $lvl3 = IF $safe_depth >= 3 AND array::len($lvl2) > 0 THEN (SELECT VALUE from FROM edges WHERE to INSIDE $lvl2 AND edge_type = $edge_name AND (from.project_id = $project_id OR from.project_id = NONE) AND from NOTINSIDE array::concat($lvl1, $lvl2) AND from != $record) ELSE [] END;
LET $lvl4 = IF $safe_depth >= 4 AND array::len($lvl3) > 0 THEN (SELECT VALUE from FROM edges WHERE to INSIDE $lvl3 AND edge_type = $edge_name AND (from.project_id = $project_id OR from.project_id = NONE) AND from NOTINSIDE array::concat($lvl1, $lvl2, $lvl3) AND from != $record) ELSE [] END;
LET $lvl5 = IF $safe_depth >= 5 AND array::len($lvl4) > 0 THEN (SELECT VALUE from FROM edges WHERE to INSIDE $lvl4 AND edge_type = $edge_name AND (from.project_id = $project_id OR from.project_id = NONE) AND from NOTINSIDE array::concat($lvl1, $lvl2, $lvl3, $lvl4) AND from != $record) ELSE [] END;
LET $pairs = array::concat(array::map($lvl1, |$n: any| { depth: 1, id: $n }), array::map($lvl2, |$n: any| { depth: 2, id: $n }), array::map($lvl3, |$n: any| { depth: 3, id: $n }), array::map($lvl4, |$n: any| { depth: 4, id: $n }), array::map($lvl5, |$n: any| { depth: 5, id: $n }));
LET $min_depths = (SELECT id, math::min(depth) AS dependent_depth FROM $pairs GROUP BY id);
LET $raw = (SELECT fn::node_info(id) AS node, dependent_depth FROM $min_depths);
RETURN (SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, dependent_depth, $safe_depth AS requested_depth FROM $raw WHERE node != NONE);
} PERMISSIONS FULL;

DEFINE FUNCTION fn::get_transitive_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
LET $edge_name = string::lowercase($edge_type ?? 'calls');
LET $resolved_id = IF string::starts_with(<string>$node_id, 'chunks:') THEN (
  SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $node_id) LIMIT 1
)[0] ELSE $node_id END;
IF $resolved_id = NONE { RETURN []; };
LET $record = fn::parse_record_id('nodes', $resolved_id);
LET $node_check = SELECT project_id FROM $record;
IF array::len($node_check) = 0 { RETURN []; };
LET $rec_proj = $node_check[0].project_id;
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN []; };
LET $lvl1 = (SELECT VALUE to FROM edges WHERE from = $record AND edge_type = $edge_name AND (to.project_id = $project_id OR to.project_id = NONE));
LET $lvl2 = IF $safe_depth >= 2 AND array::len($lvl1) > 0 THEN (SELECT VALUE to FROM edges WHERE from INSIDE $lvl1 AND edge_type = $edge_name AND (to.project_id = $project_id OR to.project_id = NONE) AND to NOTINSIDE $lvl1 AND to != $record) ELSE [] END;
LET $lvl3 = IF $safe_depth >= 3 AND array::len($lvl2) > 0 THEN (SELECT VALUE to FROM edges WHERE from INSIDE $lvl2 AND edge_type = $edge_name AND (to.project_id = $project_id OR to.project_id = NONE) AND to NOTINSIDE array::concat($lvl1, $lvl2) AND to != $record) ELSE [] END;
LET $lvl4 = IF $safe_depth >= 4 AND array::len($lvl3) > 0 THEN (SELECT VALUE to FROM edges WHERE from INSIDE $lvl3 AND edge_type = $edge_name AND (to.project_id = $project_id OR to.project_id = NONE) AND to NOTINSIDE array::concat($lvl1, $lvl2, $lvl3) AND to != $record) ELSE [] END;
LET $lvl5 = IF $safe_depth >= 5 AND array::len($lvl4) > 0 THEN (SELECT VALUE to FROM edges WHERE from INSIDE $lvl4 AND edge_type = $edge_name AND (to.project_id = $project_id OR to.project_id = NONE) AND to NOTINSIDE array::concat($lvl1, $lvl2, $lvl3, $lvl4) AND to != $record) ELSE [] END;
LET $pairs = array::concat(array::map($lvl1, |$n: any| { depth: 1, id: $n }), array::map($lvl2, |$n: any| { depth: 2, id: $n }), array::map($lvl3, |$n: any| { depth: 3, id: $n }), array::map($lvl4, |$n: any| { depth: 4, id: $n }), array::map($lvl5, |$n: any| { depth: 5, id: $n }));
LET $min_depths = (SELECT id, math::min(depth) AS dependency_depth FROM $pairs GROUP BY id);
LET $raw = (SELECT fn::node_info(id) AS node, dependency_depth FROM $min_depths);
RETURN (SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, dependency_depth, $safe_depth AS requested_depth FROM $raw WHERE node != NONE);
} PERMISSIONS FULL;

DEFINE FUNCTION fn::node_info($node_id: any) {
IF $node_id = NONE OR !type::is::record($node_id) { RETURN NONE; };
LET $res = (SELECT <string>id AS id, name, node_type AS kind, language, content, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM ONLY $node_id);
RETURN $res;
} PERMISSIONS FULL;

DEFINE FUNCTION fn::edge_context($node_ref: any) {
    IF $node_ref = NONE { RETURN { outgoing: [], incoming: [] }; };
    RETURN {
        outgoing: (SELECT
            <string>`to` AS node_id,
            `to`.name AS name,
            `to`.node_type AS kind,
            `to`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `from` = $node_ref
        LIMIT 5),
        incoming: (SELECT
            <string>`from` AS node_id,
            `from`.name AS name,
            `from`.node_type AS kind,
            `from`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `to` = $node_ref
        LIMIT 5)
    };
} PERMISSIONS FULL;

-- ---------------------------------------------------------------------
-- Helper: normalize node to { id, name, kind, location }
-- ---------------------------------------------------------------------
DEFINE FUNCTION fn::node_reference($node_id: any) {
LET $record = fn::parse_record_id('nodes', $node_id);
LET $info = fn::node_info($record);
IF $info = NONE { RETURN NONE; };
RETURN { id: $info.id, kind: $info.kind, location: $info.location, name: $info.name };
} PERMISSIONS FULL;

-- ---------------------------------------------------------------------
-- Hybrid semantic + text search over NODES using HNSW+KNN
-- ---------------------------------------------------------------------
DEFINE FUNCTION fn::semantic_search_with_context(
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

    --------------------------------------------------------------------
    -- STAGE 1: VECTOR SEARCH (dimension-dependent, HNSW KNN)
    -- NOTE: KNN operator requires literal integers: <|k,ef|>
    --------------------------------------------------------------------
    LET $vector_candidates = IF $dimension = 384 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_384 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 768 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_768 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 1024 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_1024 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 1536 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_1536 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 2048 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_2048 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 2560 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_2560 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 3072 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_3072 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 3584 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_3584, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_3584 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE IF $dimension = 4096 THEN (
        SELECT *
        FROM (
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
                vector::distance::knn() AS distance,
                vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
            FROM nodes
            WHERE project_id = $project_id
              AND embedding_4096 <|100,200|> $query_embedding
        )
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    ) ELSE [] END;

    --------------------------------------------------------------------
    -- STAGE 2: FULL-TEXT SEARCH SIDE
    --------------------------------------------------------------------
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

    --------------------------------------------------------------------
    -- STAGE 3: HYBRID MERGE (manual weighted scoring)
    --------------------------------------------------------------------
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

    --------------------------------------------------------------------
    -- STAGE 4: BASIC RESULT SHAPE
    --------------------------------------------------------------------
    LET $base_results = SELECT
        id,
        name,
        kind,
        language,
        content,
        file_path,
        start_line,
        end_line,
        metadata,
        vector_score,
        text_score,
        combined_score
    FROM $merged;

    --------------------------------------------------------------------
    -- STAGE 5: OPTIONAL GRAPH ENRICHMENT
    --------------------------------------------------------------------
    LET $final_results =
        IF $include_graph_context THEN (
            SELECT
                $parent.id        AS id,
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

                -- Direct dependencies (calls/imports/refs from this node)
                (
                    SELECT VALUE fn::node_reference(to)
                    FROM edges
                    WHERE project_id = $project_id
                      AND from       = $parent.id
                      AND edge_type IN ["calls", "imports", "references", "uses"]
                    LIMIT 5
                ) AS direct_dependencies,

                -- Direct dependents (who calls/imports/refers to this node)
                (
                    SELECT VALUE fn::node_reference(from)
                    FROM edges
                    WHERE project_id = $project_id
                      AND to         = $parent.id
                      AND edge_type IN ["calls", "imports", "references", "uses"]
                    LIMIT 5
                ) AS direct_dependents,

                -- File siblings (other symbols in same file)
                (
                    SELECT
                        id,
                        name,
                        node_type,
                        start_line
                    FROM nodes
                    WHERE project_id = $project_id
                      AND file_path   = $parent.file_path
                      AND id         != $parent.id
                    ORDER BY start_line
                    LIMIT 5
                ) AS file_siblings
            FROM (
                SELECT * FROM $base_results
            )
        ) ELSE (
            SELECT
                id,
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
            FROM $base_results
        ) END;

    RETURN $final_results;
}
PERMISSIONS FULL;
-- Hybrid semantic + BM25 search (nodes via chunks) using HNSW KNN index
DEFINE FUNCTION fn::semantic_search_nodes_via_chunks(
    $project_id: string,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $query_embedding: array<float>
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $chunk_limit = $safe_limit * 3;

    LET $chunk_hits =
        IF $dimension = 384 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_384 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 768 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_768 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 1024 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_1024 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 1536 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_1536 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 2048 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_2048 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 2560 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_2560 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 3072 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_3072 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 3584 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_3584 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE IF $dimension = 4096 THEN (
            SELECT id, parent_node, vector::distance::knn() AS distance
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_4096 <|100,200|> $query_embedding
            LIMIT $chunk_limit
        )
        ELSE [] END;

    LET $chunk_nodes = (
        SELECT
            parent_node AS node_ref,
            <string> parent_node AS node_id,
            parent_node.name AS name,
            parent_node.node_type AS kind,
            parent_node.language AS language,
            parent_node.file_path AS file_path,
            parent_node.start_line AS start_line,
            parent_node.end_line AS end_line,
            parent_node.content AS content,
            parent_node.metadata AS metadata,
            1f - distance AS vector_score,
            0f AS text_score,
            ['chunk'] AS match_sources,
            [] AS matched_symbols
        FROM $chunk_hits
        FETCH parent_node
    );

    LET $chunk_ids = (SELECT VALUE node_id FROM $chunk_nodes);

    LET $text_candidates = (
        SELECT
            id AS node_ref,
            <string> id AS node_id,
            name,
            node_type AS kind,
            language,
            content,
            file_path,
            start_line,
            end_line,
            metadata,
            0f AS vector_score,
            (search::score(1) + search::score(2)) AS text_score,
            ['text'] AS match_sources,
            [] AS matched_symbols
        FROM nodes
        WHERE project_id = $project_id
          AND (content @1@ $query_text OR name @2@ $query_text)
          AND <string>id NOTINSIDE $chunk_ids
        ORDER BY text_score DESC
        LIMIT $safe_limit
    );

    LET $combined = array::concat($chunk_nodes ?? [], $text_candidates ?? []);

    LET $scored = (
        SELECT
            node_id,
            node_ref,
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            content,
            metadata,
            vector_score,
            text_score,
            (vector_score * 0.9f) + ((text_score ?? 0f) * 0.1f) AS combined_score,
            match_sources,
            matched_symbols
        FROM $combined
        WHERE node_id != NONE
        ORDER BY combined_score DESC
        LIMIT $safe_limit
    );

    LET $node_refs = (SELECT VALUE node_ref FROM $scored WHERE node_ref != NONE);

    LET $all_outgoing = (
        SELECT
            `from` AS source_node,
            <string>`to` AS node_id,
            `to`.name AS name,
            `to`.node_type AS kind,
            `to`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `from` INSIDE $node_refs
        LIMIT 50
    );

    LET $all_incoming = (
        SELECT
            `to` AS source_node,
            <string>`from` AS node_id,
            `from`.name AS name,
            `from`.node_type AS kind,
            `from`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `to` INSIDE $node_refs
        LIMIT 50
    );

    RETURN (
        SELECT
            node_id,
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            content,
            metadata,
            vector_score,
            text_score,
            combined_score,
            match_sources,
            matched_symbols,
            (SELECT node_id, name, kind, file_path, relationship
             FROM $all_outgoing WHERE source_node = node_ref LIMIT 5) AS outgoing_edges,
            (SELECT node_id, name, kind, file_path, relationship
             FROM $all_incoming WHERE source_node = node_ref LIMIT 5) AS incoming_edges
        FROM $scored
    );
}
PERMISSIONS FULL;

DEFINE FUNCTION fn::trace_call_chain($project_id: string, $from_node: any, $max_depth: int) {
LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;
LET $resolved_id = IF string::starts_with(<string>$from_node, 'chunks:') THEN (
  SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $from_node) LIMIT 1
)[0] ELSE $from_node END;
IF $resolved_id = NONE { RETURN []; };
LET $record = fn::parse_record_id('nodes', $resolved_id);
LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN []; };
LET $raw = (SELECT fn::node_info(id) AS node, array::distinct((SELECT fn::node_reference(from) AS caller FROM edges WHERE to = id AND edge_type = 'calls' AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)).caller) AS called_by FROM (SELECT ->edges[WHERE edge_type = 'calls' AND (to.project_id ?? $project_id) = $project_id] FROM ONLY $record)->to);
RETURN (SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, 1 AS call_depth, called_by, $safe_depth AS requested_depth FROM $raw WHERE node != NONE);
} PERMISSIONS FULL;

DEFINE FUNCTION fn::get_complexity_hotspots($project_id: string, $min_complexity: float, $limit: int) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 { $limit } ELSE { 20 };
    LET $threshold = IF $min_complexity > 0f { $min_complexity } ELSE { 5f };
    LET $edge_list = ['calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'depends_on', 'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates'];

    LET $complex_nodes = SELECT id, name, node_type AS kind, language, file_path, start_line, end_line, complexity
        FROM nodes
        WHERE project_id = $project_id
          AND complexity != NONE
          AND complexity >= $threshold
          AND node_type INSIDE ['function', 'method', 'Function', 'Method']
        ORDER BY complexity DESC
        LIMIT $safe_limit * 2;

    LET $with_coupling = SELECT
            id,
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            complexity,
            (SELECT VALUE count() FROM edges WHERE to = id AND edge_type INSIDE $edge_list AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)) AS afferent_coupling,
            (SELECT VALUE count() FROM edges WHERE from = id AND edge_type INSIDE $edge_list AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)) AS efferent_coupling
        FROM $complex_nodes;

    LET $results = SELECT
            <string>id AS id,
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            complexity,
            (IF type::is::number(afferent_coupling) THEN afferent_coupling ELSE 0 END) AS afferent_coupling,
            (IF type::is::number(efferent_coupling) THEN efferent_coupling ELSE 0 END) AS efferent_coupling,
            (
                math::floor(
                    (
                        IF ((IF type::is::number(efferent_coupling) THEN efferent_coupling ELSE 0 END) + (IF type::is::number(afferent_coupling) THEN afferent_coupling ELSE 0 END)) > 0 {
                            (IF type::is::number(efferent_coupling) THEN efferent_coupling ELSE 0 END) / ((IF type::is::number(efferent_coupling) THEN efferent_coupling ELSE 0 END) + (IF type::is::number(afferent_coupling) THEN afferent_coupling ELSE 0 END))
                        } ELSE { 0f }
                    ) * 10000f
                ) / 10000f
            ) AS instability,
            complexity * ((IF type::is::number(afferent_coupling) THEN afferent_coupling ELSE 0 END) + 1) AS risk_score
        FROM $with_coupling
        ORDER BY risk_score DESC
        LIMIT $safe_limit;

    RETURN $results;
} PERMISSIONS FULL;
-- ---------------------------------------------------------------------
-- Semantic search over CHUNKS + parent node name using HNSW+KNN
-- ---------------------------------------------------------------------
DEFINE FUNCTION fn::semantic_search_chunks_with_context(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $include_graph_context: bool
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold =
        IF $threshold >= 0.0 AND $threshold <= 1.0
        THEN $threshold
        ELSE 0.7
        END;

    LET $chunk_hits =
        IF $dimension = 384 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_384 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 768 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_768 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 1024 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_1024 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 1536 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_1536 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 2048 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_2048 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 2560 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_2560 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 3072 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_3072 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 3584 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_3584, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_3584 <|100,200|> $query_embedding
        )
        ELSE IF $dimension = 4096 THEN (
            SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance,
                   vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
            FROM chunks
            WHERE project_id = $project_id AND parent_node != NONE
              AND embedding_4096 <|100,200|> $query_embedding
        )
        ELSE [] END;

    LET $scored = (
        SELECT *
        FROM $chunk_hits
        WHERE vector_score >= $safe_threshold
        ORDER BY distance ASC
        LIMIT $safe_limit
    );

    LET $base_results = (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            vector_score,
            parent_node.name AS parent_name,
            parent_node.node_type AS parent_kind,
            parent_node.file_path AS file_path,
            parent_node.start_line AS start_line,
            parent_node.end_line AS end_line
        FROM $scored
        FETCH parent_node
    );

    LET $node_refs = IF $include_graph_context THEN (
        SELECT VALUE parent_node
        FROM $base_results
        WHERE parent_node != NONE
    ) ELSE [] END;

    LET $all_outgoing = IF $include_graph_context THEN (
        SELECT
            `from` AS source_node,
            <string>`to` AS node_id,
            `to`.name AS name,
            `to`.node_type AS kind,
            `to`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `from` INSIDE $node_refs
        LIMIT 50
    ) ELSE [] END;

    LET $all_incoming = IF $include_graph_context THEN (
        SELECT
            `to` AS source_node,
            <string>`from` AS node_id,
            `from`.name AS name,
            `from`.node_type AS kind,
            `from`.file_path AS file_path,
            edge_type AS relationship
        FROM edges
        WHERE `to` INSIDE $node_refs
        LIMIT 50
    ) ELSE [] END;

    RETURN IF $include_graph_context THEN (
        SELECT
            id,
            chunk_index,
            text,
            vector_score,
            parent_node.name AS parent_name,
            parent_node.node_type AS parent_kind,
            file_path,
            start_line,
            end_line,
            (SELECT node_id, name, kind, file_path, relationship
             FROM $all_outgoing WHERE source_node = parent_node LIMIT 5) AS outgoing_edges,
            (SELECT node_id, name, kind, file_path, relationship
             FROM $all_incoming WHERE source_node = parent_node LIMIT 5) AS incoming_edges
        FROM $base_results
    ) ELSE (
        SELECT
            id,
            chunk_index,
            text,
            vector_score,
            parent_name,
            parent_kind,
            file_path,
            start_line,
            end_line
        FROM $base_results
    ) END;
}
PERMISSIONS FULL;

-- =====================================================================
-- SCHEMA VERSIONING
-- =====================================================================

DEFINE TABLE schema_versions SCHEMAFULL PERMISSIONS FULL;

DEFINE FIELD applied_at ON TABLE schema_versions TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD checksum ON TABLE schema_versions TYPE option<string>;
DEFINE FIELD description ON TABLE schema_versions TYPE option<string>;
DEFINE FIELD name ON TABLE schema_versions TYPE string;
DEFINE FIELD version ON TABLE schema_versions TYPE int;

DEFINE INDEX idx_schema_version ON TABLE schema_versions FIELDS version UNIQUE CONCURRENTLY;
-- ---------------------------------------------------------------------
-- Graph helpers: neighbors / traversal / edge types / degree
-- ---------------------------------------------------------------------

-- One-hop neighbors by edge kind, scoped by project
DEFINE FUNCTION fn::neighbors(
    $project_id: string,
    $node: record<nodes>,
    $edge_type: string
) {
    RETURN SELECT to, edge_type
    FROM edges
    WHERE project_id = $project_id
      AND from       = $node
      AND edge_type  = $edge_type;
}
PERMISSIONS FULL;

-- Transitive traversal up to depth N (BFS) for given kind, project-scoped
DEFINE FUNCTION fn::traverse_kind(
  $project_id: string,
  $start: record<nodes>,
  $edge_type: string,
  $depth: int
) {
  LET $safe_depth = IF $depth > 0 THEN $depth ELSE 0 END;

  IF $safe_depth = 0 THEN
    RETURN [$start];
  END;

  LET $frontier = [$start];
  LET $visited  = [];

  LET $steps = array::range(1..$safe_depth);

  FOR $step IN $steps {
    LET $next = (
      SELECT VALUE to
      FROM edges
      WHERE project_id = $project_id
        AND edge_type  = $edge_type
        AND from IN $frontier
    );

    LET $frontier = array::distinct($next);
    LET $visited  = array::distinct(array::concat($visited, $frontier));
  };

  RETURN $visited;
}
PERMISSIONS FULL;

-- Enumerate supported edge kinds
DEFINE FUNCTION fn::edge_types() {
  RETURN [
    "calls",
    "defines",
    "imports",
    "uses",
    "extends",
    "implements",
    "references",
    "contains",
    "belongs_to",
    "depends_on",
    "exports",
    "reexports",
    "enables",
    "generates",
    "flows_to",
    "returns",
    "captures",
    "mutates",
    "violates_boundary",
    "documents",
    "specifies"
  ];
}
PERMISSIONS FULL;

-- Simple degree (in/out) of a node within a project
DEFINE FUNCTION fn::degree(
    $project_id: string,
    $node: record<nodes>
) {
  RETURN {
    out: (SELECT count() AS c FROM edges WHERE project_id = $project_id AND from = $node)[0].c,
    inn: (SELECT count() AS c FROM edges WHERE project_id = $project_id AND to   = $node)[0].c
  };
}
PERMISSIONS FULL;

-- =====================================================================
-- EVENTS
-- =====================================================================

-- No events required: updated_at is defined as VALUE time::now() on write.
