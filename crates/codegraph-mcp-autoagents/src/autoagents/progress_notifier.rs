// ABOUTME: MCP progress notification integration for AutoAgents workflows
// ABOUTME: Sends 3-stage progress updates: started (0.0), analyzing (0.5), complete (1.0)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Progress notification stages for agentic workflows
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressStage {
    /// Stage 1: Agent started (progress: 0.0)
    Started,
    /// Stage 2: Agent analyzing with tools (progress: 0.5)
    Analyzing,
    /// Stage 3: Agent complete or error (progress: 1.0)
    Complete,
}

impl ProgressStage {
    /// Get the progress value for this stage
    pub fn progress(&self) -> f64 {
        match self {
            ProgressStage::Started => 0.0,
            ProgressStage::Analyzing => 0.5,
            ProgressStage::Complete => 1.0,
        }
    }
}

/// Callback type for sending progress notifications
/// Takes (progress: f64, message: Option<String>) and returns a future
pub type ProgressCallback =
    Arc<dyn Fn(f64, Option<String>) -> futures::future::BoxFuture<'static, ()> + Send + Sync>;

/// 3-stage progress notifier for agentic workflows
///
/// Sends exactly 3 notifications per workflow:
/// 1. Agent started (0.0) - at workflow start
/// 2. Agent analyzing (0.5) - after first tool execution
/// 3. Agent complete (1.0) - at workflow end
///
/// Thread-safe and can be shared across async boundaries.
pub struct ProgressNotifier {
    callback: ProgressCallback,
    analysis_type: String,
    /// Track whether stage 2 (analyzing) has been sent
    stage2_sent: Arc<AtomicBool>,
}

impl ProgressNotifier {
    /// Create a new progress notifier with the given callback and analysis type
    pub fn new(callback: ProgressCallback, analysis_type: impl Into<String>) -> Self {
        Self {
            callback,
            analysis_type: analysis_type.into(),
            stage2_sent: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a no-op notifier that discards all notifications
    pub fn noop() -> Self {
        Self {
            callback: Arc::new(|_, _| Box::pin(async {})),
            analysis_type: String::new(),
            stage2_sent: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Send stage 1 notification: Agent started
    pub async fn notify_started(&self) {
        let message = format!("Agent started: {}", self.analysis_type);
        tracing::debug!(
            target: "progress_notification",
            stage = "started",
            progress = 0.0,
            message = %message,
            "Sending progress notification"
        );
        (self.callback)(ProgressStage::Started.progress(), Some(message)).await;
    }

    /// Send stage 2 notification: Agent analyzing with tools
    /// This is idempotent - calling multiple times only sends once
    pub async fn notify_analyzing(&self) {
        // Only send stage 2 once, even if called multiple times
        if self
            .stage2_sent
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let message = "Agent analyzing with tools...".to_string();
            tracing::debug!(
                target: "progress_notification",
                stage = "analyzing",
                progress = 0.5,
                message = %message,
                "Sending progress notification"
            );
            (self.callback)(ProgressStage::Analyzing.progress(), Some(message)).await;
        }
    }

    /// Send stage 3 notification: Agent complete
    pub async fn notify_complete(&self) {
        // Ensure stage 2 was sent (send all 3 notifications even if no tools were called)
        self.notify_analyzing().await;

        let message = "Agent analysis complete".to_string();
        tracing::debug!(
            target: "progress_notification",
            stage = "complete",
            progress = 1.0,
            message = %message,
            "Sending progress notification"
        );
        (self.callback)(ProgressStage::Complete.progress(), Some(message)).await;
    }

    /// Send stage 3 notification with error message
    pub async fn notify_error(&self, error: &str) {
        // Ensure stage 2 was sent even on error path
        self.notify_analyzing().await;

        let message = format!("Agent failed: {}", error);
        tracing::debug!(
            target: "progress_notification",
            stage = "error",
            progress = 1.0,
            message = %message,
            "Sending progress notification"
        );
        (self.callback)(ProgressStage::Complete.progress(), Some(message)).await;
    }

    /// Check if stage 2 (analyzing) has been sent
    pub fn is_analyzing_sent(&self) -> bool {
        self.stage2_sent.load(Ordering::SeqCst)
    }
}

impl Clone for ProgressNotifier {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            analysis_type: self.analysis_type.clone(),
            stage2_sent: self.stage2_sent.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::sync::Mutex;

    #[test]
    fn test_progress_stage_values() {
        assert_eq!(ProgressStage::Started.progress(), 0.0);
        assert_eq!(ProgressStage::Analyzing.progress(), 0.5);
        assert_eq!(ProgressStage::Complete.progress(), 1.0);
    }

    #[tokio::test]
    async fn test_notify_started() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: ProgressCallback = Arc::new(move |progress, message| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.lock().await.push((progress, message));
            })
        });

