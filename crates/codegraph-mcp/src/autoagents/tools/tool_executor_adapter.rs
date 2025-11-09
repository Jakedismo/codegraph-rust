// ABOUTME: Adapter for GraphToolExecutor integration with AutoAgents
// ABOUTME: Synchronous wrapper around async GraphToolExecutor for AutoAgents tools

use crate::graph_tool_executor::GraphToolExecutor;
use std::sync::Arc;
use serde_json::Value;

/// Synchronous wrapper around async GraphToolExecutor
///
/// AutoAgents tools must be synchronous, but GraphToolExecutor is async.
/// This wrapper uses tokio::runtime::Handle to bridge the gap.
pub struct GraphToolExecutorAdapter {
    executor: Arc<GraphToolExecutor>,
    runtime_handle: tokio::runtime::Handle,
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
        self.runtime_handle
            .block_on(self.executor.execute(function_name, params))
            .map_err(|e| e.to_string())
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
