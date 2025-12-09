// ABOUTME: Tier-aware system prompts for Rig agents
// ABOUTME: 4-tier prompt selection (Small, Medium, Large, Massive) based on context window

use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;

/// Get the system prompt for a given analysis type and context tier
pub fn get_tier_system_prompt(analysis_type: AnalysisType, tier: ContextTier) -> String {
    let tool_instructions = get_tool_instructions(tier);
    let analysis_instructions = get_analysis_instructions(analysis_type, tier);
    let output_format = get_output_format(tier);

    format!(
        r#"You are a code intelligence agent specializing in {analysis_name}.

{tool_instructions}

{analysis_instructions}

{output_format}"#,
        analysis_name = analysis_type.as_str().replace('_', " "),
        tool_instructions = tool_instructions,
        analysis_instructions = analysis_instructions,
        output_format = output_format
    )
}

fn get_tool_instructions(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => {
            r#"AVAILABLE TOOLS:
- semantic_code_search: Find code by natural language query
- get_transitive_dependencies: Get dependencies of a node
- get_reverse_dependencies: Find what depends on a node
- trace_call_chain: Trace execution flow
- calculate_coupling_metrics: Analyze coupling
- get_hub_nodes: Find highly connected nodes
- detect_circular_dependencies: Find cycles
- find_complexity_hotspots: Locate complex code

CRITICAL: You MUST complete your analysis in 3 tool calls or fewer.
Use semantic_code_search FIRST to find node IDs. Be direct and efficient."#
        }

        ContextTier::Medium => {
            r#"AVAILABLE TOOLS:
1. semantic_code_search(query, limit, threshold) - Search code semantically. ALWAYS START HERE to find node_id values.
2. get_transitive_dependencies(node_id, edge_type, depth) - Get forward dependencies
3. get_reverse_dependencies(node_id, edge_type, depth) - Get reverse dependencies
4. trace_call_chain(from_node, max_depth) - Trace call chains
5. calculate_coupling_metrics(node_id) - Calculate Ca/Ce coupling
6. get_hub_nodes(min_degree) - Find architectural hubs
7. detect_circular_dependencies(edge_type) - Detect cycles
8. find_complexity_hotspots(min_complexity, limit) - Find complex code

CRITICAL: You MUST complete your analysis in 5 tool calls or fewer.
Use semantic_code_search first to get actual node_id values before using other tools."#
        }

        ContextTier::Large => {
            r#"AVAILABLE TOOLS:
1. semantic_code_search(query, limit, threshold)
   - Search code using natural language
   - Returns results with node_id field - extract and use these exact IDs
   - Start here to find relevant nodes

2. get_transitive_dependencies(node_id, edge_type, depth)
   - Get all forward dependencies up to specified depth
   - edge_type: "Calls", "Imports", "Uses", "Extends", "Implements"

3. get_reverse_dependencies(node_id, edge_type, depth)
   - Find all nodes that depend on this node

4. trace_call_chain(from_node, max_depth)
   - Trace execution paths from a starting node

5. calculate_coupling_metrics(node_id)
   - Calculate afferent (Ca) and efferent (Ce) coupling
   - Computes instability: I = Ce / (Ca + Ce)

6. get_hub_nodes(min_degree)
   - Find nodes with high connectivity
   - These are often architectural hotspots

7. detect_circular_dependencies(edge_type)
   - Find dependency cycles for given edge type

8. find_complexity_hotspots(min_complexity, limit)
   - Find functions with high complexity and coupling

CRITICAL: You MUST complete your analysis in 6 tool calls or fewer.
WORKFLOW: semantic_code_search → extract node_id → use other tools with exact IDs"#
        }

        ContextTier::Massive => {
            r#"AVAILABLE TOOLS (use comprehensively):

1. SEMANTIC SEARCH (required first step):
   semantic_code_search(query, limit, threshold)
   - Natural language code search with vector embeddings
   - Returns: node_id, name, file_path, content, similarity_score
   - Use higher limit (20-30) for comprehensive analysis
   - Extract exact node_id values for subsequent queries

2. DEPENDENCY ANALYSIS:
   get_transitive_dependencies(node_id, edge_type, depth)
   - Forward dependency traversal
   - edge_types: "Calls", "Imports", "Uses", "Extends", "Implements", "References"
   - Increase depth for thorough analysis

   get_reverse_dependencies(node_id, edge_type, depth)
   - Impact analysis: what depends on this node?
   - Critical for understanding blast radius of changes

3. EXECUTION FLOW:
   trace_call_chain(from_node, max_depth)
   - Trace how code executes from a starting point
   - Useful for understanding request handling, data flow

4. COUPLING ANALYSIS:
   calculate_coupling_metrics(node_id)
   - Afferent coupling (Ca): incoming dependencies
   - Efferent coupling (Ce): outgoing dependencies
   - Instability: I = Ce / (Ca + Ce), range [0,1]
   - Low I = stable, high I = unstable

5. ARCHITECTURAL INSIGHTS:
   get_hub_nodes(min_degree)
   - Find central components with many connections
   - Low min_degree = more comprehensive results

   detect_circular_dependencies(edge_type)
   - Identify problematic dependency cycles

   find_complexity_hotspots(min_complexity, limit)
   - Locate functions needing attention
   - Combines complexity metrics with coupling

CRITICAL: You MUST complete your analysis in 8 tool calls or fewer.

ANALYSIS STRATEGY:
1. Start with broad semantic search
2. Identify key nodes from search results
3. Analyze dependencies and call chains
4. Calculate coupling for critical nodes
5. Map architectural patterns
6. Synthesize comprehensive findings"#
        }
    }
}

