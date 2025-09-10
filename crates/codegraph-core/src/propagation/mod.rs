//! Change propagation system for cascading updates across files.
//!
//! This module provides:
//! - DependencyGraph: tracks inter-file relationships (imports/exports/uses)
//! - Impact analysis: computes affected files from a change without cycles
//! - Priority scheduler: orders updates by impact severity and visibility
//! - Batching: coalesces updates to reduce database writes
//! - Notifications: pluggable listener + broadcast for API subscriptions
//!
//! The API crate can implement `ChangeNotifier` to forward events to
//! WebSocket/GraphQL subscriptions without introducing a crate cycle.

mod manager;

pub use manager::*;

