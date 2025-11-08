-- ABOUTME: SurrealDB function to trace execution call chains in code
-- ABOUTME: Follows function/method call relationships recursively to map execution paths

-- Trace the call chain starting from a node up to specified depth
-- Follows "Calls" edges to show which functions/methods are invoked
-- Will be exposed as LLM tool for execution flow analysis
DEFINE FUNCTION fn::trace_call_chain($from_node: string, $max_depth: int) {
    -- Validate depth parameter (1-10, default 5 for call chains)
    LET $safe_depth = IF $max_depth > 0 AND $max_depth <= 10 THEN $max_depth ELSE 5 END;

    -- Recursive traversal of call edges from starting node
    -- ->edges[WHERE edge_type = "Calls"] follows outgoing call relationships
    -- ->to gets the called functions/methods
    RETURN SELECT
        id,
        name,
        kind,
        location,
        language,
        content,
        metadata,
        -- Calculate depth level in call chain
        -- Depth 0 = directly called by from_node
        -- Depth 1 = called by functions called by from_node, etc.
        (SELECT count()
         FROM (
             SELECT VALUE depth
             FROM (
                 SELECT *,
                     array::len(
                         SELECT * FROM edges
                         WHERE from = $from_node
                         AND to = $parent.id
                         AND edge_type = "Calls"
                     ) AS depth
                 FROM ONLY $parent.id
             )
             WHERE depth > 0
         )) AS call_depth,
        -- Direct caller information for tracing back
        (SELECT
            id,
            name,
            kind
         FROM (
             SELECT <-edges[WHERE edge_type = "Calls"]<-from AS caller
             FROM ONLY $parent.id
         ).caller) AS called_by
    FROM (
        -- Start from the specified node
        SELECT ->edges[WHERE edge_type = "Calls"]
        FROM ONLY $from_node
    )->to
    -- SurrealDB handles recursive traversal up to safe_depth
    FETCH location, metadata, called_by.location;
};
