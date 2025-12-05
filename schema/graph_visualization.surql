-- ABOUTME: Graph visualization schema for CodeGraph
-- ABOUTME: Creates graph relation tables and export functions for DOT, D3.js, Cytoscape formats

-- ------------------------------
-- GRAPH EDGE RELATION TABLES
-- These use SurrealDB's native RELATE syntax for graph traversal
-- ------------------------------

-- Calls relationship: function/method calls another function/method
DEFINE TABLE calls TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'Function/method call relationships for call graph visualization'
  PERMISSIONS FULL;

DEFINE FIELD weight ON calls TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON calls FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON calls TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- Imports relationship: module/file imports another module/file
DEFINE TABLE imports TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'Import/require relationships for dependency graph visualization'
  PERMISSIONS FULL;

DEFINE FIELD weight ON imports TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON imports FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON imports TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- Uses relationship: code uses a type/variable/constant
DEFINE TABLE uses TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'Usage relationships (types, variables, constants)'
  PERMISSIONS FULL;

DEFINE FIELD weight ON uses TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON uses FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON uses TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- Extends relationship: class/interface extends another
DEFINE TABLE extends TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'Inheritance relationships for class hierarchy visualization'
  PERMISSIONS FULL;

DEFINE FIELD weight ON extends TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON extends FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON extends TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- Implements relationship: class implements interface/protocol
DEFINE TABLE implements TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'Interface implementation relationships'
  PERMISSIONS FULL;

DEFINE FIELD weight ON implements TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON implements FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON implements TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- References relationship: code references another element
DEFINE TABLE references TYPE RELATION IN nodes OUT nodes SCHEMAFULL
  COMMENT 'General reference relationships'
  PERMISSIONS FULL;

DEFINE FIELD weight ON references TYPE float DEFAULT 1f PERMISSIONS FULL;
DEFINE FIELD metadata ON references FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD created_at ON references TYPE datetime DEFAULT time::now() READONLY PERMISSIONS FULL;

-- ------------------------------
-- GRAPH POPULATION FUNCTIONS
-- Populate relation tables from the relational edges table
-- ------------------------------

-- Populate all graph edges from relational edges table
DEFINE FUNCTION fn::populate_graph_edges($project_id: string) {
  -- Clear existing graph edges for this project
  DELETE calls       WHERE in.project_id = $project_id OR out.project_id = $project_id;
  DELETE imports     WHERE in.project_id = $project_id OR out.project_id = $project_id;
  DELETE uses        WHERE in.project_id = $project_id OR out.project_id = $project_id;
  DELETE extends     WHERE in.project_id = $project_id OR out.project_id = $project_id;
  DELETE implements  WHERE in.project_id = $project_id OR out.project_id = $project_id;
  DELETE references  WHERE in.project_id = $project_id OR out.project_id = $project_id;

  ------------------------------------------------------------------
  -- calls
  ------------------------------------------------------------------
  LET $calls_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['calls', 'Calls']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $calls_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->calls->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  ------------------------------------------------------------------
  -- imports
  ------------------------------------------------------------------
  LET $imports_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['imports', 'Imports']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $imports_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->imports->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  ------------------------------------------------------------------
  -- uses
  ------------------------------------------------------------------
  LET $uses_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['uses', 'Uses']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $uses_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->uses->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  ------------------------------------------------------------------
  -- extends
  ------------------------------------------------------------------
  LET $extends_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['extends', 'Extends']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $extends_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->extends->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  ------------------------------------------------------------------
  -- implements
  ------------------------------------------------------------------
  LET $implements_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['implements', 'Implements']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $implements_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->implements->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  ------------------------------------------------------------------
  -- references
  ------------------------------------------------------------------
  LET $references_edges = (
    SELECT from, to, weight, metadata
    FROM edges
    WHERE edge_type IN ['references', 'References']
      AND (from.project_id = $project_id OR from.project_id = NONE)
      AND (to.project_id   = $project_id OR to.project_id   = NONE)
  );

  FOR $e IN $references_edges {
    LET $from = $e.from;
    LET $to   = $e.to;

    RELATE $from->references->$to
      SET
        weight   = $e.weight,
        metadata = $e.metadata;
  };

  RETURN {
    status: 'completed',
    project_id: $project_id,
    calls:       array::len($calls_edges),
    imports:     array::len($imports_edges),
    uses:        array::len($uses_edges),
    extends:     array::len($extends_edges),
    implements:  array::len($implements_edges),
    references:  array::len($references_edges)
  };
}
PERMISSIONS FULL;

