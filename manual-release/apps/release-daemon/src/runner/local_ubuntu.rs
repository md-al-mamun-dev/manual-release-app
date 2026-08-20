use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::{Runner, RunnerError, RunnerState};
use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::{ProcessOutcome, ProcessResult};

pub struct LocalUbuntuRunner {
    workspace_path: PathBuf,
    state: RunnerState,
    executor: ProcessExecutor,
}

impl LocalUbuntuRunner {
    pub fn new(workspace_path: PathBuf) -> Self {
        Self {
            workspace_path,
            state: RunnerState::Creating,
            executor: ProcessExecutor::new(10 * 1024 * 1024, Duration::from_secs(5)),
        }
    }

    /// Creates a safe environment by filtering out host secrets and credentials.
    fn get_safe_environment(requested_envs: &HashMap<String, String>) -> HashMap<String, String> {
        let mut safe_envs = HashMap::new();

        // Allow list of basic environment variables
        let allowed_keys = vec!["PATH", "HOME", "USER", "LANG", "LC_ALL"];
        for key in allowed_keys {
            if let Ok(val) = env::var(key) {
                safe_envs.insert(key.to_string(), val);
            }
        }

        // Add the requested envs explicitly, overwriting safely.
        // In a real runner, we might need to be more strict about what we accept.
        for (k, v) in requested_envs {
            safe_envs.insert(k.clone(), v.clone());
        }

        safe_envs
    }
}

#[async_trait::async_trait]
impl Runner for LocalUbuntuRunner {
    async fn create(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Ready;
        Ok(())
    }

    async fn prepare(&mut self) -> Result<(), RunnerError> {
        let empty_env = HashMap::new();
        let cancel_token = CancellationToken::new();

        // Verify Linux
        let uname = self.executor.execute(
            "uname",
            &["-s".to_string()],
            &self.workspace_path,
            &empty_env,
            Duration::from_secs(5),
            cancel_token.clone(),
            None,
        ).await;

        if uname.stdout.text.trim() != "Linux" {
            return Err(RunnerError::PreparationFailed(format!(
                "Not running on Linux. Host OS is: {}",
                uname.stdout.text.trim()
            )));
        }

        // Verify Node
        let node = self.executor.execute(
            "node",
            &["--version".to_string()],
            &self.workspace_path,
            &empty_env,
            Duration::from_secs(5),
            cancel_token.clone(),
            None,
        ).await;

        if !matches!(node.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed("Node is not available".into()));
        }

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

        // The instructions explicitly require placeholders for external infra.
        // Since no Docker/Lima etc. is currently available in the repository,
        // we use the local macOS ProcessExecutor but issue a warning.
        warn!("Running locally on macOS instead of Ubuntu VM (infrastructure not yet configured)");

        let safe_envs = Self::get_safe_environment(envs);

        Ok(self.executor
            .execute(
                program,
                args,
                &self.workspace_path,
                &safe_envs,
                timeout,
                cancel_token,
                output_sender,
            )
            .await)
    }

    async fn cleanup(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::CleaningUp;
        // In a real VM provider, we would delete the VM or clear Docker containers here.
        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Destroyed;
        Ok(())
    }
}
