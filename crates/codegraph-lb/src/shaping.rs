use governor::clock::DefaultClock;
use governor::state::direct::NotKeyed;
use governor::state::InMemoryState;
use governor::{Quota, RateLimiter};
use http::{Method, Request};
use parking_lot::RwLock;
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RouteRule {
    pub prefix: String,
    pub methods: Option<Vec<Method>>,
    pub limit_per_second: Option<u32>,
}

#[derive(Clone)]
pub struct TrafficShaper {
    rules: Arc<
        RwLock<
            Vec<(
                RouteRule,
                Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
            )>,
        >,
    >,
}

impl TrafficShaper {
    pub fn new(rules: Vec<RouteRule>) -> Self {
        let compiled = rules
            .into_iter()
            .map(|r| {
                let rl = r.limit_per_second.map(|l| {
                    Arc::new(RateLimiter::direct(Quota::per_second(
                        NonZeroU32::new(l).unwrap(),
                    )))
                });
                (r, rl)
            })
            .collect();
        Self {
            rules: Arc::new(RwLock::new(compiled)),
        }
    }

    pub fn allow<B>(&self, req: &Request<B>) -> bool {
        let path = req.uri().path();
        let method = req.method();
        for (rule, rl) in self.rules.read().iter() {
            if path.starts_with(&rule.prefix) {
                if let Some(methods) = &rule.methods {
                    if !methods.contains(method) {
                        continue;
                    }
                }
                if let Some(rl) = rl {
                    return rl.check().is_ok();
                }
                return true;
            }
        }
        true
    }
}
