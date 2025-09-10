use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::env;
use std::path::PathBuf;

pub static TEST_DB_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// RAII guard that switches current_dir to a temp dir and restores it on drop
pub struct WorkdirGuard {
    prev: PathBuf,
    _tmp: tempfile::TempDir,
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
    }
}

/// Create an isolated temporary working directory for filesystem-backed tests
pub fn temp_workdir() -> WorkdirGuard {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let prev = env::current_dir().expect("cwd");
    env::set_current_dir(tmp.path()).expect("chdir temp");
    WorkdirGuard { prev, _tmp: tmp }
}

/// Utility to quickly build a CodeNode for tests
pub fn make_node(
    name: &str,
    node_type: Option<codegraph_core::NodeType>,
    language: Option<codegraph_core::Language>,
) -> codegraph_core::CodeNode {
    use codegraph_core::{CodeNode, Location};
    CodeNode::new(
        name.to_string(),
        node_type,
        language,
        Location { file_path: "test.rs".into(), line: 1, column: 1, end_line: None, end_column: None },
    )
}

