use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::ProcessOutcome;
use crate::workspace::workspace_error::WorkspaceError;

pub struct GitWorkspaceManager {
    workspace_root: PathBuf,
}

impl GitWorkspaceManager {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Gets the isolated workspace path for a specific job.
    pub fn get_workspace_path(&self, job_id: Uuid) -> PathBuf {
        self.workspace_root.join(job_id.to_string())
    }

    /// Creates an isolated workspace and checks out the specific release SHA.
    pub async fn create_workspace(
        &self,
        original_repo_path: &str,
        job_id: Uuid,
        sha: &str,
        cancel_token: CancellationToken,
    ) -> Result<PathBuf, WorkspaceError> {
        let workspace_path = self.get_workspace_path(job_id);

        // Defend against path traversal: check if workspace_path is strictly inside workspace_root
        if !workspace_path.starts_with(&self.workspace_root) {
            return Err(WorkspaceError::InvalidWorkspacePath);
        }

        // Clean up any existing directory just in case
        if workspace_path.exists() {
            tokio::fs::remove_dir_all(&workspace_path)
                .await
                .map_err(|e| WorkspaceError::CleanupFailed(e.to_string()))?;
        }

        // Create the root if it doesn't exist
        tokio::fs::create_dir_all(&self.workspace_root)
            .await
            .map_err(|e| WorkspaceError::DirectoryCreationFailed(e.to_string()))?;

        // 1. Local clone with --no-checkout
        let clone_args = vec![
            "clone".to_string(),
            "--no-checkout".to_string(),
            original_repo_path.to_string(),
            workspace_path.to_string_lossy().to_string(),
        ];

        let executor = ProcessExecutor::new(1024 * 1024, Duration::from_secs(5));
        let empty_env = HashMap::new();

        let clone_result = executor
            .execute(
                "git",
                &clone_args,
                &self.workspace_root,
                &empty_env,
                Duration::from_secs(120),
                cancel_token.clone(),
                None,
            )
            .await;

        if !matches!(clone_result.outcome, ProcessOutcome::Succeeded) {
            return Err(WorkspaceError::CloneFailed(clone_result.stderr.text));
        }

        // 2. Checkout the exact SHA
        let checkout_args = vec!["checkout".to_string(), "-q".to_string(), sha.to_string()];

        let checkout_result = executor
            .execute(
                "git",
                &checkout_args,
                &workspace_path,
                &empty_env,
                Duration::from_secs(60),
                cancel_token.clone(),
                None,
            )
            .await;

        if !matches!(checkout_result.outcome, ProcessOutcome::Succeeded) {
            return Err(WorkspaceError::CheckoutFailed(checkout_result.stderr.text));
        }

        Ok(workspace_path)
    }

    /// Safely cleans up a job workspace.
    pub async fn cleanup_workspace(&self, job_id: Uuid) -> Result<(), WorkspaceError> {
        let workspace_path = self.get_workspace_path(job_id);

        if !workspace_path.starts_with(&self.workspace_root) {
            return Err(WorkspaceError::InvalidWorkspacePath);
        }

        if workspace_path.exists() {
            tokio::fs::remove_dir_all(&workspace_path)
                .await
                .map_err(|e| WorkspaceError::CleanupFailed(e.to_string()))?;
        }

        Ok(())
    }
}
