#[cfg(test)]
pub mod test_utils {
    use crate::AppState;
    use codegraph_core::ConfigManager;
    use std::sync::Arc;

    impl AppState {
        /// Creates a new AppState instance specifically for testing purposes.
        /// This method initializes AppState with default test configuration.
        pub async fn new_for_testing() -> Self {
            let config = Arc::new(
                ConfigManager::new().expect("Failed to create test config manager")
            );
            Self::new(config)
                .await
                .expect("Failed to create test AppState")
        }

        /// Creates a new AppState instance with a custom test configuration.
        pub async fn new_with_test_config(config: Arc<ConfigManager>) -> Self {
            Self::new(config)
                .await
                .expect("Failed to create test AppState with custom config")
        }
    }

    /// Helper function to create a minimal test configuration
    pub fn create_test_config() -> Arc<ConfigManager> {
        Arc::new(ConfigManager::new().expect("Failed to create test config"))
    }
}