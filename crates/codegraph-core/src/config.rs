use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use config as cfg;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 3000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RocksDbConfig {
    pub path: String,
    #[serde(default)]
    pub read_only: bool,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            path: "data/graph.db".into(),
            read_only: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VectorConfig {
    pub dimension: usize,
    #[serde(default = "VectorConfig::default_index")]
    pub index: String,
}

impl VectorConfig {
    fn default_index() -> String {
        "ivf_flat".to_string()
    }
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            dimension: 384,
            index: Self::default_index(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LoggingConfig {
    #[serde(default = "LoggingConfig::default_level")]
    pub level: String,
}

impl LoggingConfig {
    fn default_level() -> String {
        "info".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SecurityConfig {
    #[serde(default)]
    pub require_auth: bool,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default = "SecurityConfig::default_rate_limit")]
    pub rate_limit_per_minute: u32,
}

impl SecurityConfig {
    fn default_rate_limit() -> u32 {
        1200
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SecretsConfig {
    // Do not serialize secrets; allow deserialization from config/env only.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub openai_api_key: Option<SecretString>,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub jwt_secret: Option<SecretString>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Settings {
    #[serde(default = "Settings::default_env")]
    pub env: String,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub rocksdb: RocksDbConfig,
    #[serde(default)]
    pub vector: VectorConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub secrets: SecretsConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            env: Self::default_env(),
            server: ServerConfig::default(),
            rocksdb: RocksDbConfig::default(),
            vector: VectorConfig::default(),
            logging: LoggingConfig::default(),
            security: SecurityConfig::default(),
            secrets: SecretsConfig::default(),
        }
    }
}

impl Settings {
    fn default_env() -> String {
        env::var("APP_ENV")
            .ok()
            .or_else(|| env::var("RUST_ENV").ok())
            .unwrap_or_else(|| "development".to_string())
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.server.host.trim().is_empty(),
            "server.host cannot be empty"
        );
        anyhow::ensure!(self.server.port > 0, "server.port must be > 0");
        anyhow::ensure!(
            self.vector.dimension > 0 && self.vector.dimension <= 8192,
            "vector.dimension must be 1..=8192"
        );
        Ok(())
    }
}

pub struct ConfigManager {
    settings: Arc<RwLock<Settings>>,
    config_dir: PathBuf,
    env: String,
    _watcher: Mutex<Option<RecommendedWatcher>>,
}

impl std::fmt::Debug for ConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigManager")
            .field("config_dir", &self.config_dir)
            .field("env", &self.env)
            .finish()
    }
}

impl ConfigManager {
    pub fn settings(&self) -> &Arc<RwLock<Settings>> {
        &self.settings
    }

    pub fn new_watching(env_override: Option<String>) -> Result<Arc<Self>> {
        let env_name = env_override.unwrap_or_else(|| Settings::default_env());
        let config_dir = Self::default_config_dir();
        let loaded = Self::load_from_sources(&config_dir, &env_name)?;
        loaded.validate()?;
        let settings = Arc::new(RwLock::new(loaded));

        let manager = Arc::new(Self {
            settings,
            config_dir: config_dir.clone(),
            env: env_name.clone(),
            _watcher: Mutex::new(None),
        });
        if let Ok(w) = Self::spawn_watcher(manager.clone()) {
            *manager._watcher.lock() = Some(w);
        }
        Ok(manager)
    }

    pub fn default_config_dir() -> PathBuf {
        // Prefer ./config/, fallback to current dir
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let dir = cwd.join("config");
        if dir.exists() {
            dir
        } else {
            cwd
        }
    }

    pub fn load_from_sources(config_dir: &Path, env_name: &str) -> Result<Settings> {
        let mut builder = cfg::Config::builder()
            .add_source(cfg::File::from(config_dir.join("default.toml")).required(false))
            .add_source(cfg::File::from(config_dir.join("default.yaml")).required(false))
            .add_source(cfg::File::from(config_dir.join("default.yml")).required(false))
            .add_source(cfg::File::from(config_dir.join("default.json")).required(false))
            .add_source(
                cfg::File::from(config_dir.join(format!("{}.toml", env_name))).required(false),
            )
            .add_source(
                cfg::File::from(config_dir.join(format!("{}.yaml", env_name))).required(false),
            )
            .add_source(
                cfg::File::from(config_dir.join(format!("{}.yml", env_name))).required(false),
            )
            .add_source(
                cfg::File::from(config_dir.join(format!("{}.json", env_name))).required(false),
            )
            .add_source(cfg::File::from(config_dir.join("local.toml")).required(false))
            .add_source(cfg::Environment::with_prefix("CODEGRAPH").separator("__"));

        // Optional: merge decrypted secrets
        if let Some(secrets) = Self::try_load_encrypted_secrets(config_dir).transpose()? {
            builder = builder.add_source(
                cfg::File::from(secrets)
                    .format(cfg::FileFormat::Toml)
                    .required(true),
            );
        }

        let settings: Settings = builder
            .build()
            .context("building configuration")?
            .try_deserialize()
            .context("deserializing configuration")?;
        Ok(settings)
    }

