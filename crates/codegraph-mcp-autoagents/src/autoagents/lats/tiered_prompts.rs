// ABOUTME: Tier-aware prompt templates for LATS expansion and synthesis phases
// ABOUTME: Provides TERSE/BALANCED/DETAILED/EXPLORATORY variants matching ReAct tier system

use super::search_tree::SearchNode;
use codegraph_mcp_core::context_aware_limits::ContextTier;

// =============================================================================
// EXPANSION PROMPTS - Controls tool reasoning and action generation
// =============================================================================

/// TERSE expansion (Small tier): Minimal reasoning, 1 action only
pub const EXPANSION_TERSE: &str = r#"You are expanding a search node to answer a query.

Query: "{query}"

Current node (depth {depth}):
  Thought: {thought}
{action_context}
{observation_context}

Available tools: {tools}

CRITICAL: Use semantic_code_search FIRST to find node IDs before other tools.
- semantic_code_search(query, limit, threshold) - returns results with "node_id" field
- EXTRACT the actual node_id value from results (e.g., "nodes:abc123def456")
- Use that EXACT node_id string in other tools - NEVER use placeholders like "<NODE_ID>"

Generate exactly 1 action. Be direct and efficient.

Respond with JSON:
{{
  "actions": [
    {{
      "thought": "Brief next step",
      "tool_name": "tool_name",
      "parameters": {{"param": "value"}},
      "reasoning": "Why this helps"
    }}
  ]
}}"#;

/// BALANCED expansion (Medium tier): Standard reasoning, 1-2 actions
pub const EXPANSION_BALANCED: &str = r#"You are expanding a node in a search tree to answer this query:

Query: "{query}"

Current node (depth {depth}):
  Thought: {thought}
{action_context}
{observation_context}

Available tools: {tools}

TOOL USAGE RULES:
0. semantic_code_search(query, limit, threshold) - **ALWAYS START HERE**
   - Returns results with "node_id" field in each match
   - EXTRACT the actual node_id value (e.g., "nodes:abc123def456")
1-6. Other tools require the EXACT node_id from search results:
   - Copy the node_id value EXACTLY - never use placeholders like "<NODE_ID>"
   - Example: If search returns node_id="nodes:xyz789", use "nodes:xyz789"

Your task: Generate 1-2 thought-action pairs that could help answer the query.

Each thought should:
1. Build on the current node's progress
2. Propose a specific next step
3. Use one of the available tools

Respond with a JSON object:
{{
  "actions": [
    {{
      "thought": "Next reasoning step",
      "tool_name": "tool_name",
      "parameters": {{"param": "value"}},
      "reasoning": "Why this action helps answer the query"
    }}
  ]
}}

Generate between 1 and 2 actions."#;

/// DETAILED expansion (Large tier): Thorough reasoning, 1-3 actions with exploration
pub const EXPANSION_DETAILED: &str = r#"You are expanding a node in a search tree to answer this query:

Query: "{query}"

Current node (depth {depth}):
  Thought: {thought}
{action_context}
{observation_context}

Available tools: {tools}

TOOL USAGE HIERARCHY:
0. semantic_code_search(query, limit, threshold) - **REQUIRED FIRST**
   - Returns results with "node_id" field in each match
   - EXTRACT the actual node_id value from result (e.g., "nodes:abc123def456")
   - Parameters: query (string), limit (default 10), threshold (default 0.3)
1-6. Graph analysis tools - use EXACT node_id from search results:
   - NEVER use placeholders like "<NODE_ID>" - use the actual value
   - get_transitive_dependencies(node_id, edge_type, depth)
   - get_reverse_dependencies(node_id, edge_type, depth)
   - trace_call_chain(from_node, to_node, max_depth)
   - detect_circular_dependencies(node_id)
   - calculate_coupling_metrics(node_id)
   - get_hub_nodes(min_degree)

EXPANSION STRATEGY:
1. Consider what information is still missing to fully answer the query
2. Think about alternative paths that might yield better results
3. Balance depth (following a promising lead) with breadth (exploring alternatives)

Each thought should:
1. Build on the current node's progress or explore a promising alternative
2. Propose a specific, actionable next step
3. Use the most appropriate tool for the task
4. Consider dependencies and call chains when relevant

