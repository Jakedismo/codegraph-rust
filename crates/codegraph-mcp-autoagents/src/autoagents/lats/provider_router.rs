// ABOUTME: Routes LLM requests to appropriate providers based on LATS phase
// ABOUTME: Supports different models for selection, expansion, evaluation, backpropagation

use codegraph_ai::llm_provider::LLMProvider;
use codegraph_mcp_core::config_manager::CodeGraphConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LATSPhase {
    Selection,
    Expansion,
    Evaluation,
    Backpropagation,
}

impl std::fmt::Display for LATSPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Selection => write!(f, "selection"),
            Self::Expansion => write!(f, "expansion"),
            Self::Evaluation => write!(f, "evaluation"),
            Self::Backpropagation => write!(f, "backpropagation"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub default_provider: String,
    pub phase_providers: HashMap<String, String>,
}

/// Routes LLM requests to appropriate providers based on LATS phase
///
/// The ProviderRouter maintains a mapping from LATS phases to LLM providers,
/// enabling multi-provider configurations where different models can be used
/// for different phases of the LATS algorithm.
///
/// For Phase 2 implementation, this currently defaults to using the primary
/// LLM provider for all phases. Future implementations will support reading
/// phase-specific provider configurations from the config file.
pub struct ProviderRouter {
    providers: HashMap<LATSPhase, Arc<dyn LLMProvider>>,
    default_provider: Arc<dyn LLMProvider>,
}

impl ProviderRouter {
    /// Create a new ProviderRouter
    ///
    /// For Phase 2, this uses the default_provider for all phases.
    /// Future implementation will read from config.llm.lats to create
    /// phase-specific providers.
    ///
    /// # Arguments
    /// * `_config` - Configuration (reserved for future use)
    /// * `default_provider` - The default LLM provider to use for all phases
    ///
    /// # Returns
    /// A new ProviderRouter instance
    pub fn new(_config: &CodeGraphConfig, default_provider: Arc<dyn LLMProvider>) -> Self {
        // Phase 2 implementation: Use default provider for all phases
        // TODO: In Phase 3, read config.llm.lats to create phase-specific providers:
        // if let Some(ref lats_config) = config.llm.lats {
        //     // Create phase-specific providers from config
        //     if let (Some(provider), Some(model)) =
        //         (&lats_config.selection_provider, &lats_config.selection_model)
        //     {
        //         let llm_config = Self::create_phase_config(provider, model, config);
        //         providers.insert(
        //             LATSPhase::Selection,
        //             create_llm_provider(&llm_config)?
        //         );
        //     }
        //     // Similar for Expansion, Evaluation, Backpropagation
        // }

        Self {
            providers: HashMap::new(),
            default_provider,
        }
    }

    /// Get the LLM provider for a specific LATS phase
    ///
    /// If a phase-specific provider is configured, it will be returned.
    /// Otherwise, the default provider is used.
    ///
    /// # Arguments
    /// * `phase` - The LATS phase requiring an LLM provider
    ///
    /// # Returns
    /// An Arc to the appropriate LLM provider
    pub fn get_provider(&self, phase: LATSPhase) -> Arc<dyn LLMProvider> {
        self.providers
            .get(&phase)
            .cloned()
            .unwrap_or_else(|| self.default_provider.clone())
    }

    /// Get statistics about provider allocation
    ///
    /// Returns information about which providers are being used for which phases,
    /// useful for debugging and monitoring.
    pub fn stats(&self) -> ProviderStats {
        ProviderStats {
            default_provider: self.default_provider.provider_name().to_string(),
            phase_providers: self
                .providers
                .iter()
                .map(|(phase, provider)| {
                    (format!("{}", phase), provider.provider_name().to_string())
                })
                .collect(),
        }
    }

    /// Check if a specific phase has a dedicated provider
    pub fn has_phase_provider(&self, phase: LATSPhase) -> bool {
        self.providers.contains_key(&phase)
    }

