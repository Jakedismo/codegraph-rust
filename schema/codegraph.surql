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
-- Calculate coupling metrics (afferent/efferent coupling, instability)
-- FIXED: Added missing edge queries for dependents/dependencies
DEFINE FUNCTION fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
    LET $edge_list = ['calls', 'imports', 'uses', 'extends', 'implements', 'references'];

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
-- OPTIMIZED: Uses edge key set for O(E) instead of O(E²) correlated subqueries
DEFINE FUNCTION fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
    LET $edge_name = string::lowercase($edge_type ?? 'Calls');

    ------------------------------------------------------------------------
    -- 1) Get all edges for project in ONE query
    ------------------------------------------------------------------------
    LET $all_edges = (
        SELECT `from`, `to`
        FROM edges
        WHERE edge_type = $edge_name
          AND `from` != `to`
          AND (`from`.project_id = $project_id OR `from`.project_id = NONE)
          AND (`to`.project_id = $project_id OR `to`.project_id = NONE)
    );

    ------------------------------------------------------------------------
    -- 2) Create set of edge keys as "from:to" strings for O(1) lookup
    ------------------------------------------------------------------------
    LET $edge_keys = (SELECT VALUE (<string>`from` + ":" + <string>`to`) FROM $all_edges);

    ------------------------------------------------------------------------
    -- 3) Find cycles: edges where reverse key exists in set
    --    Use from < to to report each cycle only once
    ------------------------------------------------------------------------
    LET $cycles = (
        SELECT `from` AS node1_id, `to` AS node2_id
        FROM $all_edges
        WHERE `from` < `to`
          AND (<string>`to` + ":" + <string>`from`) INSIDE $edge_keys
    );

    ------------------------------------------------------------------------
    -- 4) Enrich with node info
    ------------------------------------------------------------------------
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
DEFINE FUNCTION fn::edge_types() { RETURN ['calls', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to']; } PERMISSIONS FULL;
DEFINE FUNCTION fn::find_nodes_by_name($project_id: string, $needle: string, $limit: int) {
LET $max = IF $limit != NONE AND $limit > 0 THEN $limit ELSE 10 END;
RETURN (SELECT id, name, node_type AS kind, language, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM nodes WHERE project_id = $project_id AND (string::lowercase(name) CONTAINS string::lowercase($needle) OR file_path CONTAINS $needle) ORDER BY name
 LIMIT $max);
} PERMISSIONS FULL;
-- Find hub nodes (highly connected nodes in the graph)
-- OPTIMIZED: Filter by threshold BEFORE enriching, use efficient lookups
DEFINE FUNCTION fn::get_hub_nodes($project_id: string, $min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;
    LET $edge_list = ['calls', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to'];

    ------------------------------------------------------------------------
    -- 1) Get all project edges in ONE query
    ------------------------------------------------------------------------
    LET $project_edges = (
        SELECT `from`, `to`, edge_type
        FROM edges
        WHERE edge_type INSIDE $edge_list
          AND (`from`.project_id = $project_id OR `from`.project_id = NONE)
          AND (`to`.project_id = $project_id OR `to`.project_id = NONE)
    );

    ------------------------------------------------------------------------
    -- 2) Compute totals per node
    ------------------------------------------------------------------------
    LET $incoming_totals = (SELECT `to` AS node_id, count() AS total FROM $project_edges GROUP BY `to`);
    LET $outgoing_totals = (SELECT `from` AS node_id, count() AS total FROM $project_edges GROUP BY `from`);

    ------------------------------------------------------------------------
    -- 3) Merge into single list with combined degree, FILTER BY THRESHOLD EARLY
    ------------------------------------------------------------------------
    LET $incoming_map = (SELECT node_id, total AS afferent, 0 AS efferent FROM $incoming_totals);
    LET $outgoing_map = (SELECT node_id, 0 AS afferent, total AS efferent FROM $outgoing_totals);
    LET $all_nodes = array::concat($incoming_map, $outgoing_map);

    -- Group by node_id and sum degrees
    LET $node_degrees = (
        SELECT node_id, math::sum(afferent) AS afferent_degree, math::sum(efferent) AS efferent_degree
        FROM $all_nodes
        WHERE node_id != NONE
        GROUP BY node_id
    );

    -- Filter by threshold BEFORE expensive node_info calls
    LET $hub_candidates = (
        SELECT node_id, afferent_degree, efferent_degree, afferent_degree + efferent_degree AS total_degree
        FROM $node_degrees
        WHERE (afferent_degree + efferent_degree) >= $threshold
        ORDER BY total_degree DESC
        LIMIT 50
    );

    ------------------------------------------------------------------------
    -- 4) Only now enrich with node_info (for filtered set only)
    ------------------------------------------------------------------------
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
-- Get edge context (outgoing/incoming edges) for a node
-- OPTIMIZED: Uses direct field access instead of nested subqueries
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
DEFINE FUNCTION fn::node_reference($node_id: any) {
-- Use helper to handle record IDs, "nodes:⟨uuid⟩", "nodes:id", etc.
LET $record = fn::parse_record_id('nodes', $node_id);
LET $info = fn::node_info($record);
IF $info = NONE { RETURN NONE; };
RETURN { id: $info.id, kind: $info.kind, location: $info.location, name: $info.name };
} PERMISSIONS FULL;
-- Hybrid semantic + BM25 search (nodes via chunks) using HNSW KNN index
-- OPTIMIZED: Uses <|K,EF|> operator for O(log n) instead of O(n) vector search
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

    ------------------------------------------------------------------------
    -- 1) HNSW KNN vector search - uses index for O(log n) performance
    -- NOTE: KNN operator requires literal integers, so we branch on $dimension
    ------------------------------------------------------------------------
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

    ------------------------------------------------------------------------
    -- 2) Enrich chunk hits with parent node data
    ------------------------------------------------------------------------
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

    ------------------------------------------------------------------------
    -- 3) BM25 / full-text candidates (using analyzer-backed indexes)
    ------------------------------------------------------------------------
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

    ------------------------------------------------------------------------
    -- 4) Combine, score, sort, and return
    ------------------------------------------------------------------------
    LET $combined = array::concat($chunk_nodes ?? [], $text_candidates ?? []);

    ------------------------------------------------------------------------
    -- 4a) Score and limit first (before expensive edge lookups)
    ------------------------------------------------------------------------
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

    ------------------------------------------------------------------------
    -- 4b) Batch fetch edges for all result nodes in TWO queries (not 30+ per row)
    ------------------------------------------------------------------------
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

    ------------------------------------------------------------------------
    -- 4c) Join edges back to results
    ------------------------------------------------------------------------
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
DEFINE FIELD embedding_384 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON chunks TYPE float PERMISSIONS FULL;
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
DEFINE FIELD embedding_3584 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3584 PERMISSIONS FULL;
DEFINE FIELD embedding_3584[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON chunks TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON chunks TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_model ON chunks TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD parent_node ON chunks TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD project_id ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD text ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON chunks TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_chunks_embedding_384 ON chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_768 ON chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1024 ON chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1536 ON chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2048 ON chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2560 ON chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3072 ON chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3584 ON chunks FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;    
DEFINE INDEX idx_chunks_embedding_4096 ON chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_order ON chunks FIELDS parent_node, chunk_index CONCURRENTLY;
DEFINE INDEX idx_chunks_parent ON chunks FIELDS parent_node CONCURRENTLY;
DEFINE INDEX idx_chunks_project ON chunks FIELDS project_id CONCURRENTLY;


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
DEFINE FIELD project_id ON edges TYPE option<string> PERMISSIONS FULL;

DEFINE INDEX idx_edges_from ON edges FIELDS from CONCURRENTLY;
DEFINE INDEX idx_edges_from_to ON edges FIELDS from, to CONCURRENTLY;
DEFINE INDEX idx_edges_to ON edges FIELDS to CONCURRENTLY;
DEFINE INDEX idx_edges_type ON edges FIELDS edge_type CONCURRENTLY;
DEFINE INDEX idx_edges_type_from ON edges FIELDS edge_type, from CONCURRENTLY;
DEFINE INDEX idx_edges_project ON edges FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_edges_project_type ON edges FIELDS project_id, edge_type CONCURRENTLY;


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
DEFINE FIELD embedding_384 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON nodes TYPE float PERMISSIONS FULL;
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
DEFINE FIELD embedding_3584 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3584 PERMISSIONS FULL;
DEFINE FIELD embedding_3584[*] ON nodes TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON nodes TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON nodes TYPE float PERMISSIONS FULL;
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
DEFINE INDEX idx_nodes_embedding_384 ON nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_768 ON nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1024 ON nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1536 ON nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2048 ON nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2560 ON nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3072 ON nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3584 ON nodes FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_4096 ON nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_file_path ON nodes FIELDS file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_project_file ON nodes FIELDS project_id, file_path CONCURRENTLY;
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
DEFINE FIELD embedding_384 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 384 PERMISSIONS FULL;
DEFINE FIELD embedding_384[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 768 PERMISSIONS FULL;
DEFINE FIELD embedding_768[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
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
DEFINE FIELD embedding_3584 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 3584 PERMISSIONS FULL;
DEFINE FIELD embedding_3584[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON symbol_embeddings TYPE option<array<float>> ASSERT $value = NONE OR array::len($value) = 4096 PERMISSIONS FULL;
DEFINE FIELD embedding_4096[*] ON symbol_embeddings TYPE float PERMISSIONS FULL;
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
DEFINE INDEX idx_symbol_embeddings_project ON symbol_embeddings FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_symbol ON symbol_embeddings FIELDS normalized_symbol CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_384 ON symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_768 ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1024 ON symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1536 ON symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2048 ON symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2560 ON symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3072 ON symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3584 ON symbol_embeddings FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_4096 ON symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
--------------------------------
