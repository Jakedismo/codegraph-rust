// ABOUTME: Context-tier aware prompt selector for agentic analysis workflows
// ABOUTME: Selects optimized system prompts based on analysis type and LLM context window size

use crate::agentic_api_surface_prompts::{
    API_SURFACE_BALANCED, API_SURFACE_DETAILED, API_SURFACE_EXPLORATORY, API_SURFACE_TERSE,
};
use crate::architecture_analysis_prompts::{
    ARCHITECTURE_ANALYSIS_BALANCED, ARCHITECTURE_ANALYSIS_DETAILED,
    ARCHITECTURE_ANALYSIS_EXPLORATORY, ARCHITECTURE_ANALYSIS_TERSE,
};
use crate::call_chain_prompts::{
    CALL_CHAIN_BALANCED, CALL_CHAIN_DETAILED, CALL_CHAIN_EXPLORATORY, CALL_CHAIN_TERSE,
};
use crate::code_search_prompts::{
    CODE_SEARCH_BALANCED, CODE_SEARCH_DETAILED, CODE_SEARCH_EXPLORATORY, CODE_SEARCH_TERSE,
};
use crate::context_aware_limits::ContextTier;
use crate::context_builder_prompts::{
    CONTEXT_BUILDER_BALANCED, CONTEXT_BUILDER_DETAILED, CONTEXT_BUILDER_EXPLORATORY,
    CONTEXT_BUILDER_TERSE,
};
use crate::dependency_analysis_prompts::{
    DEPENDENCY_ANALYSIS_BALANCED, DEPENDENCY_ANALYSIS_DETAILED, DEPENDENCY_ANALYSIS_EXPLORATORY,
    DEPENDENCY_ANALYSIS_TERSE,
};
use crate::error::McpError;
use crate::semantic_question_prompts::{
    SEMANTIC_QUESTION_BALANCED, SEMANTIC_QUESTION_DETAILED, SEMANTIC_QUESTION_EXPLORATORY,
    SEMANTIC_QUESTION_TERSE,
};
use crate::Result;
use std::collections::HashMap;
use tracing::debug;

/// Types of code analysis that can be performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalysisType {
    /// Search for code patterns, symbols, or references
    CodeSearch,
    /// Analyze dependency relationships and impact
    DependencyAnalysis,
    /// Trace execution call chains
    CallChainAnalysis,
    /// Analyze architectural patterns and quality
    ArchitectureAnalysis,
    /// Analyze public API surface and contracts
    ApiSurfaceAnalysis,
    /// Build comprehensive context for code generation
    ContextBuilder,
    /// Answer semantic questions about code
    SemanticQuestion,
}

impl AnalysisType {
    /// Get the string identifier for this analysis type
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalysisType::CodeSearch => "code_search",
            AnalysisType::DependencyAnalysis => "dependency_analysis",
            AnalysisType::CallChainAnalysis => "call_chain_analysis",
            AnalysisType::ArchitectureAnalysis => "architecture_analysis",
            AnalysisType::ApiSurfaceAnalysis => "api_surface_analysis",
            AnalysisType::ContextBuilder => "context_builder",
            AnalysisType::SemanticQuestion => "semantic_question",
        }
    }

    /// Parse from string identifier
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "code_search" => Some(AnalysisType::CodeSearch),
            "dependency_analysis" => Some(AnalysisType::DependencyAnalysis),
            "call_chain_analysis" => Some(AnalysisType::CallChainAnalysis),
            "architecture_analysis" => Some(AnalysisType::ArchitectureAnalysis),
            "api_surface_analysis" => Some(AnalysisType::ApiSurfaceAnalysis),
            "context_builder" => Some(AnalysisType::ContextBuilder),
            "semantic_question" => Some(AnalysisType::SemanticQuestion),
            _ => None,
        }
    }

    /// Get all analysis types
    pub fn all() -> Vec<Self> {
        vec![
            AnalysisType::CodeSearch,
            AnalysisType::DependencyAnalysis,
            AnalysisType::CallChainAnalysis,
            AnalysisType::ArchitectureAnalysis,
            AnalysisType::ApiSurfaceAnalysis,
            AnalysisType::ContextBuilder,
            AnalysisType::SemanticQuestion,
        ]
    }
}

