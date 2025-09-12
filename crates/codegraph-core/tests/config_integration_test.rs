use codegraph_core::{
    AdvancedConfig, ConfigurationManager, EmbeddingModelConfig, EmbeddingPreset, PerformanceMode,
    PerformanceModeConfig, PerformanceProfile,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_default_configuration() {
    let config = AdvancedConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_embedding_presets() {
    let presets = EmbeddingPreset::all_presets();
    assert!(!presets.is_empty());

    for preset in presets {
        assert!(preset.config.validate().is_ok());
    }

    let openai_small = EmbeddingPreset::get_by_name("openai-small");
    assert!(openai_small.is_some());
}

#[test]
fn test_performance_profiles() {
    let profiles = PerformanceProfile::all_profiles();
    assert_eq!(profiles.len(), 4);

    for profile in profiles {
        assert!(profile.config.validate().is_ok());
    }

    let research = PerformanceProfile::get_by_name("research");
    assert!(research.is_some());
}

#[test]
fn test_performance_mode_creation() {
    let modes = vec![
        PerformanceMode::HighAccuracy,
        PerformanceMode::Balanced,
        PerformanceMode::HighSpeed,
        PerformanceMode::UltraFast,
        PerformanceMode::Custom,
    ];

    for mode in modes {
        let config = PerformanceModeConfig::from_mode(mode);
        assert_eq!(config.mode, mode);
        assert!(config.validate().is_ok());
    }
}

#[test]
fn test_config_serialization() {
    let config = AdvancedConfig::default();

    // Test JSON serialization
    let json = config.to_json().unwrap();
    let from_json = AdvancedConfig::from_json(&json).unwrap();
    assert_eq!(config.embedding.dimension, from_json.embedding.dimension);
    assert_eq!(config.performance.mode, from_json.performance.mode);

    // Test TOML serialization
    let toml = toml::to_string(&config).unwrap();
    let from_toml: AdvancedConfig = toml::from_str(&toml).unwrap();
    assert_eq!(config.embedding.dimension, from_toml.embedding.dimension);
}

#[test]
fn test_config_file_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    let config = AdvancedConfig::default();
    config.to_file(&config_path).unwrap();
    assert!(config_path.exists());

    let loaded = AdvancedConfig::from_file(&config_path).unwrap();
    assert_eq!(config.embedding.dimension, loaded.embedding.dimension);
}

#[test]
fn test_template_application() {
    let mut config = AdvancedConfig::default();

    assert!(config.apply_template("development").is_ok());
    assert!(config.validate().is_ok());

    assert!(config.apply_template("production").is_ok());
    assert!(config.validate().is_ok());

    assert!(config.apply_template("nonexistent").is_err());
}

#[test]
fn test_auto_tuning() {
    let mut config = PerformanceModeConfig::balanced();
    config.apply_auto_tuning(4096, 8);

    assert_eq!(config.cache.max_size_mb, 1024);
    assert_eq!(config.processing.parallel_workers, 8);
    assert_eq!(config.processing.batch_size, 64);
}

#[test]
fn test_validation_rules() {
    let mut config = EmbeddingModelConfig::default();

    // Test invalid dimension
    config.dimension = 0;
    assert!(config.validate().is_err());

    config.dimension = 10000;
    assert!(config.validate().is_err());

    config.dimension = 768;
    assert!(config.validate().is_ok());

    // Test OpenAI provider without config
    config.provider = codegraph_core::EmbeddingProvider::OpenAI;
    config.openai = None;
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_configuration_manager() {
    let config = AdvancedConfig::default();
    let manager = ConfigurationManager::new(config);

    // Test getting config
    let current = manager.get_config().await;
    assert_eq!(current.performance.mode, PerformanceMode::Balanced);

    // Test switching performance mode
    assert!(manager
        .switch_performance_mode(PerformanceMode::HighSpeed)
        .await
        .is_ok());
    let updated = manager.get_config().await;
    assert_eq!(updated.performance.mode, PerformanceMode::HighSpeed);

    // Test switching embedding preset
    assert!(manager
        .switch_embedding_preset("local-minilm")
        .await
        .is_ok());
    let updated = manager.get_config().await;
    assert_eq!(updated.embedding.dimension, 384);
}

#[tokio::test]
async fn test_runtime_switching_disabled() {
    let mut config = AdvancedConfig::default();
    config.runtime.allow_runtime_switching = false;

    let manager = ConfigurationManager::new(config);

    // Should fail when runtime switching is disabled
    assert!(manager
        .switch_performance_mode(PerformanceMode::HighSpeed)
        .await
        .is_err());
    assert!(manager
        .switch_embedding_preset("openai-small")
        .await
        .is_err());
}

#[test]
fn test_environment_overrides() {
    std::env::set_var("CODEGRAPH_EMBEDDING_PROVIDER", "openai");
    std::env::set_var("CODEGRAPH_PERFORMANCE_MODE", "high_speed");

    let mut config = AdvancedConfig::default();
    config.apply_environment_overrides();

    assert!(matches!(
        config.embedding.provider,
        codegraph_core::EmbeddingProvider::OpenAI
    ));
    assert_eq!(config.performance.mode, PerformanceMode::HighSpeed);

    // Clean up
    std::env::remove_var("CODEGRAPH_EMBEDDING_PROVIDER");
    std::env::remove_var("CODEGRAPH_PERFORMANCE_MODE");
}

#[test]
fn test_quick_configs() {
    let configs = codegraph_core::QuickConfig::all_configs();
    assert_eq!(configs.len(), 4);

    assert!(configs.contains_key("development"));
    assert!(configs.contains_key("staging"));
    assert!(configs.contains_key("production"));
    assert!(configs.contains_key("edge"));

    let dev_config = &configs["development"];
    assert_eq!(dev_config.embedding_preset, "local-minilm");
    assert_eq!(dev_config.performance_profile, "research");
    assert!(dev_config.monitoring_enabled);
}

#[test]
fn test_performance_mode_descriptions() {
    assert!(!PerformanceMode::HighAccuracy.description().is_empty());
    assert!(!PerformanceMode::Balanced.description().is_empty());
    assert!(!PerformanceMode::HighSpeed.description().is_empty());
    assert!(!PerformanceMode::UltraFast.description().is_empty());
    assert!(!PerformanceMode::Custom.description().is_empty());
}
