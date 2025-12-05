-- =====================================================================
-- Codegraph experimental DB
-- =====================================================================

USE DB codegraph_experimental;

-- =====================================================================
-- ANALYZER FOR CODE / TEXT SEARCH
-- =====================================================================

DEFINE ANALYZER code_analyzer
  TOKENIZERS class, camel, punct
  FILTERS lowercase, ascii;

-- =====================================================================
-- CORE TABLES
-- =====================================================================

-- -----------------------------
-- nodes: code entities / symbols
-- -----------------------------
DEFINE TABLE nodes SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id      ON TABLE nodes TYPE string ASSERT $value != '';
DEFINE FIELD language        ON TABLE nodes TYPE option<string>;
DEFINE FIELD node_type       ON TABLE nodes TYPE string;              -- e.g. function, struct, class
DEFINE FIELD name            ON TABLE nodes TYPE string;              -- symbol name
DEFINE FIELD file_path       ON TABLE nodes TYPE string;              -- file path
DEFINE FIELD content         ON TABLE nodes TYPE option<string>;      -- optional full body / doc
DEFINE FIELD start_line      ON TABLE nodes TYPE option<int>;
DEFINE FIELD end_line        ON TABLE nodes TYPE option<int>;
DEFINE FIELD metadata        ON TABLE nodes TYPE option<object>;      -- arbitrary metadata
DEFINE FIELD range           ON TABLE nodes TYPE option<object>;      -- {start:{line}, end:{line}}
DEFINE FIELD chunk_count     ON TABLE nodes TYPE option<int> DEFAULT 0;

DEFINE FIELD embedding_384   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_768   ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_1024  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1536  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_2048  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2560  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_3072  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_4096  ON TABLE nodes TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;

DEFINE FIELD embedding_model ON TABLE nodes TYPE option<string>;
DEFINE FIELD created_at      ON TABLE nodes TYPE datetime VALUE time::now();
DEFINE FIELD updated_at      ON TABLE nodes TYPE datetime VALUE time::now();

-- -----------------------------
-- edges: typed relationships between nodes
-- -----------------------------
DEFINE TABLE edges SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id   ON TABLE edges TYPE string ASSERT $value != '';
DEFINE FIELD kind         ON TABLE edges TYPE string;             -- "Calls", "Imports", "Inherits", ...
DEFINE FIELD from         ON TABLE edges TYPE record<nodes>;
DEFINE FIELD to           ON TABLE edges TYPE record<nodes>;
DEFINE FIELD weight       ON TABLE edges TYPE option<float>;
DEFINE FIELD metadata     ON TABLE edges TYPE option<object>;
DEFINE FIELD created_at   ON TABLE edges TYPE datetime VALUE time::now();
DEFINE FIELD updated_at   ON TABLE edges TYPE datetime VALUE time::now();

-- -----------------------------
-- chunks: text chunks linked to parent node
-- -----------------------------
DEFINE TABLE chunks SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id      ON TABLE chunks TYPE string ASSERT $value != '';
DEFINE FIELD parent_node     ON TABLE chunks TYPE record<nodes>;
DEFINE FIELD chunk_index     ON TABLE chunks TYPE int;
DEFINE FIELD text            ON TABLE chunks TYPE string;

DEFINE FIELD embedding_384   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_768   ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_1024  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1536  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_2048  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2560  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_3072  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_4096  ON TABLE chunks TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;

DEFINE FIELD embedding_model ON TABLE chunks TYPE option<string>;
DEFINE FIELD created_at      ON TABLE chunks TYPE datetime VALUE time::now();
DEFINE FIELD updated_at      ON TABLE chunks TYPE datetime VALUE time::now();

-- -----------------------------
-- file_metadata: per-file aggregation
-- -----------------------------
DEFINE TABLE file_metadata SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id  ON TABLE file_metadata TYPE string ASSERT $value != '';
DEFINE FIELD file_path   ON TABLE file_metadata TYPE string;
DEFINE FIELD language    ON TABLE file_metadata TYPE option<string>;
DEFINE FIELD node_ids    ON TABLE file_metadata TYPE option<array<record<nodes>>>;
DEFINE FIELD created_at  ON TABLE file_metadata TYPE datetime VALUE time::now();
DEFINE FIELD updated_at  ON TABLE file_metadata TYPE datetime VALUE time::now();

-- -----------------------------
-- project_metadata: project-level stats
-- -----------------------------
DEFINE TABLE project_metadata SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_root   ON TABLE project_metadata TYPE string;
DEFINE FIELD language       ON TABLE project_metadata TYPE option<string>;
DEFINE FIELD node_count     ON TABLE project_metadata TYPE option<int>;
DEFINE FIELD edge_count     ON TABLE project_metadata TYPE option<int>;
DEFINE FIELD chunk_count    ON TABLE project_metadata TYPE option<int>;
DEFINE FIELD last_analyzed  ON TABLE project_metadata TYPE option<datetime>;
DEFINE FIELD created_at     ON TABLE project_metadata TYPE datetime VALUE time::now();
DEFINE FIELD updated_at     ON TABLE project_metadata TYPE datetime VALUE time::now();

