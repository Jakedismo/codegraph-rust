// ABOUTME: Factory for creating Rig graph tools with call counting
// ABOUTME: Instantiates all 8 tools with shared CountingExecutor

use super::counting_executor::{CountingExecutor, ToolTrace};
use super::graph_tools::*;
use codegraph_mcp_tools::GraphToolExecutor;
use std::sync::Arc;

/// Factory for creating graph analysis tools for Rig agents
pub struct GraphToolFactory {
    executor: CountingExecutor,
}

impl GraphToolFactory {
    /// Create a new factory with shared executor wrapped in CountingExecutor
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            executor: CountingExecutor::new(executor),
        }
    }

    /// Create the transitive dependencies tool
    pub fn transitive_dependencies(&self) -> GetTransitiveDependencies {
        GetTransitiveDependencies::new(self.executor.clone())
    }

    /// Create the circular dependencies detection tool
    pub fn circular_dependencies(&self) -> DetectCircularDependencies {
        DetectCircularDependencies::new(self.executor.clone())
    }

    /// Create the call chain tracing tool
    pub fn call_chain(&self) -> TraceCallChain {
        TraceCallChain::new(self.executor.clone())
    }

    /// Create the coupling metrics tool
    pub fn coupling_metrics(&self) -> CalculateCouplingMetrics {
        CalculateCouplingMetrics::new(self.executor.clone())
    }

    /// Create the hub nodes tool
    pub fn hub_nodes(&self) -> GetHubNodes {
        GetHubNodes::new(self.executor.clone())
    }

    /// Create the reverse dependencies tool
    pub fn reverse_dependencies(&self) -> GetReverseDependencies {
        GetReverseDependencies::new(self.executor.clone())
    }

    /// Create the semantic search tool
    pub fn semantic_search(&self) -> SemanticCodeSearch {
        SemanticCodeSearch::new(self.executor.clone())
    }

    /// Create the complexity hotspots tool
    pub fn complexity_hotspots(&self) -> FindComplexityHotspots {
        FindComplexityHotspots::new(self.executor.clone())
    }

    /// Get the underlying executor for direct access
    pub fn executor(&self) -> Arc<GraphToolExecutor> {
        self.executor.inner().clone()
    }

    /// Get the current tool call count
    pub fn call_count(&self) -> usize {
        self.executor.call_count()
    }

    /// Get and reset the tool call count (for per-query tracking)
    pub fn take_call_count(&self) -> usize {
        self.executor.take_call_count()
    }

    /// Get and reset the tool traces since last query.
    pub fn take_traces(&self) -> Vec<ToolTrace> {
        self.executor.take_traces()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Integration tests require GraphToolExecutor with SurrealDB
    // Unit tests verify factory construction patterns

    #[test]
    fn test_factory_methods_exist() {
        // This test verifies the API surface exists
        // Actual instantiation requires GraphToolExecutor
        fn _assert_factory_api<F: Fn(Arc<GraphToolExecutor>) -> GraphToolFactory>(f: F) {
            let _ = f;
        }
        _assert_factory_api(GraphToolFactory::new);
    }
}
