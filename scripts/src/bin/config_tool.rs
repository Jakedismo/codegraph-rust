use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codegraph_core::{crypto, Settings};
use schemars::schema_for;

#[derive(Parser)]
#[command(name = "config_tool", about = "CodeGraph configuration utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate JSON schema for Settings into config/schema.json
    GenerateSchema {
        #[arg(long, default_value = "config/schema.json")]
        out: PathBuf,
    },
    /// Encrypt a plaintext TOML/JSON/YAML file to an encoded secrets file
    Encrypt {
        /// Base64-encoded 32-byte key (or set CONFIG_ENC_KEY)
        #[arg(long)]
        key: Option<String>,
        /// Input plaintext file
        #[arg(long)]
        input: PathBuf,
        /// Output encrypted file
        #[arg(long, default_value = "config/secrets.enc")]
        output: PathBuf,
    },
    /// Decrypt an encoded secrets file to stdout
    Decrypt {
        /// Base64-encoded 32-byte key (or set CONFIG_ENC_KEY)
        #[arg(long)]
        key: Option<String>,
        /// Input encrypted file
        #[arg(long, default_value = "config/secrets.enc")]
        input: PathBuf,
    },
    /// Generate a random encryption key (base64 32 bytes)
    GenerateKey,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::GenerateSchema { out } => {
            let schema = schema_for!(Settings);
            let json = serde_json::to_string_pretty(&schema)?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, json)?;
            println!("Wrote schema to {:?}", out);
        }
        Commands::Encrypt { key, input, output } => {
            let key_b64 = key
                .or_else(|| std::env::var("CONFIG_ENC_KEY").ok())
                .context("Provide --key or set CONFIG_ENC_KEY")?;
            let data = fs::read(&input).with_context(|| format!("reading input: {:?}", input))?;
            let enc = crypto::encrypt_bytes(&key_b64, &data)?;
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, enc)?;
            println!("Encrypted secrets to {:?}", output);
        }
        Commands::Decrypt { key, input } => {
            let key_b64 = key
                .or_else(|| std::env::var("CONFIG_ENC_KEY").ok())
                .context("Provide --key or set CONFIG_ENC_KEY")?;
            let data = fs::read(&input).with_context(|| format!("reading input: {:?}", input))?;
            let pt = crypto::decrypt_bytes(&key_b64, &data)?;
            println!("{}", String::from_utf8_lossy(&pt));
        }
        Commands::GenerateKey => {
            println!("{}", crypto::generate_key());
        }
    }
    Ok(())
}