Respond with a JSON object:
{{
  "actions": [
    {{
      "thought": "Detailed reasoning about next step",
      "tool_name": "tool_name",
      "parameters": {{"param": "value"}},
      "reasoning": "Explanation of why this action advances toward the answer"
    }}
  ]
}}

Generate between 1 and 3 actions. Prioritize quality over quantity."#;

/// EXPLORATORY expansion (Massive tier): Comprehensive reasoning, up to 4 actions with deep exploration
pub const EXPANSION_EXPLORATORY: &str = r#"You are expanding a node in a comprehensive search tree to thoroughly answer this query:

Query: "{query}"

Current node (depth {depth}):
  Thought: {thought}
{action_context}
{observation_context}

Available tools: {tools}

CRITICAL TOOL USAGE RULES:
0. semantic_code_search(query, limit, threshold) - **ALWAYS START HERE**
   - Returns results with "node_id" field in each match
   - EXTRACT the actual node_id value (e.g., "nodes:abc123def456")
   - Use descriptive queries: "error handling middleware", "JWT validation"
   - Parameters: query (string), limit (10-30 for exhaustive), threshold (0.3)

1-6. Graph analysis tools - use EXACT node_id from search results:
   - NEVER use placeholders like "<NODE_ID>" or "<FROM_PREVIOUS_STEP>"
   - Copy the exact node_id string from search result's "node_id" field
   - get_transitive_dependencies(node_id, edge_type, depth) - forward deps
   - get_reverse_dependencies(node_id, edge_type, depth) - what depends on this
   - trace_call_chain(from_node, to_node, max_depth) - execution flow
   - detect_circular_dependencies(node_id) - cycle detection
   - calculate_coupling_metrics(node_id) - Ca, Ce, instability
   - get_hub_nodes(min_degree) - find architectural hotspots

COMPREHENSIVE EXPANSION STRATEGY:
1. COVERAGE: Ensure all aspects of the query are being explored
2. DEPTH: Follow promising paths to their logical conclusion
3. BREADTH: Explore alternative interpretations or approaches
4. VALIDATION: Consider cross-checking findings through multiple tools
5. DEPENDENCIES: Map out transitive relationships and call chains

For each action, consider:
- What unique information does this action provide?
- Does this overlap with or complement other actions?
- What follow-up actions might this enable?
- Are there architectural or design patterns worth exploring?

Respond with a JSON object:
{{
  "actions": [
    {{
      "thought": "Comprehensive reasoning about this exploration path",
      "tool_name": "tool_name",
      "parameters": {{"param": "value"}},
      "reasoning": "Detailed explanation of contribution to answering the query"
    }}
  ]
}}

Generate 1-4 high-quality actions. Maximize coverage while avoiding redundancy."#;

// =============================================================================
// SYNTHESIS PROMPTS - Controls final answer composition
// =============================================================================

/// TERSE synthesis (Small tier): Brief, focused answer
pub const SYNTHESIS_TERSE: &str = r#"Synthesize a concise answer from search results.

Query: "{query}"

Best path through search:
{path_summary}

Provide a brief, direct answer. Focus on key findings only.

Respond with JSON:
{{
  "answer": "Concise answer (2-3 sentences)",
  "findings": "Key points",
  "steps_taken": "{steps}"
}}"#;

/// BALANCED synthesis (Medium tier): Standard comprehensive answer
pub const SYNTHESIS_BALANCED: &str = r#"You are synthesizing the final answer from a search tree exploration.

Original query: "{query}"

Best path through the search tree:
{path_summary}

Your task: Synthesize a comprehensive answer to the query based on this path.

The answer should:
1. Directly address the original query
2. Incorporate key findings from each step
3. Be clear, concise, and well-organized
4. Highlight the most important insights

Respond with a JSON object:
{{
  "answer": "Comprehensive answer to the query",
  "findings": "Key findings and insights",
  "steps_taken": "{steps}"
}}

Provide a complete, professional response."#;

/// DETAILED synthesis (Large tier): Thorough answer with structure
pub const SYNTHESIS_DETAILED: &str = r#"You are synthesizing a thorough answer from comprehensive search tree exploration.

