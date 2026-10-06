use crate::rerank_config::RerankConfig;
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

    /// Reranking provider configuration
    #[serde(default)]
    pub rerank: RerankConfig,

    /// LLM configuration for insights
    #[serde(default)]
    pub llm: LLMConfig,

    /// Performance and resource settings
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Indexing configuration
    #[serde(default)]
    pub indexing: IndexingConfig,

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

    /// Jina late chunking
    #[serde(default)]
    pub jina_late_chunking: bool,

    /// Jina task type; "auto" selects a passage task supported by the model.
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

    /// Use legacy Chat Completions API instead of modern Responses API
    /// Set CODEGRAPH_USE_COMPLETIONS_API=true to enable backward compatibility
    #[serde(default)]
    pub use_completions_api: bool,
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
            use_completions_api: false, // Default to Responses API
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

/// Indexing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    /// Indexing tier: fast | balanced | full
    #[serde(default)]
    pub tier: IndexingTier,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            tier: IndexingTier::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexingTier {
    Fast,
    Balanced,
    Full,
}

impl Default for IndexingTier {
    fn default() -> Self {
        IndexingTier::Fast
    }
}

impl std::str::FromStr for IndexingTier {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "fast" => Ok(IndexingTier::Fast),
            "balanced" => Ok(IndexingTier::Balanced),
            "full" => Ok(IndexingTier::Full),
            other => Err(format!("Invalid indexing tier: {}", other)),
        }
    }
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
fn default_jina_task() -> String {
    "auto".to_string()
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

/// Context window assumed when none is configured.
pub const DEFAULT_CONTEXT_WINDOW: usize = 128_000;

fn default_context_window() -> usize {
    DEFAULT_CONTEXT_WINDOW
}

/// `[llm]` settings exactly as written in a config file; `None` means the key is absent.
/// All fields are `None` when the section sets `enabled = false`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplicitLlmSettings {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub context_window: Option<usize>,
    pub ollama_url: Option<String>,
    pub lmstudio_url: Option<String>,
    pub openai_compatible_url: Option<String>,
}

