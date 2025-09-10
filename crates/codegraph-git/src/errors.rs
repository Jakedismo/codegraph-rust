use thiserror::Error;

pub type Result<T> = std::result::Result<T, GitIntegrationError>;

#[derive(Debug, Error)]
pub enum GitIntegrationError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Watcher error: {0}")]
    Notify(#[from] notify::Error),

    #[error("Repository not found at path: {0}")]
    RepoNotFound(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Invalid UTF-8 in path")] 
    InvalidUtf8,

    #[error("Watcher error: {0}")]
    Watcher(String),

    #[error("Operation not supported in bare repository")] 
    BareRepository,
}