-- ------------------------------
-- GRAPH STATISTICS FUNCTION
-- ------------------------------

DEFINE FUNCTION fn::graph_stats($project_id: string) {
  LET $nodes = (SELECT count() AS cnt FROM nodes WHERE project_id = $project_id)[0].cnt ?? 0;
  LET $calls = (SELECT count() AS cnt FROM calls WHERE in.project_id = $project_id)[0].cnt ?? 0;
  LET $imports = (SELECT count() AS cnt FROM imports WHERE in.project_id = $project_id)[0].cnt ?? 0;
  LET $uses = (SELECT count() AS cnt FROM uses WHERE in.project_id = $project_id)[0].cnt ?? 0;
  LET $extends = (SELECT count() AS cnt FROM extends WHERE in.project_id = $project_id)[0].cnt ?? 0;
  LET $implements = (SELECT count() AS cnt FROM implements WHERE in.project_id = $project_id)[0].cnt ?? 0;
  LET $references = (SELECT count() AS cnt FROM references WHERE in.project_id = $project_id)[0].cnt ?? 0;

  RETURN {
    project_id: $project_id,
    node_count: $nodes,
    edge_counts: {
      calls: $calls,
      imports: $imports,
      uses: $uses,
      extends: $extends,
      implements: $implements,
      references: $references,
      total: $calls + $imports + $uses + $extends + $implements + $references
    }
  };
} PERMISSIONS FULL;

-- ------------------------------
-- GRAPHVIZ DOT EXPORT FUNCTION
-- ------------------------------

DEFINE FUNCTION fn::export_graphviz($project_id: string, $edge_types: array<string>) {
  LET $selected_types = IF array::len($edge_types) > 0 THEN $edge_types
    ELSE ['calls', 'imports', 'uses', 'extends', 'implements', 'references'] END;

  -- Get all nodes for the project
  LET $project_nodes = (SELECT id, name, node_type, file_path FROM nodes
    WHERE project_id = $project_id);

  -- Build node definitions
  LET $node_defs = (SELECT
    string::concat('  "', <string>id, '" [label="', name, '\\n(', node_type ?? 'unknown', ')"];') AS def
    FROM $project_nodes);

  -- Collect edges based on selected types
  LET $edge_list = [];

  IF 'calls' INSIDE $selected_types {
    LET $calls_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="red" label="calls"];') AS def
      FROM calls WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $calls_list);
  };

  IF 'imports' INSIDE $selected_types {
    LET $imports_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="blue" label="imports"];') AS def
      FROM imports WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $imports_list);
  };

  IF 'uses' INSIDE $selected_types {
    LET $uses_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="green" label="uses"];') AS def
      FROM uses WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $uses_list);
  };

  IF 'extends' INSIDE $selected_types {
    LET $extends_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="purple" style="bold" label="extends"];') AS def
      FROM extends WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $extends_list);
  };

  IF 'implements' INSIDE $selected_types {
    LET $implements_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="orange" style="dashed" label="implements"];') AS def
      FROM implements WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $implements_list);
  };

  IF 'references' INSIDE $selected_types {
    LET $references_list = (SELECT
      string::concat('  "', <string>in, '" -> "', <string>out, '" [color="gray" label="references"];') AS def
      FROM references WHERE in.project_id = $project_id);
    LET $edge_list = array::concat($edge_list, $references_list);
  };

  LET $nodes_str = string::join('\n', (SELECT VALUE def FROM $node_defs));
  LET $edges_str = string::join('\n', (SELECT VALUE def FROM $edge_list));

  RETURN string::concat(
    'digraph codegraph {\n',
    '  rankdir=LR;\n',
    '  node [shape=box fontname="Helvetica"];\n',
    '  edge [fontname="Helvetica" fontsize=10];\n\n',
    '  // Nodes\n',
    $nodes_str,
    '\n\n  // Edges\n',
    $edges_str,
    '\n}\n'
  );
} PERMISSIONS FULL;

-- ------------------------------
-- D3.JS JSON EXPORT FUNCTION
-- ------------------------------

