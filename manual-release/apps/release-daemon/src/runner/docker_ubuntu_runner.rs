use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use super::{Runner, RunnerError, RunnerState};
use crate::config::AppConfig;
use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::{ProcessOutcome, ProcessResult};

pub struct LocalDockerUbuntuRunner {
    workspace_path: PathBuf,
    state: RunnerState,
    executor: ProcessExecutor,
    container_name: String,
    config: AppConfig,
}

impl LocalDockerUbuntuRunner {
    pub fn new(workspace_path: PathBuf, config: AppConfig) -> Self {
        let container_name = format!("cicd-runner-{}", Uuid::new_v4());
        Self {
            workspace_path,
            state: RunnerState::Creating,
            executor: ProcessExecutor::new(10 * 1024 * 1024, Duration::from_secs(5)),
            container_name,
            config,
        }
    }

    async fn exec_command(
        &self,
        command: &[&str],
        timeout: Duration,
        cancel_token: CancellationToken,
    ) -> Result<ProcessResult, RunnerError> {
        let empty_env = HashMap::new();

        let mut args = vec!["exec".to_string(), self.container_name.clone()];
        args.extend(command.iter().map(|s| s.to_string()));

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path, // local CWD for the executor
                &empty_env,
                timeout,
                cancel_token,
                None,
            )
            .await;

        Ok(result)
    }

    async fn verify_dependency(
        &self,
        command: &[&str],
        expected: &str,
        component: &str,
    ) -> Result<(), RunnerError> {
        let cancel_token = CancellationToken::new();
        let result = self
            .exec_command(command, Duration::from_secs(10), cancel_token)
            .await?;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "{} validation failed. Error: {}",
                component, result.stderr.text
            )));
        }

        if !result
            .stdout
            .text
            .to_lowercase()
            .contains(&expected.to_lowercase())
        {
            return Err(RunnerError::PreparationFailed(format!(
                "{} validation failed. Expected '{}' in output, got: {}",
                component, expected, result.stdout.text
            )));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Runner for LocalDockerUbuntuRunner {
    async fn create(&mut self) -> Result<(), RunnerError> {
        let empty_env = HashMap::new();
        let cancel_token = CancellationToken::new();

        info!("Creating Docker container: {}", self.container_name);

        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            self.container_name.clone(),
            // Filesystem Isolation
            "-v".to_string(),
            format!("{}:/workspace", self.workspace_path.display()),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
            "--tmpfs".to_string(),
            "/run".to_string(),
            "-w".to_string(),
            "/workspace".to_string(),
            // Security profile
            "--security-opt=no-new-privileges".to_string(),
            // Limits
            format!("--memory={}", self.config.runner_memory_limit),
            format!("--cpus={}", self.config.runner_cpus_limit),
            format!("--pids-limit={}", self.config.runner_pids_limit),
            // Network policy
            format!("--network={}", self.config.runner_network_policy),
        ];

        args.push(self.config.runner_ubuntu_image.clone());
        args.push("tail".to_string());
        args.push("-f".to_string());
        args.push("/dev/null".to_string());

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(300),
                cancel_token,
                None,
            )
            .await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::CreationFailed(format!(
                "Failed to start docker container: {}",
                result.stderr.text
            )));
        }

        self.state = RunnerState::Ready;
        Ok(())
    }

    async fn prepare(&mut self) -> Result<(), RunnerError> {
        info!("Preparing Docker container: {}", self.container_name);

        let cancel_token = CancellationToken::new();

        // 1. apt-get update
        let res = self
            .exec_command(
                &["apt-get", "update"],
                Duration::from_secs(60),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get update failed: {}",
                res.stderr.text
            )));
        }

        // 2. Install basic tools
        let res = self
            .exec_command(
                &[
                    "apt-get",
                    "install",
                    "-y",
                    "curl",
                    "git",
                    "build-essential",
                    "ca-certificates",
                ],
                Duration::from_secs(300),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get install tools failed: {}",
                res.stderr.text
            )));
        }

        // 3. Download nodesource setup script
        let res = self
            .exec_command(
                &[
                    "curl",
                    "-fsSL",
                    "-o",
                    "/tmp/setup_node.sh",
                    "https://deb.nodesource.com/setup_20.x",
                ],
                Duration::from_secs(30),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "curl nodesource failed: {}",
                res.stderr.text
            )));
        }

        // 4. Run nodesource setup script (using bash since it's a downloaded script from NodeSource, but we avoid bash -c string interpolation)
        let res = self
            .exec_command(
                &["bash", "/tmp/setup_node.sh"],
                Duration::from_secs(60),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "nodesource script failed: {}",
                res.stderr.text
            )));
        }

        // 5. Install Node.js
        let res = self
            .exec_command(
                &["apt-get", "install", "-y", "nodejs"],
                Duration::from_secs(300),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get install nodejs failed: {}",
                res.stderr.text
            )));
        }

        // 6. Create non-root user 'ci_user'
        let res = self
            .exec_command(
                &["useradd", "-m", "-s", "/bin/bash", "-u", "1000", "ci_user"],
                Duration::from_secs(10),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded)
            && !res.stderr.text.contains("already exists")
        {
            return Err(RunnerError::PreparationFailed(format!(
                "useradd failed: {}",
                res.stderr.text
            )));
        }

        // 7. Change ownership of /workspace
        let res = self
            .exec_command(
                &["chown", "-R", "ci_user:ci_user", "/workspace"],
                Duration::from_secs(10),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "chown failed: {}",
                res.stderr.text
            )));
        }

        // Verify Environment
        self.verify_dependency(&["uname", "-s"], "Linux", "OS")
            .await?;
        self.verify_dependency(&["cat", "/etc/os-release"], "ubuntu", "Distribution")
            .await?;
        self.verify_dependency(&["git", "--version"], "git", "Git")
            .await?;
        self.verify_dependency(&["node", "-v"], "v20.", "Node.js")
            .await?;
        self.verify_dependency(&["npm", "-v"], "", "npm").await?; // just ensure it doesn't fail

        self.state = RunnerState::Running;
        Ok(())
    }

    async fn workspace(&self) -> PathBuf {
        PathBuf::from("/workspace")
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
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        let mut docker_args = vec![
            "exec".to_string(),
            "-u".to_string(),
            "ci_user".to_string(),
            "-w".to_string(),
            "/workspace".to_string(),
        ];

        for (k, v) in envs {
            docker_args.push("-e".to_string());
            docker_args.push(format!("{}={}", k, v));
        }

        docker_args.push(self.container_name.clone());
        docker_args.push(program.to_string());
        docker_args.extend_from_slice(args);

        Ok(self
            .executor
            .execute(
                "docker",
                &docker_args,
                &self.workspace_path,
                &HashMap::new(), // Envs passed explicitly via docker arguments
                timeout,
                cancel_token,
                output_sender,
            )
            .await)
    }

    async fn cleanup(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::CleaningUp;

        info!("Cleaning up Docker container: {}", self.container_name);

        let empty_env = HashMap::new();
        let cancel_token = CancellationToken::new();

        let args = vec![
            "rm".to_string(),
            "-f".to_string(),
            self.container_name.clone(),
        ];

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(10),
                cancel_token,
                None,
            )
            .await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            warn!(
                "Failed to remove docker container {}: {}",
                self.container_name, result.stderr.text
            );
            return Err(RunnerError::CleanupFailed(format!(
                "Failed to remove docker container: {}",
                result.stderr.text
            )));
        }

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Destroyed;
        Ok(())
    }
}
