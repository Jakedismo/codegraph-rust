use crate::{errors::*, types::*};

impl From<HookKind> for &'static str {
    fn from(k: HookKind) -> Self {
        match k { HookKind::PreCommit => "pre-commit", HookKind::PostCommit => "post-commit" }
    }
}

// Hook installation is implemented on GitRepository in repo.rs

