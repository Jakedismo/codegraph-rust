use prometheus::{Counter, Histogram, Opts, Registry};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    pub static ref SYNC_OPERATIONS_TOTAL: Counter = 
        Counter::with_opts(Opts::new("sync_operations_total", "Total number of sync operations"))
            .unwrap();

    pub static ref SYNC_OPERATION_DURATION_SECONDS: Histogram = 
        Histogram::with_opts(Opts::new("sync_operation_duration_seconds", "Duration of sync operations in seconds"))
            .unwrap();
}

pub fn register_metrics() {
    REGISTRY.register(Box::new(SYNC_OPERATIONS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(SYNC_OPERATION_DURATION_SECONDS.clone())).unwrap();
}
