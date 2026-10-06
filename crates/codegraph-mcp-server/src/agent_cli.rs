// ABOUTME: One-shot CLI access to the four public agentic tools.
// ABOUTME: Writes answers to stdout and diagnostics to stderr without an MCP server.

use crate::{CodeGraphMCPServer, agent_hooks::CLI_INSTRUCTIONS, agentic_tools::AgenticTool};
use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::Value;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Gather context, discover code, or answer a codebase question.
    Context {
        #[command(flatten)]
        request: AgentQuery,
        #[arg(long, value_parser = ["search", "builder", "question"])]
        focus: Option<String>,
    },
    /// Assess dependencies and call flows affected by a proposed change.
    Impact {
        #[command(flatten)]
        request: AgentQuery,
        #[arg(long, value_parser = ["dependencies", "call_chain"])]
        focus: Option<String>,
    },
    /// Explain system structure or public interfaces.
    Architecture {
        #[command(flatten)]
        request: AgentQuery,
        #[arg(long, value_parser = ["structure", "api_surface"])]
        focus: Option<String>,
    },
    /// Identify complexity, coupling, and hotspots.
    Quality {
        #[command(flatten)]
        request: AgentQuery,
        #[arg(long, value_parser = ["complexity", "coupling", "hotspots"])]
        focus: Option<String>,
    },
    /// Print harness-neutral CLI guidance without loading configuration or calling providers.
    Instructions,
}

