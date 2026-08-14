use std::{path::Path, process::Stdio, time::Duration};

use anyhow::{Context, bail};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

#[derive(Debug)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,

    pub stdout: String,

    pub stderr: String,

    pub stdout_truncated: bool,

    pub stderr_truncated: bool,
}

#[derive(Clone)]
pub struct CommandExecutor {
    max_output_bytes: usize,
}

impl CommandExecutor {
    pub fn new(max_output_bytes: usize) -> Self {
        Self { max_output_bytes }
    }

    pub async fn run(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        timeout_duration: Duration,
    ) -> anyhow::Result<CommandOutput> {
        let mut command = Command::new(program);

        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start executable: {program}"))?;

        let stdout = child.stdout.take().context("stdout was not captured")?;

        let stderr = child.stderr.take().context("stderr was not captured")?;

        let max_output_bytes = self.max_output_bytes;

        let stdout_task = tokio::spawn(read_bounded(stdout, max_output_bytes));

        let stderr_task = tokio::spawn(read_bounded(stderr, max_output_bytes));

        let status = match timeout(timeout_duration, child.wait()).await {
            Ok(result) => result?,

            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;

                let _ = stdout_task.await;
                let _ = stderr_task.await;

                bail!(
                    "{program} timed out after {} seconds",
                    timeout_duration.as_secs()
                );
            }
        };

        let stdout = stdout_task.await.context("stdout reader task failed")??;

        let stderr = stderr_task.await.context("stderr reader task failed")??;

        Ok(CommandOutput {
            exit_code: status.code(),

            stdout: stdout.text,

            stderr: stderr.text,

            stdout_truncated: stdout.truncated,

            stderr_truncated: stderr.truncated,
        })
    }
}

struct CapturedOutput {
    text: String,
    truncated: bool,
}

async fn read_bounded<R>(mut reader: R, max_bytes: usize) -> std::io::Result<CapturedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::new();

    let mut buffer = [0_u8; 8192];

    let mut total_bytes = 0_usize;

    loop {
        let bytes_read = reader.read(&mut buffer).await?;

        if bytes_read == 0 {
            break;
        }

        total_bytes += bytes_read;

        if retained.len() < max_bytes {
            let remaining = max_bytes - retained.len();

            let keep = remaining.min(bytes_read);

            retained.extend_from_slice(&buffer[..keep]);
        }
    }

    Ok(CapturedOutput {
        text: String::from_utf8_lossy(&retained).into_owned(),

        truncated: total_bytes > max_bytes,
    })
}
