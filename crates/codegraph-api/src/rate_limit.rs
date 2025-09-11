use tower::layer::util::Identity;

pub struct RateLimitManager;

impl RateLimitManager {
    pub fn new() -> Identity {
        Identity::new()
    }
}