-- -----------------------------
-- symbol_embeddings: identifier-level embeddings per node
-- -----------------------------
DEFINE TABLE symbol_embeddings SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD project_id         ON TABLE symbol_embeddings TYPE string ASSERT $value != '';
DEFINE FIELD node_id            ON TABLE symbol_embeddings TYPE record<nodes>;
DEFINE FIELD symbol             ON TABLE symbol_embeddings TYPE string;
DEFINE FIELD normalized_symbol  ON TABLE symbol_embeddings TYPE option<string>;

DEFINE FIELD embedding_384      ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 384;
DEFINE FIELD embedding_768      ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 768;
DEFINE FIELD embedding_1024     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1024;
DEFINE FIELD embedding_1536     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 1536;
DEFINE FIELD embedding_2048     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2048;
DEFINE FIELD embedding_2560     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 2560;
DEFINE FIELD embedding_3072     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 3072;
DEFINE FIELD embedding_4096     ON TABLE symbol_embeddings TYPE option<array<float>>
  ASSERT $value = NONE OR array::len($value) = 4096;

DEFINE FIELD embedding_model    ON TABLE symbol_embeddings TYPE option<string>;
DEFINE FIELD created_at         ON TABLE symbol_embeddings TYPE datetime VALUE time::now();
DEFINE FIELD updated_at         ON TABLE symbol_embeddings TYPE datetime VALUE time::now();

-- =====================================================================
-- INDEXES
-- =====================================================================

-- nodes: scalar indexes
DEFINE INDEX idx_nodes_name       ON TABLE nodes COLUMNS name CONCURRENTLY;
DEFINE INDEX idx_nodes_type       ON TABLE nodes COLUMNS node_type CONCURRENTLY;
DEFINE INDEX idx_nodes_language   ON TABLE nodes COLUMNS language CONCURRENTLY;
DEFINE INDEX idx_nodes_file_path  ON TABLE nodes COLUMNS file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_project    ON TABLE nodes COLUMNS project_id CONCURRENTLY;
-- nodes: full-text indexes for hybrid search
DEFINE INDEX idx_nodes_name_search
  ON TABLE nodes FIELDS name
  SEARCH ANALYZER code_analyzer BM25 CONCURRENTLY;

DEFINE INDEX idx_nodes_content_search
  ON TABLE nodes FIELDS content
  SEARCH ANALYZER code_analyzer BM25 CONCURRENTLY;

-- nodes: HNSW vector indexes (COSINE metric)
DEFINE INDEX idx_nodes_embedding_384
  ON TABLE nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_768
  ON TABLE nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1024
  ON TABLE nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1536
  ON TABLE nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2048
  ON TABLE nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2560
  ON TABLE nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3072
  ON TABLE nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_4096
  ON TABLE nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 150 M 12 CONCURRENTLY;

-- edges: scalar indexes (project-scoped)
DEFINE INDEX idx_edges_project      ON TABLE edges COLUMNS project_id CONCURRENTLY;
DEFINE INDEX idx_edges_from         ON TABLE edges COLUMNS from CONCURRENTLY;
DEFINE INDEX idx_edges_to           ON TABLE edges COLUMNS to CONCURRENTLY;
DEFINE INDEX idx_edges_kind         ON TABLE edges COLUMNS kind CONCURRENTLY;
DEFINE INDEX idx_edges_from_to      ON TABLE edges COLUMNS from, to CONCURRENTLY;
DEFINE INDEX idx_edges_kind_from    ON TABLE edges COLUMNS kind, from CONCURRENTLY;
DEFINE INDEX idx_edges_project_kind_from
  ON TABLE edges COLUMNS project_id, kind, from CONCURRENTLY;
DEFINE INDEX idx_edges_project_kind_to
  ON TABLE edges COLUMNS project_id, kind, to CONCURRENTLY;

-- chunks: scalar + vector indexes
DEFINE INDEX idx_chunks_parent
  ON TABLE chunks COLUMNS parent_node CONCURRENTLY;
DEFINE INDEX idx_chunks_order
  ON TABLE chunks COLUMNS parent_node, chunk_index CONCURRENTLY;
DEFINE INDEX idx_chunks_project
  ON TABLE chunks COLUMNS project_id CONCURRENTLY;

DEFINE INDEX idx_chunks_embedding_384
  ON TABLE chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_768
  ON TABLE chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1024
  ON TABLE chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1536
  ON TABLE chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2048
  ON TABLE chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2560
  ON TABLE chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3072
  ON TABLE chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_4096
  ON TABLE chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 150 M 12 CONCURRENTLY;

