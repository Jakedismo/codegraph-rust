use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInstallOptions {
    pub pre_commit: bool,
    pub post_commit: bool,
    pub overwrite: bool,
}

impl Default for HookInstallOptions {
    fn default() -> Self {
        Self {
            pre_commit: true,
            post_commit: true,
            overwrite: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HookKind {
    PreCommit,
    PostCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub added: usize,
    pub deleted: usize,
    pub modified: usize,
    pub renamed: usize,
    pub files_changed: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Prefer our changes on conflict
    Ours,
    /// Prefer their changes on conflict
    Theirs,
    /// Use libgit2 default merge and leave conflicts if unresolved
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub fast_forward: bool,
    pub conflicts: usize,
    pub committed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryOptions {
    pub branch: Option<String>,
    pub max_commits: Option<usize>,
    pub since_timestamp: Option<i64>,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            branch: None,
            max_commits: Some(5000),
            since_timestamp: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorStat {
    pub name: String,
    pub email: String,
    pub commits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChurn {
    pub additions: usize,
    pub deletions: usize,
}

use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryInsights {
    pub total_commits: usize,
    pub authors: Vec<AuthorStat>,
    pub file_churn: BTreeMap<String, FileChurn>,
    pub branches_analyzed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchOptions {
    pub debounce_ms: u64,
    pub ignore_dot_git: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce_ms: 200,
            ignore_dot_git: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    pub kind: String,
    pub path: Option<String>,
}
