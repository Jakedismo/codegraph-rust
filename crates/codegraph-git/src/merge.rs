use crate::{errors::*, types::*};

// Merge logic resides in GitRepository::merge_branches.
// This module exports strategy and outcome types (in types.rs).

impl MergeOutcome {
    pub fn has_conflicts(&self) -> bool { self.conflicts > 0 }
}

