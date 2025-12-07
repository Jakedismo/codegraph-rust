-- ------------------------------
-- OPTION
-- ------------------------------

OPTION IMPORT;

-- ------------------------------
-- FUNCTIONS
-- ------------------------------

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
DEFINE FUNCTION fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
LET $edge_list = [
	'calls',
	'imports',
	'uses',
	'extends',
	'implements',
	'references'
];
LET $resolved_id = IF string::starts_with($node_id, 'chunks:') THEN
	(SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $node_id) LIMIT 1)[0]
ELSE
	$node_id
END;
IF $resolved_id = NONE
	{
		RETURN NONE;
	}
;
LET $record = fn::parse_record_id('nodes', $resolved_id);
LET $node_check = (SELECT project_id FROM $record);
IF array::len($node_check) = 0
	{
		RETURN NONE;
	}
;
LET $rec_proj = $node_check[0].project_id;
LET $effective_project = IF $rec_proj != NONE THEN
	$rec_proj
ELSE
	$project_id
END;
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
LET $dependents_info = (SELECT VALUE fn::node_reference(`value`) FROM $dependents WHERE fn::node_reference(`value`) != NONE);
LET $dependencies_info = (SELECT VALUE fn::node_reference(`value`) FROM $dependencies WHERE fn::node_reference(`value`) != NONE);
LET $afferent = array::len($dependents_info);
LET $efferent = array::len($dependencies_info);
LET $total = $afferent + $efferent;
LET $instability = IF $total > 0 THEN
	math::round(($efferent / $total) * 1000000f) / 1000000f
ELSE
	0
