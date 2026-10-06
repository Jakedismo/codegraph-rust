-- ABOUTME: CodeGraph schema v2 for SurrealDB 3.x: same tables and fields the storage layer writes,
-- ABOUTME: with index-backed graph functions, RRF hybrid search, cascading references and code-aware analyzers.
--
-- Drop-in alternative to codegraph.surql. Select it with CODEGRAPH_SCHEMA=v2 (embedded store)
-- or `apply-schema.sh -s codegraph_v2.surql` (server). Function names, parameters and result
-- fields match v1 so graph_functions.rs and the agent tools work unchanged.
--
-- What changed from v1 and why:
--   * Every graph function filters edges by their own project_id column instead of dereferencing
--     from.project_id / to.project_id, which fetched a node record per edge. Compound indexes
--     (project_id, edge_type, from) and (project_id, edge_type, to) make those lookups index scans.
--   * Dependency traversal is a FOR loop over depth with frontier and visited sets, so depth is not
--     capped at five hardcoded levels and each level is one indexed query.
--   * trace_call_chain is a breadth-first walk that returns one row per reached node with its depth
--     and a path, instead of a single level.
--   * Hybrid search fuses the vector list and the BM25 lists with search::rrf (reciprocal rank
--     fusion) instead of a fixed 0.9 / 0.1 weighting.
--   * Analyzers split identifiers on case and punctuation (CAMEL, PUNCT) so `parseConfigFile`
--     matches `config`; the name index adds edge n-grams for prefix matches. The content index
--     keeps HIGHLIGHTS so fn::search_snippets can return matched fragments.
--   * chunks.parent_node is REFERENCE ON DELETE CASCADE, so deleting a node removes its chunks.
--     Edges deliberately do not cascade: an incremental re-index deletes and re-creates a changed
--     file's nodes under the same ids, and cascading would permanently drop the edges that other,
--     unchanged files point at them.
--   * Functions are DEFINE FUNCTION OVERWRITE so re-applying the file to a server updates them
--     without touching tables or rebuilding indexes.

OPTION IMPORT;

-- ------------------------------
-- ANALYZERS
-- ------------------------------

-- Prose and code bodies: split on whitespace, character class, case changes and punctuation,
-- then lowercase, fold accents and stem English words.
DEFINE ANALYZER OVERWRITE code_text
    TOKENIZERS BLANK, CLASS, CAMEL, PUNCT
    FILTERS LOWERCASE, ASCII, SNOWBALL(ENGLISH);

-- Identifiers: same splitting, plus edge n-grams so a prefix such as `pars` matches `parse_file`.
DEFINE ANALYZER OVERWRITE code_identifier
    TOKENIZERS CLASS, CAMEL, PUNCT
    FILTERS LOWERCASE, ASCII, EDGENGRAM(2, 12);

-- ------------------------------
-- PARAMS
-- ------------------------------

-- Edge types that count as dependencies for coupling, hubs and hotspots. Structural edges
-- (contains, belongs_to) are excluded, matching v1.
DEFINE PARAM OVERWRITE $dependency_edge_types VALUE [
    'calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'depends_on',
    'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates'
];

-- ------------------------------
-- TABLE: nodes
-- ------------------------------

DEFINE TABLE nodes TYPE NORMAL SCHEMAFULL COMMENT 'Code entities from AST parsing with semantic embeddings' PERMISSIONS FULL;

DEFINE FIELD chunk_count ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD complexity ON nodes TYPE option<float> PERMISSIONS FULL;
DEFINE FIELD content ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON nodes TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON nodes TYPE option<array<float, 384>> PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON nodes TYPE option<array<float, 768>> PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON nodes TYPE option<array<float, 1024>> PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON nodes TYPE option<array<float, 1536>> PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON nodes TYPE option<array<float, 2048>> PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON nodes TYPE option<array<float, 2560>> PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON nodes TYPE option<array<float, 3072>> PERMISSIONS FULL;
DEFINE FIELD embedding_3584 ON nodes TYPE option<array<float, 3584>> PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON nodes TYPE option<array<float, 4096>> PERMISSIONS FULL;
DEFINE FIELD embedding_model ON nodes TYPE option<string> DEFAULT 'jina-embeddings-v4' PERMISSIONS FULL;
DEFINE FIELD end_line ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD file_path ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD language ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD metadata ON nodes TYPE option<object | string> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD name ON nodes TYPE string PERMISSIONS FULL;
DEFINE FIELD node_type ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD organization_id ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD repository_url ON nodes TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD start_line ON nodes TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD updated_at ON nodes TYPE datetime VALUE time::now() PERMISSIONS FULL;

-- Lookups the storage layer and functions actually run, all project-scoped.
DEFINE INDEX idx_nodes_project ON nodes FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_nodes_project_file ON nodes FIELDS project_id, file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_project_name ON nodes FIELDS project_id, name CONCURRENTLY;
DEFINE INDEX idx_nodes_project_type_complexity ON nodes FIELDS project_id, node_type, complexity CONCURRENTLY;
DEFINE INDEX idx_nodes_file_path ON nodes FIELDS file_path CONCURRENTLY;
DEFINE INDEX idx_nodes_name ON nodes FIELDS name CONCURRENTLY;

-- Full text: content with highlights for snippets, identifiers with prefix n-grams.
DEFINE INDEX idx_nodes_content_search ON nodes FIELDS content FULLTEXT ANALYZER code_text BM25(1.2, 0.75) HIGHLIGHTS CONCURRENTLY;
DEFINE INDEX idx_nodes_name_search ON nodes FIELDS name FULLTEXT ANALYZER code_identifier BM25(1.2, 0.75) CONCURRENTLY;