-- file_metadata indexes
DEFINE INDEX idx_file_metadata_project_path
  ON TABLE file_metadata COLUMNS project_id, file_path CONCURRENTLY;

-- symbol_embeddings indexes
DEFINE INDEX idx_symbol_embeddings_symbol
  ON TABLE symbol_embeddings COLUMNS normalized_symbol CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_project_symbol
  ON TABLE symbol_embeddings COLUMNS project_id, normalized_symbol CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_vector_384
  ON TABLE symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_768
  ON TABLE symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1024
  ON TABLE symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1536
  ON TABLE symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2048
  ON TABLE symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2560
  ON TABLE symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3072
  ON TABLE symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_4096
  ON TABLE symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE EFC 150 M 12 CONCURRENTLY;

-- =====================================================================
-- FUNCTIONS
-- =====================================================================

-- ---------------------------------------------------------------------
-- Helper: normalize node to { id, name, kind, location }
-- ---------------------------------------------------------------------
DEFINE FUNCTION fn::node_reference($node: record<nodes>) {
    LET $info = (
        SELECT
            id,
            name,
            node_type AS kind,
            file_path,
            start_line,
            end_line
        FROM $node
        LIMIT 1
    )[0];

    IF $info = NONE THEN
        RETURN NONE;
    END;

    RETURN {
        id: $info.id,
        name: $info.name,
        kind: $info.kind,
        location: {
            file_path:  $info.file_path,
            start_line: $info.start_line,
            end_line:   $info.end_line
        }
    };
}
PERMISSIONS FULL;

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
                      AND kind IN ["Calls", "Imports", "References"]
                    LIMIT 5
                ) AS direct_dependencies,

                -- Direct dependents (who calls/imports/refers to this node)
                (
                    SELECT VALUE fn::node_reference(from)
                    FROM edges
                    WHERE project_id = $project_id
                      AND to         = $parent.id
                      AND kind IN ["Calls", "Imports", "References"]
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
-- ---------------------------------------------------------------------
-- Semantic search over CHUNKS + parent node name using HNSW+KNN
-- ---------------------------------------------------------------------
DEFINE FUNCTION fn::semantic_search_chunks_with_context(
    $project: string,
    $query_vec: array<float>,
    $dim: int,
    $limit: int
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;

    LET $candidates =
        IF $dim = 384 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id   = $project
                  AND embedding_384 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 768 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id   = $project
                  AND embedding_768 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 1024 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_1024 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 1536 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_1536 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 2048 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_2048 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 2560 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_2560 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 3072 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_3072 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE IF $dim = 4096 THEN (
            SELECT *
            FROM (
                SELECT
                    id,
                    parent_node,
                    chunk_index,
                    text,
                    vector::distance::knn() AS distance
                FROM chunks
                WHERE project_id    = $project
                  AND embedding_4096 <|100,200|> $query_vec
            )
            ORDER BY distance ASC
            LIMIT $safe_limit
        )
        ELSE [] END;

    LET $results = (
        SELECT
            id,
            parent_node,
            chunk_index,
            text,
            distance,
            parent_node.name AS parent_name
        FROM $candidates
    );

    RETURN $results;
}
PERMISSIONS FULL;
-- ---------------------------------------------------------------------
-- Graph helpers: neighbors / traversal / edge types / degree
-- ---------------------------------------------------------------------

-- One-hop neighbors by edge kind, scoped by project
DEFINE FUNCTION fn::neighbors(
    $project_id: string,
    $node: record<nodes>,
    $kind: string
) {
    RETURN SELECT to, kind
    FROM edges
    WHERE project_id = $project_id
      AND from       = $node
      AND kind       = $kind;
}
PERMISSIONS FULL;

-- Transitive traversal up to depth N (BFS) for given kind, project-scoped
DEFINE FUNCTION fn::traverse_kind(
  $project_id: string,
  $start: record<nodes>,
  $kind: string,
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
        AND kind       = $kind
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
  RETURN ["Calls", "Imports", "Inherits", "Contains", "References"];
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
-- EVENTS TO KEEP updated_at IN SYNC
-- =====================================================================

DEFINE EVENT edges_touch_updated
ON TABLE edges
WHEN $event != "DELETE"
THEN (
  UPDATE $after.id SET updated_at = time::now()
);

DEFINE EVENT nodes_touch_updated
ON TABLE nodes
WHEN $event != "DELETE"
THEN (
  UPDATE $after.id SET updated_at = time::now()
);

DEFINE EVENT chunks_touch_updated
ON TABLE chunks
WHEN $event != "DELETE"
THEN (
  UPDATE $after.id SET updated_at = time::now()
);