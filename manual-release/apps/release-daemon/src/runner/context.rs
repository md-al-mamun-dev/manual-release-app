use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{Runner, RunnerError};
use crate::executor::process_result::ProcessResult;

pub struct RunnerExecutionContext<'a> {
    runner: &'a dyn Runner,
    cancel_token: CancellationToken,
}

impl<'a> RunnerExecutionContext<'a> {
    pub fn new(runner: &'a dyn Runner, cancel_token: CancellationToken) -> Self {
        Self {
            runner,
            cancel_token,
        }
    }

    pub fn runner(&self) -> &dyn Runner {
        self.runner
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub async fn execute(
        &self,
        program: &str,
        args: &[String],
        envs: &HashMap<String, String>,
        timeout: Duration,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        self.runner
            .execute(
                program,
                args,
                envs,
                timeout,
                self.cancel_token.clone(),
                output_sender,
            )
            .await
    }
}
