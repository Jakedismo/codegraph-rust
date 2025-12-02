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

    -- Dimension-dependent vector search
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
    ) END;

    LET $vector_results = array::flatten([$vector_candidates]);

    -- Attach node context
    LET $results = (
        SELECT
            id,
            chunk_index,
            text,
            vector_score,

            -- Node context (resolved per row)
            (SELECT VALUE id        FROM nodes WHERE id = parent_node LIMIT 1)[0] AS node_id,
            (SELECT VALUE name      FROM nodes WHERE id = parent_node LIMIT 1)[0] AS name,
            (SELECT VALUE node_type FROM nodes WHERE id = parent_node LIMIT 1)[0] AS kind,
            (SELECT VALUE language  FROM nodes WHERE id = parent_node LIMIT 1)[0] AS language,
            (SELECT VALUE file_path FROM nodes WHERE id = parent_node LIMIT 1)[0] AS file_path,
            (SELECT VALUE start_line FROM nodes WHERE id = parent_node LIMIT 1)[0] AS start_line,
            (SELECT VALUE end_line   FROM nodes WHERE id = parent_node LIMIT 1)[0] AS end_line,
            (SELECT VALUE metadata   FROM nodes WHERE id = parent_node LIMIT 1)[0] AS metadata
        FROM $vector_results
    );

    RETURN $results;
}
PERMISSIONS FULL;