/// Verbosity level based on context tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptVerbosity {
    /// Terse prompts for small context windows
    Terse,
    /// Balanced prompts for medium context windows
    Balanced,
    /// Detailed prompts for large context windows
    Detailed,
    /// Exploratory prompts for massive context windows
    Exploratory,
}

impl From<ContextTier> for PromptVerbosity {
    fn from(tier: ContextTier) -> Self {
        match tier {
            ContextTier::Small => PromptVerbosity::Terse,
            ContextTier::Medium => PromptVerbosity::Balanced,
            ContextTier::Large => PromptVerbosity::Detailed,
            ContextTier::Massive => PromptVerbosity::Exploratory,
        }
    }
}

impl PromptVerbosity {
    /// Get string identifier for this verbosity level
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptVerbosity::Terse => "terse",
            PromptVerbosity::Balanced => "balanced",
            PromptVerbosity::Detailed => "detailed",
            PromptVerbosity::Exploratory => "exploratory",
        }
    }
}

/// Prompt selector that chooses appropriate system prompts based on context
pub struct PromptSelector {
    /// In-memory prompt storage: (analysis_type, verbosity) -> prompt_text
    prompts: HashMap<(AnalysisType, PromptVerbosity), String>,
}

impl PromptSelector {
    /// Create a new prompt selector with default prompts
    pub fn new() -> Self {
        let mut selector = Self {
            prompts: HashMap::new(),
        };
        selector.load_default_prompts();
        selector
    }

    /// Select appropriate prompt for given analysis type and context tier
    pub fn select_prompt(&self, analysis_type: AnalysisType, tier: ContextTier) -> Result<&str> {
        let verbosity = PromptVerbosity::from(tier);
        debug!(
            "Selecting prompt: type={:?}, tier={:?}, verbosity={:?}",
            analysis_type, tier, verbosity
        );

        self.prompts
            .get(&(analysis_type, verbosity))
            .map(|s| s.as_str())
            .ok_or_else(|| {
                McpError::Protocol(format!(
                    "No prompt found for analysis_type={:?}, verbosity={:?}",
                    analysis_type, verbosity
                ))
            })
    }

    /// Register a custom prompt for a specific analysis type and verbosity
    pub fn register_prompt(
        &mut self,
        analysis_type: AnalysisType,
        verbosity: PromptVerbosity,
        prompt: String,
    ) {
        self.prompts.insert((analysis_type, verbosity), prompt);
    }

    /// Get recommended max_steps for a given tier and analysis type
    pub fn recommended_max_steps(&self, tier: ContextTier, analysis_type: AnalysisType) -> usize {
        // Base max_steps from tier
        let base_steps = match tier {
            ContextTier::Small => 5,
            ContextTier::Medium => 10,
            ContextTier::Large => 15,
            ContextTier::Massive => 20,
        };

        // Some analysis types may benefit from more or fewer steps
        let multiplier = match analysis_type {
            AnalysisType::CodeSearch => 0.8,         // Typically quick searches
            AnalysisType::DependencyAnalysis => 1.2, // May need more exploration
            AnalysisType::CallChainAnalysis => 1.0,  // Standard depth
            AnalysisType::ArchitectureAnalysis => 1.5, // Deep architectural analysis
            AnalysisType::ApiSurfaceAnalysis => 1.0, // Standard depth
            AnalysisType::ContextBuilder => 1.3,     // Building comprehensive context
            AnalysisType::SemanticQuestion => 1.0,   // Standard depth
        };

        ((base_steps as f32) * multiplier).ceil() as usize
    }

