use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct SetupConfig {
    #[serde(default)]
    embedding: EmbeddingSetup,
    #[serde(default)]
    llm: LLMSetup,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct EmbeddingSetup {
    provider: String,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lmstudio_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ollama_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    openai_api_key: Option<String>,
    dimension: usize,
    batch_size: usize,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct LLMSetup {
    enabled: bool,
    provider: String,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lmstudio_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ollama_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    openai_compatible_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anthropic_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    openai_api_key: Option<String>,
    context_window: usize,
    temperature: f32,
    max_tokens: usize,
    timeout_secs: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔═══════════════════════════════════════════════════╗");
    println!("║   🚀 CodeGraph Configuration Wizard              ║");
    println!("║   Setup your embedding and LLM providers         ║");
    println!("╚═══════════════════════════════════════════════════╝\n");

    let config = run_setup_wizard().await?;
    save_config(&config)?;

    println!("\n✅ Configuration saved successfully!");
    println!("📄 Config file: ~/.codegraph/config.toml");
    println!("\n💡 You can now start using CodeGraph with your configured providers.");

    Ok(())
}

async fn run_setup_wizard() -> Result<SetupConfig> {
    let theme = ColorfulTheme::default();

    // Embedding provider setup
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 EMBEDDING MODEL CONFIGURATION");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let embedding_providers = vec![
        "ONNX (Local, CPU/GPU)",
        "Ollama (Local, requires Ollama)",
        "LM Studio (Local, requires LM Studio)",
        "OpenAI (Cloud, requires API key)",
    ];

    let embedding_selection = Select::with_theme(&theme)
        .with_prompt("Select your embedding provider")
        .items(&embedding_providers)
        .default(0)
        .interact()?;

    let mut embedding_setup = EmbeddingSetup {
        batch_size: 64,
        ..Default::default()
    };

    match embedding_selection {
        0 => {
            // ONNX
            embedding_setup.provider = "onnx".to_string();
            println!("\n📝 ONNX will use local models from HuggingFace Hub.");

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("Model identifier (or press Enter for default)")
                .default("sentence-transformers/all-MiniLM-L6-v2".to_string())
                .interact_text()?;

            embedding_setup.model = Some(model);
            embedding_setup.dimension = 384; // all-MiniLM-L6-v2 dimension
        }
        1 => {
            // Ollama
            embedding_setup.provider = "ollama".to_string();

            let url = Input::<String>::with_theme(&theme)
                .with_prompt("Ollama URL")
                .default("http://localhost:11434".to_string())
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("Model name")
                .default("nomic-embed-code".to_string())
                .interact_text()?;

            embedding_setup.ollama_url = Some(url);
            embedding_setup.model = Some(model);
            embedding_setup.dimension = 768; // nomic-embed-code dimension
        }
        2 => {
            // LM Studio
            embedding_setup.provider = "lmstudio".to_string();

            let url = Input::<String>::with_theme(&theme)
                .with_prompt("LM Studio URL")
                .default("http://localhost:1234".to_string())
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("Model name")
                .default("jinaai/jina-embeddings-v3".to_string())
                .interact_text()?;

            embedding_setup.lmstudio_url = Some(url);
            embedding_setup.model = Some(model);
            embedding_setup.dimension = 1536; // jina-embeddings-v3 dimension
        }
        3 => {
            // OpenAI
            embedding_setup.provider = "openai".to_string();

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("OpenAI API Key (or set OPENAI_API_KEY env var)")
                .allow_empty(true)
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("Model name")
                .default("text-embedding-3-small".to_string())
                .interact_text()?;

            if !api_key.is_empty() {
                embedding_setup.openai_api_key = Some(api_key);
            }
            embedding_setup.model = Some(model);
            embedding_setup.dimension = 1536; // text-embedding-3-small dimension
        }
        _ => unreachable!(),
    }

    // LLM provider setup
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🤖 LLM PROVIDER CONFIGURATION");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let enable_llm = Confirm::with_theme(&theme)
        .with_prompt("Enable LLM for code intelligence and insights?")
        .default(true)
        .interact()?;

    let mut llm_setup = LLMSetup {
        enabled: enable_llm,
        temperature: 0.1,
        max_tokens: 4096,
        timeout_secs: 120,
        ..Default::default()
    };

    if enable_llm {
        let llm_providers = vec![
            "Ollama (Local, e.g., Qwen2.5-Coder)",
            "LM Studio (Local, e.g., DeepSeek Coder)",
            "Anthropic Claude (Cloud, requires API key)",
            "OpenAI (Cloud, requires API key)",
            "OpenAI-compatible (Custom endpoint)",
        ];

        let llm_selection = Select::with_theme(&theme)
            .with_prompt("Select your LLM provider")
            .items(&llm_providers)
            .default(0)
            .interact()?;

        match llm_selection {
            0 => {
                // Ollama
                llm_setup.provider = "ollama".to_string();

                let url = Input::<String>::with_theme(&theme)
                    .with_prompt("Ollama URL")
                    .default("http://localhost:11434".to_string())
                    .interact_text()?;

                let model = Input::<String>::with_theme(&theme)
                    .with_prompt("Model name")
                    .default("qwen2.5-coder:14b".to_string())
                    .interact_text()?;

                llm_setup.ollama_url = Some(url);
                llm_setup.model = Some(model);
                llm_setup.context_window = 128_000;
            }
            1 => {
                // LM Studio
                llm_setup.provider = "lmstudio".to_string();

                let url = Input::<String>::with_theme(&theme)
                    .with_prompt("LM Studio URL")
                    .default("http://localhost:1234".to_string())
                    .interact_text()?;

                let model = Input::<String>::with_theme(&theme)
                    .with_prompt("Model name")
                    .default("lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF".to_string())
                    .interact_text()?;

                llm_setup.lmstudio_url = Some(url);
                llm_setup.model = Some(model);
                llm_setup.context_window = 32_000;
            }
            2 => {
                // Anthropic
                llm_setup.provider = "anthropic".to_string();

                let api_key = Input::<String>::with_theme(&theme)
                    .with_prompt("Anthropic API Key (or set ANTHROPIC_API_KEY env var)")
                    .allow_empty(true)
                    .interact_text()?;

                let models = vec![
                    "claude-3-5-sonnet-20241022",
                    "claude-3-5-haiku-20241022",
                    "claude-3-opus-20240229",
                ];

                let model_selection = Select::with_theme(&theme)
                    .with_prompt("Select Claude model")
                    .items(&models)
                    .default(0)
                    .interact()?;

                if !api_key.is_empty() {
                    llm_setup.anthropic_api_key = Some(api_key);
                }
                llm_setup.model = Some(models[model_selection].to_string());
                llm_setup.context_window = 200_000;
            }
            3 => {
                // OpenAI
                llm_setup.provider = "openai".to_string();

                let api_key = Input::<String>::with_theme(&theme)
                    .with_prompt("OpenAI API Key (or set OPENAI_API_KEY env var)")
                    .allow_empty(true)
                    .interact_text()?;

                let models = vec!["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4"];

                let model_selection = Select::with_theme(&theme)
                    .with_prompt("Select OpenAI model")
                    .items(&models)
                    .default(0)
                    .interact()?;

                if !api_key.is_empty() {
                    llm_setup.openai_api_key = Some(api_key);
                }
                llm_setup.model = Some(models[model_selection].to_string());
                llm_setup.context_window = 128_000;
            }
            4 => {
                // OpenAI-compatible
                llm_setup.provider = "openai-compatible".to_string();

                let url = Input::<String>::with_theme(&theme)
                    .with_prompt("API Base URL (e.g., http://localhost:1234/v1)")
                    .interact_text()?;

                let model = Input::<String>::with_theme(&theme)
                    .with_prompt("Model name")
                    .interact_text()?;

                let api_key = Input::<String>::with_theme(&theme)
                    .with_prompt("API Key (optional, leave empty if not required)")
                    .allow_empty(true)
                    .interact_text()?;

                llm_setup.openai_compatible_url = Some(url);
                llm_setup.model = Some(model);
                if !api_key.is_empty() {
                    llm_setup.openai_api_key = Some(api_key);
                }
                llm_setup.context_window = 32_000;
            }
            _ => unreachable!(),
        }

        // Advanced settings
        println!("\n⚙️  Advanced LLM Settings (press Enter for defaults)");

        let temperature: f32 = Input::with_theme(&theme)
            .with_prompt("Temperature (0.0-2.0)")
            .default(0.1)
            .interact_text()?;

        let max_tokens: usize = Input::with_theme(&theme)
            .with_prompt("Max tokens to generate")
            .default(4096)
            .interact_text()?;

        llm_setup.temperature = temperature;
        llm_setup.max_tokens = max_tokens;
    }

    Ok(SetupConfig {
        embedding: embedding_setup,
        llm: llm_setup,
    })
}

fn save_config(config: &SetupConfig) -> Result<()> {
    // Determine config path
    let config_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".codegraph");

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

    let config_path = config_dir.join("config.toml");

    // Serialize config to TOML
    let toml_content = toml::to_string_pretty(config).context("Failed to serialize config")?;

    // Write to file
    fs::write(&config_path, toml_content).context("Failed to write config file")?;

    println!("\n📝 Configuration preview:");
    println!("─────────────────────────────────────────────────");
    println!("{}", toml::to_string_pretty(config).unwrap());
    println!("─────────────────────────────────────────────────");

    Ok(())
}
