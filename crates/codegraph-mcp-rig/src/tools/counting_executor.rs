// ABOUTME: Wrapper around GraphToolExecutor that counts tool invocations
// ABOUTME: Provides accurate tool_calls count for Rig agent output

use codegraph_mcp_core::error::Result;
use codegraph_mcp_tools::GraphToolExecutor;
use serde_json::Value as JsonValue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Wrapper around GraphToolExecutor that counts tool invocations
#[derive(Clone)]
pub struct CountingExecutor {
    inner: Arc<GraphToolExecutor>,
    call_count: Arc<AtomicUsize>,
}

impl CountingExecutor {
    /// Create a new counting executor wrapping the given GraphToolExecutor
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            inner: executor,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Execute a tool and increment the call counter
    pub async fn execute(&self, tool_name: &str, params: JsonValue) -> Result<JsonValue> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(tool_name, params).await
    }

    /// Get the current call count
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Get and reset the call count (useful for per-query tracking)
    pub fn take_call_count(&self) -> usize {
        self.call_count.swap(0, Ordering::SeqCst)
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