    /// Load default placeholder prompts for all analysis types and verbosity levels
    ///
    /// These will be replaced by subagent-generated prompts in Phase 2B
    fn load_default_prompts(&mut self) {
        for analysis_type in AnalysisType::all() {
            for verbosity in [
                PromptVerbosity::Terse,
                PromptVerbosity::Balanced,
                PromptVerbosity::Detailed,
                PromptVerbosity::Exploratory,
            ] {
                let prompt = self.generate_default_prompt(analysis_type, verbosity);
                self.register_prompt(analysis_type, verbosity, prompt);
            }
        }
    }

    /// Generate a default prompt (now using specialized prompts for all analysis types)
    fn generate_default_prompt(
        &self,
        analysis_type: AnalysisType,
        verbosity: PromptVerbosity,
    ) -> String {
        // Use specialized prompts for all analysis types
        match analysis_type {
            AnalysisType::CodeSearch => match verbosity {
                PromptVerbosity::Terse => CODE_SEARCH_TERSE.to_string(),
                PromptVerbosity::Balanced => CODE_SEARCH_BALANCED.to_string(),
                PromptVerbosity::Detailed => CODE_SEARCH_DETAILED.to_string(),
                PromptVerbosity::Exploratory => CODE_SEARCH_EXPLORATORY.to_string(),
            },
            AnalysisType::DependencyAnalysis => match verbosity {
                PromptVerbosity::Terse => DEPENDENCY_ANALYSIS_TERSE.to_string(),
                PromptVerbosity::Balanced => DEPENDENCY_ANALYSIS_BALANCED.to_string(),
                PromptVerbosity::Detailed => DEPENDENCY_ANALYSIS_DETAILED.to_string(),
                PromptVerbosity::Exploratory => DEPENDENCY_ANALYSIS_EXPLORATORY.to_string(),
            },
            AnalysisType::CallChainAnalysis => match verbosity {
                PromptVerbosity::Terse => CALL_CHAIN_TERSE.to_string(),
                PromptVerbosity::Balanced => CALL_CHAIN_BALANCED.to_string(),
                PromptVerbosity::Detailed => CALL_CHAIN_DETAILED.to_string(),
                PromptVerbosity::Exploratory => CALL_CHAIN_EXPLORATORY.to_string(),
            },
            AnalysisType::ArchitectureAnalysis => match verbosity {
                PromptVerbosity::Terse => ARCHITECTURE_ANALYSIS_TERSE.to_string(),
                PromptVerbosity::Balanced => ARCHITECTURE_ANALYSIS_BALANCED.to_string(),
                PromptVerbosity::Detailed => ARCHITECTURE_ANALYSIS_DETAILED.to_string(),
                PromptVerbosity::Exploratory => ARCHITECTURE_ANALYSIS_EXPLORATORY.to_string(),
            },
            AnalysisType::ApiSurfaceAnalysis => match verbosity {
                PromptVerbosity::Terse => API_SURFACE_TERSE.to_string(),
                PromptVerbosity::Balanced => API_SURFACE_BALANCED.to_string(),
                PromptVerbosity::Detailed => API_SURFACE_DETAILED.to_string(),
                PromptVerbosity::Exploratory => API_SURFACE_EXPLORATORY.to_string(),
            },
            AnalysisType::ContextBuilder => match verbosity {
                PromptVerbosity::Terse => CONTEXT_BUILDER_TERSE.to_string(),
                PromptVerbosity::Balanced => CONTEXT_BUILDER_BALANCED.to_string(),
                PromptVerbosity::Detailed => CONTEXT_BUILDER_DETAILED.to_string(),
                PromptVerbosity::Exploratory => CONTEXT_BUILDER_EXPLORATORY.to_string(),
            },
            AnalysisType::SemanticQuestion => match verbosity {
                PromptVerbosity::Terse => SEMANTIC_QUESTION_TERSE.to_string(),
                PromptVerbosity::Balanced => SEMANTIC_QUESTION_BALANCED.to_string(),
                PromptVerbosity::Detailed => SEMANTIC_QUESTION_DETAILED.to_string(),
                PromptVerbosity::Exploratory => SEMANTIC_QUESTION_EXPLORATORY.to_string(),
            },
        }
    }

