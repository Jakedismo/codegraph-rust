// ABOUTME: CodeGraph agent definition for AutoAgents ReAct workflow
// ABOUTME: Defines output format and behavior for graph analysis (tools registered manually)

use autoagents::core::agent::prebuilt::executor::ReActAgentOutput;
use autoagents_derive::AgentOutput;
use serde::{Deserialize, Serialize};

/// CodeGraph agent output format
#[derive(Debug, Serialize, Deserialize, AgentOutput)]
pub struct CodeGraphAgentOutput {
    #[output(description = "Final answer to the query")]
    pub answer: String,

    #[output(description = "Key findings from graph analysis")]
    pub findings: String,

    #[output(description = "Number of analysis steps performed")]
    pub steps_taken: String,
}

impl From<ReActAgentOutput> for CodeGraphAgentOutput {
    fn from(output: ReActAgentOutput) -> Self {
        let resp = output.response.clone();
        let num_steps = output.tool_calls.len();

        if output.done && !resp.trim().is_empty() {
            // Try to parse as structured JSON
            if let Ok(mut value) = serde_json::from_str::<CodeGraphAgentOutput>(&resp) {
                // Override steps_taken with actual count from ReActAgentOutput
                value.steps_taken = num_steps.to_string();
                return value;
            }
        }

        // Fallback: create output from raw response with actual step count
        CodeGraphAgentOutput {
            answer: resp,
            findings: String::new(),
            steps_taken: num_steps.to_string(),
        }
    }
}