impl ExplicitLlmSettings {
    /// Extract the `[llm]` keys present in a TOML document. Invalid TOML and keys of
    /// the wrong type are treated as absent; full validation is `ConfigManager::load`'s job.
    pub fn from_toml_str(content: &str) -> Self {
        let Ok(document) = content.parse::<toml::Table>() else {
            return Self::default();
        };
        let Some(llm) = document.get("llm").and_then(|v| v.as_table()) else {
            return Self::default();
        };
        // `enabled = false` switches the section off. `codegraph config init` writes every
        // default explicitly (including provider = "lmstudio") with enabled = false, and
        // that generated file must not count as the user's choice of provider.
        if llm.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            return Self::default();
        }
        let text = |key: &str| {
            llm.get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        Self {
            provider: text("provider"),
            model: text("model"),
            context_window: llm
                .get("context_window")
                .and_then(|v| v.as_integer())
                .and_then(|v| usize::try_from(v).ok())
                .filter(|v| *v > 0),
            ollama_url: text("ollama_url"),
            lmstudio_url: text("lmstudio_url"),
            openai_compatible_url: text("openai_compatible_url"),
        }
    }
}
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
    /// 1. Environment variables (initialize dotenv before starting workers)
    /// 2. Config file (.codegraph.toml)
    /// 3. Sensible defaults
    pub fn load() -> Result<Self, ConfigError> {
        info!("🔧 Loading CodeGraph configuration...");

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

    /// Load project `.env` or user `.codegraph.env` before starting workers.
    /// `load()` itself only reads configuration and never mutates the environment.
    ///
    /// # Safety
    /// Call only from a single-threaded process entry point, before other threads
    /// can access the environment (including through foreign libraries).
    pub unsafe fn initialize_environment() {
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
    /// 1. CODEGRAPH_CONFIG_PATH (explicit CLI/environment selection)
    /// 2. ./.codegraph.toml (current directory)
    /// 3. ~/.codegraph/config.toml (user config)
    /// 4. Use defaults
    /// Locate the configuration file: an explicit CLI/environment selection, then the
    /// project file in the working directory, then the user file.
    fn config_file_path() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("CODEGRAPH_CONFIG_PATH") {
            return Some(PathBuf::from(path));
        }
        let local_config = Path::new(".codegraph.toml");
        if local_config.exists() {
            return Some(local_config.to_path_buf());
        }
        dirs::home_dir()
            .map(|home| home.join(".codegraph").join("config.toml"))
            .filter(|user_config| user_config.exists())
    }

    fn load_config_file() -> Result<(CodeGraphConfig, Option<PathBuf>), ConfigError> {
        match Self::config_file_path() {
            Some(path) => {
                let config = Self::read_toml_file(&path)?;
                Ok((config, Some(path)))
            }
            None => {
                info!("📋 No config file found, using defaults");
                Ok((CodeGraphConfig::default(), None))
            }
        }
    }

    /// The `[llm]` keys the user actually wrote in the config file, without defaults.
    ///
    /// The agent backend layers these under its environment variables. Reading only
    /// explicit keys matters because `LLMConfig` fills unset fields with defaults (for
    /// example `provider = "lmstudio"`), which must not override provider detection
    /// from API keys. A missing or unreadable file yields empty settings.
    pub fn explicit_llm_settings() -> ExplicitLlmSettings {
        let Some(path) = Self::config_file_path() else {
            return ExplicitLlmSettings::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => ExplicitLlmSettings::from_toml_str(&content),
            Err(e) => {
                warn!(
                    "Could not read {} for [llm] settings: {}",
                    path.display(),
                    e
                );
                ExplicitLlmSettings::default()
            }
        }
    }

    /// Context window the agent and its tool-result limits are sized for:
    /// `CODEGRAPH_CONTEXT_WINDOW`, then `CODEGRAPH_LLM_CONTEXT_WINDOW`, then
    /// `[llm] context_window` in the config file, then the default.
    pub fn agent_context_window() -> usize {
        std::env::var("CODEGRAPH_CONTEXT_WINDOW")
            .or_else(|_| std::env::var("CODEGRAPH_LLM_CONTEXT_WINDOW"))
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .or_else(|| Self::explicit_llm_settings().context_window)
            .unwrap_or(DEFAULT_CONTEXT_WINDOW)
    }

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
        // Resolve batch aliases once, before explicit CLI overrides are applied.
        if let Some(size) = [
            "CODEGRAPH_EMBEDDINGS_BATCH_SIZE",
            "CODEGRAPH_EMBEDDING_BATCH_SIZE",
        ]
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|size| *size > 0)
        }) {
            config.embedding.batch_size = size;
        }

        // Jina configuration
        if let Ok(key) = std::env::var("JINA_API_KEY") {
            config.embedding.jina_api_key = Some(key);
        }
        if let Ok(base) = std::env::var("JINA_API_BASE") {
            config.embedding.jina_api_base = base;
        }
        if let Ok(enable) = std::env::var("JINA_ENABLE_RERANKING") {
            if enable.to_lowercase() == "true" {
                config.rerank.provider = crate::RerankProvider::Jina;
            }
        }
        if let Ok(model) = std::env::var("JINA_RERANKING_MODEL") {
            if config.rerank.jina.is_none() {
                config.rerank.jina = Some(crate::JinaRerankConfig::default());
            }
            if let Some(ref mut jina) = config.rerank.jina {
                jina.model = model;
            }
        }
        if let Ok(top_n) = std::env::var("JINA_RERANKING_TOP_N") {
            if let Ok(n) = top_n.parse() {
                config.rerank.top_n = n;
            }
        }

        // Generic rerank toggles (CODEGRAPH_*), higher priority than JINA_ENABLE_RERANKING
        if let Ok(enable) = std::env::var("CODEGRAPH_ENABLE_RERANKING") {
            if enable.to_lowercase() == "true" {
                // Default to Jina unless a provider is explicitly set elsewhere
                if matches!(config.rerank.provider, crate::RerankProvider::None) {
                    config.rerank.provider = crate::RerankProvider::Jina;
                }
            } else {
                config.rerank.provider = crate::RerankProvider::None;
            }
        }

        if let Ok(top_n) = std::env::var("CODEGRAPH_RERANKING_CANDIDATES") {
            if let Ok(n) = top_n.parse() {
                config.rerank.top_n = n;
            }
        }
        if let Ok(chunking) = std::env::var("JINA_LATE_CHUNKING") {
            config.embedding.jina_late_chunking = chunking.to_lowercase() == "true";
        }
        if let Ok(task) = std::env::var("JINA_API_TASK").or_else(|_| std::env::var("JINA_TASK")) {
            config.embedding.jina_task = task;
        }

        // LLM configuration
        if let Ok(provider) =
            std::env::var("CODEGRAPH_LLM_PROVIDER").or_else(|_| std::env::var("LLM_PROVIDER"))
        {
            config.llm.provider = provider;
        }
        // Same precedence the agent backend uses, so status output names the model it requests.
        if let Ok(model) = std::env::var("CODEGRAPH_LLM_MODEL")
            .or_else(|_| std::env::var("CODEGRAPH_AGENT_MODEL"))
            .or_else(|_| std::env::var("CODEGRAPH_MODEL"))
        {
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

        // API selection
        if let Ok(use_completions) = std::env::var("CODEGRAPH_USE_COMPLETIONS_API") {
            config.llm.use_completions_api =
                use_completions.to_lowercase() == "true" || use_completions == "1";
        }

        // Indexing configuration
        if let Ok(tier) = std::env::var("CODEGRAPH_INDEX_TIER") {
            match tier.parse::<IndexingTier>() {
                Ok(parsed) => config.indexing.tier = parsed,
                Err(err) => warn!("Invalid CODEGRAPH_INDEX_TIER: {}", err),
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
                )));
            }
        }

        // Validate insights mode
        match config.llm.insights_mode.as_str() {
            "context-only" | "balanced" | "deep" => {}
            other => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid insights mode: {}. Must be one of: context-only, balanced, deep",
                    other
                )));
            }
        }

        // Warn about deprecated API usage
        if config.llm.use_completions_api {
            tracing::warn!(
                "⚠️  Using legacy Chat Completions API. \
                 Consider upgrading to Responses API for better performance. \
                 Set CODEGRAPH_USE_COMPLETIONS_API=false or remove the variable."
            );
        }

        // Validate log level
        match config.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            other => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid log level: {}. Must be one of: trace, debug, info, warn, error",
                    other
                )));
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
        assert_eq!(config.indexing.tier, IndexingTier::Fast);
    }

    #[test]
    fn test_indexing_tier_env_override() {
        if !test_env::run(
            concat!(module_path!(), "::test_indexing_tier_env_override"),
            &[("CODEGRAPH_INDEX_TIER", Some("balanced"))],
        ) {
            return;
        }

        let config = ConfigManager::apply_env_overrides(CodeGraphConfig::default());
        assert_eq!(config.indexing.tier, IndexingTier::Balanced);
    }

    #[test]
    fn test_explicit_llm_settings_reads_only_present_keys() {
        let settings = ExplicitLlmSettings::from_toml_str(
            r#"
            [embedding]
            provider = "ollama"

            [llm]
            provider = "anthropic"
            model = "claude-sonnet-4"
            context_window = 200000
            ollama_url = "  "
            "#,
        );
        assert_eq!(settings.provider.as_deref(), Some("anthropic"));
        assert_eq!(settings.model.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(settings.context_window, Some(200_000));
        assert_eq!(settings.ollama_url, None);
        assert_eq!(settings.lmstudio_url, None);
    }

    #[test]
    fn test_explicit_llm_settings_ignores_disabled_and_generated_defaults() {
        let disabled = ExplicitLlmSettings::from_toml_str(
            "[llm]\nenabled = false\nprovider = \"lmstudio\"\nmodel = \"m\"\n",
        );
        assert_eq!(disabled, ExplicitLlmSettings::default());

        // What `codegraph config init` writes: every default spelled out, enabled = false.
        let generated = toml::to_string_pretty(&CodeGraphConfig::default()).unwrap();
        assert!(generated.contains("provider = \"lmstudio\""));
        assert_eq!(
            ExplicitLlmSettings::from_toml_str(&generated),
            ExplicitLlmSettings::default()
        );

        let enabled =
            ExplicitLlmSettings::from_toml_str("[llm]\nenabled = true\nprovider = \"ollama\"\n");
        assert_eq!(enabled.provider.as_deref(), Some("ollama"));
    }

    #[test]
    fn test_explicit_llm_settings_absent_or_invalid() {
        assert_eq!(
            ExplicitLlmSettings::from_toml_str("[embedding]\nprovider = \"ollama\"\n"),
            ExplicitLlmSettings::default()
        );
        assert_eq!(
            ExplicitLlmSettings::from_toml_str("not = [valid"),
            ExplicitLlmSettings::default()
        );
        let wrong_types =
            ExplicitLlmSettings::from_toml_str("[llm]\nprovider = 3\ncontext_window = \"big\"\n");
        assert_eq!(wrong_types, ExplicitLlmSettings::default());
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

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
