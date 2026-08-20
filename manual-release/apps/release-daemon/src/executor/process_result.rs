use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessOutcome {
    Succeeded,
    NonZeroExit,
    TimedOut,
    Cancelled,
    SpawnFailed,
}

#[derive(Debug, Clone)]
pub struct CapturedOutput {
    pub text: String,
    pub truncated: bool,
    pub bytes_read: usize,
}

#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub outcome: ProcessOutcome,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration: Duration,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub spawn_error: Option<String>,
}