#[derive(Debug, Args)]
pub struct AgentQuery {
    /// Natural-language question or task; quote it as one argument.
    #[arg(required_unless_present = "query_file", conflicts_with = "query_file")]
    query: Option<String>,
    /// Read the question from a UTF-8 file; use '-' for stdin.
    #[arg(long)]
    query_file: Option<PathBuf>,
    /// Indexed project root; configuration is loaded from this directory.
    #[arg(long)]
    project: Option<PathBuf>,
    /// Override CODEGRAPH_PROJECT_ID for projects indexed with a custom identifier.
    #[arg(long)]
    project_id: Option<String>,
    /// Whole-command deadline in seconds, including provider/database setup.
    #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_secs: u64,
    /// JSON preserves the complete MCP response; text prints its answer field.
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

impl AgentCommand {
    fn request(&self) -> Option<(AgenticTool, &AgentQuery, Option<&str>)> {
        match self {
            Self::Context { request, focus } => {
                Some((AgenticTool::Context, request, focus.as_deref()))
            }
            Self::Impact { request, focus } => {
                Some((AgenticTool::Impact, request, focus.as_deref()))
            }
            Self::Architecture { request, focus } => {
                Some((AgenticTool::Architecture, request, focus.as_deref()))
            }
            Self::Quality { request, focus } => {
                Some((AgenticTool::Quality, request, focus.as_deref()))
            }
            Self::Instructions => None,
        }
    }
}

/// Execute a CLI command, preparing process configuration before starting workers.
///
/// # Safety
/// The caller must be the single-threaded process entry point; no other threads may
/// read the environment while configuration overrides and dotenv are applied.
pub unsafe fn run(command: &AgentCommand, verbose: bool, config_path: Option<&Path>) -> Result<()> {
    let Some((tool, request, focus)) = command.request() else {
        println!("{CLI_INSTRUCTIONS}");
        return Ok(());
    };
    let result = (|| {
        // SAFETY: The caller guarantees this runs before any worker threads exist.
        let query = unsafe { prepare_environment(request, config_path) }?;
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(execute(tool, request, focus, verbose, &query))
    })();
    let mut stdout = io::stdout().lock();
    match result {
        Ok(response) => {
            write_response(&mut stdout, &response, request.format)?;
            Ok(())
        }
        Err(error) => {
            if matches!(request.format, OutputFormat::Json) {
                let response = serde_json::json!({"error": {
                    "tool": tool.name(), "message": format!("{error:#}")
                }});
                writeln!(stdout, "{response}")?;
                stdout.flush()?;
            }
            Err(error).with_context(|| format!("{} failed", tool.name()))
        }
    }
}

async fn execute(
    tool: AgenticTool,
    request: &AgentQuery,
    focus: Option<&str>,
    verbose: bool,
    query: &str,
) -> Result<Value> {
    if !cfg!(feature = "ai-enhanced") {
        bail!("Agentic tools require a build with --features ai-enhanced (or full)");
    }
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(if verbose { "info" } else { "warn" })
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_ansi(false)
        .try_init();
    codegraph_mcp_core::debug_logger::DebugLogger::init();

    let server = CodeGraphMCPServer::new();
    tokio::time::timeout(
        Duration::from_secs(request.timeout_secs),
        server.execute_agentic_tool(tool, query, focus),
    )
    .await
    .with_context(|| {
        format!(
            "Agentic command timed out after {} seconds",
            request.timeout_secs
        )
    })?
    .map_err(|error| anyhow::anyhow!("{}", error.message))
}

unsafe fn prepare_environment(request: &AgentQuery, config_path: Option<&Path>) -> Result<String> {
    let query = read_query(request)?;
    // Resolve relative config paths before changing to the selected project.
    let config_path = config_path
        .map(std::fs::canonicalize)
        .transpose()
        .context("Cannot resolve --config file")?;
    if let Some(project) = &request.project {
        std::env::set_current_dir(project)
            .with_context(|| format!("Cannot open project {}", project.display()))?;
    }
    // SAFETY: The caller guarantees configuration precedes worker startup.
    unsafe { codegraph_core::config_manager::ConfigManager::initialize_environment() };
    if let Some(project_id) = &request.project_id {
        if project_id.trim().is_empty() {
            bail!("--project-id must not be blank");
        }
        // SAFETY: Called from the single-threaded CLI before creating the async runtime.
        unsafe { std::env::set_var("CODEGRAPH_PROJECT_ID", project_id) };
    }
    if let Some(config_path) = config_path {
        // SAFETY: Called from the single-threaded CLI before creating the async runtime.
        unsafe { std::env::set_var("CODEGRAPH_CONFIG_PATH", config_path) };
    }
    Ok(query)
}

fn read_query(request: &AgentQuery) -> Result<String> {
    let query = match (&request.query, &request.query_file) {
        (Some(query), _) => query.clone(),
        (_, Some(path)) if path == Path::new("-") => {
            let mut query = String::new();
            io::stdin()
                .read_to_string(&mut query)
                .context("Cannot read query from stdin")?;
            query
        }
        (_, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read query file {}", path.display()))?,
        _ => bail!("A query or --query-file is required"),
    };
    if query.trim().is_empty() {
        bail!("Query must not be blank");
    }
    Ok(query)
}

fn write_response(writer: &mut impl Write, response: &Value, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(writer, "{response}")?,
        OutputFormat::Text => {
            let answer = response
                .get("answer")
                .and_then(Value::as_str)
                .context("Agentic response has no answer field")?;
            writeln!(writer, "{answer}")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: AgentCommand,
    }

    #[test]
    fn cli_accepts_only_public_tools_and_each_tools_focuses() {
        for (tool, focuses) in [
            ("context", &["search", "builder", "question"][..]),
            ("impact", &["dependencies", "call_chain"][..]),
            ("architecture", &["structure", "api_surface"][..]),
            ("quality", &["complexity", "coupling", "hotspots"][..]),
        ] {
            let parsed = TestCli::try_parse_from(["agent", tool, "question"]).unwrap();
            let (_, request, focus) = parsed.command.request().unwrap();
            assert!(focus.is_none());
            assert_eq!(request.query.as_deref(), Some("question"));
            for focus in focuses {
                let parsed =
                    TestCli::try_parse_from(["agent", tool, "question", "--focus", focus]).unwrap();
                assert_eq!(parsed.command.request().unwrap().2, Some(*focus));
            }
            assert!(
                TestCli::try_parse_from(["agent", tool, "question", "--focus", "invalid"]).is_err()
            );
        }
        assert!(
            TestCli::try_parse_from(["agent", "context", "question", "--focus", "call_chain"])
                .is_err()
        );
        for internal in [
            "semantic_code_search",
            "complexity_analysis",
            "get_call_chain",
        ] {
            assert!(TestCli::try_parse_from(["agent", internal, "question"]).is_err());
        }
    }

    #[test]
    fn queries_are_required_and_file_input_is_exclusive() {
        assert!(TestCli::try_parse_from(["agent", "context"]).is_err());
        assert!(
            TestCli::try_parse_from(["agent", "context", "question", "--query-file", "-"]).is_err()
        );
        assert!(
            TestCli::try_parse_from(["agent", "context", "question", "--timeout-secs", "0"])
                .is_err()
        );
        let parsed = TestCli::try_parse_from(["agent", "context", "--query-file", "-"]).unwrap();
        assert_eq!(
            parsed.command.request().unwrap().1.query_file.as_deref(),
            Some(Path::new("-"))
        );
        let parsed = TestCli::try_parse_from(["agent", "context", "   "]).unwrap();
        assert!(read_query(parsed.command.request().unwrap().1).is_err());
    }

    #[test]
    fn json_preserves_structured_response_and_partial_result_warnings() {
        let response = serde_json::json!({
            "answer": "partial answer", "findings": "Timeout. Result may be partial.",
            "structured_output": {"highlights": [{"file_path": "src/lib.rs", "start_line": 12}]},
            "tool_use_count": 3, "framework": "Rig"
        });
        let mut output = Vec::new();
        write_response(&mut output, &response, OutputFormat::Json).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), response);
        output.clear();
        write_response(&mut output, &response, OutputFormat::Text).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "partial answer\n");
    }
}
