//! CodeGraph Git integration: hooks, branch tracking, change detection,
//! conflict resolution strategies, and history analysis using libgit2.
//!
//! This crate aims to be thread-safe and efficient for large repositories.

pub mod errors;
pub mod history;
pub mod hooks;
pub mod merge;
pub mod repo;
pub mod types;
pub mod watcher;

pub use errors::{GitIntegrationError, Result};
pub use repo::GitRepository;
pub use types::MergeStrategy;
pub use types::*;
pub use types::{HistoryInsights, HistoryOptions, MergeOutcome};
pub use types::{HookInstallOptions, HookKind};
pub use types::{WatchEvent, WatchOptions};
pub use watcher::RepoWatcher;