    /// Get the number of unique providers configured
    pub fn unique_provider_count(&self) -> usize {
        let mut unique_providers = std::collections::HashSet::new();
        unique_providers.insert(self.default_provider.provider_name());
        for provider in self.providers.values() {
            unique_providers.insert(provider.provider_name());
        }
        unique_providers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use codegraph_ai::llm_provider::LLMProvider;
    use codegraph_mcp_core::config_manager::CodeGraphConfig;

    // Mock LLM provider for testing
    struct MockProvider {
        name: &'static str,
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        fn provider_name(&self) -> &str {
            self.name
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }

        async fn is_available(&self) -> bool {
            true
        }

        fn characteristics(&self) -> codegraph_ai::llm_provider::ProviderCharacteristics {
            codegraph_ai::llm_provider::ProviderCharacteristics {
                max_tokens: 100000,
                avg_latency_ms: 100,
                rpm_limit: None,
                tpm_limit: None,
                supports_streaming: false,
                supports_functions: false,
            }
        }

        async fn generate_chat(
            &self,
            _messages: &[codegraph_ai::llm_provider::Message],
            _config: &codegraph_ai::llm_provider::GenerationConfig,
        ) -> codegraph_ai::llm_provider::LLMResult<codegraph_ai::llm_provider::LLMResponse>
        {
            Ok(codegraph_ai::llm_provider::LLMResponse {
                content: "mock response".to_string(),
                answer: String::new(),
                total_tokens: Some(10),
                prompt_tokens: Some(5),
                completion_tokens: Some(5),
                finish_reason: Some("stop".to_string()),
                model: "mock-model".to_string(),
            })
        }
    }

    #[test]
    fn test_provider_router_creation() {
        let config = CodeGraphConfig::default();
        let mock_provider = Arc::new(MockProvider { name: "mock" }) as Arc<dyn LLMProvider>;

        let router = ProviderRouter::new(&config, mock_provider);

        // Should use default provider for all phases
        assert_eq!(
            router.get_provider(LATSPhase::Selection).provider_name(),
            "mock"
        );
        assert_eq!(
            router.get_provider(LATSPhase::Expansion).provider_name(),
            "mock"
        );
        assert_eq!(
            router.get_provider(LATSPhase::Evaluation).provider_name(),
            "mock"
        );
        assert_eq!(
            router
                .get_provider(LATSPhase::Backpropagation)
                .provider_name(),
            "mock"
        );
    }

    #[test]
    fn test_provider_stats() {
        let config = CodeGraphConfig::default();
        let mock_provider = Arc::new(MockProvider {
            name: "test-provider",
        }) as Arc<dyn LLMProvider>;

        let router = ProviderRouter::new(&config, mock_provider);
        let stats = router.stats();

        assert_eq!(stats.default_provider, "test-provider");
        assert_eq!(stats.phase_providers.len(), 0); // No phase-specific providers in Phase 2
    }

    #[test]
    fn test_has_phase_provider() {
        let config = CodeGraphConfig::default();
        let mock_provider = Arc::new(MockProvider { name: "mock" }) as Arc<dyn LLMProvider>;

        let router = ProviderRouter::new(&config, mock_provider);

        // Phase 2: No phase-specific providers
        assert!(!router.has_phase_provider(LATSPhase::Selection));
        assert!(!router.has_phase_provider(LATSPhase::Expansion));
        assert!(!router.has_phase_provider(LATSPhase::Evaluation));
        assert!(!router.has_phase_provider(LATSPhase::Backpropagation));
    }

    #[test]
    fn test_unique_provider_count() {
        let config = CodeGraphConfig::default();
        let mock_provider = Arc::new(MockProvider { name: "mock" }) as Arc<dyn LLMProvider>;

        let router = ProviderRouter::new(&config, mock_provider);

        // Phase 2: Only default provider
        assert_eq!(router.unique_provider_count(), 1);
    }

    #[test]
    fn test_lats_phase_display() {
        assert_eq!(format!("{}", LATSPhase::Selection), "selection");
        assert_eq!(format!("{}", LATSPhase::Expansion), "expansion");
        assert_eq!(format!("{}", LATSPhase::Evaluation), "evaluation");
        assert_eq!(format!("{}", LATSPhase::Backpropagation), "backpropagation");
    }
}
