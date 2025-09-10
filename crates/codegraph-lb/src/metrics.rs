use prometheus::{IntCounter, IntGauge, Histogram, Registry, Opts, HistogramOpts};
use once_cell::sync::Lazy;

pub static REQUESTS_TOTAL: Lazy<IntCounter> = Lazy::new(|| IntCounter::new("lb_requests_total", "Total proxied requests").unwrap());
pub static FAILURES_TOTAL: Lazy<IntCounter> = Lazy::new(|| IntCounter::new("lb_failures_total", "Total failed proxy requests").unwrap());
pub static ACTIVE_CONNECTIONS: Lazy<IntGauge> = Lazy::new(|| IntGauge::new("lb_active_connections", "Active upstream connections").unwrap());
pub static LATENCY_HIST: Lazy<Histogram> = Lazy::new(|| Histogram::with_opts(HistogramOpts::new("lb_upstream_latency_seconds", "Upstream response latency")).unwrap());

pub fn register(reg: &Registry) {
    reg.register(Box::new(REQUESTS_TOTAL.clone())).ok();
    reg.register(Box::new(FAILURES_TOTAL.clone())).ok();
    reg.register(Box::new(ACTIVE_CONNECTIONS.clone())).ok();
    reg.register(Box::new(LATENCY_HIST.clone())).ok();
}

