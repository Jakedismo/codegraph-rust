// ABOUTME: CodeGraph agent definition for AutoAgents ReAct workflow
// ABOUTME: Defines tools, output format, and behavior for graph analysis

use autoagents::core::agent::prebuilt::executor::ReActAgentOutput;
use autoagents_derive::{agent, AgentHooks, AgentOutput};
use serde::{Deserialize, Serialize};

use crate::autoagents::tools::graph_tools::*;

/// CodeGraph agent output format
#[derive(Debug, Serialize, Deserialize, AgentOutput)]
pub struct CodeGraphAgentOutput {
    #[output(description = "Final answer to the query")]
    answer: String,

    #[output(description = "Key findings from graph analysis")]
    findings: String,

    #[output(description = "Number of analysis steps performed")]
    steps_taken: String,
}

/// CodeGraph agent for code analysis via graph traversal
#[agent(
    name = "codegraph_agent",
    description = "You are a code analysis agent with access to graph database tools. \
                   Analyze code dependencies, call chains, and architectural patterns.",
    tools = [
        GetTransitiveDependencies,
        GetReverseDependencies,
        TraceCallChain,
        DetectCycles,
        CalculateCoupling,
        GetHubNodes
    ],
    output = CodeGraphAgentOutput,
)]
#[derive(Default, Clone, AgentHooks)]
pub struct CodeGraphAgent {}

impl From<ReActAgentOutput> for CodeGraphAgentOutput {
    fn from(output: ReActAgentOutput) -> Self {
        let resp = output.response;

        if output.done && !resp.trim().is_empty() {
            // Try to parse as structured JSON
            if let Ok(value) = serde_json::from_str::<CodeGraphAgentOutput>(&resp) {
                return value;
            }
        }

        // Fallback: create output from raw response
        CodeGraphAgentOutput {
            answer: resp,
            findings: String::new(),
            steps_taken: "0".to_string(),
        }
    }
}
