use sqlx::PgPool;
use std::path::Path;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::job::{append_job_event, fail_job, fail_step, succeed_step};
use crate::repositories::release_repository::ReleaseRepository;
use crate::runner::Runner;
use crate::runner::context::RunnerExecutionContext;
use crate::runner::manager::RunnerManager;
use crate::services::node_ci_service::NodeCiService;
use crate::services::source_validation_service::SourceValidationService;
use crate::workspace::git_workspace::GitWorkspaceManager;

pub struct PrepareReleaseExecutor {
    pool: PgPool,
    validation_service: SourceValidationService,
    runner_manager: RunnerManager,
    workspace_manager: GitWorkspaceManager,
}

impl PrepareReleaseExecutor {
    pub fn new(
        pool: PgPool,
        validation_service: SourceValidationService,
        runner_manager: RunnerManager,
        workspace_manager: GitWorkspaceManager,
    ) -> Self {
        Self {
            pool,
            validation_service,
            runner_manager,
            workspace_manager,
        }
    }

    pub async fn execute(
        &self,
        job_id: Uuid,
        release_id: Uuid,
        validate_step_id: Uuid,
        cancel_token: CancellationToken,
    ) -> Result<(), String> {
        let release_repo = ReleaseRepository::new(self.pool.clone());
        let mut current_status = "CREATED".to_string();

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(validate_step_id),
            "SYSTEM",
            "INFO",
            "Starting PREPARE_RELEASE job",
        )
        .await;

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(validate_step_id),
            "SYSTEM",
            "INFO",
            "workspace creation started",
        )
        .await;

        let workspace_path = self.workspace_manager.get_workspace_path(job_id);

        let mut runner = match self.runner_manager.create_runner(workspace_path.clone()) {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Failed to create runner: {}", e);
                let _ = fail_job(&self.pool, job_id, "RUNNER_ERROR", &error_msg).await;
                let _ = release_repo
                    .transition_status(release_id, &current_status, "FAILED", "SYSTEM", &error_msg)
                    .await;
                return Err(error_msg);
            }
        };

        // Use tokio::select! to race the inner execution against the cancellation token
        let execution_result = tokio::select! {
            res = self.execute_ci_pipeline(
                runner.as_mut(),
                job_id,
                release_id,
                validate_step_id,
                cancel_token.clone(),
                &workspace_path,
                &mut current_status,
                &release_repo,
            ) => res,
            _ = cancel_token.cancelled() => {
                let error_msg = "Job cancelled by user".to_string();
                let _ = fail_job(&self.pool, job_id, "CANCELLED", &error_msg).await;
                Err(error_msg)
            }
        };

        // Guaranteed cleanup block (finally)
        if let Err(e) = runner.cleanup().await {
            let _ = append_job_event(
                &self.pool,
                job_id,
                None,
                "SYSTEM",
                "ERROR",
                &format!("Runner cleanup failed: {}", e),
            )
            .await;
        }

        if let Err(e) = runner.destroy().await {
            let _ = append_job_event(
                &self.pool,
                job_id,
                None,
                "SYSTEM",
                "ERROR",
                &format!("Runner destroy failed: {}", e),
            )
            .await;
        } else {
            let _ = append_job_event(
                &self.pool,
                job_id,
                None,
                "SYSTEM",
                "INFO",
                "Runner cleaned up and destroyed",
            )
            .await;
        }

        match execution_result {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = release_repo
                    .transition_status(release_id, &current_status, "FAILED", "SYSTEM", &e)
                    .await;
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_ci_pipeline(
        &self,
        runner: &mut dyn Runner,
        job_id: Uuid,
        release_id: Uuid,
        validate_step_id: Uuid,
        cancel_token: CancellationToken,
        workspace_path: &Path,
        current_status: &mut String,
        release_repo: &ReleaseRepository,
    ) -> Result<(), String> {
        let _ =
            append_job_event(&self.pool, job_id, None, "SYSTEM", "INFO", "Runner created").await;

        if let Err(e) = runner.create().await {
            let error_msg = format!("Failed to initialize runner: {}", e);
            let _ = fail_job(&self.pool, job_id, "RUNNER_ERROR", &error_msg).await;
            return Err(error_msg);
        }

        if let Err(e) = runner.prepare().await {
            let error_msg = format!("Failed to prepare runner: {}", e);
            let _ = fail_job(&self.pool, job_id, "RUNNER_ERROR", &error_msg).await;
            return Err(error_msg);
        }

        let _ = append_job_event(
            &self.pool,
            job_id,
            None,
            "SYSTEM",
            "INFO",
            "Runner prepared",
        )
        .await;

        let context = RunnerExecutionContext::new(runner, cancel_token.clone());

        match self
            .validation_service
            .validate_source(job_id, release_id, &context)
            .await
        {
            Ok(_) => {
                // Source validation service no longer needs to transition the status
                // but since it still does, we'll update our tracker to match it.
                // Wait, it is better to track it here.
                *current_status = "SOURCE_VALIDATED".to_string();

                let _ = append_job_event(
                    &self.pool,
                    job_id,
                    Some(validate_step_id),
                    "SYSTEM",
                    "INFO",
                    "workspace checkout completed",
                )
                .await;

                let _ = append_job_event(
                    &self.pool,
                    job_id,
                    Some(validate_step_id),
                    "SYSTEM",
                    "INFO",
                    "source validation succeeded",
                )
                .await;

                let _ = succeed_step(&self.pool, validate_step_id).await;

                let _ = release_repo
                    .transition_status(
                        release_id,
                        current_status,
                        "CI_RUNNING",
                        "SYSTEM",
                        "Starting CI",
                    )
                    .await;

                *current_status = "CI_RUNNING".to_string();

                let node_ci = NodeCiService::new(self.pool.clone());
                let ci_result = node_ci
                    .execute_ci(job_id, validate_step_id, workspace_path, &context)
                    .await;

                match ci_result {
                    Ok(_) => {
                        let _ = release_repo
                            .transition_status(
                                release_id,
                                current_status,
                                "CI_PASSED",
                                "SYSTEM",
                                "CI Passed",
                            )
                            .await;

                        *current_status = "CI_PASSED".to_string();

                        // Simulate remaining steps
                        let simulated_steps = vec![
                            "CREATE_RUNNER",
                            "BUILD_IMAGE",
                            "TEST_IMAGE",
                            "SCAN_IMAGE",
                            "PUBLISH_IMAGE",
                        ];

                        for step in simulated_steps {
                            let _ = append_job_event(
                                &self.pool,
                                job_id,
                                None,
                                "SYSTEM",
                                "INFO",
                                &format!("Simulated step {} succeeded", step),
                            )
                            .await;
                        }

                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        Err(error_msg)
                    }
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = append_job_event(
                    &self.pool,
                    job_id,
                    Some(validate_step_id),
                    "SYSTEM",
                    "ERROR",
                    &format!("source validation failed: {}", error_msg),
                )
                .await;

                let _ =
                    fail_step(&self.pool, validate_step_id, "VALIDATION_ERROR", &error_msg).await;
                let _ = fail_job(&self.pool, job_id, "VALIDATION_ERROR", &error_msg).await;

                Err(error_msg)
            }
        }
    }
}