DEFINE INDEX idx_nodes_embedding_384 ON nodes FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_768 ON nodes FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1024 ON nodes FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_1536 ON nodes FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2048 ON nodes FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_2560 ON nodes FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3072 ON nodes FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_3584 ON nodes FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_nodes_embedding_4096 ON nodes FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- ------------------------------
-- TABLE: edges
-- ------------------------------

DEFINE TABLE edges TYPE NORMAL SCHEMAFULL COMMENT 'Code relationships (Calls, Imports, Uses, Extends, Implements, References)' PERMISSIONS FULL;

DEFINE FIELD created_at ON edges TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD edge_type ON edges TYPE string VALUE string::lowercase($value) PERMISSIONS FULL;
DEFINE FIELD from ON edges TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD to ON edges TYPE record<nodes> PERMISSIONS FULL;
DEFINE FIELD metadata ON edges TYPE option<object> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD weight ON edges TYPE float DEFAULT 1f ASSERT $value > 0f PERMISSIONS FULL;
DEFINE FIELD project_id ON edges TYPE option<string> PERMISSIONS FULL;

DEFINE INDEX idx_edges_project ON edges FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_edges_project_type ON edges FIELDS project_id, edge_type CONCURRENTLY;
DEFINE INDEX idx_edges_project_type_from ON edges FIELDS project_id, edge_type, from CONCURRENTLY;
DEFINE INDEX idx_edges_project_type_to ON edges FIELDS project_id, edge_type, to CONCURRENTLY;
DEFINE INDEX idx_edges_from_to ON edges FIELDS from, to CONCURRENTLY;
DEFINE INDEX idx_edges_to ON edges FIELDS to CONCURRENTLY;

-- ------------------------------
-- TABLE: chunks
-- ------------------------------

DEFINE TABLE chunks TYPE NORMAL SCHEMAFULL COMMENT 'Chunked embeddings for long nodes; removed with the parent node' PERMISSIONS FULL;

DEFINE FIELD chunk_index ON chunks TYPE int PERMISSIONS FULL;
DEFINE FIELD created_at ON chunks TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON chunks TYPE option<array<float, 384>> PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON chunks TYPE option<array<float, 768>> PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON chunks TYPE option<array<float, 1024>> PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON chunks TYPE option<array<float, 1536>> PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON chunks TYPE option<array<float, 2048>> PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON chunks TYPE option<array<float, 2560>> PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON chunks TYPE option<array<float, 3072>> PERMISSIONS FULL;
DEFINE FIELD embedding_3584 ON chunks TYPE option<array<float, 3584>> PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON chunks TYPE option<array<float, 4096>> PERMISSIONS FULL;
DEFINE FIELD embedding_model ON chunks TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD parent_node ON chunks TYPE record<nodes> REFERENCE ON DELETE CASCADE PERMISSIONS FULL;
DEFINE FIELD project_id ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD text ON chunks TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON chunks TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_chunks_project ON chunks FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_chunks_parent_order ON chunks FIELDS parent_node, chunk_index CONCURRENTLY;

DEFINE INDEX idx_chunks_embedding_384 ON chunks FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_768 ON chunks FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1024 ON chunks FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_1536 ON chunks FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2048 ON chunks FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_2560 ON chunks FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3072 ON chunks FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_3584 ON chunks FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_chunks_embedding_4096 ON chunks FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- ------------------------------
-- TABLE: symbol_embeddings
-- ------------------------------

DEFINE TABLE symbol_embeddings TYPE NORMAL SCHEMAFULL COMMENT 'Cached embeddings for normalized symbols used during edge resolution' PERMISSIONS FULL;

DEFINE FIELD access_count ON symbol_embeddings TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD embedding_384 ON symbol_embeddings TYPE option<array<float, 384>> PERMISSIONS FULL;
DEFINE FIELD embedding_768 ON symbol_embeddings TYPE option<array<float, 768>> PERMISSIONS FULL;
DEFINE FIELD embedding_1024 ON symbol_embeddings TYPE option<array<float, 1024>> PERMISSIONS FULL;
DEFINE FIELD embedding_1536 ON symbol_embeddings TYPE option<array<float, 1536>> PERMISSIONS FULL;
DEFINE FIELD embedding_2048 ON symbol_embeddings TYPE option<array<float, 2048>> PERMISSIONS FULL;
DEFINE FIELD embedding_2560 ON symbol_embeddings TYPE option<array<float, 2560>> PERMISSIONS FULL;
DEFINE FIELD embedding_3072 ON symbol_embeddings TYPE option<array<float, 3072>> PERMISSIONS FULL;
DEFINE FIELD embedding_3584 ON symbol_embeddings TYPE option<array<float, 3584>> PERMISSIONS FULL;
DEFINE FIELD embedding_4096 ON symbol_embeddings TYPE option<array<float, 4096>> PERMISSIONS FULL;
DEFINE FIELD embedding_model ON symbol_embeddings TYPE string DEFAULT 'jina-embeddings-v4' PERMISSIONS FULL;
DEFINE FIELD last_computed_at ON symbol_embeddings TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD metadata ON symbol_embeddings TYPE option<object> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD node_id ON symbol_embeddings TYPE option<record<nodes>> REFERENCE ON DELETE UNSET PERMISSIONS FULL;
DEFINE FIELD normalized_symbol ON symbol_embeddings TYPE string PERMISSIONS FULL;
DEFINE FIELD organization_id ON symbol_embeddings TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON symbol_embeddings TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD source_edge_id ON symbol_embeddings TYPE option<record<edges>> REFERENCE ON DELETE UNSET PERMISSIONS FULL;
DEFINE FIELD symbol ON symbol_embeddings TYPE string PERMISSIONS FULL;

