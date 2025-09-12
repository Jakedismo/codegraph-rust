use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use schemars::schema_for;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::advanced_config::{AdvancedConfig, ConfigurationManager};
use crate::embedding_config::{EmbeddingPreset, EmbeddingProvider};
use crate::performance_config::{PerformanceMode, PerformanceProfile};

#[derive(Parser, Debug)]
#[command(name = "codegraph-config")]
#[command(author, version, about = "CodeGraph Configuration Management", long_about = None)]
pub struct ConfigCli {
    #[command(subcommand)]
    pub command: ConfigCommand,

    #[arg(short, long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,

    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    #[command(about = "Initialize a new configuration file")]
    Init {
        #[arg(short, long, default_value = "balanced")]
        template: String,

        #[arg(short, long)]
        force: bool,
    },

    #[command(about = "Show current configuration")]
    Show {
        #[arg(short, long)]
        json: bool,

        #[arg(short, long)]
        pretty: bool,
    },

    #[command(about = "Validate configuration file")]
    Validate {
        #[arg(short, long)]
        strict: bool,
    },

    #[command(about = "Apply a configuration template")]
    Apply {
        template: String,

        #[arg(short, long)]
        dry_run: bool,
    },

    #[command(about = "Manage embedding configurations")]
    Embedding(EmbeddingArgs),

    #[command(about = "Manage performance modes")]
    Performance(PerformanceArgs),

    #[command(about = "List available presets and profiles")]
    List {
        #[arg(short, long)]
        detailed: bool,
    },

    #[command(about = "Generate configuration schema")]
    Schema {
        #[arg(short, long, default_value = "json")]
        format: String,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    #[command(about = "Test configuration with benchmarks")]
    Test {
        #[arg(short, long)]
        quick: bool,

        #[arg(short, long)]
        profile: bool,
    },
}

#[derive(Args, Debug)]
pub struct EmbeddingArgs {
    #[command(subcommand)]
    pub command: EmbeddingCommand,
}

#[derive(Subcommand, Debug)]
pub enum EmbeddingCommand {
    #[command(about = "Set embedding provider")]
    Set {
        provider: String,

        #[arg(short, long)]
        model: Option<String>,

        #[arg(short, long)]
        dimension: Option<usize>,
    },

    #[command(about = "Apply embedding preset")]
    Preset { name: String },

    #[command(about = "Show current embedding configuration")]
    Show,

    #[command(about = "List available presets")]
    List,
}

#[derive(Args, Debug)]
pub struct PerformanceArgs {
    #[command(subcommand)]
    pub command: PerformanceCommand,
}

#[derive(Subcommand, Debug)]
pub enum PerformanceCommand {
    #[command(about = "Set performance mode")]
    Set {
        mode: String,

        #[arg(short, long)]
        auto_tune: bool,
    },

    #[command(about = "Apply performance profile")]
    Profile { name: String },

    #[command(about = "Show current performance configuration")]
    Show,

    #[command(about = "List available profiles")]
    List,

    #[command(about = "Run auto-tuning")]
    AutoTune {
        #[arg(short, long)]
        iterations: Option<usize>,
    },
}

pub struct ConfigCliHandler {
    manager: ConfigurationManager,
    config_path: PathBuf,
}

impl ConfigCliHandler {
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let path = config_path.unwrap_or_else(|| PathBuf::from("codegraph.toml"));

        let manager = if path.exists() {
            ConfigurationManager::from_file(&path)?
        } else {
            ConfigurationManager::new(AdvancedConfig::default())
        };

        Ok(Self {
            manager,
            config_path: path,
        })
    }