    /// Get statistics about loaded prompts
    pub fn stats(&self) -> PromptSelectorStats {
        let total_prompts = self.prompts.len();
        let expected_prompts = AnalysisType::all().len() * 4; // 4 verbosity levels

        PromptSelectorStats {
            total_prompts,
            expected_prompts,
            coverage_percentage: (total_prompts as f32 / expected_prompts as f32) * 100.0,
        }
    }
}

impl Default for PromptSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about loaded prompts
#[derive(Debug, Clone)]
pub struct PromptSelectorStats {
    /// Total number of prompts loaded
    pub total_prompts: usize,
    /// Expected number of prompts (7 analysis types × 4 verbosity levels = 28)
    pub expected_prompts: usize,
    /// Coverage percentage
    pub coverage_percentage: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_type_round_trip() {
        for analysis_type in AnalysisType::all() {
            let s = analysis_type.as_str();
            let parsed = AnalysisType::from_str(s).expect("Should parse");
            assert_eq!(analysis_type, parsed);
        }
    }

    #[test]
    fn test_verbosity_from_tier() {
        assert_eq!(
            PromptVerbosity::from(ContextTier::Small),
            PromptVerbosity::Terse
        );
        assert_eq!(
            PromptVerbosity::from(ContextTier::Medium),
            PromptVerbosity::Balanced
        );
        assert_eq!(
            PromptVerbosity::from(ContextTier::Large),
            PromptVerbosity::Detailed
        );
        assert_eq!(
            PromptVerbosity::from(ContextTier::Massive),
            PromptVerbosity::Exploratory
        );
    }

    #[test]
    fn test_default_prompts_loaded() {
        let selector = PromptSelector::new();
        let stats = selector.stats();

        assert_eq!(stats.total_prompts, 28); // 7 analysis types × 4 verbosity levels
        assert_eq!(stats.expected_prompts, 28);
        assert_eq!(stats.coverage_percentage, 100.0);
    }

    #[test]
    fn test_select_prompt() {
        let selector = PromptSelector::new();

        // Test all combinations
        for analysis_type in AnalysisType::all() {
            for tier in [
                ContextTier::Small,
                ContextTier::Medium,
                ContextTier::Large,
                ContextTier::Massive,
            ] {
                let prompt = selector
                    .select_prompt(analysis_type, tier)
                    .expect("Should have prompt");
                assert!(!prompt.is_empty());
                // Prompts should not contain placeholder text anymore
                assert!(!prompt.contains("placeholder"));
                assert!(!prompt.contains("Phase 2B"));
            }
        }
    }

    #[test]
    fn test_tier_to_verbosity_prompt_content() {
        let selector = PromptSelector::new();

        // Test that Small tier gets TERSE prompts with appropriate constraints
        let terse_prompt = selector
            .select_prompt(AnalysisType::ArchitectureAnalysis, ContextTier::Small)
            .expect("Should have terse prompt");
        assert!(
            terse_prompt.contains("TERSE")
                || terse_prompt.contains("5 STEPS")
                || terse_prompt.contains("MAX 5")
        );

        // Test that Medium tier gets BALANCED prompts
        let balanced_prompt = selector
            .select_prompt(AnalysisType::ArchitectureAnalysis, ContextTier::Medium)
            .expect("Should have balanced prompt");
        assert!(
            balanced_prompt.contains("BALANCED")
                || balanced_prompt.contains("10 STEPS")
                || balanced_prompt.contains("MAX 10")
        );

        // Test that Large tier gets DETAILED prompts
        let detailed_prompt = selector
            .select_prompt(AnalysisType::ArchitectureAnalysis, ContextTier::Large)
            .expect("Should have detailed prompt");
        assert!(
            detailed_prompt.contains("DETAILED")
                || detailed_prompt.contains("15 STEPS")
                || detailed_prompt.contains("MAX 15")
        );

        // Test that Massive tier gets EXPLORATORY prompts
        let exploratory_prompt = selector
            .select_prompt(AnalysisType::ArchitectureAnalysis, ContextTier::Massive)
            .expect("Should have exploratory prompt");
        assert!(
            exploratory_prompt.contains("EXPLORATORY")
                || exploratory_prompt.contains("20 STEPS")
                || exploratory_prompt.contains("MAX 20")
        );
    }

