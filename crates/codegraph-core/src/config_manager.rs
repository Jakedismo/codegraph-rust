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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    /// Daemon configuration for automatic file watching
    #[serde(default)]
    pub daemon: DaemonConfig,
}

/// Embedding provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Provider: "onnx", "ollama", "openai", "lmstudio", "jina", or "auto"
    #[serde(default = "default_embedding_provider")]
    pub provider: String,

    /// Model path or identifier
    /// For ONNX: path to model directory
    /// For Ollama: model name (e.g., "all-minilm:latest")
    /// For LM Studio: model name (e.g., "jinaai/jina-embeddings-v3")
    /// For OpenAI: model name (e.g., "text-embedding-3-small")
    /// For Jina: model name (e.g., "jina-embeddings-v4")
    #[serde(default)]
    pub model: Option<String>,

    /// LM Studio URL (if using LM Studio)
    #[serde(default = "default_lmstudio_url")]
    pub lmstudio_url: String,

    /// Ollama URL (if using Ollama)
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// OpenAI API key (if using OpenAI)
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// Jina API key (if using Jina)
    #[serde(default)]
    pub jina_api_key: Option<String>,

    /// Jina API base URL
    #[serde(default = "default_jina_api_base")]
    pub jina_api_base: String,

    /// Enable Jina reranking
    #[serde(default)]
    pub jina_enable_reranking: bool,

    /// Jina reranking model
    #[serde(default = "default_jina_reranking_model")]
    pub jina_reranking_model: String,

    /// Jina reranking top N results
    #[serde(default = "default_jina_reranking_top_n")]
    pub jina_reranking_top_n: usize,

    /// Jina late chunking
    #[serde(default)]
    pub jina_late_chunking: bool,

    /// Jina task type
    #[serde(default = "default_jina_task")]
    pub jina_task: String,

    /// Embedding dimension (1024 for jina-embeddings-v4, 1536 for jina-code, 384 for all-MiniLM)
    #[serde(default = "default_embedding_dimension")]
    pub dimension: usize,

    /// Batch size for embedding generation
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: None, // Auto-detect
            lmstudio_url: default_lmstudio_url(),
            ollama_url: default_ollama_url(),
            openai_api_key: None,
            jina_api_key: None,
            jina_api_base: default_jina_api_base(),
            jina_enable_reranking: false,
            jina_reranking_model: default_jina_reranking_model(),
            jina_reranking_top_n: default_jina_reranking_top_n(),
            jina_late_chunking: false,
            jina_task: default_jina_task(),
            dimension: default_embedding_dimension(),
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

    /// LLM provider: "ollama", "lmstudio", "anthropic", "openai", "xai", "openai-compatible"
    #[serde(default = "default_llm_provider")]
    pub provider: String,

    /// Model identifier
    /// For LM Studio: model name (e.g., "lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF")
    /// For Ollama: model name (e.g., "qwen2.5-coder:14b")
    /// For Anthropic: model name (e.g., "claude-3-5-sonnet-20241022")
    /// For OpenAI: model name (e.g., "gpt-4o")
    /// For xAI: model name (e.g., "grok-4-fast", "grok-4-turbo")
    /// For OpenAI-compatible: custom model name
    #[serde(default)]
    pub model: Option<String>,

    /// LM Studio URL
    #[serde(default = "default_lmstudio_url")]
    pub lmstudio_url: String,

    /// Ollama URL
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// OpenAI-compatible base URL (for custom endpoints)
    #[serde(default)]
    pub openai_compatible_url: Option<String>,

    /// Anthropic API key
    #[serde(default)]
    pub anthropic_api_key: Option<String>,

    /// OpenAI API key
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// xAI API key
    #[serde(default)]
    pub xai_api_key: Option<String>,

    /// xAI base URL (default: https://api.x.ai/v1)
    #[serde(default = "default_xai_base_url")]
    pub xai_base_url: String,

    /// Context window size
    #[serde(default = "default_context_window")]
    pub context_window: usize,

    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Insights mode: "context-only", "balanced", or "deep"
    #[serde(default = "default_insights_mode")]
    pub insights_mode: String,

    /// Maximum tokens to generate (legacy parameter)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Maximum output tokens (for Responses API and reasoning models)
    #[serde(default)]
    pub max_completion_token: Option<usize>,

    /// MCP code agent maximum output tokens (for agentic workflows)
    /// Overrides tier-based defaults if set
    #[serde(default)]
    pub mcp_code_agent_max_output_tokens: Option<usize>,

    /// Reasoning effort for reasoning models: "minimal", "medium", "high"
    #[serde(default)]
    pub reasoning_effort: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Default to context-only for speed
            provider: default_llm_provider(),
            model: None,
            lmstudio_url: default_lmstudio_url(),
            ollama_url: default_ollama_url(),
            openai_compatible_url: None,
            anthropic_api_key: None,
            openai_api_key: None,
            xai_api_key: None,
            xai_base_url: default_xai_base_url(),
            context_window: default_context_window(),
            temperature: default_temperature(),
            insights_mode: default_insights_mode(),
            max_tokens: default_max_tokens(),
            max_completion_token: None, // Will use max_tokens if not set
            mcp_code_agent_max_output_tokens: None, // Use tier-based defaults if not set
            reasoning_effort: None,     // Only for reasoning models
            timeout_secs: default_timeout_secs(),
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
            enable_gpu: false, // Conservative default
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

