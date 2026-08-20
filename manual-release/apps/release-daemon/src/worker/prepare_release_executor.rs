use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::job::{append_job_event, fail_job, fail_step, succeed_step};
use crate::repositories::release_repository::ReleaseRepository;
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

        let release_repo = ReleaseRepository::new(self.pool.clone());

        let mut runner = match self.runner_manager.create_runner(workspace_path.clone()) {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Failed to create runner: {}", e);
                let _ = fail_job(&self.pool, job_id, "RUNNER_ERROR", &error_msg).await;
                return Err(error_msg);
            }
        };

        let _ = append_job_event(
            &self.pool,
            job_id,
            None,
            "SYSTEM",
            "INFO",
            "Runner created",
        )
        .await;

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

        let context = RunnerExecutionContext::new(runner.as_ref(), cancel_token.clone());

        match self
            .validation_service
            .validate_source(job_id, release_id, &context)
            .await
        {
            Ok(_) => {
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
                        "CREATED", // Note: Source validation used to transition this to SOURCE_VALIDATED
                        "SOURCE_VALIDATED",
                        "SYSTEM",
                        "Source validation passed",
                    )
                    .await;

                let _ = release_repo
                    .transition_status(
                        release_id,
                        "SOURCE_VALIDATED",
                        "CI_RUNNING",
                        "SYSTEM",
                        "Starting CI",
                    )
                    .await;

                let node_ci = NodeCiService::new(self.pool.clone());
                let ci_result = node_ci
                    .execute_ci(
                        job_id,
                        validate_step_id,
                        &workspace_path,
                        cancel_token,
                        &context,
                    )
                    .await;

                let _ = runner.cleanup().await;
                let _ = runner.destroy().await;

                let _ = append_job_event(
                    &self.pool,
                    job_id,
                    None,
                    "SYSTEM",
                    "INFO",
                    "Runner cleaned up and destroyed",
                )
                .await;

                match ci_result {
                    Ok(_) => {
                        let _ = release_repo
                            .transition_status(
                                release_id,
                                "CI_RUNNING",
                                "CI_PASSED",
                                "SYSTEM",
                                "CI Passed",
                            )
                            .await;

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
                        let _ = release_repo
                            .transition_status(
                                release_id,
                                "CI_RUNNING",
                                "FAILED",
                                "SYSTEM",
                                &error_msg,
                            )
                            .await;
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