DEFINE INDEX idx_symbol_embeddings_project ON symbol_embeddings FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_project_symbol ON symbol_embeddings FIELDS project_id, normalized_symbol CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_node ON symbol_embeddings FIELDS node_id CONCURRENTLY;

DEFINE INDEX idx_symbol_embeddings_vector_384 ON symbol_embeddings FIELDS embedding_384 HNSW DIMENSION 384 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_768 ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1024 ON symbol_embeddings FIELDS embedding_1024 HNSW DIMENSION 1024 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_1536 ON symbol_embeddings FIELDS embedding_1536 HNSW DIMENSION 1536 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2048 ON symbol_embeddings FIELDS embedding_2048 HNSW DIMENSION 2048 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_2560 ON symbol_embeddings FIELDS embedding_2560 HNSW DIMENSION 2560 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3072 ON symbol_embeddings FIELDS embedding_3072 HNSW DIMENSION 3072 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_3584 ON symbol_embeddings FIELDS embedding_3584 HNSW DIMENSION 3584 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;
DEFINE INDEX idx_symbol_embeddings_vector_4096 ON symbol_embeddings FIELDS embedding_4096 HNSW DIMENSION 4096 DIST COSINE TYPE F32 EFC 150 M 12 CONCURRENTLY;

-- ------------------------------
-- TABLE: file_metadata
-- ------------------------------

DEFINE TABLE file_metadata TYPE NORMAL SCHEMAFULL COMMENT 'Tracks file state for incremental indexing and change detection' PERMISSIONS FULL;

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
DEFINE FIELD project_id ON file_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON file_metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_file_metadata_composite ON file_metadata FIELDS project_id, file_path UNIQUE CONCURRENTLY;
DEFINE INDEX idx_file_metadata_project ON file_metadata FIELDS project_id CONCURRENTLY;
DEFINE INDEX idx_file_metadata_hash ON file_metadata FIELDS content_hash CONCURRENTLY;

-- ------------------------------
-- TABLE: project_metadata
-- ------------------------------

DEFINE TABLE project_metadata TYPE NORMAL SCHEMAFULL COMMENT 'Project registry with CodeGraph statistics' PERMISSIONS FULL;

DEFINE FIELD avg_coverage_score ON project_metadata TYPE option<float> PERMISSIONS FULL;
DEFINE FIELD codegraph_version ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON project_metadata TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD domain ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD edge_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD file_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD last_analyzed ON project_metadata TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD metadata ON project_metadata TYPE option<object> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD name ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD node_count ON project_metadata TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD organization_id ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD primary_language ON project_metadata TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD project_id ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD root_path ON project_metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON project_metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_project_id ON project_metadata FIELDS project_id UNIQUE CONCURRENTLY;
DEFINE INDEX idx_project_name ON project_metadata FIELDS name CONCURRENTLY;
DEFINE INDEX idx_project_org ON project_metadata FIELDS organization_id CONCURRENTLY;
DEFINE INDEX idx_project_domain ON project_metadata FIELDS domain CONCURRENTLY;

-- ------------------------------
-- TABLE: metadata, schema_versions
-- ------------------------------

DEFINE TABLE metadata TYPE NORMAL SCHEMAFULL PERMISSIONS FULL;
DEFINE FIELD `value` ON metadata TYPE option<string | number | bool | object | array> FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD key ON metadata TYPE string PERMISSIONS FULL;
DEFINE FIELD updated_at ON metadata TYPE datetime VALUE time::now() PERMISSIONS FULL;
DEFINE INDEX idx_metadata_key ON metadata FIELDS key UNIQUE CONCURRENTLY;

DEFINE TABLE schema_versions TYPE NORMAL SCHEMAFULL PERMISSIONS FULL;
DEFINE FIELD applied_at ON schema_versions TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;
DEFINE FIELD checksum ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD description ON schema_versions TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD name ON schema_versions TYPE string PERMISSIONS FULL;
DEFINE FIELD version ON schema_versions TYPE int PERMISSIONS FULL;
DEFINE INDEX idx_schema_version ON schema_versions FIELDS version UNIQUE CONCURRENTLY;

-- ------------------------------
-- HELPER FUNCTIONS
-- ------------------------------

DEFINE FUNCTION OVERWRITE fn::edge_types() {
    RETURN ['calls', 'defines', 'imports', 'uses', 'extends', 'implements', 'references', 'contains', 'belongs_to', 'depends_on', 'exports', 'reexports', 'enables', 'generates', 'flows_to', 'returns', 'captures', 'mutates', 'violates_boundary', 'documents', 'specifies'];
} PERMISSIONS FULL;

-- Accepts a record, "nodes:`uuid`", "nodes:⟨uuid⟩", "nodes:id", "`uuid`" or "id" and returns a record id.
DEFINE FUNCTION OVERWRITE fn::parse_record_id($table: string, $input: any) {
    IF type::is_record($input) { RETURN $input; };
    LET $str = <string>$input;
    LET $after_prefix = IF string::starts_with($str, $table + ':') THEN string::slice($str, string::len($table) + 1) ELSE $str END;
    -- Take the id out of SurrealDB's escaping: backticks (3.x, what <string>id produces) or
    -- ⟨⟩ (2.x). Only the quoted part is used, so stray characters a model appends after the
    -- closing quote (for example "nodes:`uuid`}},{") do not make the lookup miss.
    LET $clean_id = IF string::starts_with($after_prefix, '`') THEN string::split($after_prefix, '`')[1]
        ELSE IF string::starts_with($after_prefix, '⟨') THEN string::split(string::replace($after_prefix, '⟩', '⟨'), '⟨')[1]
        ELSE $after_prefix END;
    RETURN type::record($table, $clean_id);
} PERMISSIONS FULL;

