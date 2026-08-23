use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::domain::build_plan::{BuildPlan, CommandData};
use crate::domain::project_build_config::{PackageManager, ProjectBuildConfig};
use crate::executor::process_result::ProcessOutcome;
use crate::runner::context::RunnerExecutionContext;

#[derive(Debug, thiserror::Error)]
pub enum BuildPlanGeneratorError {
    #[error("Failed to read package.json: {0}")]
    PackageJsonError(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(serde::Deserialize)]
struct PackageJson {
    #[serde(default)]
    scripts: HashMap<String, String>,
}

pub struct BuildPlanGenerator;

impl BuildPlanGenerator {
    pub async fn generate(
        config: &ProjectBuildConfig,
        context: &RunnerExecutionContext<'_>,
        _workspace_path: &Path,
        source_sha: String,
    ) -> Result<BuildPlan, BuildPlanGeneratorError> {
        let scripts = Self::parse_package_scripts(context).await?;

        let install_command = match config.package_manager {
            PackageManager::Npm => CommandData {
                program: "npm".to_string(),
                args: vec!["ci".to_string()],
            },
            PackageManager::Pnpm => CommandData {
                program: "pnpm".to_string(),
                args: vec!["install".to_string(), "--frozen-lockfile".to_string()],
            },
            PackageManager::Yarn => CommandData {
                program: "yarn".to_string(),
                args: vec!["install".to_string(), "--immutable".to_string()],
            },
            _ => CommandData {
                program: "npm".to_string(),
                args: vec!["ci".to_string()],
            },
        };

        let program = match config.package_manager {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            _ => "npm",
        };

        let lint_command = if scripts.contains_key("lint") {
            Some(CommandData {
                program: program.to_string(),
                args: vec!["run".to_string(), "lint".to_string()],
            })
        } else {
            None
        };

        let typecheck_command = if scripts.contains_key("typecheck") {
            Some(CommandData {
                program: program.to_string(),
                args: vec!["run".to_string(), "typecheck".to_string()],
            })
        } else {
            None
        };

        let test_command = if scripts.contains_key("test") {
            Some(CommandData {
                program: program.to_string(),
                args: vec!["run".to_string(), "test".to_string()],
            })
        } else {
            None
        };

        let build_command = if scripts.contains_key("build") {
            Some(CommandData {
                program: program.to_string(),
                args: vec!["run".to_string(), "build".to_string()],
            })
        } else {
            None
        };

        Ok(BuildPlan {
            source_sha,
            application_type: config.application_type,
            framework: config.framework,
            runtime: config.runtime_version.clone(),
            package_manager: config.package_manager,
            install_command: Some(install_command),
            lint_command,
            typecheck_command,
            test_command,
            build_command,
            build_image_command: None,
            test_image_command: None,
            dockerfile: config.dockerfile_path.clone(),
            docker_context: config.build_context.clone(),
            application_port: config.application_port,
            health_endpoint: config.health_endpoint.clone(),
        })
    }

    async fn parse_package_scripts(
        context: &RunnerExecutionContext<'_>,
    ) -> Result<HashMap<String, String>, BuildPlanGeneratorError> {
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
            .map_err(|e| BuildPlanGeneratorError::ExecutionFailed(e.to_string()))?;

        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Ok(HashMap::new()); // No package.json, return empty scripts
        }

        let package_json: PackageJson = serde_json::from_str(&res.stdout.text)
            .map_err(|e| BuildPlanGeneratorError::PackageJsonError(e.to_string()))?;

        Ok(package_json.scripts)
    }
}
