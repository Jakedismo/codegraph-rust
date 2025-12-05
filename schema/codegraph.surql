-- ------------------------------
-- OPTION
-- ------------------------------

OPTION IMPORT;

-- ------------------------------
-- FUNCTIONS
-- ------------------------------

DEFINE FUNCTION fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
LET $edge_list = ['calls', 'imports', 'uses', 'extends', 'implements', 'references'];
-- Resolve chunk IDs to their parent nodes (chunks have parent_node field)
LET $resolved_id = IF string::starts_with($node_id, 'chunks:') THEN (
  SELECT VALUE parent_node FROM type::thing('chunks', string::slice($node_id, 7)) LIMIT 1
)[0] ELSE $node_id END;
-- Handle NONE case when chunk doesn't exist
IF $resolved_id = NONE { RETURN NONE; };
LET $record = IF type::is::record($resolved_id) THEN $resolved_id ELSE (IF string::starts_with(<string>$resolved_id, 'nodes:') THEN type::thing('nodes', string::slice(<string>$resolved_id, 6)) ELSE type::thing('nodes', <string>$resolved_id) END) END;
LET $node_check = (SELECT project_id FROM $record);
IF array::len($node_check) = 0 { RETURN NONE; };
LET $rec_proj = $node_check[0].project_id;
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN NONE; };
LET $dependents = array::distinct((SELECT VALUE id FROM $record<-edges[WHERE edge_type INSIDE $edge_list AND (from.project_id ?? $project_id) = $project_id AND (to.project_id ?? $project_id) = $project_id]<-from));
LET $dependencies = array::distinct((SELECT VALUE id FROM $record->edges[WHERE edge_type INSIDE $edge_list AND (to.project_id ?? $project_id) = $project_id]->to));
LET $dependents_info = (SELECT VALUE fn::node_reference(`value`) FROM $dependents WHERE fn::node_reference(`value`) != NONE);
LET $dependencies_info = (SELECT VALUE fn::node_reference(`value`) FROM $dependencies WHERE fn::node_reference(`value`) != NONE);
LET $afferent = array::len($dependents_info);
LET $efferent = array::len($dependencies_info);
LET $total = $afferent + $efferent;
LET $instability = IF $total > 0 THEN math::round($efferent / $total, 6) ELSE 0 END;
RETURN { dependencies: $dependencies_info, dependents: $dependents_info, metrics: { afferent_coupling: $afferent, coupling_category: IF $instability < 0.3f THEN 'stable' ELSE IF $instability > 0.7f THEN 'unstable' ELSE 'balanced' END, efferent_coupling: $efferent, instability: $instability, is_stable: $instability < 0.3f, is_unstable: $instability > 0.7f, stability: 1f - $instability, total_coupling: $total }, node: fn::node_info($record) };
} PERMISSIONS FULL;
DEFINE FUNCTION fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
LET $edge_name = string::lowercase($edge_type ?? 'Calls');
-- Use direct field access for project_id filtering
LET $pairs = (SELECT from AS node1_id, to AS node2_id FROM edges WHERE edge_type = $edge_name AND from != to AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE));
LET $cycles = (SELECT node1_id, node2_id FROM $pairs WHERE node1_id < node2_id AND (SELECT VALUE count() FROM edges WHERE edge_type = $edge_name AND from = node2_id AND to = node1_id AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)) > 0 GROUP BY node1_id, node2_id);
LET $raw = (SELECT node1_id, node2_id, fn::node_info(node1_id) AS node1, fn::node_info(node2_id) AS node2 FROM $cycles);
RETURN (SELECT node1_id, node2_id, $edge_name AS dependency_type, node1, node2 FROM $raw WHERE node1 != NONE AND node2 != NONE);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::edge_types() { RETURN ['calls', 'imports', 'uses', 'extends', 'implements', 'references']; } PERMISSIONS FULL;
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
LET $raw = (SELECT candidate_id, fn::node_info(candidate_id) AS node, (array::first((SELECT VALUE total FROM $incoming_totals WHERE node_id = candidate_id LIMIT 1)) ?? 0) AS afferent_degree, (array::first((SELECT VALUE total FROM $outgoing_totals WHERE node_id = candidate_id LIMIT 1)) ?? 0) AS efferent_degree, (SELECT edge_type, count FROM $incoming_by_type WHERE node_id = candidate_id) AS incoming_by_type, (SELECT edge_type, count FROM $outgoing_by_type WHERE node_id = candidate_id) AS outgoing_by_type FROM (SELECT node_id AS candidate_id FROM $candidates));
RETURN (SELECT candidate_id AS node_id, node, afferent_degree, efferent_degree, afferent_degree + efferent_degree AS total_degree, incoming_by_type, outgoing_by_type FROM $raw WHERE node != NONE AND (afferent_degree + efferent_degree) >= $threshold ORDER BY total_degree DESC
);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::get_reverse_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
LET $safe_depth = IF $depth > 0 AND $depth <= 5 THEN $depth ELSE 3 END;
LET $edge_name = string::lowercase($edge_type ?? 'calls');
-- Resolve chunk IDs to their parent nodes
LET $resolved_id = IF string::starts_with(<string>$node_id, 'chunks:') THEN (
  SELECT VALUE parent_node FROM type::thing('chunks', string::slice(<string>$node_id, 7)) LIMIT 1
)[0] ELSE $node_id END;
IF $resolved_id = NONE { RETURN []; };
LET $record = IF type::is::record($resolved_id) THEN $resolved_id ELSE (IF string::starts_with(<string>$resolved_id, 'nodes:') THEN type::thing('nodes', string::slice(<string>$resolved_id, 6)) ELSE type::thing('nodes', <string>$resolved_id) END) END;
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
-- Resolve chunk IDs to their parent nodes
LET $resolved_id = IF string::starts_with(<string>$node_id, 'chunks:') THEN (
  SELECT VALUE parent_node FROM type::thing('chunks', string::slice(<string>$node_id, 7)) LIMIT 1
)[0] ELSE $node_id END;
IF $resolved_id = NONE { RETURN []; };
LET $record = IF type::is::record($resolved_id) THEN $resolved_id ELSE (IF string::starts_with(<string>$resolved_id, 'nodes:') THEN type::thing('nodes', string::slice(<string>$resolved_id, 6)) ELSE type::thing('nodes', <string>$resolved_id) END) END;
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
DEFINE FUNCTION fn::node_info($node_id: record) {
LET $res = (SELECT id, name, node_type AS kind, language, content, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM ONLY $node_id);
RETURN $res;
} PERMISSIONS FULL;
DEFINE FUNCTION fn::node_reference($node_id: any) {
-- Handle both record IDs (nodes:abc) and string IDs ("abc" or "nodes:abc")
LET $record = IF type::is::record($node_id) THEN $node_id
    ELSE IF string::starts_with(<string>$node_id, 'nodes:') THEN type::thing('nodes', string::slice(<string>$node_id, 6))
    ELSE type::thing('nodes', <string>$node_id) END;
LET $info = fn::node_info($record);
IF $info = NONE { RETURN NONE; };
RETURN { id: $info.id, kind: $info.kind, location: $info.location, name: $info.name };
} PERMISSIONS FULL;
DEFINE FUNCTION fn::semantic_search_chunks_with_context($project_id: string, $query_embedding: array<float>, $query_text: string, $dimension: int, $limit: int, $threshold: float, $include_graph_context: bool) { LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END; LET $safe_threshold = IF $threshold >= 0f AND $threshold <= 1f THEN $threshold ELSE 0.7f END; LET $symbol_limit = $safe_limit * 2; LET $vector_candidates = IF $dimension = 384 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 768 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 1024 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 1536 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 2048 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 2560 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE IF $dimension = 3072 THEN (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) ELSE (SELECT id, parent_node, chunk_index, text, vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score, 'chunk' AS match_source FROM chunks WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit FETCH parent_node) END; LET $symbol_matches = IF $dimension = 384 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_384, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 768 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_768, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1024 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1024, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1536 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1536, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2048 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2048, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2560 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2560, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 3072 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_3072, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 4096 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_4096, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE [] END; LET $symbol_results = (SELECT NONE AS id, -1 AS chunk_index, source_edge_id.from.content AS text, symbol_score * 0.8f AS vector_score, 'symbol_reference' AS match_source, symbol AS matched_symbol, source_edge_id.from AS parent_node FROM $symbol_matches WHERE source_edge_id.from != NONE AND source_edge_id.from.project_id = $project_id); LET $chunk_results = (SELECT id, chunk_index, text, vector_score, match_source, NONE AS matched_symbol, parent_node FROM array::flatten([$vector_candidates])); LET $all_results = array::concat($chunk_results, $symbol_results); LET $results = (SELECT IF id != NONE THEN <string> id ELSE NONE END AS id, chunk_index, text, vector_score, match_source, matched_symbol, IF parent_node != NONE THEN <string> parent_node.id ELSE NONE END AS node_id, parent_node.name AS name, parent_node.node_type AS kind, parent_node.language AS language, parent_node.file_path AS file_path, parent_node.start_line AS start_line, parent_node.end_line AS end_line, parent_node.metadata AS metadata FROM $all_results ORDER BY vector_score DESC LIMIT $safe_limit); RETURN $results; } PERMISSIONS FULL;
DEFINE FUNCTION fn::semantic_search_nodes_via_chunks($project_id: string, $query_embedding: array<float>, $query_text: string, $dimension: int, $limit: int, $threshold: float) { LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END; LET $safe_threshold = IF $threshold >= 0f AND $threshold <= 1f THEN $threshold ELSE 0.7f END; LET $chunk_limit = $safe_limit * 3; LET $symbol_limit = $safe_limit * 2; LET $chunk_results = IF $dimension = 384 THEN (SELECT parent_node, vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 768 THEN (SELECT parent_node, vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 1024 THEN (SELECT parent_node, vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 1536 THEN (SELECT parent_node, vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 2048 THEN (SELECT parent_node, vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 2560 THEN (SELECT parent_node, vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE IF $dimension = 3072 THEN (SELECT parent_node, vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) ELSE (SELECT parent_node, vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score FROM chunks WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $chunk_limit FETCH parent_node) END; LET $symbol_matches = IF $dimension = 384 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_384, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 768 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_768, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1024 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1024, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1536 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1536, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2048 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2048, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2560 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2560, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 3072 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_3072, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 4096 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_4096, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE [] END; LET $symbol_nodes = (SELECT source_edge_id.from AS parent_node, symbol_score * 0.8f AS vector_score, 'symbol_reference' AS match_source, symbol AS matched_symbol FROM $symbol_matches WHERE source_edge_id.from != NONE AND source_edge_id.from.project_id = $project_id); LET $chunk_nodes = (SELECT parent_node, vector_score, 'chunk' AS match_source, NONE AS matched_symbol FROM array::flatten([$chunk_results])); LET $all_nodes = array::concat($chunk_nodes, $symbol_nodes); LET $deduplicated = (SELECT math::max(vector_score) AS vector_score, parent_node.id AS node_id, parent_node.name AS name, parent_node.node_type AS kind, parent_node.language AS language, parent_node.file_path AS file_path, parent_node.start_line AS start_line, parent_node.end_line AS end_line, parent_node.content AS content, parent_node.metadata AS metadata, array::distinct(array::flatten([match_source])) AS match_sources, array::distinct(array::filter(array::flatten([matched_symbol]), |$v| $v != NONE)) AS matched_symbols FROM $all_nodes WHERE parent_node != NONE GROUP BY parent_node.id ORDER BY vector_score DESC LIMIT $safe_limit); LET $results = (SELECT IF node_id != NONE THEN <string> node_id ELSE NONE END AS node_id, name, kind, language, file_path, start_line, end_line, content, metadata, vector_score, match_sources, matched_symbols, (SELECT <string> to.id AS node_id, to.name AS name, to.node_type AS kind, to.file_path AS file_path, edge_type AS relationship FROM edges WHERE from = $parent.node_id LIMIT 20 FETCH to) AS outgoing_edges, (SELECT <string> from.id AS node_id, from.name AS name, from.node_type AS kind, from.file_path AS file_path, edge_type AS relationship FROM edges WHERE to = $parent.node_id LIMIT 20 FETCH from) AS incoming_edges FROM $deduplicated); RETURN $results; } PERMISSIONS FULL;
DEFINE FUNCTION fn::semantic_search_with_context($project_id: string, $query_embedding: array<float>, $query_text: string, $dimension: int, $limit: int, $threshold: float, $include_graph_context: bool) {
LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
LET $safe_threshold = IF $threshold >= 0f AND $threshold <= 1f THEN $threshold ELSE 0.7f END;
LET $symbol_limit = $safe_limit * 2;
LET $node_candidates = IF $dimension = 384 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_384, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 768 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_768, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 1024 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_1024, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 1536 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_1536, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 2048 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_2048, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 2560 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_2560, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 3072 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_3072, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE IF $dimension = 4096 THEN (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, vector::similarity::cosine(embedding_4096, $query_embedding) AS vector_score, 'node' AS match_source FROM nodes WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY vector_score DESC LIMIT $safe_limit) ELSE [] END;
LET $symbol_matches = IF $dimension = 384 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_384, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_384 != NONE AND vector::similarity::cosine(embedding_384, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 768 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_768, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_768 != NONE AND vector::similarity::cosine(embedding_768, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1024 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1024, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1024 != NONE AND vector::similarity::cosine(embedding_1024, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 1536 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_1536, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_1536 != NONE AND vector::similarity::cosine(embedding_1536, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2048 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2048, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2048 != NONE AND vector::similarity::cosine(embedding_2048, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 2560 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_2560, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_2560 != NONE AND vector::similarity::cosine(embedding_2560, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 3072 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_3072, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_3072 != NONE AND vector::similarity::cosine(embedding_3072, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE IF $dimension = 4096 THEN (SELECT source_edge_id, symbol, vector::similarity::cosine(embedding_4096, $query_embedding) AS symbol_score FROM symbol_embeddings WHERE project_id = $project_id AND embedding_4096 != NONE AND vector::similarity::cosine(embedding_4096, $query_embedding) >= $safe_threshold ORDER BY symbol_score DESC LIMIT $symbol_limit FETCH source_edge_id) ELSE [] END;
LET $symbol_node_candidates = (SELECT source_edge_id.from.id AS id, source_edge_id.from.name AS name, source_edge_id.from.node_type AS kind, source_edge_id.from.language AS language, source_edge_id.from.content AS content, source_edge_id.from.file_path AS file_path, source_edge_id.from.start_line AS start_line, source_edge_id.from.end_line AS end_line, source_edge_id.from.metadata AS metadata, symbol_score * 0.8f AS vector_score, 'symbol_reference' AS match_source, symbol AS matched_symbol FROM $symbol_matches WHERE source_edge_id.from != NONE AND source_edge_id.from.project_id = $project_id);
LET $text_candidates = (SELECT id, name, node_type AS kind, language, content, file_path, start_line, end_line, metadata, 0f AS vector_score, search::score(1) AS text_score, 'text' AS match_source FROM nodes WHERE project_id = $project_id AND (name @1@ $query_text OR content @2@ $query_text) ORDER BY text_score DESC LIMIT $safe_limit);
LET $all_candidates = array::concat(array::concat((SELECT *, (vector_score * 0.7f) AS combined_score, 0f AS text_score FROM $node_candidates), (SELECT *, (vector_score * 0.6f) AS combined_score, 0f AS text_score FROM $symbol_node_candidates)), (SELECT *, (text_score * 0.3f) AS combined_score FROM $text_candidates));
LET $deduped = (SELECT id, math::max(combined_score) AS combined_score, math::max(vector_score) AS vector_score, math::max(text_score) AS text_score, array::first(name) AS name, array::first(kind) AS kind, array::first(language) AS language, array::first(content) AS content, array::first(file_path) AS file_path, array::first(start_line) AS start_line, array::first(end_line) AS end_line, array::first(metadata) AS metadata, array::distinct(array::flatten([match_source])) AS match_sources, array::distinct(array::filter(array::flatten([matched_symbol]), |$v| $v != NONE)) AS matched_symbols FROM $all_candidates GROUP BY id ORDER BY combined_score DESC LIMIT $safe_limit);
LET $final_results = IF $include_graph_context THEN (SELECT $parent.id AS id, $parent.name AS name, $parent.kind AS kind, $parent.language AS language, $parent.content AS content, { end_line: $parent.end_line, file_path: $parent.file_path, start_line: $parent.start_line } AS location, $parent.metadata AS metadata, $parent.vector_score AS vector_score, $parent.text_score AS text_score, $parent.combined_score AS combined_score, $parent.match_sources AS match_sources, $parent.matched_symbols AS matched_symbols, (SELECT VALUE fn::node_reference(<string> out) FROM ($parent.id)->calls, ($parent.id)->imports, ($parent.id)->uses WHERE out.project_id = $project_id LIMIT 5) AS direct_dependencies, (SELECT VALUE fn::node_reference(<string> in) FROM ($parent.id)<-calls, ($parent.id)<-imports, ($parent.id)<-uses WHERE in.project_id = $project_id LIMIT 5) AS direct_dependents, (SELECT id, name, node_type, start_line FROM nodes WHERE project_id = $project_id AND file_path = $parent.file_path AND id != $parent.id ORDER BY start_line LIMIT 5) AS file_siblings FROM (SELECT * FROM $deduped)) ELSE (SELECT id, name, kind, language, content, { end_line: end_line, file_path: file_path, start_line: start_line } AS location, metadata, vector_score, text_score, combined_score, match_sources, matched_symbols FROM $deduped) END;
RETURN $final_results;
} PERMISSIONS FULL;
DEFINE FUNCTION fn::trace_call_chain($project_id: string, $from_node: any, $max_depth: int) {
LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;
-- Resolve chunk IDs to their parent nodes
LET $resolved_id = IF string::starts_with(<string>$from_node, 'chunks:') THEN (
  SELECT VALUE parent_node FROM type::thing('chunks', string::slice(<string>$from_node, 7)) LIMIT 1
)[0] ELSE $from_node END;
IF $resolved_id = NONE { RETURN []; };
LET $record = IF type::is::record($resolved_id) THEN $resolved_id ELSE (IF string::starts_with(<string>$resolved_id, 'nodes:') THEN type::thing('nodes', string::slice(<string>$resolved_id, 6)) ELSE type::thing('nodes', <string>$resolved_id) END) END;
LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
IF $rec_proj != NONE AND $rec_proj != $project_id { RETURN []; };
-- Use direct field access for project_id filtering
LET $raw = (SELECT fn::node_info(id) AS node, array::distinct((SELECT fn::node_reference(from) AS caller FROM edges WHERE to = id AND edge_type = 'Calls' AND (from.project_id = $project_id OR from.project_id = NONE) AND (to.project_id = $project_id OR to.project_id = NONE)).caller) AS called_by FROM (SELECT ->edges[WHERE edge_type = 'Calls' AND (to.project_id ?? $project_id) = $project_id] FROM ONLY $record)->to);
RETURN (SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, 1 AS call_depth, called_by, $safe_depth AS requested_depth FROM $raw WHERE node != NONE);
} PERMISSIONS FULL;
DEFINE FUNCTION fn::get_complexity_hotspots($project_id: string, $min_complexity: float, $limit: int) {
LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 20 END;
LET $threshold = IF $min_complexity > 0f THEN $min_complexity ELSE 5f END;
LET $edge_list = ['calls', 'imports', 'uses', 'extends', 'implements', 'references'];
LET $complex_nodes = (SELECT id, name, node_type AS kind, language, file_path, start_line, end_line, complexity FROM nodes WHERE project_id = $project_id AND complexity != NONE AND complexity >= $threshold AND node_type IN ['function', 'method', 'Function', 'Method'] ORDER BY complexity DESC LIMIT $safe_limit * 2);
LET $with_coupling = (SELECT id, name, kind, language, file_path, start_line, end_line, complexity, (SELECT VALUE count() FROM edges WHERE to = $parent.id AND edge_type INSIDE $edge_list) AS afferent_coupling, (SELECT VALUE count() FROM edges WHERE from = $parent.id AND edge_type INSIDE $edge_list) AS efferent_coupling FROM $complex_nodes);
LET $results = (SELECT id, name, kind, language, file_path, start_line, end_line, complexity, afferent_coupling[0] ?? 0 AS afferent_coupling, efferent_coupling[0] ?? 0 AS efferent_coupling, IF (afferent_coupling[0] ?? 0) + (efferent_coupling[0] ?? 0) > 0 THEN math::round((efferent_coupling[0] ?? 0) / ((afferent_coupling[0] ?? 0) + (efferent_coupling[0] ?? 0)), 4) ELSE 0f END AS instability, complexity * ((afferent_coupling[0] ?? 0) + 1) AS risk_score FROM $with_coupling ORDER BY risk_score DESC LIMIT $safe_limit);
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

DEFINE INDEX idx_chunks_embedding_1024 ON chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_1536 ON chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_2048 ON chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_2560 ON chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_3072 ON chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_384 ON chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_4096 ON chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_embedding_768 ON chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_chunks_order ON chunks FIELDS parent_node, chunk_index;
DEFINE INDEX idx_chunks_parent ON chunks FIELDS parent_node;


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

DEFINE INDEX idx_edges_from ON edges FIELDS from;
DEFINE INDEX idx_edges_from_to ON edges FIELDS from, to;
DEFINE INDEX idx_edges_to ON edges FIELDS to;
DEFINE INDEX idx_edges_type ON edges FIELDS edge_type;
DEFINE INDEX idx_edges_type_from ON edges FIELDS edge_type, from;


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

DEFINE INDEX idx_file_metadata_composite ON file_metadata FIELDS project_id, file_path UNIQUE;
DEFINE INDEX idx_file_metadata_hash ON file_metadata FIELDS content_hash;
DEFINE INDEX idx_file_metadata_modified ON file_metadata FIELDS modified_at;
DEFINE INDEX idx_file_metadata_project ON file_metadata FIELDS project_id;


-- ------------------------------
-- TABLE: metadata
-- ------------------------------

DEFINE TABLE metadata TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD `value` ON metadata FLEXIBLE TYPE option<string | number | bool | object | array> PERMISSIONS FULL;
DEFINE FIELD `value`[*] ON metadata FLEXIBLE TYPE any PERMISSIONS FULL;
DEFINE FIELD key ON metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_metadata_key ON metadata FIELDS key UNIQUE;


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

DEFINE INDEX idx_nodes_content_search ON nodes FIELDS content SEARCH ANALYZER code_analyzer BM25(1.2,0.75) DOC_IDS_ORDER 100 DOC_LENGTHS_ORDER 100 POSTINGS_ORDER 100 TERMS_ORDER 100 DOC_IDS_CACHE 100 DOC_LENGTHS_CACHE 100 POSTINGS_CACHE 100 TERMS_CACHE 100;
DEFINE INDEX idx_nodes_embedding_1024 ON nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_1536 ON nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_2048 ON nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_2560 ON nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_3072 ON nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_384 ON nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_4096 ON nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_embedding_768 ON nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_nodes_file_path ON nodes FIELDS file_path;
DEFINE INDEX idx_nodes_file_type ON nodes FIELDS file_path, node_type;
DEFINE INDEX idx_nodes_language ON nodes FIELDS language;
DEFINE INDEX idx_nodes_name ON nodes FIELDS name;
DEFINE INDEX idx_nodes_name_search ON nodes FIELDS name SEARCH ANALYZER code_analyzer BM25(1.2,0.75) DOC_IDS_ORDER 100 DOC_LENGTHS_ORDER 100 POSTINGS_ORDER 100 TERMS_ORDER 100 DOC_IDS_CACHE 100 DOC_LENGTHS_CACHE 100 POSTINGS_CACHE 100 TERMS_CACHE 100;
DEFINE INDEX idx_nodes_project ON nodes FIELDS project_id;
DEFINE INDEX idx_nodes_project_type ON nodes FIELDS project_id, node_type;
DEFINE INDEX idx_nodes_type ON nodes FIELDS node_type;


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

DEFINE INDEX idx_project_domain ON project_metadata FIELDS domain;
DEFINE INDEX idx_project_id ON project_metadata FIELDS project_id UNIQUE;
DEFINE INDEX idx_project_name ON project_metadata FIELDS name;
DEFINE INDEX idx_project_org ON project_metadata FIELDS organization_id;


-- ------------------------------
-- TABLE: schema_versions
-- ------------------------------

DEFINE TABLE schema_versions TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD applied_at ON schema_versions TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD checksum ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD description ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD name ON schema_versions TYPE string PERMISSIONS FULL;
DEFINE FIELD version ON schema_versions TYPE int PERMISSIONS FULL;

DEFINE INDEX idx_schema_version ON schema_versions FIELDS version UNIQUE;


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

DEFINE INDEX idx_symbol_embeddings_edge ON symbol_embeddings FIELDS source_edge_id;
DEFINE INDEX idx_symbol_embeddings_node ON symbol_embeddings FIELDS node_id;
DEFINE INDEX idx_symbol_embeddings_project_symbol ON symbol_embeddings FIELDS project_id, normalized_symbol;
DEFINE INDEX idx_symbol_embeddings_symbol ON symbol_embeddings FIELDS normalized_symbol;
DEFINE INDEX idx_symbol_embeddings_vector_1024 ON symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_1536 ON symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_2048 ON symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_2560 ON symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_3072 ON symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_384 ON symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_4096 ON symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
DEFINE INDEX idx_symbol_embeddings_vector_768 ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 200 M 16 M0 32 LM 0.36067376022224085f;
-- ------------------------------