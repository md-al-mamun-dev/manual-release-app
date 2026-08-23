use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::domain::build_plan::{BuildPlan, CommandData};
use crate::domain::job::{append_job_event, create_step, fail_step, start_step, succeed_step};
use crate::executor::process_result::ProcessOutcome;
use crate::runner::context::RunnerExecutionContext;

#[derive(Debug, thiserror::Error)]
pub enum BuildPlanExecutorError {
    #[error("Step '{0}' failed: {1}")]
    StepFailed(String, String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

pub struct BuildPlanExecutor {
    pool: PgPool,
}

impl BuildPlanExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn execute_ci(
        &self,
        build_plan: &BuildPlan,
        job_id: Uuid,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<(), BuildPlanExecutorError> {
        let mut step_order = 1;

        // INSTALL
        if let Some(cmd) = &build_plan.install_command {
            let install_step =
                create_step(&self.pool, job_id, "INSTALL_DEPENDENCIES", step_order).await?;
            step_order += 1;
            self.execute_command_step(install_step, job_id, "INSTALL_DEPENDENCIES", cmd, context)
                .await?;
        }

        // LINT
        if let Some(cmd) = &build_plan.lint_command {
            let step_id = create_step(&self.pool, job_id, "LINT", step_order).await?;
            step_order += 1;
            self.execute_command_step(step_id, job_id, "LINT", cmd, context)
                .await?;
        }

        // TYPECHECK
        if let Some(cmd) = &build_plan.typecheck_command {
            let step_id = create_step(&self.pool, job_id, "TYPECHECK", step_order).await?;
            step_order += 1;
            self.execute_command_step(step_id, job_id, "TYPECHECK", cmd, context)
                .await?;
        }

        // TEST
        if let Some(cmd) = &build_plan.test_command {
            let step_id = create_step(&self.pool, job_id, "TEST", step_order).await?;
            step_order += 1;
            self.execute_command_step(step_id, job_id, "TEST", cmd, context)
                .await?;
        }

        // BUILD
        if let Some(cmd) = &build_plan.build_command {
            let step_id = create_step(&self.pool, job_id, "BUILD_APPLICATION", step_order).await?;
            self.execute_command_step(step_id, job_id, "BUILD_APPLICATION", cmd, context)
                .await?;
        }

        Ok(())
    }

    pub async fn execute_image_build(
        &self,
        build_plan: &BuildPlan,
        release_id: Uuid,
        job_id: Uuid,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<bool, BuildPlanExecutorError> {
        if let Some(dockerfile) = &build_plan.dockerfile {
            let context_dir = build_plan.docker_context.as_deref().unwrap_or(".");
            let tag = format!("target-img:{}", build_plan.source_sha);

            // Assume step order 6 for image build
            let step_id = create_step(&self.pool, job_id, "BUILD_IMAGE", 6).await?;
            self.execute_build_image_step(
                step_id,
                release_id,
                job_id,
                build_plan.source_sha.clone(),
                dockerfile,
                context_dir,
                &tag,
                context,
            )
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn execute_image_test(
        &self,
        build_plan: &BuildPlan,
        job_id: Uuid,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<bool, BuildPlanExecutorError> {
        if build_plan.dockerfile.is_some() {
            let tag = format!("target-img:{}", build_plan.source_sha);
            if let (Some(port), Some(endpoint)) =
                (build_plan.application_port, &build_plan.health_endpoint)
            {
                // Assume step order 7 for image test
                let test_step_id = create_step(&self.pool, job_id, "TEST_IMAGE", 7).await?;
                self.execute_test_image_step(test_step_id, job_id, &tag, port, endpoint, context)
                    .await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn execute_image_scan(
        &self,
        build_plan: &BuildPlan,
        release_id: Uuid,
        job_id: Uuid,
        policy: &crate::domain::security_policy::SecurityPolicy,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<bool, BuildPlanExecutorError> {
        if build_plan.dockerfile.is_some() {
            // Assume step order 8 for image scan
            let scan_step_id = create_step(&self.pool, job_id, "SCAN_IMAGE", 8).await?;
            let passed = self
                .execute_scan_image_step(scan_step_id, release_id, job_id, policy, context)
                .await?;
            Ok(passed)
        } else {
            Ok(true)
        }
    }

    async fn execute_command_step(
        &self,
        step_id: Uuid,
        job_id: Uuid,
        step_name: &str,
        command: &CommandData,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<(), BuildPlanExecutorError> {
        start_step(&self.pool, step_id).await?;

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            &format!("Running step {}", step_name),
        )
        .await;

        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);
        let pool = self.pool.clone();

        let log_task = tokio::spawn(async move {
            while let Some((stream, line)) = rx.recv().await {
                let _ =
                    append_job_event(&pool, job_id, Some(step_id), &stream, "INFO", &line).await;
            }
        });

        let empty_env = HashMap::new();

        let result = match context
            .execute(
                &command.program,
                &command.args,
                &empty_env,
                Duration::from_secs(300), // Default 5 min timeout
                Some(tx),
            )
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let error_msg = format!("Execution failed: {}", e);
                fail_step(&self.pool, step_id, "EXECUTION_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    step_name.to_string(),
                    error_msg,
                ));
            }
        };

        // Ensure logs are processed
        let _ = log_task.await;

        self.handle_process_outcome(step_id, step_name, result)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_build_image_step(
        &self,
        step_id: Uuid,
        release_id: Uuid,
        job_id: Uuid,
        git_sha: String,
        dockerfile: &str,
        context_dir: &str,
        tag: &str,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<(), BuildPlanExecutorError> {
        start_step(&self.pool, step_id).await?;

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            "Running step BUILD_IMAGE",
        )
        .await;

        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);
        let pool = self.pool.clone();

        let log_task = tokio::spawn(async move {
            while let Some((stream, line)) = rx.recv().await {
                let _ =
                    append_job_event(&pool, job_id, Some(step_id), &stream, "INFO", &line).await;
            }
        });

        let (result, digest_opt) = match context
            .runner()
            .build_image(
                dockerfile,
                context_dir,
                tag,
                context.cancel_token(),
                Some(tx),
            )
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let error_msg = format!("Execution failed: {}", e);
                fail_step(&self.pool, step_id, "EXECUTION_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "BUILD_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        let _ = log_task.await;

        if let Some(digest) =
            digest_opt.filter(|_| matches!(result.outcome, ProcessOutcome::Succeeded))
        {
            let release_image = crate::domain::release_image::ReleaseImage {
                id: Uuid::new_v4(),
                release_id,
                job_id,
                git_sha,
                image_tag: tag.to_string(),
                image_digest: digest,
                created_at: chrono::Utc::now(),
            };
            let repo = crate::repositories::release_image_repository::ReleaseImageRepository::new(
                self.pool.clone(),
            );
            if let Err(e) = repo.create_release_image(&release_image).await {
                let error_msg = format!("Failed to save image metadata: {}", e);
                fail_step(&self.pool, step_id, "DATABASE_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "BUILD_IMAGE".to_string(),
                    error_msg,
                ));
            }
        }

        self.handle_process_outcome(step_id, "BUILD_IMAGE", result)
            .await
    }

    async fn execute_test_image_step(
        &self,
        step_id: Uuid,
        job_id: Uuid,
        tag: &str,
        port: i32,
        health_endpoint: &str,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<(), BuildPlanExecutorError> {
        start_step(&self.pool, step_id).await?;

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            "Running step TEST_IMAGE",
        )
        .await;

        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);
        let pool = self.pool.clone();

        let log_task = tokio::spawn(async move {
            while let Some((stream, line)) = rx.recv().await {
                let _ =
                    append_job_event(&pool, job_id, Some(step_id), &stream, "INFO", &line).await;
            }
        });

        let result = match context
            .runner()
            .test_image(tag, port, health_endpoint, context.cancel_token(), Some(tx))
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let error_msg = format!("Execution failed: {}", e);
                fail_step(&self.pool, step_id, "EXECUTION_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "TEST_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        let _ = log_task.await;

        self.handle_process_outcome(step_id, "TEST_IMAGE", result)
            .await
    }

    async fn execute_scan_image_step(
        &self,
        step_id: Uuid,
        release_id: Uuid,
        job_id: Uuid,
        policy: &crate::domain::security_policy::SecurityPolicy,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<bool, BuildPlanExecutorError> {
        start_step(&self.pool, step_id).await?;

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            "Running step SCAN_IMAGE with Trivy",
        )
        .await;

        // Fetch the release image record to get the exact digest
        let image_repo = crate::repositories::release_image_repository::ReleaseImageRepository::new(
            self.pool.clone(),
        );
        let release_image = match image_repo.get_by_release_id(release_id).await? {
            Some(img) => img,
            None => {
                let error_msg = format!("No release image found for release {}", release_id);
                fail_step(&self.pool, step_id, "IMAGE_NOT_FOUND", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "SCAN_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);
        let pool = self.pool.clone();

        let log_task = tokio::spawn(async move {
            while let Some((stream, line)) = rx.recv().await {
                let _ =
                    append_job_event(&pool, job_id, Some(step_id), &stream, "INFO", &line).await;
            }
        });

        let tar_input = "/workspace/image.tar";
        let report_output = "/workspace/trivy-report.json";

        let result = match context
            .runner()
            .scan_image(tar_input, report_output, context.cancel_token(), Some(tx))
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let error_msg = format!("Vulnerability scan execution failed: {}", e);
                fail_step(&self.pool, step_id, "EXECUTION_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "SCAN_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        let _ = log_task.await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            self.handle_process_outcome(step_id, "SCAN_IMAGE", result)
                .await?;
            return Ok(false);
        }

        // Read report from workspace
        let report_path = context.runner().workspace().await.join("trivy-report.json");
        
        // Enforce 10MB report size limit
        if let Ok(metadata) = tokio::fs::metadata(&report_path).await {
            if metadata.len() > 10 * 1024 * 1024 {
                let error_msg = format!("Trivy report file is too large ({} bytes). Maximum allowed is 10MB.", metadata.len());
                fail_step(&self.pool, step_id, "REPORT_TOO_LARGE", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed("SCAN_IMAGE".to_string(), error_msg));
            }
        }

        let report_content = match tokio::fs::read_to_string(&report_path).await {
            Ok(c) => c,
            Err(e) => {
                let error_msg = format!(
                    "Failed to read Trivy report file at {}: {}",
                    report_path.display(),
                    e
                );
                fail_step(&self.pool, step_id, "REPORT_READ_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "SCAN_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        // Parse Trivy report
        let report = match crate::domain::trivy_parser::TrivyParser::parse_json_str(&report_content)
        {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Failed to parse Trivy report JSON: {}", e);
                fail_step(&self.pool, step_id, "REPORT_PARSE_ERROR", &error_msg).await?;
                return Err(BuildPlanExecutorError::StepFailed(
                    "SCAN_IMAGE".to_string(),
                    error_msg,
                ));
            }
        };

        let summary = crate::domain::trivy_parser::TrivyParser::summarize(&report);
        let decision = policy.evaluate(&summary);

        let report_json_val: serde_json::Value = match serde_json::from_str(&report_content) {
            Ok(v) => v,
            Err(_) => serde_json::json!({}),
        };

        // Persist SecurityScan
        let scan_record = crate::domain::security_scan::SecurityScan {
            id: Uuid::new_v4(),
            release_id,
            image_digest: release_image.image_digest.clone(),
            critical_vulnerabilities: summary.critical,
            high_vulnerabilities: summary.high,
            medium_vulnerabilities: summary.medium,
            low_vulnerabilities: summary.low,
            passed: decision.passed,
            report_json: report_json_val,
            created_at: chrono::Utc::now(),
        };

        let scan_repo = crate::repositories::security_scan_repository::SecurityScanRepository::new(
            self.pool.clone(),
        );

        if let Err(e) = scan_repo.create(&scan_record).await {
            let error_msg = format!("Failed to save security scan record: {}", e);
            fail_step(&self.pool, step_id, "DATABASE_ERROR", &error_msg).await?;
            return Err(BuildPlanExecutorError::StepFailed(
                "SCAN_IMAGE".to_string(),
                error_msg,
            ));
        }

        let scan_summary_msg = format!(
            "Vulnerability Scan Results: CRITICAL={}, HIGH={}, MEDIUM={}, LOW={}, UNKNOWN={}. Policy Decision: {}",
            summary.critical,
            summary.high,
            summary.medium,
            summary.low,
            summary.unknown,
            if decision.passed { "PASSED" } else { "FAILED" }
        );

        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            if decision.passed { "INFO" } else { "WARN" },
            &scan_summary_msg,
        )
        .await;

        if decision.passed {
            succeed_step(&self.pool, step_id).await?;
            Ok(true)
        } else {
            let reasons_str = decision.reasons.join("; ");
            let fail_msg = format!("Security policy rejected image: {}", reasons_str);
            fail_step(&self.pool, step_id, "SECURITY_POLICY_VIOLATION", &fail_msg).await?;
            Ok(false)
        }
    }

    async fn handle_process_outcome(
        &self,
        step_id: Uuid,
        step_name: &str,
        result: crate::executor::process_result::ProcessResult,
    ) -> Result<(), BuildPlanExecutorError> {
        match result.outcome {
            ProcessOutcome::Succeeded => {
                if let Some(0) = result.exit_code {
                    succeed_step(&self.pool, step_id).await?;
                    Ok(())
                } else {
                    let error_msg = format!("Process failed with exit code {:?}", result.exit_code);
                    fail_step(&self.pool, step_id, "NON_ZERO_EXIT", &error_msg).await?;
                    Err(BuildPlanExecutorError::StepFailed(
                        step_name.to_string(),
                        error_msg,
                    ))
                }
            }
            ProcessOutcome::NonZeroExit => {
                let error_msg = format!("Process failed with exit code {:?}", result.exit_code);
                fail_step(&self.pool, step_id, "NON_ZERO_EXIT", &error_msg).await?;
                Err(BuildPlanExecutorError::StepFailed(
                    step_name.to_string(),
                    error_msg,
                ))
            }
            ProcessOutcome::TimedOut => {
                fail_step(&self.pool, step_id, "TIMED_OUT", "Process timed out").await?;
                Err(BuildPlanExecutorError::StepFailed(
                    step_name.to_string(),
                    "Process timed out".to_string(),
                ))
            }
            ProcessOutcome::Cancelled => {
                fail_step(&self.pool, step_id, "CANCELLED", "Process was cancelled").await?;
                Err(BuildPlanExecutorError::StepFailed(
                    step_name.to_string(),
                    "Process was cancelled".to_string(),
                ))
            }
            ProcessOutcome::SpawnFailed => {
                let error_msg = format!("Spawn failed: {:?}", result.spawn_error);
                fail_step(&self.pool, step_id, "SPAWN_FAILED", &error_msg).await?;
                Err(BuildPlanExecutorError::StepFailed(
                    step_name.to_string(),
                    error_msg,
                ))
            }
        }
    }
}
