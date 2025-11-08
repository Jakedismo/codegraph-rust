// Integration test for semantic_question prompts

#[cfg(feature = "ai-enhanced")]
mod tests {
    use codegraph_mcp::context_aware_limits::ContextTier;
    use codegraph_mcp::prompt_selector::{AnalysisType, PromptSelector};

    #[test]
    fn test_semantic_question_terse_prompt_loads() {
        let selector = PromptSelector::new();
        let prompt = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Small)
            .expect("Should have terse prompt");

        assert!(!prompt.is_empty(), "Prompt should not be empty");
        assert!(
            prompt.contains("ZERO HEURISTICS"),
            "Should emphasize zero heuristics approach"
        );
        assert!(
            prompt.contains("get_transitive_dependencies"),
            "Should list available tools"
        );
        assert!(
            prompt.contains("TERSE TIER GUIDANCE"),
            "Should have terse-specific guidance"
        );
        assert!(
            prompt.contains(r#""reasoning""#),
            "Should specify JSON response format"
        );
        assert!(
            prompt.contains("1-2 targeted tool calls"),
            "Should specify tool call limits for terse tier"
        );
    }

    #[test]
    fn test_semantic_question_balanced_prompt_loads() {
        let selector = PromptSelector::new();
        let prompt = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("Should have balanced prompt");

        assert!(!prompt.is_empty(), "Prompt should not be empty");
        assert!(
            prompt.contains("ZERO HEURISTICS"),
            "Should emphasize zero heuristics approach"
        );
        assert!(
            prompt.contains("BALANCED TIER GUIDANCE"),
            "Should have balanced-specific guidance"
        );
        assert!(
            prompt.contains("2-4 targeted tool calls"),
            "Should specify tool call range for balanced tier"
        );
        assert!(
            prompt.contains("INVESTIGATION PATTERNS"),
            "Should include investigation patterns"
        );
    }

    #[test]
    fn test_semantic_question_detailed_prompt_loads() {
        let selector = PromptSelector::new();
        let prompt = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("Should have detailed prompt");

        assert!(!prompt.is_empty(), "Prompt should not be empty");
        assert!(
            prompt.contains("ZERO HEURISTICS"),
            "Should emphasize zero heuristics approach"
        );
        assert!(
            prompt.contains("DETAILED TIER GUIDANCE"),
            "Should have detailed-specific guidance"
        );
        assert!(
            prompt.contains("4-7 strategic tool calls"),
            "Should specify tool call range for detailed tier"
        );
        assert!(
            prompt.contains("MULTI-ANGLE INVESTIGATION"),
            "Should emphasize multi-angle investigation"
        );
        assert!(
            prompt.contains("CROSS-VERIFICATION"),
            "Should include cross-verification strategies"
        );
    }

    #[test]
    fn test_semantic_question_exploratory_prompt_loads() {
        let selector = PromptSelector::new();
        let prompt = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("Should have exploratory prompt");

        assert!(!prompt.is_empty(), "Prompt should not be empty");
        assert!(
            prompt.contains("ZERO HEURISTICS"),
            "Should emphasize zero heuristics approach"
        );
        assert!(
            prompt.contains("EXPLORATORY TIER"),
            "Should have exploratory-specific guidance"
        );
        assert!(
            prompt.contains("7-12+ tool calls"),
            "Should specify extensive tool call range"
        );
        assert!(
            prompt.contains("Phase 1"),
            "Should include multi-phase investigation strategy"
        );
        assert!(
            prompt.contains("STATISTICAL ANALYSIS"),
            "Should include statistical analysis techniques"
        );
        assert!(
            prompt.contains("CONFIDENCE CALCULATION"),
            "Should include confidence calculation guidance"
        );
    }

