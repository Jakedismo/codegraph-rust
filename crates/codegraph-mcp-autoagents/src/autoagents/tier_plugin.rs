// ABOUTME: AutoAgents plugin for tier-aware prompt injection
// ABOUTME: Selects prompts based on LLM context window and analysis type

use crate::autoagents::prompt_selector::PromptSelector;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_core::error::McpError;

/// Plugin that provides tier-aware system prompts and limits
pub struct TierAwarePromptPlugin {
    prompt_selector: PromptSelector,
    analysis_type: AnalysisType,
    tier: ContextTier,
}

impl TierAwarePromptPlugin {
    pub fn new(analysis_type: AnalysisType, tier: ContextTier) -> Self {
        Self {
            prompt_selector: PromptSelector::new(),
            analysis_type,
            tier,
        }
    }

    /// Get tier-appropriate system prompt
    pub fn get_system_prompt(&self) -> Result<String, McpError> {
        let mut prompt = self
            .prompt_selector
            .select_prompt(self.analysis_type, self.tier)
            .map(|s: &str| s.to_string())?;

        if matches!(
            self.analysis_type,
            AnalysisType::ArchitectureAnalysis | AnalysisType::ContextBuilder
        ) {
            if let Ok(primer) = std::env::var("CODEGRAPH_ARCH_PRIMER") {
                if !primer.trim().is_empty() {
                    prompt.push_str("\n\n[Architecture Primer]\n");
                    prompt.push_str(&primer);
                }
            }
        }

        Ok(prompt)
    }

    /// Get tier-appropriate max_iterations (max_steps in original plan)
    pub fn get_max_iterations(&self) -> usize {
        self.prompt_selector
            .recommended_max_steps(self.tier, self.analysis_type)
    }

    /// Get tier-appropriate max_tokens for LLM responses
    pub fn get_max_tokens(&self) -> usize {
        match self.tier {
            ContextTier::Small => 2048,
            ContextTier::Medium => 4096,
            ContextTier::Large => 8192,
            ContextTier::Massive => 16384,
        }
    }

    /// Get temperature setting (consistent across tiers)
    pub fn get_temperature(&self) -> f32 {
        0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_plugin_max_iterations() {
        let small = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Small);
        let massive = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Massive);

        // Small: 3 * 0.8 = 2.4 -> 3 (ceil)
        // Massive: 10 * 0.8 = 8
        assert_eq!(small.get_max_iterations(), 3);
        assert_eq!(massive.get_max_iterations(), 8);
    }

    #[test]
    fn primer_appended_for_architecture() {
        std::env::set_var("CODEGRAPH_ARCH_PRIMER", "layers: api -> core");
        let plugin =
            TierAwarePromptPlugin::new(AnalysisType::ArchitectureAnalysis, ContextTier::Massive);
        let prompt = plugin.get_system_prompt().unwrap();
        assert!(prompt.contains("[Architecture Primer]"));
        assert!(prompt.contains("layers: api -> core"));
        std::env::remove_var("CODEGRAPH_ARCH_PRIMER");
    }

    #[test]
    fn test_tier_plugin_max_tokens() {
        let plugin = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Medium);
        assert_eq!(plugin.get_max_tokens(), 4096);
    }
}
