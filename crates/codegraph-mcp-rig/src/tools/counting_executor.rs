// ABOUTME: Wrapper around GraphToolExecutor that records tool invocations
// ABOUTME: Captures parameters/results and enforces the run's tool-result byte budget

use codegraph_core::config_manager::ConfigManager;
use codegraph_mcp_core::error::Result;
use codegraph_mcp_tools::GraphToolExecutor;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::info;

/// Bounds for the bytes of tool results one agent run may put in front of the model.
/// Every round resends all earlier results, so an unbounded run grows each request until
/// the model call itself stalls; the upper bound applies however large the window is.
const MIN_RESULT_BUDGET_BYTES: usize = 48_000;
const MAX_RESULT_BUDGET_BYTES: usize = 600_000;

/// Result budget for a run: about a third of the context window at 4 bytes per token,
/// within the bounds above. `CODEGRAPH_AGENT_RESULT_BUDGET_BYTES` overrides it.
pub fn result_budget_bytes(context_window_tokens: usize) -> usize {
    std::env::var("CODEGRAPH_AGENT_RESULT_BUDGET_BYTES")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|bytes: &usize| *bytes > 0)
        .unwrap_or_else(|| {
            (context_window_tokens.saturating_mul(4) / 3)
                .clamp(MIN_RESULT_BUDGET_BYTES, MAX_RESULT_BUDGET_BYTES)
        })
}

