#[cfg(feature = "leak-detect")]
pub struct LeakGuard;

#[cfg(feature = "leak-detect")]
impl LeakGuard {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "leak-detect")]
impl Drop for LeakGuard {
    fn drop(&mut self) {
        // On shutdown, emit a final metric update and log if leaks are present
        crate::metrics::update_memory_metrics();
        let leaked = crate::metrics::MEM_LEAKED_ALLOCATIONS.get();
        if leaked > 0 {
            let leaked_bytes = crate::metrics::MEM_LEAKED_BYTES.get();
            tracing::error!(
                leaked_allocations = leaked,
                leaked_bytes = leaked_bytes,
                "LeakGuard detected outstanding allocations at shutdown"
            );
        } else {
            tracing::info!("LeakGuard: no outstanding allocations at shutdown");
        }
    }
}

// No-op stub when feature disabled
#[cfg(not(feature = "leak-detect"))]
pub struct LeakGuard;

#[cfg(not(feature = "leak-detect"))]
impl LeakGuard {
    pub fn new() -> Self { Self }
}

