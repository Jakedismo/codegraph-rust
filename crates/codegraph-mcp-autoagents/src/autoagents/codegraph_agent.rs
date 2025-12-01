// ABOUTME: CodeGraph agent definition for AutoAgents ReAct workflow
// ABOUTME: Defines output format, failure detection, and fallback behavior for graph analysis

use autoagents::core::agent::prebuilt::executor::ReActAgentOutput;
use autoagents_derive::AgentOutput;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Fallback message returned when the agent fails to complete its task properly.
/// This guides the calling agent to retry or proceed without CodeGraph.
pub const FALLBACK_MESSAGE: &str = "Tool failed to find answers to the question. Retry once with a rewritten question. If tool fails to find answers again, proceed normally without CodeGraph for this phase of the task.";

/// Minimum response length to be considered substantive (not a minimal/failed response)
const MIN_RESPONSE_LENGTH: usize = 100;

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

impl CodeGraphAgentOutput {
    /// Check if this output represents a failed/fallback agent execution
    pub fn is_failure(&self) -> bool {
        self.answer == FALLBACK_MESSAGE || self.answer.is_empty()
    }

    /// Create a fallback response with debug information
    fn fallback(reason: &str, steps: usize) -> Self {
        CodeGraphAgentOutput {
            answer: FALLBACK_MESSAGE.to_string(),
            findings: format!("Fallback triggered: {}", reason),
            steps_taken: steps.to_string(),
        }
    }

    /// Detect if the agent output represents a failure that should trigger fallback
    ///
    /// Failure conditions:
    /// 1. Agent never completed (done=false)
    /// 2. Agent completed but didn't use tools and gave minimal response
    /// 3. Empty or whitespace-only response
    fn detect_failure(output: &ReActAgentOutput) -> Option<String> {
        // Check for environment variable to disable fallback (for debugging)
        if std::env::var("CODEGRAPH_DISABLE_FALLBACK").is_ok() {
            return None;
        }

        let resp = &output.response;
        let tool_count = output.tool_calls.len();

        // Case 1: Agent never completed
        if !output.done {
            return Some(format!(
                "agent did not complete (done=false, tools={}, response_len={})",
                tool_count,
                resp.len()
            ));
        }

        // Case 2: Empty or whitespace-only response
        if resp.trim().is_empty() {
            return Some(format!("empty response (done=true, tools={})", tool_count));
        }

        // Case 3: Agent completed but didn't use any tools and gave minimal response
        if tool_count == 0 && resp.len() < MIN_RESPONSE_LENGTH {
            return Some(format!(
                "no tools used with minimal response (done=true, tools=0, response_len={})",
                resp.len()
            ));
        }

        None
    }
}

impl From<ReActAgentOutput> for CodeGraphAgentOutput {
    fn from(output: ReActAgentOutput) -> Self {
        let resp = output.response.clone();
        let num_steps = output.tool_calls.len();

        // Step 1: Check for explicit failure conditions
        if let Some(reason) = CodeGraphAgentOutput::detect_failure(&output) {
            info!(
                target: "codegraph::agent::fallback",
                reason = %reason,
                done = output.done,
                tool_calls = num_steps,
                response_len = resp.len(),
                "Agent failed to complete task, returning fallback response"
            );
            return CodeGraphAgentOutput::fallback(&reason, num_steps);
        }

        // Step 2: Try to parse as structured JSON (agent completed successfully)
        if output.done && !resp.trim().is_empty() {
            match serde_json::from_str::<CodeGraphAgentOutput>(&resp) {
                Ok(mut value) => {
                    // Override steps_taken with actual count from ReActAgentOutput
                    value.steps_taken = num_steps.to_string();
                    return value;
                }
                Err(parse_err) => {
                    // Schema parse failure - agent returned text but not in expected format
                    // Only trigger fallback if the agent didn't use any tools
                    // If tools were used, the raw response may still be valuable
                    if num_steps == 0 {
                        info!(
                            target: "codegraph::agent::fallback",
                            error = %parse_err,
                            done = output.done,
                            tool_calls = num_steps,
                            response_preview = %resp.chars().take(100).collect::<String>(),
                            "Agent response failed schema validation with no tool calls, returning fallback"
                        );
                        return CodeGraphAgentOutput::fallback(
                            &format!("schema parse failure: {}", parse_err),
                            num_steps,
                        );
                    }
                    // Agent used tools but didn't format JSON - use raw response
                    info!(
                        target: "codegraph::agent",
                        error = %parse_err,
                        tool_calls = num_steps,
                        "Agent response is not JSON but tools were used, using raw response"
                    );
                }
            }
        }

        // If we reach here, agent completed with tool calls but non-JSON response
        // Use the raw response - the agent did work but didn't format output correctly
        CodeGraphAgentOutput {
            answer: resp,
            findings: String::new(),
            steps_taken: num_steps.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_tool_call() -> autoagents::core::tool::ToolCallResult {
        autoagents::core::tool::ToolCallResult {
            tool_name: "test_tool".to_string(),
            success: true,
            arguments: serde_json::json!({}),
            result: serde_json::json!({"status": "ok"}),
        }
    }

    #[test]
    fn test_fallback_on_not_done() {
        let output = ReActAgentOutput {
            done: false,
            response: "partial answer that is long enough to pass length check".to_string(),
            tool_calls: vec![],
        };
        let result: CodeGraphAgentOutput = output.into();
        assert!(result.is_failure());
        assert!(result.answer.contains("Tool failed"));
    }

    #[test]
    fn test_fallback_on_empty_response() {
        let output = ReActAgentOutput {
            done: true,
            response: "   ".to_string(),
            tool_calls: vec![mock_tool_call()],
        };
        let result: CodeGraphAgentOutput = output.into();
        assert!(result.is_failure());
    }

    #[test]
    fn test_fallback_on_no_tools_minimal_response() {
        let output = ReActAgentOutput {
            done: true,
            response: "ok".to_string(),
            tool_calls: vec![],
        };
        let result: CodeGraphAgentOutput = output.into();
        assert!(result.is_failure());
    }

    #[test]
    fn test_fallback_on_schema_parse_failure() {
        let output = ReActAgentOutput {
            done: true,
            response: "This is a plain text response that is definitely long enough but not JSON formatted at all".to_string(),
            tool_calls: vec![], // No tools = fallback
        };
        let result: CodeGraphAgentOutput = output.into();
        assert!(result.is_failure());
    }

    #[test]
    fn test_no_fallback_on_valid_json() {
        let valid_output = serde_json::json!({
            "answer": "Detailed analysis of the codebase showing authentication flow...",
            "findings": "Found 5 key components",
            "steps_taken": "3"
        });
        let output = ReActAgentOutput {
            done: true,
            response: valid_output.to_string(),
            tool_calls: vec![mock_tool_call()],
        };
        let result: CodeGraphAgentOutput = output.into();
        assert!(!result.is_failure());
        assert!(result.answer.contains("authentication"));
    }

    #[test]
    fn test_raw_response_with_tool_calls_not_fallback() {
        // If agent used tools but gave non-JSON response, we use the raw response
        let output = ReActAgentOutput {
            done: true,
            response: "The authentication system uses JWT tokens stored in the database. Key files: auth.rs, token.rs".to_string(),
            tool_calls: vec![mock_tool_call(), mock_tool_call()],
        };
        let result: CodeGraphAgentOutput = output.into();
        // This should NOT be a fallback because the agent did use tools
        assert!(!result.is_failure());
        assert!(result.answer.contains("JWT"));
    }
}