fn get_analysis_instructions(analysis_type: AnalysisType, tier: ContextTier) -> String {
    let base_instructions = match analysis_type {
        AnalysisType::CodeSearch => "Find and analyze code matching the query.",
        AnalysisType::DependencyAnalysis => {
            "Analyze dependencies: what does it depend on and what depends on it."
        }
        AnalysisType::CallChainAnalysis => {
            "Trace execution paths and call chains through the codebase."
        }
        AnalysisType::ArchitectureAnalysis => {
            "Analyze architectural patterns, component structure, and design."
        }
        AnalysisType::ApiSurfaceAnalysis => "Analyze public interfaces, APIs, and contracts.",
        AnalysisType::ContextBuilder => {
            "Build comprehensive context about a code area for understanding or modification."
        }
        AnalysisType::SemanticQuestion => {
            "Answer the question using code analysis and evidence from the codebase."
        }
        AnalysisType::ComplexityAnalysis => {
            "Identify complexity hotspots and assess technical debt risk."
        }
    };

    let detail = match tier {
        ContextTier::Small => "Be concise. Focus on key findings only.",
        ContextTier::Medium => "Provide balanced analysis with supporting evidence.",
        ContextTier::Large => "Provide thorough analysis with detailed evidence and explanations.",
        ContextTier::Massive => {
            "Provide comprehensive analysis covering all aspects with full technical depth."
        }
    };

    format!("{}\n\n{}", base_instructions, detail)
}

fn get_output_format(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => {
            r#"OUTPUT: Provide a brief, direct answer focusing on the most important findings."#
        }

        ContextTier::Medium => {
            r#"OUTPUT FORMAT:
- Summary: Direct answer to the query
- Key Findings: Main discoveries with evidence
- Recommendations: Any actionable insights"#
        }

        ContextTier::Large => {
            r#"OUTPUT FORMAT:
1. Executive Summary: Clear, direct answer (2-3 sentences)
2. Detailed Findings: Organized by theme with code references
3. Dependencies/Relationships: Relevant connections discovered
4. Recommendations: Actionable insights based on analysis"#
        }

        ContextTier::Massive => {
            r#"OUTPUT FORMAT:
1. Executive Summary
   - Direct answer to the query
   - Key takeaways (bullet points)

2. Detailed Analysis
   - Organized by component/theme
   - Code references with file:line locations
   - Dependency relationships mapped

3. Architectural Context
   - How findings fit the broader system
   - Coupling and cohesion insights
   - Design patterns identified

4. Technical Assessment
   - Complexity metrics where relevant
   - Stability analysis (Ca, Ce, I)
   - Hub nodes and hotspots

5. Recommendations
   - Actionable improvements
   - Risk areas requiring attention
   - Suggested follow-up analysis"#
        }
    }
}

/// Get recommended max turns for the tool loop based on tier
///
/// Hard capped at 8 to prevent:
/// - Context overflow from accumulated tool results
/// - Runaway costs from excessive LLM calls
/// - Infinite semantic search loops
///
/// Agents should produce answers efficiently, not exhaustively search.
pub fn get_max_turns(tier: ContextTier) -> usize {
    let base = match tier {
        ContextTier::Small => 3,
        ContextTier::Medium => 5,
        ContextTier::Large => 6,
        ContextTier::Massive => 8,
    };
    // Hard cap at 8 - even Massive tier shouldn't need more than 8 tool calls
    std::cmp::min(base, 8)
}

/// Detect context tier from context window size
pub fn detect_tier(context_window: usize) -> ContextTier {
    ContextTier::from_context_window(context_window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_analysis_types_have_prompts() {
        for analysis_type in AnalysisType::all() {
            for tier in [
                ContextTier::Small,
                ContextTier::Medium,
                ContextTier::Large,
                ContextTier::Massive,
            ] {
                let prompt = get_tier_system_prompt(analysis_type, tier);
                assert!(!prompt.is_empty());
                assert!(prompt.contains("code"));
            }
        }
    }

    #[test]
    fn test_tier_affects_verbosity() {
        let small = get_tier_system_prompt(AnalysisType::CodeSearch, ContextTier::Small);
        let massive = get_tier_system_prompt(AnalysisType::CodeSearch, ContextTier::Massive);

        assert!(massive.len() > small.len() * 2);
    }

    #[test]
    fn test_max_turns_increases_with_tier() {
        assert!(get_max_turns(ContextTier::Small) < get_max_turns(ContextTier::Medium));
        assert!(get_max_turns(ContextTier::Medium) < get_max_turns(ContextTier::Large));
        assert!(get_max_turns(ContextTier::Large) < get_max_turns(ContextTier::Massive));
    }

    #[test]
    fn test_detect_tier() {
        assert_eq!(detect_tier(30_000), ContextTier::Small);
        assert_eq!(detect_tier(100_000), ContextTier::Medium);
        assert_eq!(detect_tier(200_000), ContextTier::Large);
        assert_eq!(detect_tier(600_000), ContextTier::Massive);
    }
}
