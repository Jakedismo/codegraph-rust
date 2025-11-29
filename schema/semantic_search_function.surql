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
    -- Validate and normalize parameters
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold = IF $threshold >= 0.0 AND $threshold <= 1.0 THEN $threshold ELSE 0.7 END;

    -- =========================================================================
    -- STAGE 1: HNSW Vector Search (Semantic Similarity)
    -- =========================================================================

    -- Select appropriate embedding field based on dimension
    -- SurrealDB requires we query each dimension separately due to type system
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

    -- =========================================================================
    -- STAGE 2: Full-Text Search (using code_analyzer)
    -- =========================================================================

    -- Search using the code_analyzer for fuzzy matching on content and name
    -- This catches exact matches and variations the vector search might miss
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
        0.0 AS vector_score,  -- No vector score for text matches
        search::score(1) AS text_score  -- Full-text relevance score
    FROM nodes
    WHERE project_id = $project_id
      AND (
          name @1@ $query_text
          OR content @1@ $query_text
      )
    ORDER BY text_score DESC
    LIMIT $safe_limit;

    -- =========================================================================
    -- STAGE 3: Merge Results (Hybrid Search)
    -- =========================================================================

    -- Combine vector and text results, deduplicate by id
    -- For duplicates, keep the one with higher combined score
    LET $all_candidates = array::concat($vector_candidates, $text_candidates);

    -- Group by id and select best score
    LET $merged = (
        SELECT
            id,
            array::first(name) AS name,
            array::first(kind) AS kind,
            array::first(language) AS language,
            array::first(content) AS content,
            array::first(file_path) AS file_path,
            array::first(start_line) AS start_line,
            array::first(end_line) AS end_line,
            array::first(metadata) AS metadata,
            math::max(vector_score) AS vector_score,
            math::max(text_score ?? 0.0) AS text_score,
            -- Combined score: 70% vector + 30% text (configurable weights)
            (math::max(vector_score) * 0.7) + (math::max(text_score ?? 0.0) * 0.3) AS combined_score
        FROM $all_candidates
        GROUP BY id
        ORDER BY combined_score DESC
        LIMIT $safe_limit
    );

    -- =========================================================================
    -- STAGE 4: Graph Enrichment (Optional)
    -- =========================================================================

    LET $final_results = IF $include_graph_context THEN (
        -- Enrich each result with graph context
        SELECT
            id,
            name,
            kind,
            language,
            content,
            {
                file_path: file_path,
                start_line: start_line,
                end_line: end_line
            } AS location,
            metadata,
            vector_score,
            text_score,
            combined_score,
            -- Direct dependencies (what this node calls/uses)
            (
                SELECT VALUE fn::node_reference(<string>out)
                FROM ->calls, ->imports, ->uses
                WHERE out.project_id = $project_id
                LIMIT 5
            ) AS direct_dependencies,
            -- Direct dependents (what calls/uses this node)
            (
                SELECT VALUE fn::node_reference(<string>in)
                FROM <-calls, <-imports, <-uses
                WHERE in.project_id = $project_id
                LIMIT 5
            ) AS direct_dependents,
            -- File siblings (other nodes in same file, for context)
            (
                SELECT id, name, node_type, start_line AS kind
                FROM nodes
                WHERE project_id = $project_id
                  AND file_path = $parent.file_path
                  AND id != $parent.id
                ORDER BY start_line
                LIMIT 5
            ) AS file_siblings
        FROM $merged
    ) ELSE (
        -- Without graph context, just return basic results
        SELECT
            id,
            name,
            kind,
            language,
            content,
            {
                file_path: file_path,
                start_line: start_line,
                end_line: end_line
            } AS location,
            metadata,
            vector_score,
            text_score,
            combined_score
        FROM $merged
    ) END;
    };
    RETURN $final_results;