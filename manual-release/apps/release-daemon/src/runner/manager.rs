use std::path::PathBuf;

use super::local_ubuntu::LocalUbuntuRunner;
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

    pub fn create_runner(&self, workspace_path: PathBuf) -> Result<Box<dyn Runner>, RunnerError> {
        match self.config.runner_type.as_str() {
            "LOCAL_UBUNTU" => Ok(Box::new(LocalUbuntuRunner::new(workspace_path))),
            "MOCK" => Ok(Box::new(MockRunner::new(workspace_path))),
            other => Err(RunnerError::Configuration(format!(
                "Unsupported runner type: {}",
                other
            ))),
        }
    }
}
