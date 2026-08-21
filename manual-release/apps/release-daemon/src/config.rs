use anyhow::Context;
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub backend_host: String,
    pub backend_port: u16,
    pub job_workspace_root: String,
    pub runner_type: String,
    pub runner_timeout_seconds: u64,
    pub runner_cleanup_on_success: bool,
    pub runner_cleanup_on_failure: bool,
    pub runner_ubuntu_image: String,
    pub runner_memory_limit: String,
    pub runner_cpus_limit: String,
    pub runner_pids_limit: String,
    pub runner_network_policy: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;

        let backend_host = env::var("BACKEND_HOST").context("BACKEND_HOST is required")?;

        let backend_port = env::var("BACKEND_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .context("BACKEND_PORT must be a valid TCP port")?;

        let job_workspace_root = env::var("JOB_WORKSPACE_ROOT")
            .unwrap_or_else(|_| "/tmp/release-daemon-workspaces".to_string());

        let runner_type = env::var("RUNNER_TYPE").unwrap_or_else(|_| "LOCAL_UBUNTU".to_string());

        let runner_timeout_seconds = env::var("RUNNER_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse::<u64>()
            .context("RUNNER_TIMEOUT_SECONDS must be a valid integer")?;

        let runner_cleanup_on_success = env::var("RUNNER_CLEANUP_ON_SUCCESS")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        let runner_cleanup_on_failure = env::var("RUNNER_CLEANUP_ON_FAILURE")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let runner_ubuntu_image =
            env::var("RUNNER_UBUNTU_IMAGE").unwrap_or_else(|_| "ubuntu:22.04".to_string());

        let runner_memory_limit =
            env::var("RUNNER_MEMORY_LIMIT").unwrap_or_else(|_| "512m".to_string());

        let runner_cpus_limit = env::var("RUNNER_CPUS_LIMIT").unwrap_or_else(|_| "1.0".to_string());

        let runner_pids_limit = env::var("RUNNER_PIDS_LIMIT").unwrap_or_else(|_| "100".to_string());

        let runner_network_policy =
            env::var("RUNNER_NETWORK_POLICY").unwrap_or_else(|_| "none".to_string());

        Ok(Self {
            database_url,
            backend_host,
            backend_port,
            job_workspace_root,
            runner_type,
            runner_timeout_seconds,
            runner_cleanup_on_success,
            runner_cleanup_on_failure,
            runner_ubuntu_image,
            runner_memory_limit,
            runner_cpus_limit,
            runner_pids_limit,
            runner_network_policy,
        })
    }
}