DEFINE FUNCTION fn::export_d3_json($project_id: string, $edge_types: array<string>) {
  LET $selected_types = IF array::len($edge_types) > 0 THEN $edge_types
    ELSE ['calls', 'imports', 'uses', 'extends', 'implements', 'references'] END;

  -- Get all nodes for the project
  LET $project_nodes = (SELECT
    <string>id AS id,
    name,
    node_type AS type,
    language,
    file_path
    FROM nodes WHERE project_id = $project_id);

  -- Collect links based on selected types
  LET $links = [];

  IF 'calls' INSIDE $selected_types {
    LET $calls_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'calls' AS type,
      weight AS value
      FROM calls WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $calls_links);
  };

  IF 'imports' INSIDE $selected_types {
    LET $imports_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'imports' AS type,
      weight AS value
      FROM imports WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $imports_links);
  };

  IF 'uses' INSIDE $selected_types {
    LET $uses_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'uses' AS type,
      weight AS value
      FROM uses WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $uses_links);
  };

  IF 'extends' INSIDE $selected_types {
    LET $extends_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'extends' AS type,
      weight AS value
      FROM extends WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $extends_links);
  };

  IF 'implements' INSIDE $selected_types {
    LET $implements_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'implements' AS type,
      weight AS value
      FROM implements WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $implements_links);
  };

  IF 'references' INSIDE $selected_types {
    LET $references_links = (SELECT
      <string>in AS source,
      <string>out AS target,
      'references' AS type,
      weight AS value
      FROM references WHERE in.project_id = $project_id);
    LET $links = array::concat($links, $references_links);
  };

  RETURN {
    nodes: $project_nodes,
    links: $links
  };
} PERMISSIONS FULL;

-- ------------------------------
-- CYTOSCAPE.JS JSON EXPORT FUNCTION
-- ------------------------------

DEFINE FUNCTION fn::export_cytoscape($project_id: string, $edge_types: array<string>) {
  LET $selected_types = IF array::len($edge_types) > 0 THEN $edge_types
    ELSE ['calls', 'imports', 'uses', 'extends', 'implements', 'references'] END;

  -- Get all nodes for the project (Cytoscape format)
  LET $project_nodes = (SELECT {
    data: {
      id: <string>id,
      label: name,
      type: node_type,
      language: language,
      file_path: file_path
    }
  } AS element FROM nodes WHERE project_id = $project_id);

  -- Collect edges based on selected types (Cytoscape format)
  LET $edges = [];

  IF 'calls' INSIDE $selected_types {
    LET $calls_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'calls',
        weight: weight
      }
    } AS element FROM calls WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $calls_edges);
  };

  IF 'imports' INSIDE $selected_types {
    LET $imports_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'imports',
        weight: weight
      }
    } AS element FROM imports WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $imports_edges);
  };

  IF 'uses' INSIDE $selected_types {
    LET $uses_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'uses',
        weight: weight
      }
    } AS element FROM uses WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $uses_edges);
  };

  IF 'extends' INSIDE $selected_types {
    LET $extends_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'extends',
        weight: weight
      }
    } AS element FROM extends WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $extends_edges);
  };

  IF 'implements' INSIDE $selected_types {
    LET $implements_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'implements',
        weight: weight
      }
    } AS element FROM implements WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $implements_edges);
  };

  IF 'references' INSIDE $selected_types {
    LET $references_edges = (SELECT {
      data: {
        id: <string>id,
        source: <string>in,
        target: <string>out,
        type: 'references',
        weight: weight
      }
    } AS element FROM references WHERE in.project_id = $project_id);
    LET $edges = array::concat($edges, $references_edges);
  };

  LET $node_elements = (SELECT VALUE element FROM $project_nodes);
  LET $edge_elements = (SELECT VALUE element FROM $edges);

  RETURN {
    elements: array::concat($node_elements, $edge_elements)
  };
} PERMISSIONS FULL;

-- ------------------------------
-- FILE SUBGRAPH EXPORT FUNCTION
-- Export graph for a specific file and its dependencies
-- ------------------------------

