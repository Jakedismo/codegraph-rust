-- Hybrid semantic + BM25 search (nodes via chunks) with dim fallback and EXISTS guards
-- This is a drop-in replacement candidate for fn::semantic_search_nodes_via_chunks

DEFINE FUNCTION fn::semantic_search_nodes_via_chunks_hybrid(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold = IF $threshold >= 0f AND $threshold <= 1f THEN $threshold ELSE 0.7f END;
    LET $chunk_limit = $safe_limit * 3;
    LET $symbol_limit = $safe_limit * 2;

    LET $dim =
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_1024 != NONE LIMIT 1)[0] > 0 THEN 1024 ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_768  != NONE LIMIT 1)[0] > 0 THEN 768  ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_384  != NONE LIMIT 1)[0] > 0 THEN 384  ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_1536 != NONE LIMIT 1)[0] > 0 THEN 1536 ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_2048 != NONE LIMIT 1)[0] > 0 THEN 2048 ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_2560 != NONE LIMIT 1)[0] > 0 THEN 2560 ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_3072 != NONE LIMIT 1)[0] > 0 THEN 3072 ELSE
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_4096 != NONE LIMIT 1)[0] > 0 THEN 4096 ELSE $dimension;

    LET $chunk_results = IF $dim = 384 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_384 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 768 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_768 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 1024 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_1024 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 1536 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_1536 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 2048 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_2048 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 2560 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_2560 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE IF $dim = 3072 THEN (
        SELECT parent_node,
               vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_3072 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    ) ELSE (
        SELECT parent_node,
               vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score
        FROM chunks
        WHERE project_id = $project_id
          AND embedding_4096 != NONE
          AND parent_node != NONE
          AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $chunk_limit
        FETCH parent_node
    );

    LET $symbol_matches = IF $dim = 384 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_384, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_384 != NONE
          AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 768 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_768, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_768 != NONE
          AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 1024 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_1024, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_1024 != NONE
          AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 1536 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_1536, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_1536 != NONE
          AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 2048 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_2048, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_2048 != NONE
          AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 2560 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_2560, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_2560 != NONE
          AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE IF $dim = 3072 THEN (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_3072, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_3072 != NONE
          AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    ) ELSE (
        SELECT source_edge_id, symbol,
               vector::similarity::cosine(embedding_4096, $query_embedding) AS symbol_score
        FROM symbol_embeddings
        WHERE project_id = $project_id
          AND embedding_4096 != NONE
          AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
        ORDER BY symbol_score DESC
        LIMIT $symbol_limit
        FETCH source_edge_id
    );

    LET $symbol_nodes = (
        SELECT source_edge_id.from AS parent_node,
               symbol_score * 0.8f AS vector_score,
               'symbol_reference' AS match_source,
               symbol AS matched_symbol
        FROM $symbol_matches
        WHERE source_edge_id.from != NONE
          AND source_edge_id.from.project_id = $project_id
          AND (SELECT id FROM nodes WHERE id = source_edge_id.from LIMIT 1)
    );

    LET $chunk_nodes = (
        SELECT parent_node,
               vector_score,
               'chunk' AS match_source,
               NONE AS matched_symbol
        FROM array::flatten([$chunk_results])
        WHERE parent_node != NONE
          AND (SELECT id FROM nodes WHERE id = parent_node LIMIT 1)
    );

    LET $all_nodes = array::concat($chunk_nodes, $symbol_nodes);

    LET $vector_ranked = (
        SELECT math::max(vector_score) AS vector_score,
               parent_node.id AS node_id,
               parent_node.name AS name,
               parent_node.node_type AS kind,
               parent_node.language AS language,
               parent_node.file_path AS file_path,
               parent_node.start_line AS start_line,
               parent_node.end_line AS end_line,
               parent_node.content AS content,
               parent_node.metadata AS metadata,
               array::distinct(array::flatten([match_source])) AS match_sources,
               array::distinct(array::filter(array::flatten([matched_symbol]), |$v| $v != NONE)) AS matched_symbols
        FROM $all_nodes
        WHERE parent_node != NONE
        GROUP BY parent_node.id
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    );

    LET $text_candidates = (
        SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata,
               0f AS vector_score,
               search::score(1) AS text_score,
               ['text'] AS match_sources,
               [] AS matched_symbols
        FROM nodes
        WHERE project_id = $project_id
          AND (name @1@ $query_text OR content @2@ $query_text)
        ORDER BY text_score DESC
        LIMIT $safe_limit
    );

    LET $combined = array::concat($vector_ranked, $text_candidates);

    LET $deduped = (
        SELECT node_id,
               math::max(vector_score) AS vector_score,
               math::max(text_score ?? 0) AS text_score,
               array::first(name) AS name,
               array::first(kind) AS kind,
               array::first(language) AS language,
               array::first(content) AS content,
               array::first(file_path) AS file_path,
               array::first(start_line) AS start_line,
               array::first(end_line) AS end_line,
               array::first(metadata) AS metadata,
               array::distinct(array::flatten([match_sources])) AS match_sources,
               array::distinct(array::flatten([matched_symbols])) AS matched_symbols,
               (math::max(vector_score) * 0.7f) + (math::max(text_score ?? 0) * 0.3f) AS combined_score
        FROM $combined
        WHERE node_id != NONE
        GROUP BY node_id
        ORDER BY combined_score DESC
        LIMIT $safe_limit
    );

    RETURN (
        SELECT node_id,
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
               (SELECT <string> to.id AS node_id, to.name AS name, to.node_type AS kind, to.file_path AS file_path, edge_type AS relationship FROM edges WHERE from = $parent.node_id LIMIT 20 FETCH to) AS outgoing_edges,
               (SELECT <string> from.id AS node_id, from.name AS name, from.node_type AS kind, from.file_path AS file_path, edge_type AS relationship FROM edges WHERE to = $parent.node_id LIMIT 20 FETCH from) AS incoming_edges
        FROM $deduped
    );
}
PERMISSIONS FULL;
