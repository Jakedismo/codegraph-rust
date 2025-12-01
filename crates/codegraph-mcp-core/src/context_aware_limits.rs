// ABOUTME: Context-aware parameter calculation for MCP tools
// ABOUTME: Adjusts retrieval limits and LLM parameters based on model capabilities and MCP constraints

use codegraph_core::config_manager::CodeGraphConfig;
use tracing::info;

/// MCP protocol limitation: Most clients fail to retrieve results if output > 52,000 tokens
const MCP_MAX_OUTPUT_TOKENS: usize = 52_000;

/// Safety margin to account for formatting overhead and token counting variance
const TOKEN_SAFETY_MARGIN: f32 = 0.85; // Use 85% of limit to be safe

/// Maximum safe output tokens for MCP responses
const SAFE_MCP_OUTPUT_TOKENS: usize =
    ((MCP_MAX_OUTPUT_TOKENS as f32) * TOKEN_SAFETY_MARGIN) as usize;

/// Context window tiers for different model capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTier {
    /// Small models (< 50K context) - conservative limits
    Small,
    /// Medium models (50K-150K context) - moderate limits
    Medium,
    /// Large models (150K-500K context) - generous limits
    Large,
    /// Massive models (> 500K context, e.g., Grok 2M) - very generous limits
    Massive,
}

impl ContextTier {
    /// Detect tier from context window size
    pub fn from_context_window(context_window: usize) -> Self {
        match context_window {
            0..=50_000 => ContextTier::Small,
            50_001..=150_000 => ContextTier::Medium,
            150_001..=500_000 => ContextTier::Large,
            _ => ContextTier::Massive,
        }
    }

    /// Get appropriate base retrieval limit for this tier
    pub fn base_limit(&self) -> usize {
        match self {
            ContextTier::Small => 10,    // 10 results for small context
            ContextTier::Medium => 25,   // 25 results for medium (e.g., GPT-4)
            ContextTier::Large => 50,    // 50 results for large (e.g., Claude)
            ContextTier::Massive => 100, // 100 results for massive (e.g., Grok 2M)
        }
    }

    /// Get appropriate overretrieve multiplier for local search
    pub fn local_overretrieve_multiplier(&self) -> usize {
        match self {
            ContextTier::Small => 5,    // 5x overretrieve
            ContextTier::Medium => 8,   // 8x overretrieve
            ContextTier::Large => 10,   // 10x overretrieve
            ContextTier::Massive => 15, // 15x overretrieve (can afford more)
        }
    }

    /// Get appropriate overretrieve multiplier for cloud search (with reranking)
    pub fn cloud_overretrieve_multiplier(&self) -> usize {
        match self {
            ContextTier::Small => 3,   // 3x overretrieve for reranking
            ContextTier::Medium => 4,  // 4x overretrieve
            ContextTier::Large => 5,   // 5x overretrieve
            ContextTier::Massive => 8, // 8x overretrieve (Jina batching can handle it)
        }
    }
}

/// Context-aware limits for MCP tool operations
#[derive(Debug, Clone)]
pub struct ContextAwareLimits {
    /// Context tier based on configured LLM
    pub tier: ContextTier,
    /// Actual context window size in tokens
    pub context_window: usize,
    /// Maximum search result limit
    pub max_search_limit: usize,
    /// Local search overretrieve limit
    pub local_overretrieve_limit: usize,
    /// Cloud search overretrieve limit (for reranking)
    pub cloud_overretrieve_limit: usize,
    /// Maximum output tokens for LLM calls (MCP constraint)
    pub max_completion_token: usize,
    /// LLM provider name
    pub llm_provider: String,
    /// LLM model name
    pub llm_model: String,
}

impl ContextAwareLimits {
    /// Calculate limits from CodeGraph configuration
    pub fn from_config(config: &CodeGraphConfig) -> Self {
        let context_window = config.llm.context_window;
        let tier = ContextTier::from_context_window(context_window);

        let base_limit = tier.base_limit();
        let local_multiplier = tier.local_overretrieve_multiplier();
        let cloud_multiplier = tier.cloud_overretrieve_multiplier();

        let limits = Self {
            tier,
            context_window,
            max_search_limit: base_limit,
            local_overretrieve_limit: base_limit * local_multiplier,
            cloud_overretrieve_limit: base_limit * cloud_multiplier,
            max_completion_token: SAFE_MCP_OUTPUT_TOKENS,
            llm_provider: config.llm.provider.clone(),
            llm_model: config
                .llm
                .model
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        };

        info!(
            "🎯 Context-Aware Limits: tier={:?}, context={}K, max_results={}, local_overretrieve={}, cloud_overretrieve={}, max_completion_token={}",
            limits.tier,
            limits.context_window / 1000,
            limits.max_search_limit,
            limits.local_overretrieve_limit,
            limits.cloud_overretrieve_limit,
            limits.max_completion_token
        );

        info!(
            "🤖 LLM: provider={}, model={}",
            limits.llm_provider, limits.llm_model
        );

        limits
    }

    /// Get search limit adjusted for user request (cap at tier max)
    pub fn adjust_search_limit(&self, requested_limit: usize) -> usize {
        requested_limit.min(self.max_search_limit)
    }

    /// Get local overretrieve limit for a given search limit
    pub fn get_local_overretrieve(&self, search_limit: usize) -> usize {
        (search_limit * self.tier.local_overretrieve_multiplier())
            .min(self.local_overretrieve_limit)
    }

    /// Get cloud overretrieve limit for a given search limit
    pub fn get_cloud_overretrieve(&self, search_limit: usize) -> usize {
        (search_limit * self.tier.cloud_overretrieve_multiplier())
            .min(self.cloud_overretrieve_limit)
    }

    // Note: generation_config() method removed to avoid dependency on codegraph-ai
    // When implementing semantic_intelligence, construct GenerationConfig manually
    // with max_completion_token set to self.max_completion_token (44,200)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_tier_detection() {
        assert_eq!(ContextTier::from_context_window(32_000), ContextTier::Small);
        assert_eq!(
            ContextTier::from_context_window(128_000),
            ContextTier::Medium
        );
        assert_eq!(
            ContextTier::from_context_window(200_000),
            ContextTier::Large
        );
        assert_eq!(
            ContextTier::from_context_window(2_000_000),
            ContextTier::Massive
        );
    }

    #[test]
    fn test_base_limits() {
        assert_eq!(ContextTier::Small.base_limit(), 10);
        assert_eq!(ContextTier::Medium.base_limit(), 25);
        assert_eq!(ContextTier::Large.base_limit(), 50);
        assert_eq!(ContextTier::Massive.base_limit(), 100);
    }

    #[test]
    fn test_overretrieve_multipliers() {
        let small = ContextTier::Small;
        assert_eq!(small.local_overretrieve_multiplier(), 5);
        assert_eq!(small.cloud_overretrieve_multiplier(), 3);

        let massive = ContextTier::Massive;
        assert_eq!(massive.local_overretrieve_multiplier(), 15);
        assert_eq!(massive.cloud_overretrieve_multiplier(), 8);
    }

    #[test]
    fn test_mcp_output_limit() {
        // Ensure we're under 52K with safety margin
        assert!(SAFE_MCP_OUTPUT_TOKENS < MCP_MAX_OUTPUT_TOKENS);
        assert_eq!(SAFE_MCP_OUTPUT_TOKENS, 44_200); // 85% of 52K
    }
}
