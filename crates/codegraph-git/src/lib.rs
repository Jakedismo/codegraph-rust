//! CodeGraph Git integration: hooks, branch tracking, change detection,
//! conflict resolution strategies, and history analysis using libgit2.
//!
//! This crate aims to be thread-safe and efficient for large repositories.

pub mod errors;
pub mod types;
pub mod repo;
pub mod hooks;
pub mod watcher;
pub mod merge;
pub mod history;

pub use errors::{GitIntegrationError, Result};
pub use types::*;
pub use repo::GitRepository;
pub use types::{HookInstallOptions, HookKind};
pub use watcher::RepoWatcher;
pub use types::{WatchEvent, WatchOptions};
pub use types::MergeStrategy;
pub use types::{MergeOutcome, HistoryInsights, HistoryOptions};
