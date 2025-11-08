-- ABOUTME: SurrealDB function to detect circular dependencies in the code graph
-- ABOUTME: Identifies bidirectional dependency relationships between code nodes

-- Detect circular dependencies for a given edge type
-- Returns pairs of nodes that have mutual dependencies (A -> B and B -> A)
-- Will be exposed as LLM tool for dependency analysis
DEFINE FUNCTION fn::coupling_metrics($node_id: string, $afferent_count: int, $efferent_count: int) {
    IF $node_id = NONE OR $node_id = "" {
        RETURN {
            error: "Invalid or missing node_id"
        };
    };

    LET $afferent = $afferent_count ?? 0;
    LET $efferent = $efferent_count ?? 0;
    LET $total_coupling = $afferent + $efferent;
    LET $instability = IF $total_coupling > 0 THEN $efferent / $total_coupling ELSE 0.0 END;

    LET $node_details = (
        SELECT id, name, kind, location, language, metadata
        FROM ONLY $node_id
        FETCH location, metadata
    )[0];

    RETURN {
        node: $node_details,
        metrics: {
            afferent_coupling: $afferent,
            efferent_coupling: $efferent,
            total_coupling: $total_coupling,
            instability: $instability,
            stability: 1.0 - $instability,
            is_stable: $instability < 0.3,
            is_unstable: $instability > 0.7,
            coupling_category: IF $instability < 0.3 THEN "stable"
                              ELSE IF $instability > 0.7 THEN "unstable"
                              ELSE "balanced"
                              END
        },
        dependents: $dependents,
        dependencies: $dependencies
    };
}

        