-- Resolves any node or chunk reference to a nodes record, or NONE.
DEFINE FUNCTION OVERWRITE fn::resolve_node($node_id: any) {
    IF $node_id = NONE { RETURN NONE; };
    IF string::starts_with(<string>$node_id, 'chunks:') {
        RETURN (SELECT VALUE parent_node FROM ONLY fn::parse_record_id('chunks', $node_id));
    };
    RETURN fn::parse_record_id('nodes', $node_id);
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::node_info($node_id: any) {
    IF $node_id = NONE OR !type::is_record($node_id) { RETURN NONE; };
    RETURN (SELECT <string>id AS id, name, node_type AS kind, language, content, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM ONLY $node_id);
} PERMISSIONS FULL;

-- Node identity and location without content or metadata, for ranked lists such as hubs
-- where fifty rows of source text would crowd out everything else.
DEFINE FUNCTION OVERWRITE fn::node_summary($node_id: any) {
    IF $node_id = NONE OR !type::is_record($node_id) { RETURN NONE; };
    RETURN (SELECT <string>id AS id, name, node_type AS kind, language, { end_line: end_line, file_path: file_path, start_line: start_line } AS location FROM ONLY $node_id);
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::node_reference($node_id: any) {
    LET $info = fn::node_info(fn::parse_record_id('nodes', $node_id));
    IF $info = NONE { RETURN NONE; };
    RETURN { id: $info.id, kind: $info.kind, location: $info.location, name: $info.name };
} PERMISSIONS FULL;

-- Up to five outgoing and five incoming edges, resolved through the endpoint link.
DEFINE FUNCTION OVERWRITE fn::edge_context($node_ref: any) {
    IF $node_ref = NONE { RETURN { outgoing: [], incoming: [] }; };
    RETURN {
        outgoing: (SELECT <string>`to` AS node_id, `to`.name AS name, `to`.node_type AS kind, `to`.file_path AS file_path, edge_type AS relationship FROM edges WHERE `from` = $node_ref LIMIT 5),
        incoming: (SELECT <string>`from` AS node_id, `from`.name AS name, `from`.node_type AS kind, `from`.file_path AS file_path, edge_type AS relationship FROM edges WHERE `to` = $node_ref LIMIT 5)
    };
} PERMISSIONS FULL;

-- One indexed query per direction for a set of nodes: the edge lists attached to search results.
DEFINE FUNCTION OVERWRITE fn::edges_for_nodes($node_refs: array, $per_node: int) {
    IF array::len($node_refs) = 0 { RETURN { outgoing: [], incoming: [] }; };
    RETURN {
        outgoing: (SELECT `from` AS source_node, <string>`to` AS node_id, `to`.name AS name, `to`.node_type AS kind, `to`.file_path AS file_path, edge_type AS relationship FROM edges WHERE `from` INSIDE $node_refs LIMIT 200),
        incoming: (SELECT `to` AS source_node, <string>`from` AS node_id, `from`.name AS name, `from`.node_type AS kind, `from`.file_path AS file_path, edge_type AS relationship FROM edges WHERE `to` INSIDE $node_refs LIMIT 200)
    };
} PERMISSIONS FULL;

-- ------------------------------
-- GRAPH FUNCTIONS
-- ------------------------------

-- Breadth-first expansion over one edge type. $direction is 'out' (dependencies) or 'in'
-- (dependents). Returns [{ id, depth }] with the first depth each node was reached at.
-- LET inside a FOR body is block-scoped in SurrealDB 3.x, so the walk is an array::fold over
-- the depth range carrying { visited, frontier, reached }.
DEFINE FUNCTION OVERWRITE fn::expand($project_id: string, $start: record<nodes>, $edge_name: string, $direction: string, $max_depth: int) {
    LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 3 END;
    LET $walk = array::fold(array::range(1, $safe_depth), { visited: [$start], frontier: [$start], reached: [] }, |$acc, $depth| {
        IF array::len($acc.frontier) = 0 { RETURN $acc; };
        LET $next = IF $direction = 'in' THEN
            (SELECT VALUE `from` FROM edges WHERE project_id = $project_id AND edge_type = $edge_name AND `to` INSIDE $acc.frontier)
        ELSE
            (SELECT VALUE `to` FROM edges WHERE project_id = $project_id AND edge_type = $edge_name AND `from` INSIDE $acc.frontier)
        END;
        LET $new = array::complement(array::distinct($next), $acc.visited);
        RETURN {
            visited: array::concat($acc.visited, $new),
            frontier: $new,
            reached: array::concat($acc.reached, $new.map(|$n| { id: $n, depth: $depth }))
        };
    });
    RETURN $walk.reached;
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::get_transitive_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;
    LET $record = fn::resolve_node($node_id);
    IF $record = NONE { RETURN []; };
    LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
    IF $rec_proj = NONE OR $rec_proj != $project_id { RETURN []; };
    LET $reached = fn::expand($project_id, $record, string::lowercase($edge_type ?? 'calls'), 'out', $safe_depth);
    RETURN (
        SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, depth AS dependency_depth, $safe_depth AS requested_depth
        FROM (SELECT fn::node_info(id) AS node, depth FROM $reached)
        WHERE node != NONE
        ORDER BY dependency_depth
    );
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::get_reverse_dependencies($project_id: string, $node_id: any, $edge_type: string, $depth: int) {
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;
    LET $record = fn::resolve_node($node_id);
    IF $record = NONE { RETURN []; };
    LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
    IF $rec_proj = NONE OR $rec_proj != $project_id { RETURN []; };
    LET $reached = fn::expand($project_id, $record, string::lowercase($edge_type ?? 'calls'), 'in', $safe_depth);
    RETURN (
        SELECT node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata, depth AS dependent_depth, $safe_depth AS requested_depth
        FROM (SELECT fn::node_info(id) AS node, depth FROM $reached)
        WHERE node != NONE
        ORDER BY dependent_depth
    );
} PERMISSIONS FULL;

-- Breadth-first call walk. Each row is a reached callee with its depth, the path of names from
-- the start node, and its direct callers.
DEFINE FUNCTION OVERWRITE fn::trace_call_chain($project_id: string, $from_node: any, $max_depth: int) {
    LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;
    LET $record = fn::resolve_node($from_node);
    IF $record = NONE { RETURN []; };
    LET $rec_proj = (SELECT VALUE project_id FROM ONLY $record);
    IF $rec_proj = NONE OR $rec_proj != $project_id { RETURN []; };

    LET $start_name = (SELECT VALUE name FROM ONLY $record);
    LET $walk = array::fold(array::range(1, $safe_depth), { visited: [$record], frontier: [{ id: $record, path: [$start_name] }], reached: [] }, |$acc, $depth| {
        IF array::len($acc.frontier) = 0 { RETURN $acc; };
        LET $frontier_ids = $acc.frontier.map(|$f| $f.id);
        LET $hops = (
            SELECT `from` AS caller, `to` AS callee, `to`.name AS callee_name
            FROM edges
            WHERE project_id = $project_id AND edge_type = 'calls' AND `from` INSIDE $frontier_ids AND `to` NOTINSIDE $acc.visited
        );
        -- One entry per new callee; the first hop seen provides its path.
        LET $next = array::distinct($hops.map(|$h| $h.callee)).map(|$callee| {
            LET $hop = $hops.find(|$h| $h.callee = $callee);
            LET $parent = $acc.frontier.find(|$f| $f.id = $hop.caller);
            RETURN { id: $callee, path: array::append($parent.path, $hop.callee_name) };
        });
        RETURN {
            visited: array::concat($acc.visited, $next.map(|$n| $n.id)),
            frontier: $next,
            reached: array::concat($acc.reached, $next.map(|$n| { id: $n.id, path: $n.path, depth: $depth }))
        };
    });
    LET $reached = $walk.reached;
    IF array::len($reached) = 0 { RETURN []; };

    LET $reached_ids = $reached.map(|$r| $r.id);
    -- Direct callers, capped per node so hubs with thousands of callers stay cheap.
    LET $callers = (
        SELECT `to` AS callee, array::slice(array::distinct(array::group(`from`)), 0, 10) AS callers
        FROM edges
        WHERE project_id = $project_id AND edge_type = 'calls' AND `to` INSIDE $reached_ids
        GROUP BY `to`
    );

    RETURN (
        SELECT
            node.id AS id, node.name AS name, node.kind AS kind, node.location AS location, node.language AS language, node.content AS content, node.metadata AS metadata,
            depth AS call_depth,
            path AS call_path,
            ($callers.find(|$c| $c.callee = rid).callers ?? []).map(|$caller| fn::node_reference($caller)).filter(|$r| $r != NONE) AS called_by,
            $safe_depth AS requested_depth
        FROM (SELECT id AS rid, fn::node_info(id) AS node, depth, path FROM $reached)
        WHERE node != NONE
        ORDER BY call_depth
    );
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::calculate_coupling_metrics($project_id: string, $node_id: string) {
    LET $record = fn::resolve_node($node_id);
    IF $record = NONE { RETURN NONE; };
    LET $node = fn::node_info($record);
    IF $node = NONE { RETURN NONE; };

    LET $dependents = (SELECT VALUE `from` FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `to` = $record);
    LET $dependencies = (SELECT VALUE `to` FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `from` = $record);

    LET $dependents_info = array::distinct($dependents).map(|$d| fn::node_reference($d)).filter(|$r| $r != NONE);
    LET $dependencies_info = array::distinct($dependencies).map(|$d| fn::node_reference($d)).filter(|$r| $r != NONE);

    LET $afferent = array::len($dependents_info);
    LET $efferent = array::len($dependencies_info);
    LET $total = $afferent + $efferent;
    -- Cast before dividing: integer division would truncate the ratio to 0.
    LET $instability = IF $total > 0 THEN math::round((<float>$efferent / <float>$total) * 1000000f) / 1000000f ELSE 0f END;

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
            coupling_category: IF $instability < 0.3f THEN 'stable' ELSE IF $instability > 0.7f THEN 'unstable' ELSE 'balanced' END
        },
        node: $node
    };
} PERMISSIONS FULL;

-- A→B and B→A over one edge type. One indexed scan; the reciprocal pairs come from
-- array::intersect of forward and reversed edge keys, so the per-edge membership test only runs
-- against the (small) set of keys that have a reverse.
DEFINE FUNCTION OVERWRITE fn::detect_circular_dependencies($project_id: string, $edge_type: string) {
    LET $edge_name = string::lowercase($edge_type ?? 'calls');
    LET $all_edges = (SELECT `from`, `to` FROM edges WHERE project_id = $project_id AND edge_type = $edge_name AND `from` != `to`);
    LET $forward = $all_edges.map(|$e| <string>$e.from + '>' + <string>$e.to);
    LET $reverse = $all_edges.map(|$e| <string>$e.to + '>' + <string>$e.from);
    LET $mutual = array::intersect($forward, $reverse);
    IF array::len($mutual) = 0 { RETURN []; };
    LET $cycles = (SELECT `from` AS node1_id, `to` AS node2_id FROM $all_edges WHERE `from` < `to` AND (<string>`from` + '>' + <string>`to`) INSIDE $mutual);
    RETURN (
        SELECT <string>node1_id AS node1_id, <string>node2_id AS node2_id, $edge_name AS dependency_type, node1, node2
        FROM (SELECT node1_id, node2_id, fn::node_info(node1_id) AS node1, fn::node_info(node2_id) AS node2 FROM $cycles)
        WHERE node1 != NONE AND node2 != NONE
    );
} PERMISSIONS FULL;

-- Hub nodes by total degree. Two indexed GROUP BY queries rank nodes by incoming and by
-- outgoing degree; the 500 strongest of each form the candidate set, and exact degrees are
-- then counted for those candidates only. Merging the whole project's degree lists with
-- array::concat fails on large graphs (its output is limited to 1 MiB), so the only merge here
-- is over the bounded candidate rows. A node must rank in the top 500 of one direction to be
-- considered, which holds in practice for any node that belongs among the 50 returned.
DEFINE FUNCTION OVERWRITE fn::get_hub_nodes($project_id: string, $min_degree: int) {
    LET $threshold = IF $min_degree != NONE AND $min_degree > 0 THEN $min_degree ELSE 5 END;

    LET $top_in = (SELECT VALUE node_id FROM (SELECT `to` AS node_id, count() AS degree FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types GROUP BY `to`) ORDER BY degree DESC LIMIT 500);
    LET $top_out = (SELECT VALUE node_id FROM (SELECT `from` AS node_id, count() AS degree FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types GROUP BY `from`) ORDER BY degree DESC LIMIT 500);
    LET $candidates = array::union($top_in, $top_out);
    IF array::len($candidates) = 0 { RETURN []; };

    LET $afferent = (SELECT `to` AS node_id, count() AS degree FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `to` INSIDE $candidates GROUP BY `to`);
    LET $efferent = (SELECT `from` AS node_id, count() AS degree FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `from` INSIDE $candidates GROUP BY `from`);
    LET $hubs = (
        SELECT node_id, afferent_degree, efferent_degree, afferent_degree + efferent_degree AS total_degree
        FROM (
            SELECT node_id, math::sum(afferent) AS afferent_degree, math::sum(efferent) AS efferent_degree
            FROM array::concat(
                $afferent.map(|$r| { node_id: $r.node_id, afferent: $r.degree, efferent: 0 }),
                $efferent.map(|$r| { node_id: $r.node_id, afferent: 0, efferent: $r.degree })
            )
            GROUP BY node_id
        )
        WHERE afferent_degree + efferent_degree >= $threshold
        ORDER BY total_degree DESC
        LIMIT 50
    );
    IF array::len($hubs) = 0 { RETURN []; };

    LET $hub_ids = $hubs.map(|$h| $h.node_id);
    LET $incoming_by_type = (SELECT `to` AS node_id, edge_type, count() AS count FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `to` INSIDE $hub_ids GROUP BY `to`, edge_type);
    LET $outgoing_by_type = (SELECT `from` AS node_id, edge_type, count() AS count FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `from` INSIDE $hub_ids GROUP BY `from`, edge_type);

    RETURN (
        SELECT node_id, node, afferent_degree, efferent_degree, total_degree,
            $incoming_by_type.filter(|$r| $r.node_id = node_id).map(|$r| { edge_type: $r.edge_type, count: $r.count }) AS incoming_by_type,
            $outgoing_by_type.filter(|$r| $r.node_id = node_id).map(|$r| { edge_type: $r.edge_type, count: $r.count }) AS outgoing_by_type
        FROM (SELECT node_id, fn::node_summary(node_id) AS node, afferent_degree, efferent_degree, total_degree FROM $hubs)
        WHERE node != NONE
    );
} PERMISSIONS FULL;

-- Complex functions ranked by complexity × (dependents + 1). The compound index on
-- (project_id, node_type, complexity) serves the filter and the ORDER BY; coupling comes from
-- two GROUP BY queries over the candidate set instead of two subqueries per row.
DEFINE FUNCTION OVERWRITE fn::get_complexity_hotspots($project_id: string, $min_complexity: float, $limit: int) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 20 END;
    LET $threshold = IF $min_complexity > 0f THEN $min_complexity ELSE 5f END;

    LET $candidates = (
        SELECT id, name, node_type AS kind, language, file_path, start_line, end_line, complexity
        FROM nodes
        WHERE project_id = $project_id AND node_type INSIDE ['function', 'method', 'Function', 'Method'] AND complexity >= $threshold
        ORDER BY complexity DESC
        LIMIT $safe_limit * 2
    );
    LET $ids = $candidates.map(|$c| $c.id);
    LET $afferent = (SELECT `to` AS node_id, count() AS count FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `to` INSIDE $ids GROUP BY `to`);
    LET $efferent = (SELECT `from` AS node_id, count() AS count FROM edges WHERE project_id = $project_id AND edge_type INSIDE $dependency_edge_types AND `from` INSIDE $ids GROUP BY `from`);

    RETURN (
        SELECT
            <string>id AS id, name, kind, language, file_path, start_line, end_line, complexity,
            afferent_coupling, efferent_coupling,
            (IF afferent_coupling + efferent_coupling > 0 THEN math::round((<float>efferent_coupling / <float>(afferent_coupling + efferent_coupling)) * 10000f) / 10000f ELSE 0f END) AS instability,
            complexity * (afferent_coupling + 1) AS risk_score
        FROM (
            SELECT *,
                ($afferent.find(|$a| $a.node_id = id).count ?? 0) AS afferent_coupling,
                ($efferent.find(|$e| $e.node_id = id).count ?? 0) AS efferent_coupling
            FROM $candidates
        )
        ORDER BY risk_score DESC
        LIMIT $safe_limit
    );
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::find_nodes_by_name($project_id: string, $needle: string, $limit: int) {
    LET $max = IF $limit != NONE AND $limit > 0 THEN $limit ELSE 10 END;
    RETURN (
        SELECT id, name, node_type AS kind, language, metadata, { end_line: end_line, file_path: file_path, start_line: start_line } AS location
        FROM nodes
        WHERE project_id = $project_id AND (string::lowercase(name) CONTAINS string::lowercase($needle) OR file_path CONTAINS $needle)
        ORDER BY name
        LIMIT $max
    );
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::get_top_directories($project_id: string, $limit: int) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 50 THEN $limit ELSE 10 END;
    RETURN (
        SELECT string::split(file_path, '/')[0] AS directory, count() AS node_count, array::distinct(language) AS languages
        FROM nodes
        WHERE project_id = $project_id AND file_path != NONE
        GROUP BY directory
        ORDER BY node_count DESC
        LIMIT $safe_limit
    );
} PERMISSIONS FULL;

DEFINE FUNCTION OVERWRITE fn::count_nodes_for_project($project_id: string) {
    RETURN {
        nodes: (SELECT count() AS count FROM nodes WHERE project_id = $project_id GROUP ALL)[0].count ?? 0,
        edges: (SELECT count() AS count FROM edges WHERE project_id = $project_id GROUP ALL)[0].count ?? 0,
        chunks: (SELECT count() AS count FROM chunks WHERE project_id = $project_id GROUP ALL)[0].count ?? 0
    };
} PERMISSIONS FULL;

-- ------------------------------
-- VECTOR AND HYBRID SEARCH
-- ------------------------------

-- HNSW nearest chunks for one embedding dimension. The KNN operator needs literal K and EF,
-- so the dimension dispatch lives here once instead of in every search function.
-- Returns [{ id, parent_node, chunk_index, text, distance }] ordered by distance.
DEFINE FUNCTION OVERWRITE fn::knn_chunks($project_id: string, $dimension: int, $query_embedding: array<float>) {
    RETURN
        IF $dimension = 384 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_384 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 768 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_768 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 1024 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_1024 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 1536 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_1536 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 2048 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_2048 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 2560 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_2560 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 3072 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_3072 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 3584 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_3584 <|100,200|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 4096 THEN (SELECT id, parent_node, chunk_index, text, vector::distance::knn() AS distance FROM chunks WHERE project_id = $project_id AND embedding_4096 <|100,200|> $query_embedding ORDER BY distance)
        ELSE [] END;
} PERMISSIONS FULL;

-- Nearest nodes by their own embedding (whole-node vectors), same shape as fn::knn_chunks.
DEFINE FUNCTION OVERWRITE fn::knn_nodes($project_id: string, $dimension: int, $query_embedding: array<float>) {
    RETURN
        IF $dimension = 384 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_384 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 768 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_768 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 1024 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_1024 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 1536 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_1536 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 2048 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_2048 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 2560 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_2560 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 3072 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_3072 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 3584 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_3584 <|50,150|> $query_embedding ORDER BY distance)
        ELSE IF $dimension = 4096 THEN (SELECT id, vector::distance::knn() AS distance FROM nodes WHERE project_id = $project_id AND embedding_4096 <|50,150|> $query_embedding ORDER BY distance)
        ELSE [] END;
} PERMISSIONS FULL;

-- Hybrid node search: chunk KNN (best chunk per node) and BM25 over content and name, fused
-- with reciprocal rank fusion. Result shape matches v1 so the tools need no change.
DEFINE FUNCTION OVERWRITE fn::semantic_search_nodes_via_chunks(
    $project_id: string,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $query_embedding: array<float>
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;

    -- 1) Vector list at node granularity: the closest chunk per parent node.
    LET $chunk_hits = fn::knn_chunks($project_id, $dimension, $query_embedding);
    LET $vector_list = (
        SELECT parent_node AS id, math::min(distance) AS distance
        FROM $chunk_hits
        WHERE parent_node != NONE
        GROUP BY parent_node
        ORDER BY distance
        LIMIT $safe_limit * 3
    );
    LET $vector_ranked = (SELECT id, 1f - distance AS vector_score FROM $vector_list ORDER BY distance);

    -- 2) BM25 lists over content and name (indexed, project-scoped).
    LET $content_ranked = (
        SELECT id, search::score(1) AS text_score
        FROM nodes
        WHERE project_id = $project_id AND content @1@ $query_text
        ORDER BY text_score DESC
        LIMIT $safe_limit * 2
    );
    LET $name_ranked = (
        SELECT id, search::score(1) AS text_score
        FROM nodes
        WHERE project_id = $project_id AND name @1@ $query_text
        ORDER BY text_score DESC
        LIMIT $safe_limit * 2
    );

    -- 3) Reciprocal rank fusion keeps nodes that rank well on any list.
    LET $fused = search::rrf([$vector_ranked, $name_ranked, $content_ranked], $safe_limit, 60);
    IF array::len($fused) = 0 { RETURN []; };

    -- 4) Enrich the fused ids and attach edge context from two indexed queries.
    LET $node_ids = $fused.map(|$f| $f.id);
    LET $vector_ids = $vector_ranked.map(|$v| $v.id);
    LET $text_ids = array::concat($name_ranked.map(|$t| $t.id), $content_ranked.map(|$t| $t.id));
    LET $edges = fn::edges_for_nodes($node_ids, 5);
    LET $rows = (
        SELECT
            id AS node_ref, <string>id AS node_id, name, node_type AS kind, language, file_path, start_line, end_line, content, metadata,
            ($fused.find(|$f| $f.id = id).vector_score ?? 0f) AS vector_score,
            ($fused.find(|$f| $f.id = id).text_score ?? 0f) AS text_score,
            ($fused.find(|$f| $f.id = id).rrf_score ?? 0f) AS combined_score,
            array::concat(
                IF id INSIDE $vector_ids THEN ['chunk'] ELSE [] END,
                IF id INSIDE $text_ids THEN ['text'] ELSE [] END
            ) AS match_sources,
            [] AS matched_symbols
        FROM nodes
        WHERE id INSIDE $node_ids
    );
    RETURN (
        SELECT
            node_id, name, kind, language, file_path, start_line, end_line, content, metadata,
            vector_score, text_score, combined_score, match_sources, matched_symbols,
            array::slice($edges.outgoing.filter(|$e| $e.source_node = node_ref).map(|$e| { node_id: $e.node_id, name: $e.name, kind: $e.kind, file_path: $e.file_path, relationship: $e.relationship }), 0, 5) AS outgoing_edges,
            array::slice($edges.incoming.filter(|$e| $e.source_node = node_ref).map(|$e| { node_id: $e.node_id, name: $e.name, kind: $e.kind, file_path: $e.file_path, relationship: $e.relationship }), 0, 5) AS incoming_edges
        FROM $rows
        ORDER BY combined_score DESC
    );
} PERMISSIONS FULL;

