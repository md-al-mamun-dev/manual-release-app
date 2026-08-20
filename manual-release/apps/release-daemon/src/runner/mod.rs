pub mod context;
pub mod local_ubuntu;
pub mod manager;
pub mod mock_runner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::executor::process_result::ProcessResult;

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Creation failed: {0}")]
    CreationFailed(String),
    #[error("Preparation failed: {0}")]
    PreparationFailed(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),
    #[error("Runner not implemented: {0}")]
    NotImplemented(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerState {
    Creating,
    Ready,
    Running,
    CleaningUp,
    Destroyed,
}

#[async_trait::async_trait]
pub trait Runner: Send + Sync {
    async fn create(&mut self) -> Result<(), RunnerError>;
    async fn prepare(&mut self) -> Result<(), RunnerError>;
    async fn workspace(&self) -> PathBuf;
    
    async fn execute(
        &self,
        program: &str,
        args: &[String],
        envs: &HashMap<String, String>,
        timeout: Duration,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError>;

    async fn cleanup(&mut self) -> Result<(), RunnerError>;
    async fn destroy(&mut self) -> Result<(), RunnerError>;
}
