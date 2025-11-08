-- Migration 002: Graph Analysis Functions
-- This migration creates SurrealDB functions for LLM-powered graph analysis
-- Functions are idempotent and can be safely re-run

-- ============================================================================
-- Function 1: Get Transitive Dependencies
-- ============================================================================
-- ABOUTME: SurrealDB function to get transitive dependencies of a code node
-- ABOUTME: Performs recursive graph traversal up to specified depth for dependency analysis

-- Get all transitive dependencies of a node up to specified depth
-- Will be exposed as LLM tool for agentic dependency analysis
DEFINE FUNCTION fn::get_transitive_dependencies($node_id: string, $edge_type: string, $depth: int) {
    -- Validate depth parameter
    LET $safe_depth = IF $depth > 0 AND $depth <= 10 THEN $depth ELSE 3 END;

    -- Recursive traversal using SurrealDB graph operators
    -- ->edges means follow outgoing edges
    -- WHERE filters by edge type and depth
    -- ->to gets the target nodes
    RETURN SELECT
        id,
        name,
        kind,
        location,
        language,
        content,
        metadata,
        -- Calculate depth for this dependency
        array::len(
            SELECT VALUE from
            FROM ONLY $node_id
            ->edges[WHERE edge_type = $edge_type AND from = $node_id]
            ->to
        ) AS dependency_depth
    FROM (
        SELECT ->edges[WHERE edge_type = $edge_type]
        FROM ONLY $node_id
    )->to
    -- Recursively follow edges up to max depth
    -- SurrealDB will handle the recursion
    FETCH location, metadata;
};

-- ============================================================================
-- Function 2: Detect Circular Dependencies
-- ============================================================================
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
};

-- ============================================================================
-- Function 3: Trace Call Chain
-- ============================================================================
-- Trace the call chain starting from a node up to specified depth
-- Follows "Calls" edges to show which functions/methods are invoked
-- Will be exposed as LLM tool for execution flow analysis
DEFINE FUNCTION IF NOT EXISTS fn::trace_call_chain($from_node: string, $max_depth: int) {
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

-- ============================================================================
-- Function 4: Calculate Coupling Metrics
-- ============================================================================
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

-- ============================================================================
-- Function 5: Get Hub Nodes
-- ============================================================================
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

-- ============================================================================
-- Function 6: Get Reverse Dependencies
-- ============================================================================
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


-- ============================================================================
-- Record Migration
-- ============================================================================
-- Track this migration in schema_versions table
INSERT INTO schema_versions {
    version: 2,
    name: "graph_functions",
    applied_at: time::now(),
    checksum: "graph_analysis_functions_v1"
};