    #[test]
    fn test_all_prompts_enforce_zero_heuristics() {
        let selector = PromptSelector::new();

        // Every prompt should enforce zero heuristics principle
        for analysis_type in AnalysisType::all() {
            for tier in [
                ContextTier::Small,
                ContextTier::Medium,
                ContextTier::Large,
                ContextTier::Massive,
            ] {
                let prompt = selector
                    .select_prompt(analysis_type, tier)
                    .expect("Should have prompt");

                // Should contain zero heuristics guidance
                assert!(
                    prompt.contains("ZERO HEURISTIC")
                        || prompt.contains("NO HEURISTIC")
                        || prompt.contains("ONLY structured")
                        || prompt.contains("NO assumptions"),
                    "Prompt for {:?}/{:?} should enforce zero heuristics",
                    analysis_type,
                    tier
                );

                // Should enforce JSON response format
                assert!(
                    prompt.contains("\"reasoning\"") && prompt.contains("\"tool_call\""),
                    "Prompt for {:?}/{:?} should specify JSON response format",
                    analysis_type,
                    tier
                );
            }
        }
    }

    #[test]
    fn test_specialized_prompts_for_all_analysis_types() {
        let selector = PromptSelector::new();

        // Test that we have specialized (non-generic) prompts for all analysis types
        let test_cases = vec![
            (AnalysisType::CodeSearch, "code", "search"),
            (
                AnalysisType::DependencyAnalysis,
                "dependency",
                "dependencies",
            ),
            (AnalysisType::CallChainAnalysis, "call", "chain"),
            (
                AnalysisType::ArchitectureAnalysis,
                "architecture",
                "architectural",
            ),
            (AnalysisType::ApiSurfaceAnalysis, "API", "surface"),
            (AnalysisType::ContextBuilder, "context", "build"),
            (AnalysisType::SemanticQuestion, "semantic", "question"),
        ];

        for (analysis_type, keyword1, keyword2) in test_cases {
            let prompt = selector
                .select_prompt(analysis_type, ContextTier::Medium)
                .expect("Should have prompt");

            // Each prompt should contain keywords relevant to its analysis type
            let lowercase_prompt = prompt.to_lowercase();
            assert!(
                lowercase_prompt.contains(keyword1) || lowercase_prompt.contains(keyword2),
                "Prompt for {:?} should contain '{}' or '{}' but got: {}...",
                analysis_type,
                keyword1,
                keyword2,
                &prompt[..200.min(prompt.len())]
            );
        }
    }

    #[test]
    fn test_recommended_max_steps() {
        let selector = PromptSelector::new();

        // Small tier base is 5
        assert_eq!(
            selector.recommended_max_steps(ContextTier::Small, AnalysisType::CodeSearch),
            4
        ); // 5 * 0.8 = 4
        assert_eq!(
            selector.recommended_max_steps(ContextTier::Small, AnalysisType::ArchitectureAnalysis),
            8
        ); // 5 * 1.5 = 7.5 -> 8

        // Massive tier base is 20
        assert_eq!(
            selector.recommended_max_steps(ContextTier::Massive, AnalysisType::CodeSearch),
            16
        ); // 20 * 0.8 = 16
        assert_eq!(
            selector
                .recommended_max_steps(ContextTier::Massive, AnalysisType::ArchitectureAnalysis),
            30
        ); // 20 * 1.5 = 30
    }

    #[test]
    fn test_register_custom_prompt() {
        let mut selector = PromptSelector::new();

        let custom_prompt = "Custom analysis prompt for testing".to_string();
        selector.register_prompt(
            AnalysisType::CodeSearch,
            PromptVerbosity::Terse,
            custom_prompt.clone(),
        );

        let retrieved = selector
            .select_prompt(AnalysisType::CodeSearch, ContextTier::Small)
            .expect("Should have custom prompt");

        assert_eq!(retrieved, custom_prompt);
    }
}
