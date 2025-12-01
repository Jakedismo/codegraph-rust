// ABOUTME: Adapter for GraphToolExecutor integration with AutoAgents
// ABOUTME: Synchronous wrapper around async GraphToolExecutor for AutoAgents tools

use codegraph_mcp_core::debug_logger::DebugLogger;
use codegraph_mcp_tools::GraphToolExecutor;
use serde_json::Value;
use std::sync::Arc;

/// Synchronous wrapper around async GraphToolExecutor
///
/// AutoAgents tools must be synchronous, but GraphToolExecutor is async.
/// This wrapper uses tokio::runtime::Handle to bridge the gap.
pub struct GraphToolExecutorAdapter {
    executor: Arc<GraphToolExecutor>,
    runtime_handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for GraphToolExecutorAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphToolExecutorAdapter")
            .field("executor", &"<GraphToolExecutor>")
            .field("runtime_handle", &self.runtime_handle)
            .finish()
    }
}

impl GraphToolExecutorAdapter {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            executor,
            runtime_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Execute a graph tool synchronously (blocks on async call)
    pub fn execute_sync(&self, function_name: &str, params: Value) -> Result<Value, String> {
        let params_for_log = params.clone();
        DebugLogger::log_tool_start(function_name, &params_for_log);
        // Use block_in_place to avoid "runtime within runtime" panic
        // This allows blocking the current thread without blocking the runtime
        let result = tokio::task::block_in_place(|| {
            self.runtime_handle
                .block_on(self.executor.execute(function_name, params))
        })
        .map_err(|e| e.to_string());

        match &result {
            Ok(value) => DebugLogger::log_tool_finish(function_name, value),
            Err(err) => DebugLogger::log_tool_error(function_name, &params_for_log, err),
        }

        result
    }
}

/// Factory for creating AutoAgents tools with shared executor
pub struct GraphToolFactory {
    adapter: Arc<GraphToolExecutorAdapter>,
}

impl GraphToolFactory {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            adapter: Arc::new(GraphToolExecutorAdapter::new(executor)),
        }
    }

    pub fn adapter(&self) -> Arc<GraphToolExecutorAdapter> {
        self.adapter.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_struct_exists() {
        let _ = std::mem::size_of::<GraphToolExecutorAdapter>();
        let _ = std::mem::size_of::<GraphToolFactory>();
    }
}
