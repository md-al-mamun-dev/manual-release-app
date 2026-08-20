use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub mod prepare_release_executor;

pub struct JobWorker {
    token: CancellationToken,
}

impl JobWorker {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }

    pub async fn run(&self) {
        tracing::info!("job worker started");

        loop {
            tokio::select! {
                _ = self.token.cancelled() => {
                    tracing::info!("job worker shutting down gracefully");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    // TODO: Poll for background jobs
                    tracing::debug!("job worker polling for work");
                }
            }
        }

        tracing::info!("job worker fully stopped");
    }
}
