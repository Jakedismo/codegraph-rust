use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(String),

    #[error("Failed to read config: {0}")]
    ReadError(String),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

/// Main configuration for CodeGraph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphConfig {
    /// Embedding provider configuration
    #[serde(default)]
    pub embedding: EmbeddingConfig,

    /// LLM configuration for insights
    #[serde(default)]
    pub llm: LLMConfig,

    /// Performance and resource settings
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for CodeGraphConfig {
    fn default() -> Self {
        Self {
            embedding: EmbeddingConfig::default(),
            llm: LLMConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// Embedding provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Provider: "onnx", "ollama", "openai", or "auto"
    #[serde(default = "default_embedding_provider")]
    pub provider: String,

    /// Model path or identifier
    /// For ONNX: path to model directory
    /// For Ollama: model name (e.g., "all-minilm:latest")
    /// For OpenAI: model name (e.g., "text-embedding-3-small")
    #[serde(default)]
    pub model: Option<String>,

    /// Ollama URL (if using Ollama)
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// OpenAI API key (if using OpenAI)
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// Batch size for embedding generation
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: None,  // Auto-detect
            ollama_url: default_ollama_url(),
            openai_api_key: None,
            batch_size: default_batch_size(),
        }
    }
}

/// LLM configuration for insights generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Enable LLM insights (false = context-only mode for agents)
    #[serde(default)]
    pub enabled: bool,

    /// Model identifier
    /// For Ollama: model name (e.g., "qwen2.5-coder:14b")
    #[serde(default)]
    pub model: Option<String>,

    /// Ollama URL
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// Context window size
    #[serde(default = "default_context_window")]
    pub context_window: usize,

    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Insights mode: "context-only", "balanced", or "deep"
    #[serde(default = "default_insights_mode")]
    pub insights_mode: String,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            enabled: false,  // Default to context-only for speed
            model: None,
            ollama_url: default_ollama_url(),
            context_window: default_context_window(),
            temperature: default_temperature(),
            insights_mode: default_insights_mode(),
        }
    }
}

/// Performance and resource configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    #[serde(default = "default_num_threads")]
    pub num_threads: usize,

    /// Cache size in MB
    #[serde(default = "default_cache_size_mb")]
    pub cache_size_mb: usize,

    /// Enable GPU acceleration
    #[serde(default)]
    pub enable_gpu: bool,

    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            num_threads: default_num_threads(),
            cache_size_mb: default_cache_size_mb(),
            enable_gpu: false,  // Conservative default
            max_concurrent_requests: default_max_concurrent(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: "trace", "debug", "info", "warn", "error"
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format: "pretty", "json", "compact"
    #[serde(default = "default_log_format")]
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
        }
    }
}

// Default value functions
fn default_embedding_provider() -> String { "auto".to_string() }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_batch_size() -> usize { 32 }
fn default_context_window() -> usize { 8000 }
fn default_temperature() -> f32 { 0.1 }
fn default_insights_mode() -> String { "context-only".to_string() }
fn default_num_threads() -> usize { num_cpus::get() }
fn default_cache_size_mb() -> usize { 512 }
fn default_max_concurrent() -> usize { 4 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "pretty".to_string() }

/// Configuration manager with smart defaults and auto-detection
pub struct ConfigManager {
    config: CodeGraphConfig,
    config_path: Option<PathBuf>,
}

impl ConfigManager {
    /// Load configuration with the following precedence:
    /// 1. Environment variables (.env file)
    /// 2. Config file (.codegraph.toml)
    /// 3. Sensible defaults
    pub fn load() -> Result<Self, ConfigError> {
        info!("🔧 Loading CodeGraph configuration...");

        // Try to load .env file from current directory or home
        Self::load_dotenv();

        // Try to find and load config file
        let (config, config_path) = Self::load_config_file()?;

        // Override with environment variables
        let config = Self::apply_env_overrides(config);

        // Validate configuration
        Self::validate_config(&config)?;

        info!("✅ Configuration loaded successfully");
        if let Some(ref path) = config_path {
            info!("   📄 Config file: {}", path.display());
        }
        info!("   🤖 Embedding provider: {}", config.embedding.provider);
        info!("   💬 LLM insights: {}", if config.llm.enabled { "enabled" } else { "disabled (context-only)" });

        Ok(Self { config, config_path })
    }

    /// Load .env file if it exists
    fn load_dotenv() {
        // Try current directory first
        if Path::new(".env").exists() {
            if let Err(e) = dotenv::from_filename(".env") {
                warn!("Failed to load .env file: {}", e);
            } else {
                info!("📋 Loaded .env file from current directory");
            }
            return;
        }

        // Try home directory
        if let Some(home) = dirs::home_dir() {
            let home_env = home.join(".codegraph.env");
            if home_env.exists() {
                if let Err(e) = dotenv::from_path(&home_env) {
                    warn!("Failed to load .codegraph.env: {}", e);
                } else {
                    info!("📋 Loaded .codegraph.env from home directory");
                }
            }
        }
    }

