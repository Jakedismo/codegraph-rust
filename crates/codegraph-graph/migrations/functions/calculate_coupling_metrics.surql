DEFINE FIELD IF NOT EXISTS from ON TABLE edges TYPE record<nodes>;
DEFINE FIELD IF NOT EXISTS to ON TABLE edges TYPE record<nodes>;


-- Calculate coupling metrics for a node
-- Returns Ca (afferent), Ce (efferent), and I (instability) metrics
-- Will be exposed as LLM tool for architectural quality analysis
DEFINE FUNCTION fn::calculate_coupling_metrics($node_id: string) {
    LET $distinct_from = (
        SELECT array::distinct(from) AS unique_from
        FROM edges
        WHERE to = $node_id
          AND edge_type INSIDE ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]
    )[0].unique_from;
    RETURN array::len($distinct_from);
};

    -- Count efferent coupling (Ce): outgoing dependencies
    -- How many nodes this node depends on
LET $efferent_count = (
    SELECT array::len(array::distinct(to)) AS ce
    FROM edges
    WHERE from = $node_id
      AND edge_type IN ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]
    GROUP ALL
)[0].ce;


    -- Calculate instability: I = Ce / (Ce + Ca)
    -- I = 0: maximally stable (many dependents, few dependencies)
    -- I = 1: maximally unstable (few dependents, many dependencies)
LET $afferent = $afferent_count ?? 0;
LET $efferent = $efferent_count ?? 0;
LET $total_coupling = $afferent + $efferent;
LET $instability = IF $total_coupling > 0 THEN $efferent / $total_coupling ELSE 0.0 END;

LET $node_details = (
    SELECT
        id,
        name,
        kind,
        location,
        language,
        metadata
    FROM ONLY $node_id
    FETCH location, metadata
)[0];

LET $dependents = (
    SELECT
        id,
        name,
        kind,
        location
    FROM (
        SELECT DISTINCT <-edges[WHERE edge_type IN ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]]<-from AS dependent
        FROM ONLY $node_id
    ).dependent
    FETCH location
);

LET $dependencies = (
    SELECT
        id,
        name,
        kind,
        location
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