END;
RETURN {
	dependencies: $dependencies_info,
	dependents: $dependents_info,
	metrics: {
		afferent_coupling: $afferent,
		coupling_category: IF $instability < 0.3f THEN
			'stable'
		ELSE IF $instability > 0.7f THEN
			'unstable'
		ELSE
			'balanced'
		END,
		efferent_coupling: $efferent,
		instability: $instability,
		is_stable: $instability < 0.3f,
		is_unstable: $instability > 0.7f,
		stability: 1f - $instability,
		total_coupling: $total
	},
	node: fn::node_info($record)
};
} PERMISSIONS FULL;
DEFINE FUNCTION fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
LET $edge_name = string::lowercase($edge_type ?? 'Calls');
-- Use direct field access for project_id filtering
LET $pairs = (SELECT from AS node1_id, to AS node2_id FROM edges WHERE edge_type = $edge_name AND from != to AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE));
LET $cycles = (SELECT node1_id, node2_id FROM $pairs WHERE node1_id < node2_id AND (SELECT VALUE count() FROM edges WHERE edge_type = $edge_name AND from = node2_id AND to = node1_id AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)) > 0 GROUP BY node1_id, node2_id);
LET $raw = (SELECT node1_id, node2_id, fn::node_info(node1_id) AS node1, fn::node_info(node2_id) AS node2 FROM $cycles);
RETURN (SELECT <string>node1_id AS node1_id, <string>node2_id AS node2_id, $edge_name AS dependency_type, node1, node2 FROM $raw WHERE node1 != NONE AND node2 != NONE);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::edge_types() { RETURN ['calls', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to']; } PERMISSIONS FULL;
DEFINE FUNCTION fn::find_nodes_by_name($project_id: string, $needle: string, $limit: int) {
LET $max = IF $limit != NONE AND $limit > 0 THEN $limit ELSE 10 END;
RETURN (SELECT id, name, node_type AS kind, language, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM nodes WHERE project_id = $project_id AND (string::lowercase(name) CONTAINS string::lowercase($needle) OR file_path CONTAINS $needle) ORDER BY name
 LIMIT $max);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::get_hub_nodes($project_id: string, $min_degree: int) {
LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
LET $raw_types = fn::edge_types();
LET $edge_list = array::map(array::filter($raw_types, |$v: any| $v != NONE), |$v: any| string::lowercase($v));
-- Use direct field access (from.project_id) instead of subqueries for proper filtering
LET $project_edges = (SELECT from, to, edge_type FROM edges WHERE edge_type INSIDE $edge_list AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE));
LET $incoming_by_type = (SELECT to AS node_id, edge_type, count() AS count FROM $project_edges GROUP BY to, edge_type);
LET $outgoing_by_type = (SELECT from AS node_id, edge_type, count() AS count FROM $project_edges GROUP BY from, edge_type);
LET $incoming_totals = (SELECT to AS node_id, count() AS total FROM $project_edges GROUP BY to);
LET $outgoing_totals = (SELECT from AS node_id, count() AS total FROM $project_edges GROUP BY from);
LET $candidates = array::distinct(array::concat((SELECT VALUE node_id FROM $incoming_totals), (SELECT VALUE node_id FROM $outgoing_totals)));
LET $candidates_filtered = (SELECT VALUE c FROM $candidates WHERE c != NONE);
LET $raw = (SELECT candidate_id, fn::node_info(candidate_id) AS node, (array::first((SELECT VALUE total FROM $incoming_totals WHERE node_id = candidate_id LIMIT 1)) ?? 0) AS afferent_degree, (array::first((SELECT VALUE total FROM $outgoing_totals WHERE node_id = candidate_id LIMIT 1)) ?? 0) AS efferent_degree, (SELECT edge_type, count FROM $incoming_by_type WHERE node_id = candidate_id) AS incoming_by_type, (SELECT edge_type, count FROM $outgoing_by_type WHERE node_id = candidate_id) AS outgoing_by_type FROM (SELECT node_id AS candidate_id FROM $candidates_filtered));
RETURN (SELECT candidate_id AS node_id, node, afferent_degree, efferent_degree, afferent_degree + efferent_degree AS total_degree, incoming_by_type, outgoing_by_type FROM $raw WHERE node != NONE AND (afferent_degree + efferent_degree) >= $threshold ORDER BY total_degree DESC);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::get_reverse_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
LET $edge_name = string::lowercase($edge_type ?? 'calls');
-- Resolve chunk IDs to their parent nodes using helper for proper ID parsing
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
-- Resolve chunk IDs to their parent nodes using helper for proper ID parsing
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
DEFINE FUNCTION fn::node_reference($node_id: any) {
-- Use helper to handle record IDs, "nodes:⟨uuid⟩", "nodes:id", etc.
LET $record = fn::parse_record_id('nodes', $node_id);
LET $info = fn::node_info($record);
IF $info = NONE { RETURN NONE; };
RETURN { id: $info.id, kind: $info.kind, location: $info.location, name: $info.name };
} PERMISSIONS FULL;
-- Hybrid semantic + BM25 search (nodes via chunks) with dim fallback and EXISTS guards
DEFINE FUNCTION fn::semantic_search_nodes_via_chunks(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    -- More forgiving default threshold (0.3f instead of 0.7f)
    LET $safe_threshold = IF $threshold >= 0f AND $threshold <= 1f THEN $threshold ELSE 0.3f END;
    LET $chunk_limit = $safe_limit * 3;
    LET $symbol_limit = $safe_limit * 2;

    ------------------------------------------------------------------------
    -- 1) Pick an embedding dimension to use for this project (fallback to $dimension)
    ------------------------------------------------------------------------
    LET $dim =
        IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_1024 != NONE LIMIT 1)[0] > 0 {
            1024
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_768 != NONE LIMIT 1)[0] > 0 {
            768
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_384 != NONE LIMIT 1)[0] > 0 {
            384
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_1536 != NONE LIMIT 1)[0] > 0 {
            1536
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_2048 != NONE LIMIT 1)[0] > 0 {
            2048
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_2560 != NONE LIMIT 1)[0] > 0 {
            2560
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_3072 != NONE LIMIT 1)[0] > 0 {
            3072
        } ELSE IF (SELECT VALUE count() FROM chunks WHERE project_id = $project_id AND embedding_4096 != NONE LIMIT 1)[0] > 0 {
            4096
        } ELSE {
            $dimension
        };

    ------------------------------------------------------------------------
    -- 2) Chunk matches per resolved dimension
    ------------------------------------------------------------------------
    LET $chunk_results =
        IF $dim = 384 {
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
        } ELSE IF $dim = 768 {
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
        } ELSE IF $dim = 1024 {
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
        } ELSE IF $dim = 1536 {
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
        } ELSE IF $dim = 2048 {
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
        } ELSE IF $dim = 2560 {
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
        } ELSE IF $dim = 3072 {
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
        } ELSE {
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
        };

    ------------------------------------------------------------------------
    -- 3) Symbol matches per resolved dimension
    ------------------------------------------------------------------------
    LET $symbol_matches =
        IF $dim = 384 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_384, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_384 != NONE
              AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 768 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_768, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_768 != NONE
              AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 1024 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_1024, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_1024 != NONE
              AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 1536 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_1536, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_1536 != NONE
              AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 2048 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_2048, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_2048 != NONE
              AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 2560 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_2560, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_2560 != NONE
              AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE IF $dim = 3072 {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_3072, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_3072 != NONE
              AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        } ELSE {
            SELECT source_edge_id, symbol,
                   vector::similarity::cosine(embedding_4096, $query_embedding) AS symbol_score
            FROM symbol_embeddings
            WHERE project_id = $project_id
              AND embedding_4096 != NONE
              AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold
            ORDER BY symbol_score DESC
            LIMIT $symbol_limit
            FETCH source_edge_id
        };

    ------------------------------------------------------------------------
    -- 4) Turn chunk + symbol hits into node-level vector rows
    ------------------------------------------------------------------------
    LET $symbol_nodes = (
        SELECT
            source_edge_id.from AS parent_node,
            symbol_score * 0.8f AS vector_score,
            ['symbol_reference'] AS match_sources,
            [symbol] AS matched_symbols
        FROM $symbol_matches
        WHERE source_edge_id.from != NONE
          AND source_edge_id.from.project_id = $project_id
          AND (SELECT id FROM nodes WHERE id = source_edge_id.from LIMIT 1)
    );

    LET $chunk_nodes = (
        SELECT
            parent_node,
            vector_score,
            ['chunk'] AS match_sources,
            [] AS matched_symbols
        FROM array::flatten([$chunk_results])
        WHERE parent_node != NONE
          AND (SELECT id FROM nodes WHERE id = parent_node LIMIT 1)
    );

    LET $all_vector_nodes = array::concat($chunk_nodes, $symbol_nodes);

    LET $vector_flat = (
        SELECT
            <string> parent_node.id AS node_id,
            parent_node.name AS name,
            parent_node.node_type AS kind,
            parent_node.language AS language,
            parent_node.file_path AS file_path,
            parent_node.start_line AS start_line,
            parent_node.end_line AS end_line,
            parent_node.content AS content,
            parent_node.metadata AS metadata,
            vector_score,
            0f AS text_score,
            match_sources,
            matched_symbols
        FROM $all_vector_nodes
        WHERE parent_node != NONE
    );

    ------------------------------------------------------------------------
    -- 5) BM25 / full-text candidates (using analyzer-backed indexes)
    ------------------------------------------------------------------------
    LET $text_candidates = (
        SELECT
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
        ORDER BY text_score DESC
        LIMIT $safe_limit
    );

    ------------------------------------------------------------------------
    -- 6) Combine, score, sort, and return
    ------------------------------------------------------------------------
    LET $combined = array::concat($vector_flat, $text_candidates);

    LET $sorted = (
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
            (vector_score * 0.7f) + ((text_score ?? 0f) * 0.3f) AS combined_score,
            match_sources,
            matched_symbols
        FROM $combined
        WHERE node_id != NONE
        ORDER BY combined_score DESC
        LIMIT $safe_limit
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
            (
                SELECT
                    <string> to.id AS node_id,
                    to.name AS name,
                    to.node_type AS kind,
                    to.file_path AS file_path,
                    edge_type AS relationship
                FROM edges
                WHERE from = $parent.node_id
                LIMIT 20
                FETCH to
            ) AS outgoing_edges,
            (
                SELECT
                    <string> from.id AS node_id,
                    from.name AS name,
                    from.node_type AS kind,
                    from.file_path AS file_path,
                    edge_type AS relationship
                FROM edges
                WHERE to = $parent.node_id
                LIMIT 20
                FETCH from
            ) AS incoming_edges
        FROM $sorted
    );
}
PERMISSIONS FULL;
DEFINE FUNCTION fn::trace_call_chain($project_id: string, $from_node: any, $max_depth: int) {
LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;
-- Resolve chunk IDs to their parent nodes using helper for proper ID parsing
LET $resolved_id = IF string::starts_with(<string>$from_node, 'chunks:') THEN (
  SELECT VALUE parent_node FROM fn::parse_record_id('chunks', $from_node) LIMIT 1
)[0] ELSE $from_node END;
IF $resolved_id = NONE { RETURN []; };
LET $record = fn::parse_record_id('nodes', $resolved_id);
LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN []; };
-- Use direct field access for project_id filtering
LET $raw = (SELECT fn::node_info(id) AS node, array::distinct((SELECT fn::node_reference(from) AS caller FROM edges WHERE to = id AND edge_type = 'Calls' AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)).caller) AS called_by FROM (SELECT ->edges[WHERE edge_type = 'Calls' AND (to.project_id ?? $project_id) = $project_id] FROM ONLY $record)->to);
RETURN (SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, 1 AS call_depth, called_by, $safe_depth AS requested_depth FROM $raw WHERE node != NONE);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::get_complexity_hotspots($project_id: string, $min_complexity: float, $limit: int) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 { $limit } ELSE { 20 };
    LET $threshold = IF $min_complexity > 0f { $min_complexity } ELSE { 5f };
    LET $edge_list = ['calls', 'imports', 'uses', 'extends', 'implements', 'references'];

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