    /// Find and load config file
    /// Search order:
    /// 1. ./.codegraph.toml (current directory)
    /// 2. ~/.codegraph/config.toml (user config)
    /// 3. Use defaults
    fn load_config_file() -> Result<(CodeGraphConfig, Option<PathBuf>), ConfigError> {
        // Try current directory
        let local_config = Path::new(".codegraph.toml");
        if local_config.exists() {
            let config = Self::read_toml_file(local_config)?;
            return Ok((config, Some(local_config.to_path_buf())));
        }

        // Try user config directory
        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".codegraph").join("config.toml");
            if user_config.exists() {
                let config = Self::read_toml_file(&user_config)?;
                return Ok((config, Some(user_config)));
            }
        }

        // Use defaults
        info!("📋 No config file found, using defaults");
        Ok((CodeGraphConfig::default(), None))
    }

    /// Read TOML config file
    fn read_toml_file(path: &Path) -> Result<CodeGraphConfig, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        let config: CodeGraphConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(mut config: CodeGraphConfig) -> CodeGraphConfig {
        // Embedding configuration
        if let Ok(provider) = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER") {
            config.embedding.provider = provider;
        }
        if let Ok(model) = std::env::var("CODEGRAPH_EMBEDDING_MODEL") {
            config.embedding.model = Some(model);
        }
        if let Ok(model) = std::env::var("CODEGRAPH_LOCAL_MODEL") {
            config.embedding.model = Some(model);
        }
        if let Ok(url) = std::env::var("CODEGRAPH_OLLAMA_URL") {
            config.embedding.ollama_url = url.clone();
            config.llm.ollama_url = url;
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            config.embedding.openai_api_key = Some(key);
        }

        // LLM configuration
        if let Ok(model) = std::env::var("CODEGRAPH_MODEL") {
            config.llm.model = Some(model);
            config.llm.enabled = true;  // Enable if model specified
        }
        if let Ok(context) = std::env::var("CODEGRAPH_CONTEXT_WINDOW") {
            if let Ok(size) = context.parse() {
                config.llm.context_window = size;
            }
        }
        if let Ok(temp) = std::env::var("CODEGRAPH_TEMPERATURE") {
            if let Ok(t) = temp.parse() {
                config.llm.temperature = t;
            }
        }

        // Logging
        if let Ok(level) = std::env::var("RUST_LOG") {
            config.logging.level = level;
        }

        config
    }

    /// Validate configuration
    fn validate_config(config: &CodeGraphConfig) -> Result<(), ConfigError> {
        // Validate embedding provider
        match config.embedding.provider.as_str() {
            "auto" | "onnx" | "ollama" | "openai" => {},
            other => return Err(ConfigError::ValidationError(
                format!("Invalid embedding provider: {}. Must be one of: auto, onnx, ollama, openai", other)
            )),
        }

        // Validate insights mode
        match config.llm.insights_mode.as_str() {
            "context-only" | "balanced" | "deep" => {},
            other => return Err(ConfigError::ValidationError(
                format!("Invalid insights mode: {}. Must be one of: context-only, balanced, deep", other)
            )),
        }

        // Validate log level
        match config.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {},
            other => return Err(ConfigError::ValidationError(
                format!("Invalid log level: {}. Must be one of: trace, debug, info, warn, error", other)
            )),
        }

        Ok(())
    }

    /// Get the loaded configuration
    pub fn config(&self) -> &CodeGraphConfig {
        &self.config
    }

    /// Create a default config file
    pub fn create_default_config(path: &Path) -> Result<(), ConfigError> {
        let config = CodeGraphConfig::default();
        let toml_str = toml::to_string_pretty(&config)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::ReadError(e.to_string()))?;
        }

        std::fs::write(path, toml_str)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        Ok(())
    }

    /// Auto-detect available embedding models
    pub fn auto_detect_embedding_model() -> Option<String> {
        // Check for Ollama models
        if Self::check_ollama_available() {
            return Some("ollama:all-minilm".to_string());
        }

        // Check for ONNX models in HuggingFace cache
        if let Some(model_path) = Self::find_onnx_model_in_cache() {
            return Some(model_path);
        }

        None
    }

    /// Check if Ollama is available
    fn check_ollama_available() -> bool {
        // Try to connect to Ollama
        std::process::Command::new("curl")
            .args(&["-s", "http://localhost:11434/api/tags"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Find ONNX models in HuggingFace cache
    fn find_onnx_model_in_cache() -> Option<String> {
        let home = dirs::home_dir()?;
        let hf_cache = home.join(".cache/huggingface/hub");

        if !hf_cache.exists() {
            return None;
        }

        // Look for all-MiniLM-L6-v2-onnx model
        let pattern = "models--Qdrant--all-MiniLM-L6-v2-onnx";

        if let Ok(entries) = std::fs::read_dir(&hf_cache) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name()?.to_str()?.contains(pattern) {
                    // Find snapshots directory
                    let snapshots = path.join("snapshots");
                    if let Ok(snapshot_entries) = std::fs::read_dir(&snapshots) {
                        if let Some(snapshot) = snapshot_entries.flatten().next() {
                            return Some(snapshot.path().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CodeGraphConfig::default();
        assert_eq!(config.embedding.provider, "auto");
        assert_eq!(config.llm.enabled, false);
        assert_eq!(config.llm.insights_mode, "context-only");
    }

    #[test]
    fn test_config_validation() {
        let config = CodeGraphConfig::default();
        assert!(ConfigManager::validate_config(&config).is_ok());

        let mut bad_config = config.clone();
        bad_config.embedding.provider = "invalid".to_string();
        assert!(ConfigManager::validate_config(&bad_config).is_err());
    }
}
