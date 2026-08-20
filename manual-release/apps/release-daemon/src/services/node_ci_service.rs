use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::job::append_job_event;
use crate::executor::process_result::ProcessOutcome;
use crate::runner::context::RunnerExecutionContext;

#[derive(Debug, thiserror::Error)]
pub enum NodeCiError {
    #[error("No package manager detected (missing lockfile)")]
    NoPackageManager,
    #[error("Multiple lockfiles detected: {0}")]
    MultipleLockfiles(String),
    #[error("Failed to read package.json: {0}")]
    PackageJsonError(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Runner error: {0}")]
    RunnerError(#[from] crate::runner::RunnerError),
}

#[derive(serde::Deserialize)]
struct PackageJson {
    #[serde(default)]
    scripts: HashMap<String, String>,
}

pub struct NodeCiService {
    pool: PgPool,
}

impl NodeCiService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn execute_ci(
        &self,
        job_id: Uuid,
        step_id: Uuid,
        workspace_path: &Path,
        cancel_token: CancellationToken,
        context: &RunnerExecutionContext<'_>,
    ) -> Result<(), NodeCiError> {
        let package_manager = self.detect_package_manager(context, workspace_path).await?;
        let scripts = self.parse_package_scripts(context, workspace_path).await?;

        self.run_install(
            job_id,
            step_id,
            context,
            &package_manager,
            cancel_token.clone(),
        )
        .await?;

        // LINT
        if scripts.contains_key("lint") {
            self.run_script(
                job_id,
                step_id,
                context,
                &package_manager,
                "lint",
                cancel_token.clone(),
            )
            .await?;
        } else {
            self.log_skip(job_id, step_id, "LINT").await;
        }

        // TYPE_CHECK
        if scripts.contains_key("typecheck") {
            self.run_script(
                job_id,
                step_id,
                context,
                &package_manager,
                "typecheck",
                cancel_token.clone(),
            )
            .await?;
        } else {
            self.log_skip(job_id, step_id, "TYPE_CHECK").await;
        }

        // TEST
        if scripts.contains_key("test") {
            self.run_script(
                job_id,
                step_id,
                context,
                &package_manager,
                "test",
                cancel_token.clone(),
            )
            .await?;
        } else {
            self.log_skip(job_id, step_id, "TEST").await;
        }

        // BUILD_APPLICATION
        if scripts.contains_key("build") {
            self.run_script(
                job_id,
                step_id,
                context,
                &package_manager,
                "build",
                cancel_token.clone(),
            )
            .await?;
        } else {
            self.log_skip(job_id, step_id, "BUILD_APPLICATION").await;
        }

        Ok(())
    }

    async fn detect_package_manager(
        &self,
        context: &RunnerExecutionContext<'_>,
        _workspace_path: &Path,
    ) -> Result<String, NodeCiError> {
        let empty_env = HashMap::new();
        let res = context
            .execute(
                "bash",
                &["-c".to_string(), "ls package-lock.json pnpm-lock.yaml yarn.lock 2>/dev/null".to_string()],
                &empty_env,
                Duration::from_secs(5),
                None,
            )
            .await
            .map_err(|e| NodeCiError::ExecutionFailed(e.to_string()))?;

        let output = res.stdout.text;

        let has_npm = output.contains("package-lock.json");
        let has_pnpm = output.contains("pnpm-lock.yaml");
        let has_yarn = output.contains("yarn.lock");

        match (has_npm, has_pnpm, has_yarn) {
            (true, false, false) => Ok("npm".to_string()),
            (false, true, false) => Ok("pnpm".to_string()),
            (false, false, true) => Ok("yarn".to_string()),
            (false, false, false) => Err(NodeCiError::NoPackageManager),
            _ => Err(NodeCiError::MultipleLockfiles(
                "Found multiple lockfiles. Ensure only one package manager is used.".to_string(),
            )),
        }
    }

    async fn parse_package_scripts(
        &self,
        context: &RunnerExecutionContext<'_>,
        _workspace_path: &Path,
    ) -> Result<HashMap<String, String>, NodeCiError> {
        let empty_env = HashMap::new();
        let res = context
            .execute(
                "cat",
                &["package.json".to_string()],
                &empty_env,
                Duration::from_secs(5),
                None,
            )
            .await
            .map_err(|e| NodeCiError::ExecutionFailed(e.to_string()))?;

        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(NodeCiError::PackageJsonError("package.json not found".to_string()));
        }

        let package_json: PackageJson = serde_json::from_str(&res.stdout.text)
            .map_err(|e| NodeCiError::PackageJsonError(e.to_string()))?;

        Ok(package_json.scripts)
    }