    fn spawn_watcher(this: Arc<Self>) -> Result<RecommendedWatcher> {
        let config_dir_closure = this.config_dir.clone();
        let config_dir_watch = this.config_dir.clone();
        let env_name = this.env.clone();
        let settings = this.settings.clone();
        let mut watcher = notify::recommended_watcher(move |res| {
            match res {
                Ok(_event) => {
                    // Any change triggers reload with debounce at call site if needed
                    if let Ok(new_settings) =
                        Self::load_from_sources(&config_dir_closure, &env_name)
                    {
                        if let Err(e) = new_settings.validate() {
                            warn!("config validation failed on reload: {:?}", e);
                            return;
                        }
                        let mut w = settings.blocking_write();
                        *w = new_settings;
                        info!("Configuration reloaded from {:?}", config_dir_closure);
                    }
                }
                Err(e) => error!("config watcher error: {:?}", e),
            }
        })?;
        watcher.watch(&config_dir_watch, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    }

    fn try_load_encrypted_secrets(config_dir: &Path) -> Option<Result<PathBuf>> {
        let enc_path = ["secrets.enc", "secrets.toml.enc", "secrets.enc.toml"]
            .into_iter()
            .map(|n| config_dir.join(n))
            .find(|p| p.exists())?;
        Some(Self::decrypt_to_temp_toml(&enc_path))
    }

    fn decrypt_to_temp_toml(enc_file: &Path) -> Result<PathBuf> {
        let key_b64 = env::var("CONFIG_ENC_KEY")
            .context("CONFIG_ENC_KEY env var not set (base64 32 bytes)")?;
        let key_bytes = general_purpose::STANDARD.decode(key_b64.trim())?;
        anyhow::ensure!(
            key_bytes.len() == 32,
            "CONFIG_ENC_KEY must decode to 32 bytes"
        );
        let key = Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);

        let data = fs::read(enc_file)
            .with_context(|| format!("reading encrypted secrets file {:?}", enc_file))?;
        let decoded = general_purpose::STANDARD.decode(data)?;
        anyhow::ensure!(decoded.len() > 12, "encrypted secrets too short");
        let (nonce_bytes, ct) = decoded.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ct.as_ref())
            .context("decrypting secrets")?;

        let tmp = env::temp_dir().join("codegraph_secrets.toml");
        fs::write(&tmp, plaintext)?;
        Ok(tmp)
    }
}

// Helpers for the CLI tool
pub mod crypto {
    use super::*;
    use rand::rngs::OsRng;
    use rand::TryRngCore;

    pub fn generate_key() -> String {
        let mut key = [0u8; 32];
        // rand 0.9 OsRng implements RngCore; use trait method on a mutable instance
        let mut rng = OsRng;
        // rand 0.9 switched to Result-returning try_fill_bytes
        rng.try_fill_bytes(&mut key).expect("OsRng available");
        general_purpose::STANDARD.encode(key)
    }

    pub fn encrypt_bytes(key_b64: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = general_purpose::STANDARD.decode(key_b64.trim())?;
        anyhow::ensure!(key.len() == 32, "key must be 32 bytes (base64)");
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let mut nonce = [0u8; 12];
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut nonce).expect("OsRng available");
        let nonce_obj = Nonce::from_slice(&nonce);
        let mut out = Vec::with_capacity(12 + plaintext.len() + 16);
        out.extend_from_slice(&nonce);
        let ct = cipher
            .encrypt(nonce_obj, plaintext)
            .context("encryption failed")?;
        out.extend_from_slice(&ct);
        Ok(general_purpose::STANDARD.encode(out).into_bytes())
    }

    pub fn decrypt_bytes(key_b64: &str, ciphertext_b64: &[u8]) -> Result<Vec<u8>> {
        let key = general_purpose::STANDARD.decode(key_b64.trim())?;
        anyhow::ensure!(key.len() == 32, "key must be 32 bytes (base64)");
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let decoded = general_purpose::STANDARD.decode(ciphertext_b64)?;
        anyhow::ensure!(decoded.len() > 12, "ciphertext too short");
        let (nonce_bytes, ct) = decoded.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let pt = cipher
            .decrypt(nonce, ct.as_ref())
            .context("decryption failed")?;
        Ok(pt)
    }
}