/// Daemon configuration for automatic file watching and re-indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Enable automatic daemon startup with MCP server
    #[serde(default)]
    pub auto_start_with_mcp: bool,

    /// Project path to watch (defaults to current directory)
    #[serde(default)]
    pub project_path: Option<PathBuf>,

    /// Debounce duration for file changes (ms)
    #[serde(default = "default_daemon_debounce_ms")]
    pub debounce_ms: u64,

    /// Batch timeout for collecting changes (ms)
    #[serde(default = "default_daemon_batch_timeout_ms")]
    pub batch_timeout_ms: u64,

    /// Health check interval (seconds)
    #[serde(default = "default_daemon_health_check_interval")]
    pub health_check_interval_secs: u64,

    /// Languages to watch (empty = all detected)
    #[serde(default)]
    pub languages: Vec<String>,

    /// Exclude patterns (gitignore format)
    #[serde(default = "default_daemon_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    /// Include patterns (gitignore format)
    #[serde(default)]
    pub include_patterns: Vec<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            auto_start_with_mcp: false, // Opt-in by default
            project_path: None,
            debounce_ms: default_daemon_debounce_ms(),
            batch_timeout_ms: default_daemon_batch_timeout_ms(),
            health_check_interval_secs: default_daemon_health_check_interval(),
            languages: vec![],
            exclude_patterns: default_daemon_exclude_patterns(),
            include_patterns: vec![],
        }
    }
}

// Default value functions
fn default_embedding_provider() -> String {
    "auto".to_string()
}
fn default_lmstudio_url() -> String {
    "http://localhost:1234".to_string()
}
fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_jina_api_base() -> String {
    "https://api.jina.ai/v1".to_string()
}
fn default_jina_reranking_model() -> String {
    "jina-reranker-v3".to_string()
}
fn default_jina_reranking_top_n() -> usize {
    10
}
fn default_jina_task() -> String {
    "code.query".to_string()
}
fn default_embedding_dimension() -> usize {
    2048
} // jina-embeddings-v4
fn default_batch_size() -> usize {
    64
}
fn default_llm_provider() -> String {
    "lmstudio".to_string()
}
fn default_xai_base_url() -> String {
    "https://api.x.ai/v1".to_string()
}

fn default_context_window() -> usize {
    32000
} // DeepSeek Coder v2 Lite
fn default_temperature() -> f32 {
    0.1
}
fn default_insights_mode() -> String {
    "context-only".to_string()
}
fn default_max_tokens() -> usize {
    4096
}
fn default_timeout_secs() -> u64 {
    120
}
fn default_num_threads() -> usize {
    num_cpus::get()
}
fn default_cache_size_mb() -> usize {
    512
}
fn default_max_concurrent() -> usize {
    4
}
fn default_log_level() -> String {
    "warn".to_string()
} // Clean TUI output during indexing
fn default_log_format() -> String {
    "pretty".to_string()
}

