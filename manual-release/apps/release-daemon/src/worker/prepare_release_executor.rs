use sqlx::PgPool;
use std::path::Path;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::domain::job::{append_job_event, fail_job, fail_step, succeed_step};
use crate::repositories::project_build_config_repository::ProjectBuildConfigRepository;
use crate::repositories::release_repository::ReleaseRepository;
use crate::runner::Runner;
use crate::runner::context::RunnerExecutionContext;
use crate::runner::manager::RunnerManager;
use crate::services::build_plan_executor::BuildPlanExecutor;
use crate::services::build_plan_generator::BuildPlanGenerator;
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

        let runner =
            match self
                .runner_manager
                .create_runner(workspace_path.clone(), release_id, job_id)
            {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = format!("Failed to create runner: {}", e);
                    let _ = fail_job(&self.pool, job_id, "RUNNER_ERROR", &error_msg).await;
                    let _ = release_repo
                        .transition_status(
                            release_id,
                            &current_status,
                            "FAILED",
                            "SYSTEM",
                            &error_msg,
                        )
                        .await;
                    return Err(error_msg);
                }
            };

        // Use a drop guard to ensure the runner is cleaned up if the task is aborted/dropped.
        let mut cleanup_guard = RunnerCleanupGuard::new(runner);

        // Use tokio::select! to race the inner execution against the cancellation token
        let execution_result = tokio::select! {
            res = self.execute_ci_pipeline(
                cleanup_guard.runner.as_mut(),
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

        // Normal completion path: mark guard as completed so it doesn't spawn detached cleanup,
        // and do the cleanup synchronously here to record events.
        cleanup_guard.completed = true;

        if let Err(e) = cleanup_guard.runner.cleanup().await {
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

        if let Err(e) = cleanup_guard.runner.destroy().await {
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

                let release = match release_repo.find_by_id(release_id).await {
                    Ok(Some(r)) => r,
                    Ok(None) => {
                        let error_msg = "Release not found".to_string();
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to fetch release: {}", e);
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                };

                let config_repo = ProjectBuildConfigRepository::new(self.pool.clone());
                let config = match config_repo.find_by_project_id(release.project_id).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        let error_msg = "Project build config not found".to_string();
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to fetch build config: {}", e);
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                };

                let build_plan = match BuildPlanGenerator::generate(
                    &config,
                    &context,
                    workspace_path,
                    release.git_commit.clone(),
                )
                .await
                {
                    Ok(plan) => plan,
                    Err(e) => {
                        let error_msg = format!("Failed to generate build plan: {}", e);
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                };

                let app_config = AppConfig::from_env().map_err(|e| e.to_string())?;
                let executor = BuildPlanExecutor::new(self.pool.clone(), app_config);

                // CI RUNNING Phase
                let ci_result = executor.execute_ci(&build_plan, job_id, &context).await;
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
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = fail_job(&self.pool, job_id, "CI_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                }

                // IMAGE BUILDING Phase
                let _ = release_repo
                    .transition_status(
                        release_id,
                        current_status,
                        "IMAGE_BUILDING",
                        "SYSTEM",
                        "Building image",
                    )
                    .await;
                *current_status = "IMAGE_BUILDING".to_string();

                let build_result = executor
                    .execute_image_build(&build_plan, release_id, job_id, &context)
                    .await;
                match build_result {
                    Ok(built) => {
                        if built {
                            let _ = release_repo
                                .transition_status(
                                    release_id,
                                    current_status,
                                    "IMAGE_BUILT",
                                    "SYSTEM",
                                    "Image built successfully",
                                )
                                .await;
                            *current_status = "IMAGE_BUILT".to_string();
                        } else {
                            // If it didn't build an image, we jump to SCAN_PASSED (as a placeholder)
                            // But for now let's just leave it at IMAGE_BUILT (or we can just skip)
                            let _ = release_repo
                                .transition_status(
                                    release_id,
                                    current_status,
                                    "IMAGE_BUILT",
                                    "SYSTEM",
                                    "No image configured to build",
                                )
                                .await;
                            *current_status = "IMAGE_BUILT".to_string();
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ =
                            fail_job(&self.pool, job_id, "IMAGE_BUILD_FAILED", &error_msg).await;
                        return Err(error_msg);
                    }
                }

                // IMAGE TESTING Phase
                let _ = release_repo
                    .transition_status(
                        release_id,
                        current_status,
                        "IMAGE_TESTING",
                        "SYSTEM",
                        "Testing image",
                    )
                    .await;
                *current_status = "IMAGE_TESTING".to_string();

                let test_result = executor
                    .execute_image_test(&build_plan, job_id, &context)
                    .await;
                match test_result {
                    Ok(tested) => {
                        let msg = if tested {
                            "Image tested successfully"
                        } else {
                            "No image tests configured"
                        };
                        let _ = release_repo
                            .transition_status(
                                release_id,
                                current_status,
                                "IMAGE_TESTED",
                                "SYSTEM",
                                msg,
                            )
                            .await;
                        *current_status = "IMAGE_TESTED".to_string();

                        // SECURITY SCANNING Phase
                        let _ = release_repo
                            .transition_status(
                                release_id,
                                current_status,
                                "SECURITY_SCANNING",
                                "SYSTEM",
                                "Scanning image for vulnerabilities",
                            )
                            .await;
                        *current_status = "SECURITY_SCANNING".to_string();

                        let default_policy =
                            crate::domain::security_policy::SecurityPolicy::default();
                        let scan_result = executor
                            .execute_image_scan(
                                &build_plan,
                                release_id,
                                job_id,
                                &default_policy,
                                &context,
                            )
                            .await;

                        match scan_result {
                            Ok(passed) => {
                                if passed {
                                    let _ = release_repo
                                        .transition_status(
                                            release_id,
                                            current_status,
                                            "SCAN_PASSED",
                                            "SYSTEM",
                                            "Security scan passed",
                                        )
                                        .await;
                                    *current_status = "SCAN_PASSED".to_string();

                                    let _ = release_repo
                                        .transition_status(
                                            release_id,
                                            current_status,
                                            "IMAGE_APPROVED",
                                            "SYSTEM",
                                            "Image approved for release",
                                        )
                                        .await;
                                    *current_status = "IMAGE_APPROVED".to_string();

                                    // PUBLISHING Phase
                                    let _ = release_repo
                                        .transition_status(
                                            release_id,
                                            current_status,
                                            "PUBLISHING",
                                            "SYSTEM",
                                            "Publishing image to remote registry",
                                        )
                                        .await;
                                    *current_status = "PUBLISHING".to_string();

                                    let publish_result = executor
                                        .execute_image_publish(
                                            &build_plan,
                                            release_id,
                                            job_id,
                                            &context,
                                        )
                                        .await;

                                    match publish_result {
                                        Ok(published) => {
                                            if published {
                                                let _ = release_repo
                                                    .transition_status(
                                                        release_id,
                                                        current_status,
                                                        "PUBLISHED",
                                                        "SYSTEM",
                                                        "Image successfully published",
                                                    )
                                                    .await;
                                                *current_status = "PUBLISHED".to_string();

                                                let _ = succeed_step(&self.pool, validate_step_id)
                                                    .await;
                                                Ok(())
                                            } else {
                                                // If no dockerfile, it wasn't published but it's okay? No, if dockerfile is absent, it skips to published?
                                                // Wait, if no dockerfile, it's not a containerized release. We can just skip publishing.
                                                let _ = release_repo
                                                    .transition_status(
                                                        release_id,
                                                        current_status,
                                                        "PUBLISHED",
                                                        "SYSTEM",
                                                        "No image to publish",
                                                    )
                                                    .await;
                                                *current_status = "PUBLISHED".to_string();
                                                let _ = succeed_step(&self.pool, validate_step_id)
                                                    .await;
                                                Ok(())
                                            }
                                        }
                                        Err(e) => {
                                            let error_msg = e.to_string();
                                            let _ = release_repo
                                                .transition_status(
                                                    release_id,
                                                    current_status,
                                                    "PUBLISH_FAILED",
                                                    "SYSTEM",
                                                    &format!("Image publish failed: {}", error_msg),
                                                )
                                                .await;
                                            *current_status = "PUBLISH_FAILED".to_string();
                                            let _ = fail_job(
                                                &self.pool,
                                                job_id,
                                                "PUBLISH_FAILED",
                                                &error_msg,
                                            )
                                            .await;
                                            Err(error_msg)
                                        }
                                    }
                                } else {
                                    let _ = release_repo
                                        .transition_status(
                                            release_id,
                                            current_status,
                                            "SECURITY_FAILED",
                                            "SYSTEM",
                                            "Security scan failed policy evaluation",
                                        )
                                        .await;
                                    *current_status = "SECURITY_FAILED".to_string();

                                    let error_msg = "Security scan failed policy check".to_string();
                                    let _ = fail_job(
                                        &self.pool,
                                        job_id,
                                        "SECURITY_SCAN_FAILED",
                                        &error_msg,
                                    )
                                    .await;
                                    Err(error_msg)
                                }
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                let _ = release_repo
                                    .transition_status(
                                        release_id,
                                        current_status,
                                        "SECURITY_FAILED",
                                        "SYSTEM",
                                        &format!("Security scan error: {}", error_msg),
                                    )
                                    .await;
                                *current_status = "SECURITY_FAILED".to_string();

                                let _ =
                                    fail_job(&self.pool, job_id, "SECURITY_SCAN_ERROR", &error_msg)
                                        .await;
                                Err(error_msg)
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = fail_job(&self.pool, job_id, "IMAGE_TEST_FAILED", &error_msg).await;
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

/// A drop guard that ensures the runner is cleaned up if the executing task is aborted/panics.
struct RunnerCleanupGuard {
    runner: Box<dyn Runner>,
    completed: bool,
}

impl RunnerCleanupGuard {
    fn new(runner: Box<dyn Runner>) -> Self {
        Self {
            runner,
            completed: false,
        }
    }
}

impl Drop for RunnerCleanupGuard {
    fn drop(&mut self) {
        if !self.completed {
            error!(
                "Runner cleanup guard dropped before normal completion! Spawning detached cleanup."
            );
            self.runner.spawn_detached_cleanup();
        }
    }
}
