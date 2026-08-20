use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::project::Project;
use crate::domain::release::Release;
use crate::executor::process_result::ProcessOutcome;
use crate::repositories::release_repository::ReleaseRepository;
use crate::runner::context::RunnerExecutionContext;

#[derive(Debug, thiserror::Error)]
pub enum SourceValidationError {
    #[error("Release not found")]
    ReleaseNotFound,

    #[error("Project not found")]
    ProjectNotFound,

    #[error("Invalid release state for source validation")]
    InvalidReleaseState,

    #[error("Original repository is invalid or not a git repository")]
    InvalidOriginalRepository,

    #[error("Release SHA not found in original repository")]
    ShaNotFound,

    #[error("Workspace creation failed: {0}")]
    WorkspaceCreationFailed(String),

    #[error("Workspace HEAD does not match expected SHA")]
    WorkspaceHeadMismatch,

    #[error("Workspace is not clean")]
    WorkspaceNotClean,

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

pub struct SourceValidationService {
    pool: PgPool,
}

impl SourceValidationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn validate_source(
        &self,
        job_id: Uuid,
        release_id: Uuid,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<PathBuf, SourceValidationError> {
        // 1. Verify release exists
        let release = sqlx::query_as!(
            Release,
            "SELECT id, project_id, source_inspection_id, version, git_commit, git_branch, source_dirty, status, requested_by, created_at, updated_at FROM releases WHERE id = $1",
            release_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SourceValidationError::ReleaseNotFound)?;

        // 2. Verify state is CREATED
        if release.status != "CREATED" {
            return Err(SourceValidationError::InvalidReleaseState);
        }

        // 3. Verify project exists
        let project = sqlx::query_as!(
            Project,
            "SELECT id, name, repository_path, repository_url, default_branch, archived_at, created_at, updated_at FROM projects WHERE id = $1",
            release.project_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SourceValidationError::ProjectNotFound)?;

        let empty_env = HashMap::new();

        // 4. Verify original repo is a git repository
        let is_inside_work_tree = context
            .execute(
                "git",
                &["-C".to_string(), project.repository_path.clone(), "rev-parse".to_string(), "--is-inside-work-tree".to_string()],
                &empty_env,
                Duration::from_secs(10),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        if !matches!(is_inside_work_tree.outcome, ProcessOutcome::Succeeded) {
            println!("is_inside_work_tree failed: {:?}", is_inside_work_tree);
            return Err(SourceValidationError::InvalidOriginalRepository);
        }

        // 5. Verify release SHA exists in original repo
        let cat_file = context
            .execute(
                "git",
                &[
                    "-C".to_string(),
                    project.repository_path.clone(),
                    "cat-file".to_string(),
                    "-e".to_string(),
                    format!("{}^{{commit}}", release.git_commit),
                ],
                &empty_env,
                Duration::from_secs(10),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        if !matches!(cat_file.outcome, ProcessOutcome::Succeeded) {
            return Err(SourceValidationError::ShaNotFound);
        }

        // 6. Create isolated workspace
        let workspace_path = context.runner().workspace().await;
        
        let clone_args = vec![
            "clone".to_string(),
            "--no-checkout".to_string(),
            project.repository_path.clone(),
            workspace_path.to_string_lossy().to_string(),
        ];

        let clone_result = context
            .execute(
                "git",
                &clone_args,
                &empty_env,
                Duration::from_secs(120),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        if !matches!(clone_result.outcome, ProcessOutcome::Succeeded) {
            return Err(SourceValidationError::WorkspaceCreationFailed(clone_result.stderr.text));
        }

        let checkout_args = vec!["checkout".to_string(), "-q".to_string(), release.git_commit.clone()];
        let checkout_result = context
            .execute(
                "git",
                &checkout_args,
                &empty_env,
                Duration::from_secs(60),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        if !matches!(checkout_result.outcome, ProcessOutcome::Succeeded) {
            return Err(SourceValidationError::WorkspaceCreationFailed(checkout_result.stderr.text));
        }

        // 7. Verify workspace resolves exactly to release.git_commit
        let head_sha = context
            .execute(
                "git",
                &["rev-parse".to_string(), "HEAD".to_string()],
                &empty_env,
                Duration::from_secs(10),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        let head_sha_str = head_sha.stdout.text;
        if head_sha_str.trim() != release.git_commit {
            return Err(SourceValidationError::WorkspaceHeadMismatch);
        }

        // 8. Verify workspace is clean
        let status = context
            .execute(
                "git",
                &["status".to_string(), "--porcelain".to_string()],
                &empty_env,
                Duration::from_secs(10),
                None,
            )
            .await
            .map_err(|e| SourceValidationError::WorkspaceCreationFailed(e.to_string()))?;

        if !status.stdout.text.is_empty() {
            return Err(SourceValidationError::WorkspaceNotClean);
        }

        // 9. Update release status
        let release_repo = ReleaseRepository::new(self.pool.clone());
        release_repo
            .transition_status(
                release_id,
                "CREATED",
                "SOURCE_VALIDATED",
                "SYSTEM",
                "Source validation passed",
            )
            .await?;

        Ok(workspace_path)
    }
}