Original query: "{query}"

Best path through the search tree:
{path_summary}

Your task: Synthesize a detailed, well-structured answer.

SYNTHESIS REQUIREMENTS:
1. DIRECT ANSWER: Start with a clear answer to the query
2. SUPPORTING EVIDENCE: Reference specific findings from the search path
3. STRUCTURE: Organize information logically (by component, by flow, or by importance)
4. CONNECTIONS: Highlight relationships between discovered elements
5. COMPLETENESS: Address all aspects of the original query

The answer should include:
- Executive summary (2-3 sentences)
- Detailed findings with evidence
- Key architectural or design insights if relevant
- Any caveats or limitations discovered

Respond with a JSON object:
{{
  "answer": "Detailed, structured answer to the query",
  "findings": "Comprehensive findings organized by theme",
  "steps_taken": "{steps}"
}}

Provide a thorough, professional response."#;

/// EXPLORATORY synthesis (Massive tier): Exhaustive answer with full analysis
pub const SYNTHESIS_EXPLORATORY: &str = r#"You are synthesizing an exhaustive answer from comprehensive search tree exploration.

Original query: "{query}"

Best path through the search tree:
{path_summary}

Your task: Synthesize an exhaustive, architecturally-aware answer.

COMPREHENSIVE SYNTHESIS REQUIREMENTS:
1. EXECUTIVE SUMMARY: Clear, actionable answer to the query
2. DETAILED ANALYSIS: Deep dive into each discovered aspect
3. ARCHITECTURAL CONTEXT: How findings fit into the broader system
4. DEPENDENCY MAPPING: Relationships, call chains, and coupling analysis
5. PATTERN RECOGNITION: Design patterns, anti-patterns, or conventions identified
6. QUALITY ASSESSMENT: Coupling metrics, hub nodes, and structural health
7. RECOMMENDATIONS: Actionable insights based on the analysis

Structure your answer:
1. Direct answer (what was found)
2. Architectural overview (how it fits together)
3. Detailed component analysis
4. Dependency and coupling insights
5. Key patterns and conventions
6. Potential concerns or areas for attention

Respond with a JSON object:
{{
  "answer": "Exhaustive, architecturally-complete answer",
  "findings": "Comprehensive findings with full technical depth",
  "steps_taken": "{steps}"
}}

Provide the most thorough, professional analysis possible."#;

// =============================================================================
// PROMPT SELECTION HELPERS
// =============================================================================

/// Get expansion prompt template for the given tier
pub fn get_expansion_prompt(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => EXPANSION_TERSE,
        ContextTier::Medium => EXPANSION_BALANCED,
        ContextTier::Large => EXPANSION_DETAILED,
        ContextTier::Massive => EXPANSION_EXPLORATORY,
    }
}

/// Get synthesis prompt template for the given tier
pub fn get_synthesis_prompt(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => SYNTHESIS_TERSE,
        ContextTier::Medium => SYNTHESIS_BALANCED,
        ContextTier::Large => SYNTHESIS_DETAILED,
        ContextTier::Massive => SYNTHESIS_EXPLORATORY,
    }
}

/// Get recommended max actions per expansion based on tier
pub fn max_actions_for_tier(tier: ContextTier) -> usize {
    match tier {
        ContextTier::Small => 1,
        ContextTier::Medium => 2,
        ContextTier::Large => 3,
        ContextTier::Massive => 4,
    }
}

/// Build expansion prompt with context filled in
pub fn build_expansion_prompt(
    tier: ContextTier,
    node: &SearchNode,
    query: &str,
    available_tools: &[String],
) -> String {
    let template = get_expansion_prompt(tier);
    let tools_list = available_tools.join(", ");

    // Build action context
    let action_context = if let Some(ref action) = node.action {
        format!(
            "  Previous action: {} - {}",
            action.tool_name, action.reasoning
        )
    } else {
        String::new()
    };

    // Build observation context
    let observation_context = if let Some(ref obs) = node.observation {
        let obs_str = obs.to_string();
        let preview = if obs_str.len() > 300 {
            format!("{}...", &obs_str[..300])
        } else {
            obs_str
        };
        format!("  Observation: {}", preview)
    } else {
        String::new()
    };

    template
        .replace("{query}", query)
        .replace("{depth}", &node.depth.to_string())
        .replace("{thought}", &node.thought)
        .replace("{action_context}", &action_context)
        .replace("{observation_context}", &observation_context)
        .replace("{tools}", &tools_list)
}

