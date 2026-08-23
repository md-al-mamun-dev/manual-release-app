use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{Runner, RunnerError, RunnerState};
use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::{CapturedOutput, ProcessOutcome, ProcessResult};
use chrono::Utc;

pub struct MockRunner {
    workspace_path: PathBuf,
    state: RunnerState,
    executor: ProcessExecutor,
    fail_create: bool,
    fail_prepare: bool,
    fail_cleanup: bool,
    pub fail_command: bool,
    pub fail_large_report: bool,
    pub fail_trivy_parse: bool,
}

impl MockRunner {
    pub fn new(workspace_path: PathBuf, _release_id: Uuid, _job_id: Uuid) -> Self {
        Self {
            workspace_path,
            state: RunnerState::Creating,
            executor: ProcessExecutor::new(10 * 1024 * 1024, Duration::from_secs(5)),
            fail_create: false,
            fail_prepare: false,
            fail_cleanup: false,
            fail_command: false,
            fail_large_report: false,
            fail_trivy_parse: false,
        }
    }

    pub fn with_fail_create(mut self) -> Self {
        self.fail_create = true;
        self
    }

    pub fn with_fail_prepare(mut self) -> Self {
        self.fail_prepare = true;
        self
    }

    pub fn with_fail_cleanup(mut self) -> Self {
        self.fail_cleanup = true;
        self
    }

    pub fn with_fail_command(mut self) -> Self {
        self.fail_command = true;
        self
    }
}

#[async_trait::async_trait]
impl Runner for MockRunner {
    async fn create(&mut self) -> Result<(), RunnerError> {
        if self.fail_create {
            return Err(RunnerError::CreationFailed(
                "Simulated create failure".into(),
            ));
        }
        fs::create_dir_all(&self.workspace_path)
            .await
            .map_err(|e| RunnerError::CreationFailed(e.to_string()))?;
        self.state = RunnerState::Ready;
        Ok(())
    }

