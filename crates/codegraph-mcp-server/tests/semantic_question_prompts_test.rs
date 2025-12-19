// ABOUTME: Validates semantic-question prompt content for different context tiers.
// ABOUTME: Ensures prompts include tool guidance and investigation patterns.

// Integration test for semantic_question prompts

#[cfg(feature = "ai-enhanced")]
mod tests {
    use codegraph_mcp_core::context_aware_limits::ContextTier;
    use codegraph_mcp_server::prompt_selector::{AnalysisType, PromptSelector};
    use codegraph_mcp_server::semantic_question_prompts::{
        SEMANTIC_QUESTION_BALANCED, SEMANTIC_QUESTION_DETAILED, SEMANTIC_QUESTION_EXPLORATORY,
        SEMANTIC_QUESTION_TERSE,
    };

    #[test]
    fn selects_exact_prompt_per_tier() {
        let selector = PromptSelector::new();
        let terse = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Small)
            .expect("terse");
        let balanced = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("balanced");
        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("detailed");
        let exploratory = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("exploratory");

        assert_eq!(terse, SEMANTIC_QUESTION_TERSE);
        assert_eq!(balanced, SEMANTIC_QUESTION_BALANCED);
        assert_eq!(detailed, SEMANTIC_QUESTION_DETAILED);
        assert_eq!(exploratory, SEMANTIC_QUESTION_EXPLORATORY);
    }

    #[test]
    fn prompts_include_mandatory_workflow_and_tools() {
        let selector = PromptSelector::new();

        for tier in [
            ContextTier::Small,
            ContextTier::Medium,
            ContextTier::Large,
            ContextTier::Massive,
        ] {
            let prompt = selector
                .select_prompt(AnalysisType::SemanticQuestion, tier)
                .expect("prompt");
            assert!(prompt.contains("ZERO HEURISTICS"));
            assert!(prompt.contains("semantic_code_search"));
            assert!(prompt.contains("MANDATORY WORKFLOW"));
        }
    }

    #[test]
    fn prompts_include_tier_tool_call_limits() {
        let selector = PromptSelector::new();

        let terse = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Small)
            .expect("terse");
        let balanced = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("balanced");
        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("detailed");
        let exploratory = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("exploratory");

        assert!(terse.contains("Make 1-2 targeted tool calls maximum"));
        assert!(balanced.contains("Make 2-4 targeted tool calls"));
        assert!(detailed.contains("Make 4-7 strategic tool calls"));
        assert!(exploratory.contains("Aim for 7-12 tool calls"));
    }

    #[test]
    fn balanced_and_above_include_investigation_patterns() {
        let selector = PromptSelector::new();
        let balanced = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("balanced");
        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("detailed");
        let exploratory = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("exploratory");

        for prompt in [balanced, detailed, exploratory] {
            assert!(prompt.contains("INVESTIGATION"));
            assert!(prompt.contains("How does X work?"));
            assert!(prompt.contains("What if X changes?"));
        }
    }
}
