// ABOUTME: Example integration of dependency analysis prompts into PromptSelector
// ABOUTME: Shows how to replace placeholder prompts with tier-optimized dependency analysis prompts

use crate::dependency_analysis_prompts::*;
use crate::prompt_selector::{AnalysisType, PromptSelector, PromptVerbosity};
use codegraph_mcp_core::context_aware_limits::ContextTier;

/// Replace the default placeholder prompts with optimized dependency analysis prompts
pub fn register_dependency_analysis_prompts(selector: &mut PromptSelector) {
    // TERSE tier (Small context window)
    selector.register_prompt(
        AnalysisType::DependencyAnalysis,
        PromptVerbosity::Terse,
        DEPENDENCY_ANALYSIS_TERSE.to_string(),
    );

    // BALANCED tier (Medium context window)
    selector.register_prompt(
        AnalysisType::DependencyAnalysis,
        PromptVerbosity::Balanced,
        DEPENDENCY_ANALYSIS_BALANCED.to_string(),
    );

    // DETAILED tier (Large context window)
    selector.register_prompt(
        AnalysisType::DependencyAnalysis,
        PromptVerbosity::Detailed,
        DEPENDENCY_ANALYSIS_DETAILED.to_string(),
    );

    // EXPLORATORY tier (Massive context window)
    selector.register_prompt(
        AnalysisType::DependencyAnalysis,
        PromptVerbosity::Exploratory,
        DEPENDENCY_ANALYSIS_EXPLORATORY.to_string(),
    );
}

/// Helper function to get the appropriate dependency analysis prompt for a given tier
pub fn get_dependency_analysis_prompt(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => DEPENDENCY_ANALYSIS_TERSE,
        ContextTier::Medium => DEPENDENCY_ANALYSIS_BALANCED,
        ContextTier::Large => DEPENDENCY_ANALYSIS_DETAILED,
        ContextTier::Massive => DEPENDENCY_ANALYSIS_EXPLORATORY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_all_dependency_prompts() {
        let mut selector = PromptSelector::new();

        // Register dependency analysis prompts
        register_dependency_analysis_prompts(&mut selector);

        // Verify all tiers are registered
        for tier in [
            ContextTier::Small,
            ContextTier::Medium,
            ContextTier::Large,
            ContextTier::Massive,
        ] {
            let prompt = selector
                .select_prompt(AnalysisType::DependencyAnalysis, tier)
                .expect("Dependency prompt should be registered");

            // Verify it's not the placeholder
            assert!(
                !prompt.contains("placeholder prompt"),
                "Should not be placeholder for tier {:?}",
                tier
            );

            // Verify it contains expected content
            assert!(
                prompt.contains("dependency"),
                "Should mention dependency for tier {:?}",
                tier
            );
            assert!(
                prompt.contains("tool_call"),
                "Should include JSON format for tier {:?}",
                tier
            );
        }
    }

    #[test]
    fn test_tier_appropriate_depth() {
        // TERSE should limit depth
        assert!(DEPENDENCY_ANALYSIS_TERSE.contains("depth=1 or 2 max"));

        // BALANCED should use moderate depth
        assert!(DEPENDENCY_ANALYSIS_BALANCED.contains("depth=2-3"));

        // DETAILED should use deeper analysis
        assert!(DEPENDENCY_ANALYSIS_DETAILED.contains("depth=3-5"));

        // EXPLORATORY should use maximum depth
        assert!(DEPENDENCY_ANALYSIS_EXPLORATORY.contains("depth=5-10"));
    }

    #[test]
    fn test_tool_call_requirement() {
        for prompt in [
            DEPENDENCY_ANALYSIS_TERSE,
            DEPENDENCY_ANALYSIS_BALANCED,
            DEPENDENCY_ANALYSIS_DETAILED,
            DEPENDENCY_ANALYSIS_EXPLORATORY,
        ] {
            // All prompts should require tool calls before final analysis
            assert!(
                prompt.contains("tool call") || prompt.contains("tool_call"),
                "Prompt should mention tool calls"
            );
        }
    }

    #[test]
    fn test_zero_heuristics_requirement() {
        for prompt in [
            DEPENDENCY_ANALYSIS_TERSE,
            DEPENDENCY_ANALYSIS_BALANCED,
            DEPENDENCY_ANALYSIS_DETAILED,
            DEPENDENCY_ANALYSIS_EXPLORATORY,
        ] {
            // All prompts should emphasize zero heuristics
            assert!(
                prompt.contains("ZERO HEURISTICS") || prompt.contains("NO HEURISTICS"),
                "Prompt should enforce zero heuristics"
            );
        }
    }

    #[test]
    fn test_all_tools_mentioned() {
        let expected_tools = [
            "get_transitive_dependencies",
            "detect_circular_dependencies",
            "trace_call_chain",
            "calculate_coupling_metrics",
            "get_hub_nodes",
            "get_reverse_dependencies",
        ];

        for prompt in [
            DEPENDENCY_ANALYSIS_TERSE,
            DEPENDENCY_ANALYSIS_BALANCED,
            DEPENDENCY_ANALYSIS_DETAILED,
            DEPENDENCY_ANALYSIS_EXPLORATORY,
        ] {
            for tool in &expected_tools {
                assert!(
                    prompt.contains(tool),
                    "Prompt should mention tool: {}",
                    tool
                );
            }
        }
    }

    #[test]
    fn test_tier_specific_guidance() {
        // TERSE should emphasize brevity
        assert!(DEPENDENCY_ANALYSIS_TERSE.contains("Limit tool calls to 3-5 total"));

        // BALANCED should emphasize systematic approach
        assert!(DEPENDENCY_ANALYSIS_BALANCED.contains("SYSTEMATIC APPROACH"));

        // DETAILED should emphasize comprehensiveness
        assert!(DEPENDENCY_ANALYSIS_DETAILED.contains("COMPREHENSIVE"));

        // EXPLORATORY should emphasize exhaustiveness
        assert!(DEPENDENCY_ANALYSIS_EXPLORATORY.contains("EXHAUSTIVE"));
    }
}

