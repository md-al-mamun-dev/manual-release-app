use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("Failed to create workspace directory: {0}")]
    DirectoryCreationFailed(String),

    #[error("Failed to clean up workspace: {0}")]
    CleanupFailed(String),

    #[error("Workspace path traversal detected or path outside root")]
    InvalidWorkspacePath,

    #[error("Failed to execute git clone: {0}")]
    CloneFailed(String),

    #[error("Failed to execute git checkout: {0}")]
    CheckoutFailed(String),
}
