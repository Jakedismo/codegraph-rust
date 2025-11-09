// ABOUTME: AutoAgents plugin for tier-aware prompt injection
// ABOUTME: Selects prompts based on LLM context window and analysis type

use crate::{PromptSelector, AnalysisType};
use crate::context_aware_limits::ContextTier;
use crate::McpError;

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
        self.prompt_selector
            .select_prompt(self.analysis_type, self.tier)
            .map(|s| s.to_string())
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

        assert_eq!(small.get_max_iterations(), 5);
        assert_eq!(massive.get_max_iterations(), 20);
    }

    #[test]
    fn test_tier_plugin_max_tokens() {
        let plugin = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Medium);
        assert_eq!(plugin.get_max_tokens(), 4096);
    }
}
