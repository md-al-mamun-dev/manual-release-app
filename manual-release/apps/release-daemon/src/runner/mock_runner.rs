use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::fs;
use tokio_util::sync::CancellationToken;

use super::{Runner, RunnerError, RunnerState};
use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::ProcessResult;

pub struct MockRunner {
    workspace_path: PathBuf,
    state: RunnerState,
    executor: ProcessExecutor,
}

impl MockRunner {
    pub fn new(workspace_path: PathBuf) -> Self {
        Self {
            workspace_path,
            state: RunnerState::Creating,
            executor: ProcessExecutor::new(10 * 1024 * 1024, Duration::from_secs(5)),
        }
    }
}

#[async_trait::async_trait]
impl Runner for MockRunner {
    async fn create(&mut self) -> Result<(), RunnerError> {
        fs::create_dir_all(&self.workspace_path)
            .await
            .map_err(|e| RunnerError::CreationFailed(e.to_string()))?;
        self.state = RunnerState::Ready;
        Ok(())
    }

    async fn prepare(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Running;
        Ok(())
    }

    async fn workspace(&self) -> PathBuf {
        self.workspace_path.clone()
    }

    async fn execute(
        &self,
        program: &str,
        args: &[String],
        envs: &HashMap<String, String>,
        timeout: Duration,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed("Runner is not in Running state".into()));
        }

        // We use ProcessExecutor to run local commands, but we skip uname/node validations
        Ok(self.executor
            .execute(
                program,
                args,
                &self.workspace_path,
                envs,
                timeout,
                cancel_token,
                output_sender,
            )
            .await)
    }

    async fn cleanup(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::CleaningUp;
        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Destroyed;
        Ok(())
    }
}