        let notifier = ProgressNotifier::new(callback, "code_search");
        notifier.notify_started().await;

        let notifications = received.lock().await;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].0, 0.0);
        assert_eq!(
            notifications[0].1,
            Some("Agent started: code_search".to_string())
        );
    }

    #[tokio::test]
    async fn test_notify_analyzing_idempotent() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let callback: ProgressCallback = Arc::new(move |_, _| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        });

        let notifier = ProgressNotifier::new(callback, "test");

        // Call analyzing multiple times
        notifier.notify_analyzing().await;
        notifier.notify_analyzing().await;
        notifier.notify_analyzing().await;

        // Should only have been called once
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(notifier.is_analyzing_sent());
    }

    #[tokio::test]
    async fn test_notify_complete_sends_all_stages() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: ProgressCallback = Arc::new(move |progress, message| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.lock().await.push((progress, message));
            })
        });

        let notifier = ProgressNotifier::new(callback, "test");

        // Only call complete (should also send analyzing)
        notifier.notify_complete().await;

        let notifications = received.lock().await;
        assert_eq!(notifications.len(), 2);
        // First: analyzing (0.5)
        assert_eq!(notifications[0].0, 0.5);
        // Second: complete (1.0)
        assert_eq!(notifications[1].0, 1.0);
        assert_eq!(
            notifications[1].1,
            Some("Agent analysis complete".to_string())
        );
    }

    #[tokio::test]
    async fn test_notify_error() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: ProgressCallback = Arc::new(move |progress, message| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.lock().await.push((progress, message));
            })
        });

        let notifier = ProgressNotifier::new(callback, "test");

        // First send started
        notifier.notify_started().await;
        // Then error (without calling analyzing)
        notifier.notify_error("timeout after 300 seconds").await;

        let notifications = received.lock().await;
        assert_eq!(notifications.len(), 3);
        // Started
        assert_eq!(notifications[0].0, 0.0);
        // Analyzing (auto-sent by notify_error)
        assert_eq!(notifications[1].0, 0.5);
        // Error (complete)
        assert_eq!(notifications[2].0, 1.0);
        assert_eq!(
            notifications[2].1,
            Some("Agent failed: timeout after 300 seconds".to_string())
        );
    }

    #[tokio::test]
    async fn test_full_workflow() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: ProgressCallback = Arc::new(move |progress, message| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.lock().await.push((progress, message));
            })
        });

        let notifier = ProgressNotifier::new(callback, "dependency_analysis");

        // Full workflow
        notifier.notify_started().await;
        notifier.notify_analyzing().await;
        notifier.notify_complete().await;

        let notifications = received.lock().await;
        assert_eq!(notifications.len(), 3);
        assert_eq!(notifications[0].0, 0.0);
        assert_eq!(
            notifications[0].1,
            Some("Agent started: dependency_analysis".to_string())
        );
        assert_eq!(notifications[1].0, 0.5);
        assert_eq!(
            notifications[1].1,
            Some("Agent analyzing with tools...".to_string())
        );
        assert_eq!(notifications[2].0, 1.0);
        assert_eq!(
            notifications[2].1,
            Some("Agent analysis complete".to_string())
        );
    }

    #[test]
    fn test_noop_notifier() {
        let notifier = ProgressNotifier::noop();
        assert!(!notifier.is_analyzing_sent());
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let callback: ProgressCallback = Arc::new(move |_, _| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        });

        let notifier1 = ProgressNotifier::new(callback, "test");
        let notifier2 = notifier1.clone();

        // Call analyzing on first notifier
        notifier1.notify_analyzing().await;

        // Second notifier should see the state
        assert!(notifier2.is_analyzing_sent());

        // Calling on second should not send again
        notifier2.notify_analyzing().await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