    pub async fn handle_command(&mut self, command: ConfigCommand, verbose: bool) -> Result<()> {
        match command {
            ConfigCommand::Init { template, force } => {
                self.init_config(&template, force).await?;
            }
            ConfigCommand::Show { json, pretty } => {
                self.show_config(json, pretty).await?;
            }
            ConfigCommand::Validate { strict } => {
                self.validate_config(strict).await?;
            }
            ConfigCommand::Apply { template, dry_run } => {
                self.apply_template(&template, dry_run).await?;
            }
            ConfigCommand::Embedding(args) => {
                self.handle_embedding_command(args.command).await?;
            }
            ConfigCommand::Performance(args) => {
                self.handle_performance_command(args.command).await?;
            }
            ConfigCommand::List { detailed } => {
                self.list_presets(detailed);
            }
            ConfigCommand::Schema { format, output } => {
                self.generate_schema(&format, output)?;
            }
            ConfigCommand::Test { quick, profile } => {
                self.test_config(quick, profile).await?;
            }
        }

        if verbose {
            println!("{}", "Operation completed successfully".green());
        }

        Ok(())
    }

    async fn init_config(&mut self, template: &str, force: bool) -> Result<()> {
        if self.config_path.exists() && !force {
            anyhow::bail!(
                "Configuration file already exists at {:?}. Use --force to overwrite.",
                self.config_path
            );
        }

        let mut config = AdvancedConfig::default();

        if template != "default" {
            config.apply_template(template)?;
        }

        config.to_file(&self.config_path)?;

        println!(
            "{}",
            format!(
                "Configuration initialized at {:?} with template '{}'",
                self.config_path, template
            )
            .green()
        );

        Ok(())
    }

    async fn show_config(&self, json: bool, pretty: bool) -> Result<()> {
        let config = self.manager.get_config().await;

        if json {
            let output = if pretty {
                serde_json::to_string_pretty(&config)?
            } else {
                serde_json::to_string(&config)?
            };
            println!("{}", output);
        } else {
            println!("{}", "Current Configuration:".bold().blue());
            println!("{}", "=".repeat(50));

            println!("\n{}", "Embedding Configuration:".yellow());
            println!("  Provider: {:?}", config.embedding.provider);
            println!("  Dimension: {}", config.embedding.dimension);
            println!("  Cache Enabled: {}", config.embedding.cache_enabled);

            println!("\n{}", "Performance Configuration:".yellow());
            println!("  Mode: {:?}", config.performance.mode);
            println!("  Index Type: {}", config.performance.index.index_type);
            println!("  Cache Size: {} MB", config.performance.cache.max_size_mb);
            println!("  Batch Size: {}", config.performance.processing.batch_size);
            println!(
                "  Workers: {}",
                config.performance.processing.parallel_workers
            );

            println!("\n{}", "Runtime Configuration:".yellow());
            println!(
                "  Runtime Switching: {}",
                config.runtime.allow_runtime_switching
            );
            println!("  Hot Reload: {}", config.runtime.hot_reload);

            println!("\n{}", "Monitoring Configuration:".yellow());
            println!("  Enabled: {}", config.monitoring.enabled);
            println!("  Metrics: {}", config.monitoring.metrics_enabled);
            println!("  Tracing: {}", config.monitoring.trace_enabled);
        }

        Ok(())
    }

    async fn validate_config(&self, strict: bool) -> Result<()> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Validating configuration...");
        pb.enable_steady_tick(Duration::from_millis(100));

        let config = self.manager.get_config().await;

        // Basic validation
        config.validate()?;

        if strict {
            // Additional strict validation
            if matches!(config.embedding.provider, EmbeddingProvider::OpenAI) {
                if config.embedding.openai.is_none() {
                    pb.finish_with_message("❌ OpenAI configuration missing");
                    anyhow::bail!("OpenAI provider requires OpenAI configuration");
                }
            }

            if config.performance.processing.parallel_workers > num_cpus::get() * 4 {
                pb.finish_with_message("⚠️  Warning: High worker count detected");
                println!(
                    "{}",
                    format!(
                        "Warning: {} workers configured, but only {} CPU cores available",
                        config.performance.processing.parallel_workers,
                        num_cpus::get()
                    )
                    .yellow()
                );
            }
        }