    async fn run_install(
        &self,
        job_id: Uuid,
        step_id: Uuid,
        context: &RunnerExecutionContext<'_>,
        package_manager: &str,
        cancel_token: CancellationToken,
    ) -> Result<(), NodeCiError> {
        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            &format!("Running INSTALL_DEPENDENCIES using {}", package_manager),
        )
        .await;

        let args = match package_manager {
            "npm" => vec!["ci".to_string()],
            "pnpm" => vec!["install".to_string(), "--frozen-lockfile".to_string()],
            "yarn" => vec!["install".to_string(), "--frozen-lockfile".to_string()],
            _ => {
                return Err(NodeCiError::ExecutionFailed(
                    "Unknown package manager".to_string(),
                ));
            }
        };

        self.execute_command(
            job_id,
            step_id,
            context,
            package_manager,
            &args,
            cancel_token,
        )
        .await
    }

    async fn run_script(
        &self,
        job_id: Uuid,
        step_id: Uuid,
        context: &RunnerExecutionContext<'_>,
        package_manager: &str,
        script_name: &str,
        cancel_token: CancellationToken,
    ) -> Result<(), NodeCiError> {
        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            &format!("Running script: {}", script_name),
        )
        .await;

        let args = vec!["run".to_string(), script_name.to_string()];
        self.execute_command(
            job_id,
            step_id,
            context,
            package_manager,
            &args,
            cancel_token,
        )
        .await
    }

    async fn log_skip(&self, job_id: Uuid, step_id: Uuid, step_name: &str) {
        let _ = append_job_event(
            &self.pool,
            job_id,
            Some(step_id),
            "SYSTEM",
            "INFO",
            &format!("Skipping {} (no script in package.json)", step_name),
        )
        .await;
    }

    async fn execute_command(
        &self,
        job_id: Uuid,
        step_id: Uuid,
        context: &RunnerExecutionContext<'_>,
        program: &str,
        args: &[String],
        cancel_token: CancellationToken,
    ) -> Result<(), NodeCiError> {
        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);
        let pool = self.pool.clone();

        let log_task = tokio::spawn(async move {
            while let Some((stream, line)) = rx.recv().await {
                let _ =
                    append_job_event(&pool, job_id, Some(step_id), &stream, "INFO", &line).await;
            }
        });

        let empty_env = HashMap::new();

        let result = context
            .execute(
                program,
                args,
                &empty_env,
                Duration::from_secs(300),
                Some(tx),
            )
            .await
            .map_err(|e| NodeCiError::ExecutionFailed(e.to_string()))?;

        // Ensure all logs are processed before proceeding
        let _ = log_task.await;

        match result.outcome {
            ProcessOutcome::Succeeded => {
                if let Some(0) = result.exit_code {
                    Ok(())
                } else {
                    Err(NodeCiError::ExecutionFailed(format!(
                        "Process failed with exit code {:?}",
                        result.exit_code
                    )))
                }
            }
            ProcessOutcome::NonZeroExit => Err(NodeCiError::ExecutionFailed(format!(
                "Process failed with exit code {:?}",
                result.exit_code
            ))),
            ProcessOutcome::TimedOut => Err(NodeCiError::ExecutionFailed(
                "Process timed out".to_string(),
            )),
            ProcessOutcome::Cancelled => Err(NodeCiError::ExecutionFailed(
                "Process was cancelled".to_string(),
            )),
            ProcessOutcome::SpawnFailed => Err(NodeCiError::ExecutionFailed(format!(
                "Spawn failed: {:?}",
                result.spawn_error
            ))),
        }
    }
}