/// Example usage in agentic orchestrator
#[cfg(test)]
mod integration_examples {
    use super::*;

    #[test]
    fn example_terse_usage() {
        // Example: Small tier model analyzing simple dependency question
        let tier = ContextTier::Small;
        let prompt = get_dependency_analysis_prompt(tier);

        // Terse prompt guides LLM to:
        // 1. Use minimal tool calls (3-5 max)
        // 2. Focus on immediate dependencies (depth=1-2)
        // 3. Provide direct, actionable answers
        assert!(prompt.contains("3-5 total"));
    }

    #[test]
    fn example_balanced_usage() {
        // Example: Medium tier model for production analysis
        let tier = ContextTier::Medium;
        let prompt = get_dependency_analysis_prompt(tier);

        // Balanced prompt guides LLM to:
        // 1. Systematic multi-tool analysis (5-10 calls)
        // 2. Analyze both forward and reverse dependencies
        // 3. Include coupling metrics and circular dependency checks
        assert!(prompt.contains("5-10 tool calls"));
    }

    #[test]
    fn example_detailed_usage() {
        // Example: Large tier model for architectural analysis
        let tier = ContextTier::Large;
        let prompt = get_dependency_analysis_prompt(tier);

        // Detailed prompt guides LLM to:
        // 1. Comprehensive multi-level mapping (10-15 calls)
        // 2. Deep dependency trees (depth=3-5)
        // 3. Complete refactoring roadmap
        assert!(prompt.contains("10-15 tool calls"));
    }

    #[test]
    fn example_exploratory_usage() {
        // Example: Massive tier model for codebase-wide analysis
        let tier = ContextTier::Massive;
        let prompt = get_dependency_analysis_prompt(tier);

        // Exploratory prompt guides LLM to:
        // 1. Exhaustive multi-dimensional exploration (15-20+ calls)
        // 2. Statistical analysis and pattern detection
        // 3. Complete architectural topology with metrics
        assert!(prompt.contains("15-20"));
    }
}