    #[test]
    fn test_all_semantic_question_prompts_have_critical_elements() {
        let selector = PromptSelector::new();
        let tiers = vec![
            ContextTier::Small,
            ContextTier::Medium,
            ContextTier::Large,
            ContextTier::Massive,
        ];

        for tier in tiers {
            let prompt = selector
                .select_prompt(AnalysisType::SemanticQuestion, tier)
                .expect(&format!("Should have prompt for tier {:?}", tier));

            // All prompts must have these critical elements
            assert!(
                prompt.contains("ZERO HEURISTICS"),
                "Tier {:?}: Must emphasize zero heuristics",
                tier
            );
            assert!(
                prompt.contains("AVAILABLE GRAPH TOOLS"),
                "Tier {:?}: Must list available tools",
                tier
            );
            assert!(
                prompt.contains("get_transitive_dependencies"),
                "Tier {:?}: Must include get_transitive_dependencies",
                tier
            );
            assert!(
                prompt.contains("detect_circular_dependencies"),
                "Tier {:?}: Must include detect_circular_dependencies",
                tier
            );
            assert!(
                prompt.contains("trace_call_chain"),
                "Tier {:?}: Must include trace_call_chain",
                tier
            );
            assert!(
                prompt.contains("calculate_coupling_metrics"),
                "Tier {:?}: Must include calculate_coupling_metrics",
                tier
            );
            assert!(
                prompt.contains("get_hub_nodes"),
                "Tier {:?}: Must include get_hub_nodes",
                tier
            );
            assert!(
                prompt.contains("get_reverse_dependencies"),
                "Tier {:?}: Must include get_reverse_dependencies",
                tier
            );
            assert!(
                prompt.contains("RESPONSE FORMAT"),
                "Tier {:?}: Must specify response format",
                tier
            );
            assert!(
                prompt.contains(r#""reasoning""#),
                "Tier {:?}: Must include JSON reasoning field",
                tier
            );
            assert!(
                prompt.contains(r#""tool_call""#),
                "Tier {:?}: Must include JSON tool_call field",
                tier
            );
            assert!(
                prompt.contains(r#""is_final""#),
                "Tier {:?}: Must include JSON is_final field",
                tier
            );
        }
    }

    #[test]
    fn test_prompts_increase_in_detail_by_tier() {
        let selector = PromptSelector::new();

        let terse = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Small)
            .expect("Should have terse prompt");

        let balanced = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("Should have balanced prompt");

        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("Should have detailed prompt");

        let exploratory = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("Should have exploratory prompt");

        // Prompts should increase in length/detail
        assert!(
            balanced.len() > terse.len(),
            "Balanced should be longer than terse"
        );
        assert!(
            detailed.len() > balanced.len(),
            "Detailed should be longer than balanced"
        );
        assert!(
            exploratory.len() > detailed.len(),
            "Exploratory should be longest"
        );

        // Each tier should have unique guidance
        assert!(
            terse.contains("1-2") && !balanced.contains("1-2"),
            "Terse should have unique tool call guidance"
        );
        assert!(
            balanced.contains("2-4") && !detailed.contains("2-4"),
            "Balanced should have unique tool call guidance"
        );
        assert!(
            detailed.contains("4-7") && !exploratory.contains("4-7"),
            "Detailed should have unique tool call guidance"
        );
        assert!(
            exploratory.contains("7-12+"),
            "Exploratory should mention 7-12+ tool calls"
        );
    }

    #[test]
    fn test_question_type_mapping_present_in_terse() {
        let selector = PromptSelector::new();
        let prompt = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Small)
            .expect("Should have terse prompt");

        // Terse should have quick question type mappings
        assert!(
            prompt.contains("QUESTION TYPE MAPPING"),
            "Should have question type mapping section"
        );
        assert!(
            prompt.contains("How does X work?"),
            "Should map 'how' questions"
        );
        assert!(
            prompt.contains("What depends on X?"),
            "Should map 'what depends' questions"
        );
        assert!(
            prompt.contains("Why does X depend on Y?"),
            "Should map 'why' questions"
        );
        assert!(
            prompt.contains("What if X changes?"),
            "Should map 'what if' questions"
        );
    }

    #[test]
    fn test_investigation_patterns_present_in_balanced_and_above() {
        let selector = PromptSelector::new();

        let balanced = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Medium)
            .expect("Should have balanced prompt");

        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("Should have detailed prompt");

        for (tier_name, prompt) in [("balanced", balanced), ("detailed", detailed)] {
            assert!(
                prompt.contains("INVESTIGATION PATTERNS")
                    || prompt.contains("INVESTIGATION STRATEGY"),
                "{}: Should have investigation patterns/strategy",
                tier_name
            );
            assert!(
                prompt.contains("For \"How does X work?\""),
                "{}: Should have 'how' investigation pattern",
                tier_name
            );
            assert!(
                prompt.contains("For \"What if X changes?\""),
                "{}: Should have 'what if' investigation pattern",
                tier_name
            );
        }
    }

    #[test]
    fn test_confidence_scoring_in_detailed_and_exploratory() {
        let selector = PromptSelector::new();

        let detailed = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Large)
            .expect("Should have detailed prompt");

        let exploratory = selector
            .select_prompt(AnalysisType::SemanticQuestion, ContextTier::Massive)
            .expect("Should have exploratory prompt");

        assert!(
            detailed.contains("Confidence") || detailed.contains("confidence"),
            "Detailed should mention confidence"
        );
        assert!(
            exploratory.contains("CONFIDENCE"),
            "Exploratory should have confidence section"
        );
        assert!(
            exploratory.contains("0.0-1.0"),
            "Exploratory should specify confidence range"
        );
    }
}