-- ------------------------------
-- ANALYZERS
-- ------------------------------

DEFINE ANALYZER code_analyzer TOKENIZERS BLANK,CLASS FILTERS LOWERCASE,SNOWBALL(ENGLISH);

-- ------------------------------
-- TABLE: chunks
-- ------------------------------

DEFINE TABLE chunks TYPE NORMAL SCHEMAFULL COMMENT 'Chunked embeddings for long nodes (tokenizer-aware chunking)' PERMISSIONS FULL;

DEFINE FIELD chunk_index ON chunks TYPE int PERMISSIONS FULL;
DEFINE FIELD created_at ON chunks TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1024 PERMISSIONS FULL;
DEFINE FIELD embedding_1024[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1536 PERMISSIONS FULL;
DEFINE FIELD embedding_1536[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2048 PERMISSIONS FULL;
DEFINE FIELD embedding_2048[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2560 PERMISSIONS FULL;
DEFINE FIELD embedding_2560[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3072 PERMISSIONS FULL;
DEFINE FIELD embedding_3072[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_model ON chunks TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD parent_node ON chunks TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD project_id ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD text ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON chunks TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_chunks_embedding_384 ON chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_768 ON chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1024 ON chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1536 ON chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2048 ON chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2560 ON chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3072 ON chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_4096 ON chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_chunks_order ON chunks FIELDS parent_node, chunk_index CONCURRENTLY;
DEFINE INDEX idx_chunks_parent ON chunks FIELDS parent_node CONCURRENTLY;


-- ------------------------------
-- TABLE: edges
-- ------------------------------

DEFINE TABLE edges TYPE NORMAL SCHEMAFULL COMMENT 'Code relationships (Calls, Imports, Uses, Extends, Implements, References)' PERMISSIONS FULL;

DEFINE FIELD created_at ON edges TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD edge_type ON edges TYPE string PERMISSIONS FULL;
DEFINE FIELD from ON edges TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD metadata ON edges FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD to ON edges TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD weight ON edges TYPE float DEFAULT 1f ASSERT $value > 0f PERMISSIONS FULL;

DEFINE INDEX idx_edges_from ON edges FIELDS from CONCURRENTLY;
DEFINE INDEX idx_edges_from_to ON edges FIELDS from, to CONCURRENTLY;
DEFINE INDEX idx_edges_to ON edges FIELDS to CONCURRENTLY;
DEFINE INDEX idx_edges_type ON edges FIELDS edge_type CONCURRENTLY;
DEFINE INDEX idx_edges_type_from ON edges FIELDS edge_type, from CONCURRENTLY;


-- ------------------------------
-- TABLE: file_metadata
-- ------------------------------

DEFINE TABLE file_metadata TYPE NORMAL SCHEMAFULL COMMENT 'Tracks file state for incremental indexing and change detection' PERMISSIONS NONE;

DEFINE FIELD content_hash ON file_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD created_at ON file_metadata TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD edge_count ON file_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD file_path ON file_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD file_size ON file_metadata TYPE int PERMISSIONS FULL;
DEFINE FIELD language ON file_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD last_indexed_at ON file_metadata TYPE datetime DEFAULT time::now() PERMISSIONS FULL;
DEFINE FIELD modified_at ON file_metadata TYPE datetime PERMISSIONS FULL;
DEFINE FIELD node_count ON file_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD parse_errors ON file_metadata TYPE option<array<string>> PERMISSIONS FULL;
DEFINE FIELD parse_errors[*] ON file_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD project_id ON file_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON file_metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_file_metadata_composite ON file_metadata FIELDS project_id, file_path UNIQUE CONCURRENTLY;
DEFINE INDEX idx_file_metadata_hash ON file_metadata FIELDS content_hash CONCURRENTLY;
DEFINE INDEX idx_file_metadata_modified ON file_metadata FIELDS modified_at CONCURRENTLY;
DEFINE INDEX idx_file_metadata_project ON file_metadata FIELDS project_id CONCURRENTLY;


-- ------------------------------
-- TABLE: metadata
-- ------------------------------

DEFINE TABLE metadata TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD `value` ON metadata FLEXIBLE TYPE option<string | number | bool | object | array> PERMISSIONS FULL;
DEFINE FIELD `value`[*] ON metadata FLEXIBLE TYPE any PERMISSIONS FULL;
DEFINE FIELD key ON metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_metadata_key ON metadata FIELDS key UNIQUE CONCURRENTLY;


-- ------------------------------
-- TABLE: nodes
-- ------------------------------

DEFINE TABLE nodes TYPE NORMAL SCHEMAFULL COMMENT 'Code entities from AST parsing with semantic embeddings' PERMISSIONS FULL;

DEFINE FIELD chunk_count ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD complexity ON nodes TYPE option<float> PERMISSIONS FULL;
DEFINE FIELD content ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON nodes TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1024 PERMISSIONS FULL;
DEFINE FIELD embedding_1024[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1536 PERMISSIONS FULL;
DEFINE FIELD embedding_1536[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2048 PERMISSIONS FULL;
DEFINE FIELD embedding_2048[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2560 PERMISSIONS FULL;
DEFINE FIELD embedding_2560[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3072 PERMISSIONS FULL;
DEFINE FIELD embedding_3072[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_model ON nodes TYPE option<string> DEFAULT 'jina-embeddings-v4' PERMISSIONS FULL;
DEFINE FIELD end_line ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD file_path ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD language ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD metadata ON nodes FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD name ON nodes TYPE string PERMISSIONS FULL;
DEFINE FIELD node_type ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD organization_id ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD repository_url ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD start_line ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD updated_at ON nodes TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_nodes_content_search ON nodes FIELDS content SEARCH ANALYZER code_analyzer BM25(1.2,0.75) DOC_IDS_ORDER 100 DOC_LENGTHS_ORDER 100 POSTINGS_ORDER 100 TERMS_ORDER 100 DOC_IDS_CACHE 100 DOC_LENGTHS_CACHE 100 POSTINGS_CACHE 100 TERMS_CACHE 100 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_384 ON nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_768 ON nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1024 ON nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1536 ON nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2048 ON nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2560 ON nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3072 ON nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_4096 ON nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_nodes_file_path ON nodes FIELDS file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_file_type ON nodes FIELDS file_path, node_type CONCURRENTLY;
DEFINE INDEX idx_nodes_language ON nodes FIELDS language CONCURRENTLY;
DEFINE INDEX idx_nodes_name ON nodes FIELDS name CONCURRENTLY;
DEFINE INDEX idx_nodes_name_search ON nodes FIELDS name SEARCH ANALYZER code_analyzer BM25(1.2,0.75) DOC_IDS_ORDER 100 DOC_LENGTHS_ORDER 100 POSTINGS_ORDER 100 TERMS_ORDER 100 DOC_IDS_CACHE 100 DOC_LENGTHS_CACHE 100 POSTINGS_CACHE 100 TERMS_CACHE 100 CONCURRENTLY;
DEFINE INDEX idx_nodes_project ON nodes FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_nodes_project_type ON nodes FIELDS project_id, node_type CONCURRENTLY;
DEFINE INDEX idx_nodes_type ON nodes FIELDS node_type CONCURRENTLY;


-- ------------------------------
-- TABLE: project_metadata
-- ------------------------------

DEFINE TABLE project_metadata TYPE NORMAL SCHEMAFULL COMMENT 'Project registry with CodeGraph statistics' PERMISSIONS FULL;

DEFINE FIELD codegraph_version ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON project_metadata TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD domain ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD edge_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD file_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD last_analyzed ON project_metadata TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD metadata ON project_metadata FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD name ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD node_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD organization_id ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD primary_language ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD root_path ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON project_metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_project_domain ON project_metadata FIELDS domain CONCURRENTLY;
DEFINE INDEX idx_project_id ON project_metadata FIELDS project_id UNIQUE CONCURRENTLY;
DEFINE INDEX idx_project_name ON project_metadata FIELDS name CONCURRENTLY;
DEFINE INDEX idx_project_org ON project_metadata FIELDS organization_id CONCURRENTLY;


-- ------------------------------
-- TABLE: schema_versions
-- ------------------------------

DEFINE TABLE schema_versions TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD applied_at ON schema_versions TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD checksum ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD description ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD name ON schema_versions TYPE string PERMISSIONS FULL;
DEFINE FIELD version ON schema_versions TYPE int PERMISSIONS FULL;

DEFINE INDEX idx_schema_version ON schema_versions FIELDS version UNIQUE CONCURRENTLY;


-- ------------------------------
-- TABLE: symbol_embeddings
-- ------------------------------

DEFINE TABLE symbol_embeddings TYPE NORMAL SCHEMAFULL COMMENT 'Cached embeddings for normalized symbols used during edge resolution' PERMISSIONS FULL;

DEFINE FIELD access_count ON symbol_embeddings TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1024 PERMISSIONS FULL;
DEFINE FIELD embedding_1024[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 1536 PERMISSIONS FULL;
DEFINE FIELD embedding_1536[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2048 PERMISSIONS FULL;
DEFINE FIELD embedding_2048[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 2560 PERMISSIONS FULL;
DEFINE FIELD embedding_2560[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3072 PERMISSIONS FULL;
DEFINE FIELD embedding_3072[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_model ON symbol_embeddings TYPE string DEFAULT 'jina-embeddings-v4' PERMISSIONS FULL;
DEFINE FIELD last_computed_at ON symbol_embeddings TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD metadata ON symbol_embeddings FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD node_id ON symbol_embeddings TYPE option<record<nodes>> PERMISSIONS FULL;
DEFINE FIELD normalized_symbol ON symbol_embeddings TYPE string PERMISSIONS FULL;
DEFINE FIELD organization_id ON symbol_embeddings TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON symbol_embeddings TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD source_edge_id ON symbol_embeddings TYPE option<record<edges>> PERMISSIONS FULL;
DEFINE FIELD symbol ON symbol_embeddings TYPE string PERMISSIONS FULL;

DEFINE INDEX idx_symbol_embeddings_edge ON symbol_embeddings FIELDS source_edge_id CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_node ON symbol_embeddings FIELDS node_id CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_project_symbol ON symbol_embeddings FIELDS project_id, normalized_symbol CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_symbol ON symbol_embeddings FIELDS normalized_symbol CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_384 ON symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_768 ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1024 ON symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1536 ON symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2048 ON symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2560 ON symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3072 ON symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_4096 ON symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 M0 32 LM 0.36067376022224085f CONCURRENTLY;
-- ------------------------------
