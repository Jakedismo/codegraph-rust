use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::embedding_config::{EmbeddingModelConfig, EmbeddingPreset};
use crate::performance_config::{PerformanceMode, PerformanceModeConfig, PerformanceProfile};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdvancedConfig {
    #[serde(default)]
    pub embedding: EmbeddingModelConfig,

    #[serde(default)]
    pub performance: PerformanceModeConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub monitoring: MonitoringConfig,

    #[serde(default)]
    pub templates: ConfigTemplates,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            embedding: EmbeddingModelConfig::default(),
            performance: PerformanceModeConfig::default(),
            runtime: RuntimeConfig::default(),
            monitoring: MonitoringConfig::default(),
            templates: ConfigTemplates::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeConfig {
    #[serde(default = "RuntimeConfig::default_allow_runtime_switching")]
    pub allow_runtime_switching: bool,

    #[serde(default = "RuntimeConfig::default_hot_reload")]
    pub hot_reload: bool,

    #[serde(default = "RuntimeConfig::default_config_watch_interval_secs")]
    pub config_watch_interval_secs: u64,

    #[serde(default)]
    pub fallback_configs: Vec<String>,

    #[serde(default)]
    pub environment_overrides: HashMap<String, String>,
}

impl RuntimeConfig {
    fn default_allow_runtime_switching() -> bool {
        true
    }

    fn default_hot_reload() -> bool {
        false
    }

    fn default_config_watch_interval_secs() -> u64 {
        30
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            allow_runtime_switching: Self::default_allow_runtime_switching(),
            hot_reload: Self::default_hot_reload(),
            config_watch_interval_secs: Self::default_config_watch_interval_secs(),
            fallback_configs: vec![],
            environment_overrides: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitoringConfig {
    #[serde(default = "MonitoringConfig::default_enabled")]
    pub enabled: bool,

    #[serde(default = "MonitoringConfig::default_metrics_enabled")]
    pub metrics_enabled: bool,

    #[serde(default = "MonitoringConfig::default_trace_enabled")]
    pub trace_enabled: bool,

    #[serde(default = "MonitoringConfig::default_profile_enabled")]
    pub profile_enabled: bool,

    #[serde(default = "MonitoringConfig::default_metrics_interval_secs")]
    pub metrics_interval_secs: u64,

    #[serde(default)]
    pub export_targets: Vec<String>,
}

impl MonitoringConfig {
    fn default_enabled() -> bool {
        true
    }

    fn default_metrics_enabled() -> bool {
        true
    }

    fn default_trace_enabled() -> bool {
        false
    }

    fn default_profile_enabled() -> bool {
        false
    }

    fn default_metrics_interval_secs() -> u64 {
        60
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            metrics_enabled: Self::default_metrics_enabled(),
            trace_enabled: Self::default_trace_enabled(),
            profile_enabled: Self::default_profile_enabled(),
            metrics_interval_secs: Self::default_metrics_interval_secs(),
            export_targets: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigTemplates {
    #[serde(default)]
    pub embedding_presets: HashMap<String, EmbeddingModelConfig>,

    #[serde(default)]
    pub performance_profiles: HashMap<String, PerformanceModeConfig>,

    #[serde(default)]
    pub quick_configs: HashMap<String, QuickConfig>,
}

impl Default for ConfigTemplates {
    fn default() -> Self {
        let mut embedding_presets = HashMap::new();
        for preset in EmbeddingPreset::all_presets() {
            embedding_presets.insert(preset.name, preset.config);
        }

        let mut performance_profiles = HashMap::new();
        for profile in PerformanceProfile::all_profiles() {
            performance_profiles.insert(profile.name, profile.config);
        }

        Self {
            embedding_presets,
            performance_profiles,
            quick_configs: QuickConfig::all_configs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QuickConfig {
    pub name: String,
    pub description: String,
    pub embedding_preset: String,
    pub performance_profile: String,
    pub monitoring_enabled: bool,
}

impl QuickConfig {
    pub fn all_configs() -> HashMap<String, Self> {
        let mut configs = HashMap::new();

        configs.insert(
            "development".to_string(),
            Self {
                name: "development".to_string(),
                description: "Development environment with fast iteration".to_string(),
                embedding_preset: "local-minilm".to_string(),
                performance_profile: "research".to_string(),
                monitoring_enabled: true,
            },
        );

        configs.insert(
            "staging".to_string(),
            Self {
                name: "staging".to_string(),
                description: "Staging environment with balanced performance".to_string(),
                embedding_preset: "openai-small".to_string(),
                performance_profile: "production".to_string(),
                monitoring_enabled: true,
            },
        );

        configs.insert(
            "production".to_string(),
            Self {
                name: "production".to_string(),
                description: "Production environment with high performance".to_string(),
                embedding_preset: "openai-large".to_string(),
                performance_profile: "production".to_string(),
                monitoring_enabled: true,
            },
        );

        configs.insert(
            "edge".to_string(),
            Self {
                name: "edge".to_string(),
                description: "Edge deployment with minimal resources".to_string(),
                embedding_preset: "local-minilm".to_string(),
                performance_profile: "edge".to_string(),
                monitoring_enabled: false,
            },
        );

        configs
    }

    pub fn apply(&self, config: &mut AdvancedConfig) -> Result<()> {
        // Apply embedding preset
        if let Some(embedding) = config
            .templates
            .embedding_presets
            .get(&self.embedding_preset)
        {
            config.embedding = embedding.clone();
        } else {
            warn!("Embedding preset '{}' not found", self.embedding_preset);
        }

        // Apply performance profile
        if let Some(performance) = config
            .templates
            .performance_profiles
            .get(&self.performance_profile)
        {
            config.performance = performance.clone();
        } else {
            warn!(
                "Performance profile '{}' not found",
                self.performance_profile
            );
        }

        // Apply monitoring settings
        config.monitoring.enabled = self.monitoring_enabled;

        Ok(())
    }
}

impl AdvancedConfig {
    pub fn validate(&self) -> Result<()> {
        self.embedding
            .validate()
            .context("Invalid embedding configuration")?;

        self.performance
            .validate()
            .context("Invalid performance configuration")?;

        Ok(())
    }

    pub fn apply_template(&mut self, template_name: &str) -> Result<()> {
        let quick_config = self
            .templates
            .quick_configs
            .get(template_name)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", template_name))?
            .clone();

        quick_config.apply(self)?;
        info!("Applied configuration template: {}", template_name);

        Ok(())
    }

    pub fn apply_environment_overrides(&mut self) {
        // Override embedding provider from environment
        if let Ok(provider) = env::var("CODEGRAPH_EMBEDDING_PROVIDER") {
            match provider.to_lowercase().as_str() {
                "openai" => {
                    self.embedding.provider = crate::embedding_config::EmbeddingProvider::OpenAI;
                }
                "local" => {
                    self.embedding.provider = crate::embedding_config::EmbeddingProvider::Local;
                }
                _ => {
                    warn!("Unknown embedding provider: {}", provider);
                }
            }
        }

        // Override performance mode from environment
        if let Ok(mode) = env::var("CODEGRAPH_PERFORMANCE_MODE") {
            match mode.to_lowercase().as_str() {
                "high_accuracy" => {
                    self.performance = PerformanceModeConfig::high_accuracy();
                }
                "balanced" => {
                    self.performance = PerformanceModeConfig::balanced();
                }
                "high_speed" => {
                    self.performance = PerformanceModeConfig::high_speed();
                }
                "ultra_fast" => {
                    self.performance = PerformanceModeConfig::ultra_fast();
                }
                _ => {
                    warn!("Unknown performance mode: {}", mode);
                }
            }
        }

        // Apply custom environment overrides
        let overrides: Vec<(String, String, String)> = self
            .runtime
            .environment_overrides
            .iter()
            .filter_map(|(key, value)| {
                env::var(key).ok().map(|env_value| {
                    debug!("Applying environment override: {} = {}", key, env_value);
                    (key.clone(), value.clone(), env_value)
                })
            })
            .collect();

        for (_, path, env_value) in overrides {
            self.apply_override(&path, &env_value);
        }
    }

    fn apply_override(&mut self, path: &str, value: &str) {
        // Simple path-based override system
        let parts: Vec<&str> = path.split('.').collect();

        match parts.as_slice() {
            ["embedding", "dimension"] => {
                if let Ok(dim) = value.parse::<usize>() {
                    self.embedding.dimension = dim;
                }
            }
            ["performance", "cache", "max_size_mb"] => {
                if let Ok(size) = value.parse::<usize>() {
                    self.performance.cache.max_size_mb = size;
                }
            }
            ["performance", "processing", "batch_size"] => {
                if let Ok(size) = value.parse::<usize>() {
                    self.performance.processing.batch_size = size;
                }
            }
            _ => {
                debug!("Unknown override path: {}", path);
            }
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path.as_ref()))?;

        Ok(config)
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {:?}", path.as_ref()))?;

        Ok(())
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize configuration to JSON")
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("Failed to parse JSON configuration")
    }
}

#[derive(Debug)]
pub struct ConfigurationManager {
    config: std::sync::Arc<tokio::sync::RwLock<AdvancedConfig>>,
    config_path: Option<PathBuf>,
    watch_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConfigurationManager {
    pub fn new(config: AdvancedConfig) -> Self {
        Self {
            config: std::sync::Arc::new(tokio::sync::RwLock::new(config)),
            config_path: None,
            watch_handle: None,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = AdvancedConfig::from_file(&path)?;
        let mut manager = Self::new(config);
        manager.config_path = Some(path.as_ref().to_path_buf());
        Ok(manager)
    }

    pub async fn get_config(&self) -> AdvancedConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, config: AdvancedConfig) -> Result<()> {
        config.validate()?;
        *self.config.write().await = config;

        if let Some(ref path) = self.config_path {
            self.config.read().await.to_file(path)?;
        }

        Ok(())
    }

    pub async fn switch_performance_mode(&self, mode: PerformanceMode) -> Result<()> {
        let mut config = self.config.write().await;

        if !config.runtime.allow_runtime_switching {
            anyhow::bail!("Runtime configuration switching is disabled");
        }

        config.performance = PerformanceModeConfig::from_mode(mode);
        config.validate()?;

        info!("Switched performance mode to: {:?}", mode);
        Ok(())
    }

    pub async fn switch_embedding_preset(&self, preset_name: &str) -> Result<()> {
        let mut config = self.config.write().await;

        if !config.runtime.allow_runtime_switching {
            anyhow::bail!("Runtime configuration switching is disabled");
        }

        let preset = EmbeddingPreset::get_by_name(preset_name)
            .ok_or_else(|| anyhow::anyhow!("Embedding preset '{}' not found", preset_name))?;

        config.embedding = preset.config;
        config.validate()?;

        info!("Switched embedding preset to: {}", preset_name);
        Ok(())
    }

    pub async fn start_hot_reload(&mut self) -> Result<()> {
        let config = self.config.read().await;
        if !config.runtime.hot_reload {
            return Ok(());
        }

        let Some(ref path) = self.config_path else {
            return Ok(());
        };

        let config_arc = self.config.clone();
        let path = path.clone();
        let interval = config.runtime.config_watch_interval_secs;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval));

            loop {
                interval.tick().await;

                match AdvancedConfig::from_file(&path) {
                    Ok(new_config) => {
                        if let Err(e) = new_config.validate() {
                            warn!("Invalid configuration detected during hot reload: {}", e);
                            continue;
                        }

                        *config_arc.write().await = new_config;
                        info!("Configuration reloaded from: {:?}", path);
                    }
                    Err(e) => {
                        warn!("Failed to reload configuration: {}", e);
                    }
                }
            }
        });

        self.watch_handle = Some(handle);
        info!("Started configuration hot reload");

        Ok(())
    }

    pub fn stop_hot_reload(&mut self) {
        if let Some(handle) = self.watch_handle.take() {
            handle.abort();
            info!("Stopped configuration hot reload");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AdvancedConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_apply_template() {
        let mut config = AdvancedConfig::default();
        assert!(config.apply_template("development").is_ok());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_serialization() {
        let config = AdvancedConfig::default();

        let json = config.to_json().unwrap();
        let from_json = AdvancedConfig::from_json(&json).unwrap();

        assert_eq!(config.embedding.dimension, from_json.embedding.dimension);
        assert_eq!(config.performance.mode, from_json.performance.mode);
    }

    #[tokio::test]
    async fn test_configuration_manager() {
        let config = AdvancedConfig::default();
        let manager = ConfigurationManager::new(config);

        let current = manager.get_config().await;
        assert_eq!(current.performance.mode, PerformanceMode::Balanced);

        assert!(manager
            .switch_performance_mode(PerformanceMode::HighSpeed)
            .await
            .is_ok());

        let updated = manager.get_config().await;
        assert_eq!(updated.performance.mode, PerformanceMode::HighSpeed);
    }
}