/// Build synthesis prompt with path summary filled in
pub fn build_synthesis_prompt(
    tier: ContextTier,
    path_nodes: &[&SearchNode],
    query: &str,
) -> String {
    let template = get_synthesis_prompt(tier);

    // Build path summary with tier-appropriate detail
    let max_obs_len = match tier {
        ContextTier::Small => 200,
        ContextTier::Medium => 500,
        ContextTier::Large => 800,
        ContextTier::Massive => 1200,
    };

    let mut path_summary = String::new();
    for (i, node) in path_nodes.iter().enumerate() {
        path_summary.push_str(&format!("\nStep {}:\n", i + 1));
        path_summary.push_str(&format!("  Thought: {}\n", node.thought));

        if let Some(ref action) = node.action {
            path_summary.push_str(&format!(
                "  Action: {} - {}\n",
                action.tool_name, action.reasoning
            ));
        }

        if let Some(ref obs) = node.observation {
            let obs_str = obs.to_string();
            let preview = if obs_str.len() > max_obs_len {
                format!("{}...", &obs_str[..max_obs_len])
            } else {
                obs_str
            };
            path_summary.push_str(&format!("  Result: {}\n", preview));
        }
    }

    template
        .replace("{query}", query)
        .replace("{path_summary}", &path_summary)
        .replace("{steps}", &path_nodes.len().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autoagents::lats::search_tree::ToolAction;

    #[test]
    fn test_get_expansion_prompt_by_tier() {
        assert!(get_expansion_prompt(ContextTier::Small).contains("exactly 1 action"));
        assert!(get_expansion_prompt(ContextTier::Medium).contains("1-2"));
        assert!(get_expansion_prompt(ContextTier::Large).contains("1-3"));
        assert!(get_expansion_prompt(ContextTier::Massive).contains("1-4"));
    }

    #[test]
    fn test_get_synthesis_prompt_by_tier() {
        assert!(get_synthesis_prompt(ContextTier::Small).contains("Concise"));
        assert!(get_synthesis_prompt(ContextTier::Medium).contains("Comprehensive"));
        assert!(get_synthesis_prompt(ContextTier::Large).contains("SYNTHESIS REQUIREMENTS"));
        assert!(get_synthesis_prompt(ContextTier::Massive).contains("COMPREHENSIVE SYNTHESIS"));
    }

    #[test]
    fn test_max_actions_for_tier() {
        assert_eq!(max_actions_for_tier(ContextTier::Small), 1);
        assert_eq!(max_actions_for_tier(ContextTier::Medium), 2);
        assert_eq!(max_actions_for_tier(ContextTier::Large), 3);
        assert_eq!(max_actions_for_tier(ContextTier::Massive), 4);
    }

    #[test]
    fn test_build_expansion_prompt() {
        let node = SearchNode::new_root("Initial analysis".to_string());
        let tools = vec!["tool1".to_string(), "tool2".to_string()];

        let prompt = build_expansion_prompt(ContextTier::Small, &node, "test query", &tools);
        assert!(prompt.contains("test query"));
        assert!(prompt.contains("tool1, tool2"));
        assert!(prompt.contains("exactly 1 action"));
    }

    #[test]
    fn test_build_synthesis_prompt() {
        let node1 = SearchNode::new_root("First thought".to_string());
        let mut node2 = SearchNode::new_root("Second thought".to_string());
        node2.action = Some(ToolAction {
            tool_name: "test_tool".to_string(),
            parameters: serde_json::json!({}),
            reasoning: "Test reasoning".to_string(),
        });

        let nodes = vec![&node1, &node2];
        let prompt = build_synthesis_prompt(ContextTier::Medium, &nodes, "test query");

        assert!(prompt.contains("test query"));
        assert!(prompt.contains("First thought"));
        assert!(prompt.contains("Second thought"));
        assert!(prompt.contains("test_tool"));
    }
}