-- Chunk-level vector search with optional graph context. Cosine similarity comes from the KNN
-- distance (1 - distance for a COSINE index) instead of a second vector computation per row.
DEFINE FUNCTION OVERWRITE fn::semantic_search_chunks_with_context(
    $project_id: string,
    $query_embedding: array<float>,
    $query_text: string,
    $dimension: int,
    $limit: int,
    $threshold: float,
    $include_graph_context: bool
) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 100 THEN $limit ELSE 10 END;
    LET $safe_threshold = IF $threshold >= 0.0 AND $threshold <= 1.0 THEN $threshold ELSE 0.7 END;

    LET $scored = (
        SELECT id, parent_node, chunk_index, text, 1f - distance AS vector_score
        FROM fn::knn_chunks($project_id, $dimension, $query_embedding)
        WHERE parent_node != NONE AND 1f - distance >= $safe_threshold
        ORDER BY vector_score DESC
        LIMIT $safe_limit
    );
    LET $base = (
        SELECT id, parent_node, chunk_index, text, vector_score,
            parent_node.name AS parent_name, parent_node.node_type AS parent_kind,
            parent_node.file_path AS file_path, parent_node.start_line AS start_line, parent_node.end_line AS end_line
        FROM $scored
    );
    IF !$include_graph_context {
        RETURN (SELECT id, chunk_index, text, vector_score, parent_name, parent_kind, file_path, start_line, end_line FROM $base);
    };
    LET $edges = fn::edges_for_nodes(array::distinct($base.map(|$b| $b.parent_node)), 5);
    RETURN (
        SELECT id, chunk_index, text, vector_score, parent_name, parent_kind, file_path, start_line, end_line,
            array::slice($edges.outgoing.filter(|$e| $e.source_node = parent_node).map(|$e| { node_id: $e.node_id, name: $e.name, kind: $e.kind, file_path: $e.file_path, relationship: $e.relationship }), 0, 5) AS outgoing_edges,
            array::slice($edges.incoming.filter(|$e| $e.source_node = parent_node).map(|$e| { node_id: $e.node_id, name: $e.name, kind: $e.kind, file_path: $e.file_path, relationship: $e.relationship }), 0, 5) AS incoming_edges
        FROM $base
    );
} PERMISSIONS FULL;