    async fn prepare(&mut self) -> Result<(), RunnerError> {
        if self.fail_prepare {
            return Err(RunnerError::PreparationFailed(
                "Simulated prepare failure".into(),
            ));
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
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        if self.fail_command {
            return Ok(ProcessResult {
                outcome: ProcessOutcome::NonZeroExit,
                exit_code: Some(1),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                duration: Duration::from_secs(1),
                stdout: CapturedOutput {
                    text: "".to_string(),
                    truncated: false,
                    bytes_read: 0,
                },
                stderr: CapturedOutput {
                    text: "Simulated command failure".to_string(),
                    truncated: false,
                    bytes_read: 25,
                },
                spawn_error: None,
            });
        }

        // We use ProcessExecutor to run local commands, but we skip uname/node validations
        Ok(self
            .executor
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

    async fn build_image(
        &self,
        _dockerfile: &str,
        _context: &str,
        _tag: &str,
        _cancel_token: CancellationToken,
        _output_sender: Option<tokio::sync::mpsc::Sender<(String, String)>>,
    ) -> Result<(ProcessResult, Option<String>), RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }
        if self.fail_command {
            return Ok((
                ProcessResult {
                    outcome: ProcessOutcome::NonZeroExit,
                    exit_code: Some(1),
                    started_at: Utc::now(),
                    finished_at: Utc::now(),
                    duration: std::time::Duration::from_secs(1),
                    stdout: CapturedOutput {
                        text: "".to_string(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    stderr: CapturedOutput {
                        text: "Simulated build failure".to_string(),
                        truncated: false,
                        bytes_read: 23,
                    },
                    spawn_error: None,
                },
                None,
            ));
        }
        Ok((
            ProcessResult {
                outcome: ProcessOutcome::Succeeded,
                exit_code: Some(0),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                duration: std::time::Duration::from_secs(1),
                stdout: CapturedOutput {
                    text: "Image built".to_string(),
                    truncated: false,
                    bytes_read: 11,
                },
                stderr: CapturedOutput {
                    text: "".to_string(),
                    truncated: false,
                    bytes_read: 0,
                },
                spawn_error: None,
            },
            Some("sha256:mockdigest".to_string()),
        ))
    }

    async fn test_image(
        &self,
        _tag: &str,
        _port: i32,
        _health_endpoint: &str,
        _cancel_token: CancellationToken,
        _output_sender: Option<tokio::sync::mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }
        if self.fail_command {
            return Ok(ProcessResult {
                outcome: ProcessOutcome::NonZeroExit,
                exit_code: Some(1),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                duration: std::time::Duration::from_secs(1),
                stdout: CapturedOutput {
                    text: "".to_string(),
                    truncated: false,
                    bytes_read: 0,
                },
                stderr: CapturedOutput {
                    text: "Simulated test failure".to_string(),
                    truncated: false,
                    bytes_read: 22,
                },
                spawn_error: None,
            });
        }
        Ok(ProcessResult {
            outcome: ProcessOutcome::Succeeded,
            exit_code: Some(0),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration: std::time::Duration::from_secs(1),
            stdout: CapturedOutput {
                text: "Image tested".to_string(),
                truncated: false,
                bytes_read: 12,
            },
            stderr: CapturedOutput {
                text: "".to_string(),
                truncated: false,
                bytes_read: 0,
            },
            spawn_error: None,
        })
    }

    async fn scan_image(
        &self,
        tar_path: &str,
        _report_output_path: &str,
        _cancel_token: CancellationToken,
        _output_sender: Option<tokio::sync::mpsc::Sender<(String, String)>>,
    ) -> Result<(ProcessResult, Option<String>), RunnerError> {
        if self.fail_command {
            return Ok((
                ProcessResult {
                    outcome: ProcessOutcome::NonZeroExit,
                    exit_code: Some(1),
                    started_at: Utc::now(),
                    finished_at: Utc::now(),
                    duration: std::time::Duration::from_secs(1),
                    stdout: CapturedOutput {
                        text: "".to_string(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    stderr: CapturedOutput {
                        text: "Simulated scan failure".to_string(),
                        truncated: false,
                        bytes_read: 22,
                    },
                    spawn_error: None,
                },
                None,
            ));
        }

        let full_tar_path = self.workspace_path.join(tar_path.trim_start_matches('/'));
        if !full_tar_path.exists()
            && tar_path != "/workspace/image.tar"
            && !self.workspace_path.join("image.tar").exists()
        {
            return Ok((
                ProcessResult {
                    outcome: ProcessOutcome::NonZeroExit,
                    exit_code: Some(1),
                    started_at: Utc::now(),
                    finished_at: Utc::now(),
                    duration: std::time::Duration::from_secs(1),
                    stdout: CapturedOutput {
                        text: "".to_string(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    stderr: CapturedOutput {
                        text: format!("Image tar file not found: {}", tar_path),
                        truncated: false,
                        bytes_read: 30,
                    },
                    spawn_error: None,
                },
                None,
            ));
        }

        if self.fail_large_report {
            return Ok((
                ProcessResult {
                    outcome: ProcessOutcome::Succeeded,
                    exit_code: Some(0),
                    started_at: Utc::now(),
                    finished_at: Utc::now(),
                    duration: std::time::Duration::from_secs(1),
                    stdout: CapturedOutput {
                        text: "Scan completed".to_string(),
                        truncated: false,
                        bytes_read: 14,
                    },
                    stderr: CapturedOutput {
                        text: "".to_string(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    spawn_error: None,
                },
                Some("A".repeat(11 * 1024 * 1024)),
            )); // Simulate returning >10MB string
        }

        let report_str = if self.fail_trivy_parse {
            "invalid json".to_string()
        } else {
            r#"{"SchemaVersion": 2, "Results": []}"#.to_string()
        };

        Ok((
            ProcessResult {
                outcome: ProcessOutcome::Succeeded,
                exit_code: Some(0),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                duration: std::time::Duration::from_secs(1),
                stdout: CapturedOutput {
                    text: "Scan completed".to_string(),
                    truncated: false,
                    bytes_read: 14,
                },
                stderr: CapturedOutput {
                    text: "".to_string(),
                    truncated: false,
                    bytes_read: 0,
                },
                spawn_error: None,
            },
            Some(report_str),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn publish_image(
        &self,
        _tar_path: &str,
        _registry: &str,
        _repository: &str,
        _tag: &str,
        _username: &str,
        _password: &str,
        _cancel_token: CancellationToken,
        _output_sender: Option<tokio::sync::mpsc::Sender<(String, String)>>,
    ) -> Result<(ProcessResult, Option<String>), RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }
        if self.fail_command {
            return Ok((
                ProcessResult {
                    outcome: ProcessOutcome::NonZeroExit,
                    exit_code: Some(1),
                    started_at: Utc::now(),
                    finished_at: Utc::now(),
                    duration: std::time::Duration::from_secs(1),
                    stdout: CapturedOutput {
                        text: "".to_string(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    stderr: CapturedOutput {
                        text: "Simulated publish failure".to_string(),
                        truncated: false,
                        bytes_read: 25,
                    },
                    spawn_error: None,
                },
                None,
            ));
        }
        Ok((
            ProcessResult {
                outcome: ProcessOutcome::Succeeded,
                exit_code: Some(0),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                duration: std::time::Duration::from_secs(1),
                stdout: CapturedOutput {
                    text: "Image published".to_string(),
                    truncated: false,
                    bytes_read: 15,
                },
                stderr: CapturedOutput {
                    text: "".to_string(),
                    truncated: false,
                    bytes_read: 0,
                },
                spawn_error: None,
            },
            Some("sha256:mockdigest".to_string()),
        ))
    }

    async fn cleanup(&mut self) -> Result<(), RunnerError> {
        if self.fail_cleanup {
            return Err(RunnerError::CleanupFailed(
                "Simulated cleanup failure".into(),
            ));
        }
        self.state = RunnerState::CleaningUp;
        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Destroyed;
        Ok(())
    }

    fn spawn_detached_cleanup(&self) {
        // Mock runner doesn't need real detached cleanup
    }
}
