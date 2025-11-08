-- ABOUTME: SurrealDB function to identify highly connected hub nodes in the code graph
-- ABOUTME: Finds architectural hotspots and potential bottlenecks by analyzing node connectivity

-- Get hub nodes with degree >= min_degree
-- Returns highly connected nodes sorted by total degree (descending)
-- Will be exposed as LLM tool for identifying architectural hotspots
-- Calculate afferent coupling (number of dependents)
DEFINE FUNCTION fn::get_hub_nodes($node_id: record, $afferent_count: number, $efferent_count: number) {
    LET $afferent = $afferent_count ?? 0;
    LET $efferent = $efferent_count ?? 0;
    LET $total_coupling = $afferent + $efferent;
    LET $instability = IF $total_coupling > 0 THEN $efferent / $total_coupling ELSE 0.0 END;

    LET $node_details = (
        SELECT id, name, kind, location, language, metadata
        FROM ONLY $node_id
        FETCH location, metadata
    )[0];

    LET $dependents = (
        SELECT id, name, kind, location
        FROM (
            SELECT DISTINCT <-edges[WHERE edge_type IN ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]]<-from AS dependent
            FROM ONLY $node_id
        ).dependent
        FETCH location
    );

    LET $dependencies = (
        SELECT id, name, kind, location
        FROM (
            SELECT DISTINCT ->edges[WHERE edge_type IN ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]]->to AS dependency
            FROM ONLY $node_id
        ).dependency
        FETCH location
    );

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
};
