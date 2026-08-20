use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};

use chrono::Utc;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc::Sender,
};
use tokio_util::sync::CancellationToken;

use crate::executor::process_result::{CapturedOutput, ProcessOutcome, ProcessResult};

#[derive(Clone)]
pub struct ProcessExecutor {
    max_output_bytes: usize,
    grace_period: Duration,
}

impl ProcessExecutor {
    pub fn new(max_output_bytes: usize, grace_period: Duration) -> Self {
        Self {
            max_output_bytes,
            grace_period,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        env: &HashMap<String, String>,
        timeout_duration: Duration,
        cancellation_token: CancellationToken,
        output_sender: Option<Sender<(String, String)>>,
    ) -> ProcessResult {
        let started_at = Utc::now();

        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        command.process_group(0);

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                let finished_at = Utc::now();
                return ProcessResult {
                    outcome: ProcessOutcome::SpawnFailed,
                    exit_code: None,
                    started_at,
                    finished_at,
                    duration: finished_at
                        .signed_duration_since(started_at)
                        .to_std()
                        .unwrap_or(Duration::ZERO),
                    stdout: CapturedOutput {
                        text: String::new(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    stderr: CapturedOutput {
                        text: String::new(),
                        truncated: false,
                        bytes_read: 0,
                    },
                    spawn_error: Some(e.to_string()),
                };
            }
        };

        let child_id = child.id().expect("Child process has no ID");
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let stdout_sender = output_sender.clone();
        let stderr_sender = output_sender;

        let stdout_task = tokio::spawn(read_bounded(
            stdout,
            self.max_output_bytes,
            "STDOUT".to_string(),
            stdout_sender,
        ));
        let stderr_task = tokio::spawn(read_bounded(
            stderr,
            self.max_output_bytes,
            "STDERR".to_string(),
            stderr_sender,
        ));

        let mut outcome = ProcessOutcome::Succeeded;
        let mut final_exit_code = None;

        tokio::select! {
            result = child.wait() => {
                if let Ok(status) = result {
                    final_exit_code = status.code();
                    if !status.success() {
                        outcome = ProcessOutcome::NonZeroExit;
                    }
                }
            }
            _ = tokio::time::sleep(timeout_duration) => {
                outcome = ProcessOutcome::TimedOut;
                self.terminate_process_group(child_id).await;
                let _ = child.wait().await;
            }
            _ = cancellation_token.cancelled() => {
                outcome = ProcessOutcome::Cancelled;
                self.terminate_process_group(child_id).await;
                let _ = child.wait().await;
            }
        }

        let finished_at = Utc::now();
        let duration = finished_at
            .signed_duration_since(started_at)
            .to_std()
            .unwrap_or(Duration::ZERO);

        let stdout_result = stdout_task.await.unwrap_or_else(|_| CapturedOutput {
            text: String::new(),
            truncated: false,
            bytes_read: 0,
        });
        let stderr_result = stderr_task.await.unwrap_or_else(|_| CapturedOutput {
            text: String::new(),
            truncated: false,
            bytes_read: 0,
        });

        ProcessResult {
            outcome,
            exit_code: final_exit_code,
            started_at,
            finished_at,
            duration,
            stdout: stdout_result,
            stderr: stderr_result,
            spawn_error: None,
        }
    }

    #[cfg(unix)]
    async fn terminate_process_group(&self, pid: u32) {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        let pgid = Pid::from_raw(-(pid as i32));

        // Send SIGTERM to the process group
        let _ = kill(pgid, Signal::SIGTERM);

        // Wait for the grace period
        tokio::time::sleep(self.grace_period).await;

        // Send SIGKILL to the process group
        let _ = kill(pgid, Signal::SIGKILL);
    }

    #[cfg(not(unix))]
    async fn terminate_process_group(&self, _pid: u32) {
        // Fallback for non-unix platforms
    }
}

async fn read_bounded<R>(
    mut reader: R,
    max_bytes: usize,
    stream_name: String,
    sender: Option<Sender<(String, String)>>,
) -> CapturedOutput
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0_usize;
    let mut current_line = String::new();

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(bytes_read) => {
                total_bytes += bytes_read;

                let chunk = &buffer[..bytes_read];

                if let Some(s) = &sender {
                    let text = String::from_utf8_lossy(chunk);
                    for ch in text.chars() {
                        current_line.push(ch);
                        if ch == '\n' || current_line.len() > 4096 {
                            let _ = s.send((stream_name.clone(), current_line.clone())).await;
                            current_line.clear();
                        }
                    }
                }

                if retained.len() < max_bytes {
                    let remaining = max_bytes - retained.len();
                    let keep = remaining.min(bytes_read);
                    retained.extend_from_slice(&chunk[..keep]);
                }
            }
            Err(_) => break, // On error, just return what we have
        }
    }

    #[allow(clippy::collapsible_if)]
    if let Some(s) = &sender {
        if !current_line.is_empty() {
            let _ = s.send((stream_name.clone(), current_line)).await;
        }
    }

    CapturedOutput {
        text: String::from_utf8_lossy(&retained).into_owned(),
        truncated: total_bytes > max_bytes,
        bytes_read: total_bytes,
    }
}