-- BM25 search over node content with highlighted fragments, for showing why a node matched.
DEFINE FUNCTION OVERWRITE fn::search_snippets($project_id: string, $query_text: string, $limit: int) {
    LET $safe_limit = IF $limit > 0 AND $limit <= 50 THEN $limit ELSE 10 END;
    RETURN (
        SELECT <string>id AS node_id, name, node_type AS kind, file_path, start_line,
            search::score(1) AS text_score,
            search::highlight('[[', ']]', 1) AS highlighted
        FROM nodes
        WHERE project_id = $project_id AND content @1@ $query_text
        ORDER BY text_score DESC
        LIMIT $safe_limit
    );
} PERMISSIONS FULL;

-- Lloyd's k-means over a list of { id, vector } rows. SurrealDB has no built-in clustering; this
-- groups a result set (for example the top search hits) into $k topics using vector:: math.
-- Returns [{ cluster, centroid, members: [id] }]. Intended for tens to low hundreds of vectors.
DEFINE FUNCTION OVERWRITE fn::kmeans($rows: array, $k: int, $iterations: int) {
    LET $n = array::len($rows);
    IF $n = 0 { RETURN []; };
    LET $clusters = IF $k > 0 AND $k < $n THEN $k ELSE math::max([1, math::min([$n, 2])]) END;
    LET $rounds = IF $iterations > 0 AND $iterations <= 50 THEN $iterations ELSE 10 END;

    -- Seed centroids with evenly spaced rows (deterministic), then alternate assignment and update.
    LET $step = math::floor($n / $clusters);
    LET $seeds = array::range(0, $clusters).map(|$i| $rows[$i * $step].vector);
    LET $fit = array::fold(array::range(0, $rounds), { centroids: $seeds, assignment: [] }, |$state, $round| {
        LET $assignment = $rows.map(|$row| {
            LET $distances = $state.centroids.map(|$c| vector::distance::euclidean($row.vector, $c));
            RETURN { id: $row.id, vector: $row.vector, cluster: array::find_index($distances, math::min($distances)) };
        });
        LET $centroids = array::range(0, $clusters).map(|$c| {
            LET $members = $assignment.filter(|$a| $a.cluster = $c).map(|$a| $a.vector);
            RETURN IF array::len($members) = 0 THEN $state.centroids[$c] ELSE vector::scale(vector::sum($members), 1f / array::len($members)) END;
        });
        RETURN { centroids: $centroids, assignment: $assignment };
    });

    RETURN array::range(0, $clusters).map(|$c| {
        cluster: $c,
        centroid: $fit.centroids[$c],
        members: $fit.assignment.filter(|$a| $a.cluster = $c).map(|$a| $a.id)
    });
} PERMISSIONS FULL;