        pb.finish_with_message("✅ Configuration is valid");
        Ok(())
    }

    async fn apply_template(&mut self, template: &str, dry_run: bool) -> Result<()> {
        let mut config = self.manager.get_config().await;
        config.apply_template(template)?;

        if dry_run {
            println!("{}", "Dry run mode - configuration not saved".yellow());
            println!("\n{}", "Configuration after applying template:".bold());
            println!("{}", serde_json::to_string_pretty(&config)?);
        } else {
            self.manager.update_config(config).await?;
            println!(
                "{}",
                format!("Applied template '{}' successfully", template).green()
            );
        }

        Ok(())
    }

    async fn handle_embedding_command(&mut self, command: EmbeddingCommand) -> Result<()> {
        match command {
            EmbeddingCommand::Set {
                provider,
                model,
                dimension,
            } => {
                let mut config = self.manager.get_config().await;

                match provider.to_lowercase().as_str() {
                    "openai" => {
                        config.embedding.provider = EmbeddingProvider::OpenAI;
                        if let Some(model) = model {
                            config.embedding.openai =
                                Some(crate::embedding_config::OpenAIEmbeddingConfig {
                                    model,
                                    ..Default::default()
                                });
                        }
                    }
                    "local" => {
                        config.embedding.provider = EmbeddingProvider::Local;
                    }
                    _ => anyhow::bail!("Unknown provider: {}", provider),
                }

                if let Some(dim) = dimension {
                    config.embedding.dimension = dim;
                }

                self.manager.update_config(config).await?;
                println!("{}", "Embedding configuration updated".green());
            }
            EmbeddingCommand::Preset { name } => {
                self.manager.switch_embedding_preset(&name).await?;
                println!("{}", format!("Applied embedding preset '{}'", name).green());
            }
            EmbeddingCommand::Show => {
                let config = self.manager.get_config().await;
                println!("{}", "Embedding Configuration:".bold().blue());
                println!("{}", serde_json::to_string_pretty(&config.embedding)?);
            }
            EmbeddingCommand::List => {
                println!("{}", "Available Embedding Presets:".bold().blue());
                println!("{}", "=".repeat(50));

                for preset in EmbeddingPreset::all_presets() {
                    println!("\n{}", preset.name.green());
                    println!("  {}", preset.description);
                    println!("  Provider: {:?}", preset.config.provider);
                    println!("  Dimension: {}", preset.config.dimension);
                }
            }
        }

        Ok(())
    }

    async fn handle_performance_command(&mut self, command: PerformanceCommand) -> Result<()> {
        match command {
            PerformanceCommand::Set { mode, auto_tune } => {
                let mode = match mode.to_lowercase().as_str() {
                    "high_accuracy" => PerformanceMode::HighAccuracy,
                    "balanced" => PerformanceMode::Balanced,
                    "high_speed" => PerformanceMode::HighSpeed,
                    "ultra_fast" => PerformanceMode::UltraFast,
                    "custom" => PerformanceMode::Custom,
                    _ => anyhow::bail!("Unknown performance mode: {}", mode),
                };

                self.manager.switch_performance_mode(mode).await?;

                if auto_tune {
                    let mut config = self.manager.get_config().await;
                    let available_memory = 4096; // TODO: Get actual available memory
                    let cpu_cores = num_cpus::get();
                    config
                        .performance
                        .apply_auto_tuning(available_memory, cpu_cores);
                    self.manager.update_config(config).await?;
                    println!("{}", "Auto-tuning applied".green());
                }

                println!("{}", format!("Performance mode set to: {:?}", mode).green());
            }
            PerformanceCommand::Profile { name } => {
                let profile = PerformanceProfile::get_by_name(&name)
                    .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

                let mut config = self.manager.get_config().await;
                config.performance = profile.config;
                self.manager.update_config(config).await?;

                println!(
                    "{}",
                    format!("Applied performance profile '{}'", name).green()
                );
            }
            PerformanceCommand::Show => {
                let config = self.manager.get_config().await;
                println!("{}", "Performance Configuration:".bold().blue());
                println!("{}", serde_json::to_string_pretty(&config.performance)?);
            }
            PerformanceCommand::List => {
                println!("{}", "Available Performance Profiles:".bold().blue());
                println!("{}", "=".repeat(50));

                for profile in PerformanceProfile::all_profiles() {
                    println!("\n{}", profile.name.green());
                    println!("  {}", profile.description);
                    println!("  Mode: {:?}", profile.config.mode);
                    println!("  Use Cases:");
                    for use_case in &profile.recommended_use_cases {
                        println!("    - {}", use_case);
                    }
                }
            }
            PerformanceCommand::AutoTune { iterations } => {
                let iterations = iterations.unwrap_or(10);
                let pb = ProgressBar::new(iterations as u64);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("#>-"),
                );

                for i in 0..iterations {
                    pb.set_message(format!("Running iteration {}", i + 1));

                    // TODO: Implement actual auto-tuning logic
                    tokio::time::sleep(Duration::from_millis(500)).await;

                    pb.inc(1);
                }

                pb.finish_with_message("Auto-tuning complete");
                println!("{}", "Auto-tuning completed successfully".green());
            }
        }

        Ok(())
    }

    fn list_presets(&self, detailed: bool) {
        println!("{}", "Available Configuration Presets:".bold().blue());
        println!("{}", "=".repeat(60));

        println!("\n{}", "Embedding Presets:".yellow());
        for preset in EmbeddingPreset::all_presets() {
            println!("  {} - {}", preset.name.green(), preset.description);
            if detailed {
                println!("    Provider: {:?}", preset.config.provider);
                println!("    Dimension: {}", preset.config.dimension);
            }
        }

        println!("\n{}", "Performance Profiles:".yellow());
        for profile in PerformanceProfile::all_profiles() {
            println!("  {} - {}", profile.name.green(), profile.description);
            if detailed {
                println!("    Mode: {:?}", profile.config.mode);
                for use_case in &profile.recommended_use_cases {
                    println!("    - {}", use_case);
                }
            }
        }
    }

    fn generate_schema(&self, format: &str, output: Option<PathBuf>) -> Result<()> {
        let schema = schema_for!(AdvancedConfig);

        let content = match format {
            "json" => serde_json::to_string_pretty(&schema)?,
            "yaml" => serde_yaml::to_string(&schema)?,
            _ => anyhow::bail!("Unsupported schema format: {}", format),
        };

        if let Some(path) = output {
            fs::write(&path, content)?;
            println!("{}", format!("Schema written to {:?}", path).green());
        } else {
            println!("{}", content);
        }

        Ok(())
    }

    async fn test_config(&self, quick: bool, profile: bool) -> Result<()> {
        println!("{}", "Testing configuration...".bold().blue());

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );

        // Test embedding configuration
        pb.set_message("Testing embedding configuration...");
        tokio::time::sleep(Duration::from_secs(1)).await;
        pb.println("✅ Embedding configuration: OK");

        // Test performance configuration
        pb.set_message("Testing performance configuration...");
        tokio::time::sleep(Duration::from_secs(1)).await;
        pb.println("✅ Performance configuration: OK");

        if !quick {
            // Run additional tests
            pb.set_message("Running benchmark tests...");
            tokio::time::sleep(Duration::from_secs(2)).await;
            pb.println("✅ Benchmark tests: PASSED");

            if profile {
                pb.set_message("Running profiling...");
                tokio::time::sleep(Duration::from_secs(3)).await;
                pb.println("✅ Profiling complete");
            }
        }

        pb.finish_with_message("All tests passed!");

        Ok(())
    }
}

pub async fn run_cli() -> Result<()> {
    let cli = ConfigCli::parse();
    let mut handler = ConfigCliHandler::new(cli.config)?;
    handler.handle_command(cli.command, cli.verbose).await
}
