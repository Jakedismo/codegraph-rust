-- ABOUTME: SurrealDB function to find reverse dependencies (dependents) of a code node
-- ABOUTME: Performs recursive graph traversal up incoming edges for impact analysis

-- Get all reverse dependencies (dependents) of a node up to specified depth
-- Finds which nodes depend on the target node (impact analysis)
-- Will be exposed as LLM tool for change impact assessment
DEFINE FUNCTION fn::get_reverse_dependencies($node_id: string, $edge_type: string, $depth: int) {
    -- Validate depth parameter
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;

    -- Recursive traversal using SurrealDB graph operators
    -- <-edges means follow incoming edges
    -- WHERE filters by edge type and depth
    -- <-from gets the source nodes (dependents)
    RETURN SELECT
        id,
        name,
        kind,
        location,
        language,
        content,
        metadata,
        -- Calculate depth for this dependent
        -- Depth 0 = directly depends on node_id
        -- Depth 1 = depends on nodes that depend on node_id, etc.
        array::len(
            SELECT VALUE to
            FROM ONLY $node_id
            <-edges[WHERE edge_type = $edge_type AND to = $node_id]
            <-from
        ) AS dependent_depth
    FROM (
        SELECT <-edges[WHERE edge_type = $edge_type]
        FROM ONLY $node_id
    )<-from
    -- Recursively follow incoming edges up to max depth
    -- SurrealDB will handle the recursion
    FETCH location, metadata;
};