// Daemon default functions
fn default_daemon_debounce_ms() -> u64 {
    30
}
fn default_daemon_batch_timeout_ms() -> u64 {
    200
}
fn default_daemon_health_check_interval() -> u64 {
    30
}
fn default_daemon_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/.git/**".to_string(),
        "**/build/**".to_string(),
        "**/.codegraph/**".to_string(),
        "**/dist/**".to_string(),
        "**/__pycache__/**".to_string(),
    ]
}

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
        } else {
            info!("   📄 Config file: NONE (using defaults)");
        }
        info!("   🤖 Embedding provider: {}", config.embedding.provider);
        info!("   🔧 Embedding model: {:?}", config.embedding.model);
        info!("   📐 Embedding dimension: {}", config.embedding.dimension);
        info!("   🌐 Ollama URL: {}", config.embedding.ollama_url);
        info!(
            "   💬 LLM insights: {}",
            if config.llm.enabled {
                "enabled"
            } else {
                "disabled (context-only)"
            }
        );

        Ok(Self {
            config,
            config_path,
        })
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
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError(e.to_string()))?;

        let config: CodeGraphConfig =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;

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
        if let Ok(dimension) = std::env::var("CODEGRAPH_EMBEDDING_DIMENSION") {
            if let Ok(dim) = dimension.parse() {
                config.embedding.dimension = dim;
            }
        }

        // Jina configuration
        if let Ok(key) = std::env::var("JINA_API_KEY") {
            config.embedding.jina_api_key = Some(key);
        }
        if let Ok(base) = std::env::var("JINA_API_BASE") {
            config.embedding.jina_api_base = base;
        }
        if let Ok(enable) = std::env::var("JINA_ENABLE_RERANKING") {
            config.embedding.jina_enable_reranking = enable.to_lowercase() == "true";
        }
        if let Ok(model) = std::env::var("JINA_RERANKING_MODEL") {
            config.embedding.jina_reranking_model = model;
        }
        if let Ok(top_n) = std::env::var("JINA_RERANKING_TOP_N") {
            if let Ok(n) = top_n.parse() {
                config.embedding.jina_reranking_top_n = n;
            }
        }
        if let Ok(chunking) = std::env::var("JINA_LATE_CHUNKING") {
            config.embedding.jina_late_chunking = chunking.to_lowercase() == "true";
        }
        if let Ok(task) = std::env::var("JINA_TASK") {
            config.embedding.jina_task = task;
        }

        // LLM configuration
        if let Ok(provider) =
            std::env::var("CODEGRAPH_LLM_PROVIDER").or_else(|_| std::env::var("LLM_PROVIDER"))
        {
            config.llm.provider = provider;
        }
        if let Ok(model) = std::env::var("CODEGRAPH_MODEL") {
            config.llm.model = Some(model);
            config.llm.enabled = true; // Enable if model specified
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

        if let Ok(effort) = std::env::var("CODEGRAPH_REASONING_EFFORT") {
            config.llm.reasoning_effort = Some(effort);
        }

        if let Ok(max_output) = std::env::var("MCP_CODE_AGENT_MAX_OUTPUT_TOKENS") {
            if let Ok(tokens) = max_output.parse() {
                config.llm.mcp_code_agent_max_output_tokens = Some(tokens);
            }
        }

        // Logging
        if let Ok(level) = std::env::var("RUST_LOG") {
            config.logging.level = level;
        }

        // Daemon configuration
        if let Ok(auto_start) = std::env::var("CODEGRAPH_DAEMON_AUTO_START") {
            config.daemon.auto_start_with_mcp =
                auto_start.to_lowercase() == "true" || auto_start == "1";
        }
        if let Ok(path) = std::env::var("CODEGRAPH_DAEMON_WATCH_PATH") {
            config.daemon.project_path = Some(PathBuf::from(path));
        }
        if let Ok(debounce) = std::env::var("CODEGRAPH_DAEMON_DEBOUNCE_MS") {
            if let Ok(ms) = debounce.parse() {
                config.daemon.debounce_ms = ms;
            }
        }
        if let Ok(batch) = std::env::var("CODEGRAPH_DAEMON_BATCH_TIMEOUT_MS") {
            if let Ok(ms) = batch.parse() {
                config.daemon.batch_timeout_ms = ms;
            }
        }

        config
    }

    /// Validate configuration
    fn validate_config(config: &CodeGraphConfig) -> Result<(), ConfigError> {
        // Validate embedding provider
        match config.embedding.provider.as_str() {
            "auto" | "onnx" | "ollama" | "openai" | "jina" | "lmstudio" => {}
            other => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid embedding provider: {}. Must be one of: auto, onnx, ollama, openai, jina, lmstudio",
                    other
                )))
            }
        }

        // Validate insights mode
        match config.llm.insights_mode.as_str() {
            "context-only" | "balanced" | "deep" => {}
            other => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid insights mode: {}. Must be one of: context-only, balanced, deep",
                    other
                )))
            }
        }

        // Validate log level
        match config.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            other => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid log level: {}. Must be one of: trace, debug, info, warn, error",
                    other
                )))
            }
        }

        Ok(())
    }

    /// Get the loaded configuration
    pub fn config(&self) -> &CodeGraphConfig {
        &self.config
    }

    /// Get the path to the config file that was loaded, if any
    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    /// Create a default config file
    pub fn create_default_config(path: &Path) -> Result<(), ConfigError> {
        let config = CodeGraphConfig::default();
        let toml_str =
            toml::to_string_pretty(&config).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::ReadError(e.to_string()))?;
        }

        std::fs::write(path, toml_str).map_err(|e| ConfigError::ReadError(e.to_string()))?;

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
            .args(["-s", "http://localhost:11434/api/tags"])
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
