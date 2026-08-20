use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Internal executor error: {0}")]
    Internal(String),
}