DEFINE FUNCTION fn::export_file_subgraph($project_id: string, $file_path: string, $depth: int) {
  LET $safe_depth = IF $depth > 0 AND $depth <= 3 THEN $depth ELSE 1 END;

  -- Get nodes in the target file
  LET $file_nodes = (SELECT id FROM nodes
    WHERE project_id = $project_id AND file_path = $file_path);
  LET $file_node_ids = (SELECT VALUE id FROM $file_nodes);

  -- Get depth-1 connected nodes
  LET $depth1_out = (SELECT VALUE out FROM calls, imports, uses, extends, implements, references
    WHERE in INSIDE $file_node_ids);
  LET $depth1_in = (SELECT VALUE in FROM calls, imports, uses, extends, implements, references
    WHERE out INSIDE $file_node_ids);
  LET $depth1_nodes = array::distinct(array::concat($depth1_out, $depth1_in));

  -- Get depth-2 if requested
  LET $depth2_nodes = IF $safe_depth >= 2 THEN (
    LET $d2_out = (SELECT VALUE out FROM calls, imports, uses, extends, implements, references
      WHERE in INSIDE $depth1_nodes AND out NOTINSIDE $file_node_ids);
    LET $d2_in = (SELECT VALUE in FROM calls, imports, uses, extends, implements, references
      WHERE out INSIDE $depth1_nodes AND in NOTINSIDE $file_node_ids);
    array::distinct(array::concat($d2_out, $d2_in))
  ) ELSE [] END;

  -- Get depth-3 if requested
  LET $depth3_nodes = IF $safe_depth >= 3 THEN (
    LET $all_previous = array::concat($file_node_ids, $depth1_nodes, $depth2_nodes);
    LET $d3_out = (SELECT VALUE out FROM calls, imports, uses, extends, implements, references
      WHERE in INSIDE $depth2_nodes AND out NOTINSIDE $all_previous);
    LET $d3_in = (SELECT VALUE in FROM calls, imports, uses, extends, implements, references
      WHERE out INSIDE $depth2_nodes AND in NOTINSIDE $all_previous);
    array::distinct(array::concat($d3_out, $d3_in))
  ) ELSE [] END;

  -- Combine all relevant nodes
  LET $all_relevant = array::distinct(array::concat(
    $file_node_ids, $depth1_nodes, $depth2_nodes, $depth3_nodes
  ));

  -- Get node details
  LET $subgraph_nodes = (SELECT
    <string>id AS id,
    name,
    node_type AS type,
    language,
    file_path,
    IF id INSIDE $file_node_ids THEN 0
    ELSE IF id INSIDE $depth1_nodes THEN 1
    ELSE IF id INSIDE $depth2_nodes THEN 2
    ELSE 3 END AS depth
    FROM nodes WHERE id INSIDE $all_relevant);

  -- Get edges between relevant nodes
  LET $subgraph_edges = (SELECT
    <string>in AS source,
    <string>out AS target,
    'calls' AS type,
    weight
    FROM calls WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant);

  LET $subgraph_edges = array::concat($subgraph_edges, (SELECT
    <string>in AS source,
    <string>out AS target,
    'imports' AS type,
    weight
    FROM imports WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant));

  LET $subgraph_edges = array::concat($subgraph_edges, (SELECT
    <string>in AS source,
    <string>out AS target,
    'uses' AS type,
    weight
    FROM uses WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant));

  LET $subgraph_edges = array::concat($subgraph_edges, (SELECT
    <string>in AS source,
    <string>out AS target,
    'extends' AS type,
    weight
    FROM extends WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant));

  LET $subgraph_edges = array::concat($subgraph_edges, (SELECT
    <string>in AS source,
    <string>out AS target,
    'implements' AS type,
    weight
    FROM implements WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant));

  LET $subgraph_edges = array::concat($subgraph_edges, (SELECT
    <string>in AS source,
    <string>out AS target,
    'references' AS type,
    weight
    FROM references WHERE in INSIDE $all_relevant AND out INSIDE $all_relevant));

  RETURN {
    file_path: $file_path,
    depth: $safe_depth,
    nodes: $subgraph_nodes,
    links: $subgraph_edges
  };
} PERMISSIONS FULL;

-- ------------------------------
-- USAGE EXAMPLES
-- ------------------------------
--
-- 1. First, populate the graph edges from relational edges:
--    fn::populate_graph_edges('your-project-id');
--
-- 2. Get graph statistics:
--    fn::graph_stats('your-project-id');
--
-- 3. Export to Graphviz DOT format (for Graphviz, OmniGraffle):
--    fn::export_graphviz('your-project-id', ['calls', 'imports']);
--
-- 4. Export to D3.js JSON format (for web visualization):
--    fn::export_d3_json('your-project-id', ['calls', 'extends', 'implements']);
--
-- 5. Export to Cytoscape.js format (for advanced web graphs):
--    fn::export_cytoscape('your-project-id', []);  -- all edge types
--
-- 6. Export subgraph for a specific file:
--    fn::export_file_subgraph('your-project-id', 'src/main.rs', 2);
--
-- 7. Use SurrealDB graph traversal directly:
--    SELECT ->calls->out AS called_functions FROM nodes:some_function;
--    SELECT <-calls<-in AS callers FROM nodes:some_function;
--    SELECT ->calls->out->calls->out AS transitive_calls FROM nodes:entry_point;
--
-- 8. Get all info about relations:
--    SELECT ->calls.* FROM nodes:some_function;
--    SELECT <-calls.* FROM nodes:some_function;
