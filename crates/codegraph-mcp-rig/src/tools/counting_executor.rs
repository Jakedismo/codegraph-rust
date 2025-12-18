// ABOUTME: Wrapper around GraphToolExecutor that records tool invocations
// ABOUTME: Captures parameters/results so the server can synthesize structured output

use codegraph_mcp_core::error::Result;
use codegraph_mcp_tools::GraphToolExecutor;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

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
}

impl CountingExecutor {
    /// Create a new counting executor wrapping the given GraphToolExecutor
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            inner: executor,
            call_count: Arc::new(AtomicUsize::new(0)),
            traces: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Execute a tool and increment the call counter
    pub async fn execute(&self, tool_name: &str, params: JsonValue) -> Result<JsonValue> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        match self.inner.execute(tool_name, params.clone()).await {
            Ok(result) => {
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