/// Fit a tool result into `remaining` bytes. Array results keep their leading items; when
/// not even one item fits, the result is replaced by a note telling the model to answer
/// from what it has. Returns the value to hand to the model.
fn fit_to_budget(tool_name: &str, result: JsonValue, remaining: usize) -> JsonValue {
    if result.to_string().len() <= remaining {
        return result;
    }
    // Room for the wrapper and the note itself.
    const OVERHEAD: usize = 600;
    if let Some(items) = result.get("result").and_then(|r| r.as_array()) {
        let mut kept = Vec::new();
        let mut used = OVERHEAD;
        for item in items {
            let size = item.to_string().len() + 1;
            if used + size > remaining {
                break;
            }
            used += size;
            kept.push(item.clone());
        }
        if !kept.is_empty() {
            let dropped = items.len() - kept.len();
            let mut trimmed = result.clone();
            if let Some(object) = trimmed.as_object_mut() {
                object.insert("result".to_string(), JsonValue::Array(kept));
                object.insert(
                    "_budget".to_string(),
                    json!({
                        "dropped_items": dropped,
                        "reason": "This run's tool-result budget is nearly used up; later items were dropped."
                    }),
                );
            }
            return trimmed;
        }
    }
    json!({
        "tool": tool_name,
        "result": [],
        "_budget": {
            "exhausted": true,
            "reason": "This run's tool-result budget is used up. Do not call more tools: write the answer from the evidence already gathered and name what is missing."
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTrace {
    pub tool_name: String,
    pub parameters: JsonValue,
    pub result: Option<JsonValue>,
    pub error: Option<String>,
}

/// Wrapper around GraphToolExecutor that counts tool invocations
#[derive(Clone)]
pub struct CountingExecutor {
    inner: Arc<GraphToolExecutor>,
    call_count: Arc<AtomicUsize>,
    traces: Arc<Mutex<Vec<ToolTrace>>>,
    result_budget: usize,
    result_bytes: Arc<AtomicUsize>,
}

impl CountingExecutor {
    /// Create a new counting executor wrapping the given GraphToolExecutor
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self::with_result_budget(
            executor,
            result_budget_bytes(ConfigManager::agent_context_window()),
        )
    }

    /// Create a counting executor with an explicit tool-result budget in bytes
    pub fn with_result_budget(executor: Arc<GraphToolExecutor>, result_budget: usize) -> Self {
        Self {
            inner: executor,
            call_count: Arc::new(AtomicUsize::new(0)),
            traces: Arc::new(Mutex::new(Vec::new())),
            result_budget,
            result_bytes: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Execute a tool and increment the call counter
    pub async fn execute(&self, tool_name: &str, params: JsonValue) -> Result<JsonValue> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        match self.inner.execute(tool_name, params.clone()).await {
            Ok(result) => {
                let offered = result.to_string().len();
                let used = self.result_bytes.load(Ordering::SeqCst);
                let remaining = self.result_budget.saturating_sub(used);
                let result = fit_to_budget(tool_name, result, remaining);
                let returned = result.to_string().len();
                let total = self.result_bytes.fetch_add(returned, Ordering::SeqCst) + returned;
                info!(
                    tool = tool_name,
                    offered_bytes = offered,
                    returned_bytes = returned,
                    run_total_bytes = total,
                    run_budget_bytes = self.result_budget,
                    "Tool result added to agent context"
                );
                if let Ok(mut guard) = self.traces.lock() {
                    guard.push(ToolTrace {
                        tool_name: tool_name.to_string(),
                        parameters: params,
                        result: Some(result.clone()),
                        error: None,
                    });
                }
                Ok(result)
            }
            Err(err) => {
                if let Ok(mut guard) = self.traces.lock() {
                    guard.push(ToolTrace {
                        tool_name: tool_name.to_string(),
                        parameters: params,
                        result: None,
                        error: Some(err.to_string()),
                    });
                }
                Err(err)
            }
        }
    }

    /// Bytes of tool results handed to the model so far in this run
    pub fn result_bytes(&self) -> usize {
        self.result_bytes.load(Ordering::SeqCst)
    }

    /// Get the current call count
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Get and reset the call count (useful for per-query tracking)
    pub fn take_call_count(&self) -> usize {
        self.call_count.swap(0, Ordering::SeqCst)
    }

    /// Get and reset the tool traces since last query.
    pub fn take_traces(&self) -> Vec<ToolTrace> {
        self.traces
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default()
    }

    /// Get the underlying executor for direct access when needed
    pub fn inner(&self) -> &Arc<GraphToolExecutor> {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(count: usize, bytes_each: usize) -> JsonValue {
        json!({
            "tool": "semantic_code_search",
            "parameters": { "query": "q" },
            "result": (0..count)
                .map(|i| json!({ "name": format!("row{i}"), "content": "x".repeat(bytes_each) }))
                .collect::<Vec<_>>()
        })
    }

    #[test]
    fn test_fit_to_budget_passes_results_that_fit() {
        let result = rows(3, 100);
        let size = result.to_string().len();
        assert_eq!(
            fit_to_budget("semantic_code_search", result.clone(), size),
            result
        );
    }

    #[test]
    fn test_fit_to_budget_keeps_leading_items_and_says_so() {
        let fitted = fit_to_budget("semantic_code_search", rows(10, 1_000), 4_000);
        let kept = fitted["result"].as_array().unwrap().len();
        assert!((1..10).contains(&kept), "kept {kept}");
        assert_eq!(fitted["result"][0]["name"], "row0");
        assert_eq!(fitted["_budget"]["dropped_items"], 10 - kept);
        assert!(fitted.to_string().len() <= 4_000);
        assert_eq!(fitted["parameters"]["query"], "q");
    }

    #[test]
    fn test_fit_to_budget_reports_exhaustion() {
        let fitted = fit_to_budget("get_hub_nodes", rows(5, 1_000), 200);
        assert_eq!(fitted["_budget"]["exhausted"], true);
        assert_eq!(fitted["result"].as_array().unwrap().len(), 0);
        assert_eq!(fitted["tool"], "get_hub_nodes");

        // Non-array results cannot be trimmed, so they are replaced as well.
        let object =
            json!({ "tool": "calculate_coupling_metrics", "result": { "big": "y".repeat(5_000) } });
        let fitted = fit_to_budget("calculate_coupling_metrics", object, 1_000);
        assert_eq!(fitted["_budget"]["exhausted"], true);
    }

    #[test]
    fn test_result_budget_scales_with_window_within_bounds() {
        if !test_env::run(
            concat!(
                module_path!(),
                "::test_result_budget_scales_with_window_within_bounds"
            ),
            &[("CODEGRAPH_AGENT_RESULT_BUDGET_BYTES", None)],
        ) {
            return;
        }
        assert_eq!(result_budget_bytes(8_000), MIN_RESULT_BUDGET_BYTES);
        assert_eq!(result_budget_bytes(128_000), 170_666);
        assert_eq!(result_budget_bytes(1_000_000), MAX_RESULT_BUDGET_BYTES);
    }

    #[test]
    fn test_call_count_starts_at_zero() {
        // We can't easily create a real GraphToolExecutor in unit tests
        // but we can verify the atomic counter behavior
        let counter = Arc::new(AtomicUsize::new(0));
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        let taken = counter.swap(0, Ordering::SeqCst);
        assert_eq!(taken, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
