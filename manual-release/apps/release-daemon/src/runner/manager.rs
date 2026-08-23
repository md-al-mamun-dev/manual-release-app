use std::path::PathBuf;
use uuid::Uuid;

use super::docker_ubuntu_runner::LocalDockerUbuntuRunner;
use super::mock_runner::MockRunner;
use super::{Runner, RunnerError};
use crate::config::AppConfig;

pub struct RunnerManager {
    config: AppConfig,
}

impl RunnerManager {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn create_runner(
        &self,
        workspace_path: PathBuf,
        release_id: Uuid,
        job_id: Uuid,
    ) -> Result<Box<dyn Runner>, RunnerError> {
        match self.config.runner_type.as_str() {
            "LOCAL_UBUNTU" => Ok(Box::new(LocalDockerUbuntuRunner::new(
                workspace_path,
                self.config.clone(),
                release_id,
                job_id,
            ))),
            "MOCK" => Ok(Box::new(MockRunner::new(
                workspace_path,
                release_id,
                job_id,
            ))),
            "MOCK_FAIL_CREATE" => Ok(Box::new(
                MockRunner::new(workspace_path, release_id, job_id).with_fail_create(),
            )),
            "MOCK_FAIL_COMMAND" => Ok(Box::new(
                MockRunner::new(workspace_path, release_id, job_id).with_fail_command(),
            )),
            "MOCK_FAIL_PREPARE" => Ok(Box::new(
                MockRunner::new(workspace_path, release_id, job_id).with_fail_prepare(),
            )),
            "MOCK_FAIL_CLEANUP" => Ok(Box::new(
                MockRunner::new(workspace_path, release_id, job_id).with_fail_cleanup(),
            )),
            "MOCK_FAIL_PREPARE_AND_CLEANUP" => Ok(Box::new(
                MockRunner::new(workspace_path, release_id, job_id)
                    .with_fail_prepare()
                    .with_fail_cleanup(),
            )),
            other => Err(RunnerError::Configuration(format!(
                "Unsupported runner type: {}",
                other
            ))),
        }
    }
}
