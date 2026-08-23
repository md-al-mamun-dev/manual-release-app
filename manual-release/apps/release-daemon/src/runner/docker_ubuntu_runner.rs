use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use super::{Runner, RunnerError, RunnerState};
use crate::config::AppConfig;
use crate::executor::process_executor::ProcessExecutor;
use crate::executor::process_result::{ProcessOutcome, ProcessResult};

pub struct LocalDockerUbuntuRunner {
    workspace_path: PathBuf,
    state: RunnerState,
    executor: ProcessExecutor,
    container_name: String,
    volume_name: String,
    config: AppConfig,
}

impl LocalDockerUbuntuRunner {
    pub fn new(workspace_path: PathBuf, config: AppConfig) -> Self {
        let runner_uuid = Uuid::new_v4();
        let container_name = format!("cicd-runner-{}", runner_uuid);
        let volume_name = format!("workspace-{}", runner_uuid);
        Self {
            workspace_path,
            state: RunnerState::Creating,
            executor: ProcessExecutor::new(10 * 1024 * 1024, Duration::from_secs(5)),
            container_name,
            volume_name,
            config,
        }
    }

    async fn exec_command(
        &self,
        command: &[&str],
        timeout: Duration,
        cancel_token: CancellationToken,
    ) -> Result<ProcessResult, RunnerError> {
        let empty_env = HashMap::new();

        let mut args = vec!["exec".to_string(), self.container_name.clone()];
        args.extend(command.iter().map(|s| s.to_string()));

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path, // local CWD for the executor
                &empty_env,
                timeout,
                cancel_token,
                None,
            )
            .await;

        Ok(result)
    }

    async fn verify_dependency(
        &self,
        command: &[&str],
        expected: &str,
        component: &str,
    ) -> Result<(), RunnerError> {
        let cancel_token = CancellationToken::new();
        let result = self
            .exec_command(command, Duration::from_secs(10), cancel_token)
            .await?;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "{} validation failed. Error: {}",
                component, result.stderr.text
            )));
        }

        if !result
            .stdout
            .text
            .to_lowercase()
            .contains(&expected.to_lowercase())
        {
            return Err(RunnerError::PreparationFailed(format!(
                "{} validation failed. Expected '{}' in output, got: {}",
                component, expected, result.stdout.text
            )));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Runner for LocalDockerUbuntuRunner {
    async fn create(&mut self) -> Result<(), RunnerError> {
        let empty_env = HashMap::new();
        let cancel_token = CancellationToken::new();

        info!("Creating Docker volume: {}", self.volume_name);

        let volume_args = vec![
            "volume".to_string(),
            "create".to_string(),
            "--driver".to_string(),
            "local".to_string(),
            "--opt".to_string(),
            "type=tmpfs".to_string(),
            "--opt".to_string(),
            "device=tmpfs".to_string(),
            "--opt".to_string(),
            "o=size=2G".to_string(),
            self.volume_name.clone(),
        ];
        
        let vol_result = self.executor.execute("docker", &volume_args, &self.workspace_path, &empty_env, Duration::from_secs(10), cancel_token.clone(), None).await;
        if !matches!(vol_result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::CreationFailed(format!("Failed to create workspace volume: {}", vol_result.stderr.text)));
        }

        info!("Creating Docker container: {}", self.container_name);

        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            self.container_name.clone(),
            // Filesystem Isolation using tmpfs volume
            "-v".to_string(),
            format!("{}:/workspace", self.volume_name),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
            "--tmpfs".to_string(),
            "/run".to_string(),
            "-w".to_string(),
            "/workspace".to_string(),
            // Security profile
            "--security-opt=no-new-privileges".to_string(),
            // Limits
            format!("--memory={}", self.config.runner_memory_limit),
            format!("--cpus={}", self.config.runner_cpus_limit),
            format!("--pids-limit={}", self.config.runner_pids_limit),
            // Network policy
            format!("--network={}", self.config.runner_network_policy),
        ];

        args.push(self.config.runner_ubuntu_image.clone());
        args.push("tail".to_string());
        args.push("-f".to_string());
        args.push("/dev/null".to_string());

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(300),
                cancel_token,
                None,
            )
            .await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::CreationFailed(format!(
                "Failed to start docker container: {}",
                result.stderr.text
            )));
        }

        self.state = RunnerState::Ready;
        Ok(())
    }

    async fn prepare(&mut self) -> Result<(), RunnerError> {
        info!("Preparing Docker container: {}", self.container_name);

        let cancel_token = CancellationToken::new();

        // 0. Copy source code into the Docker volume
        let cp_args = vec![
            "cp".to_string(),
            "-a".to_string(),
            format!("{}/.", self.workspace_path.display()),
            format!("{}:/workspace/", self.container_name),
        ];
        let cp_result = self.executor.execute("docker", &cp_args, &self.workspace_path, &HashMap::new(), Duration::from_secs(120), cancel_token.clone(), None).await;
        if !matches!(cp_result.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!("Failed to copy workspace to container: {}", cp_result.stderr.text)));
        }

        // 1. apt-get update
        let res = self
            .exec_command(
                &["apt-get", "update"],
                Duration::from_secs(60),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get update failed: {}",
                res.stderr.text
            )));
        }

        // 2. Install basic tools
        let res = self
            .exec_command(
                &[
                    "apt-get",
                    "install",
                    "-y",
                    "curl",
                    "git",
                    "build-essential",
                    "ca-certificates",
                ],
                Duration::from_secs(300),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get install tools failed: {}",
                res.stderr.text
            )));
        }

        // 3. Download nodesource setup script
        let res = self
            .exec_command(
                &[
                    "curl",
                    "-fsSL",
                    "-o",
                    "/tmp/setup_node.sh",
                    "https://deb.nodesource.com/setup_20.x",
                ],
                Duration::from_secs(30),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "curl nodesource failed: {}",
                res.stderr.text
            )));
        }

        // 4. Run nodesource setup script (using bash since it's a downloaded script from NodeSource, but we avoid bash -c string interpolation)
        let res = self
            .exec_command(
                &["bash", "/tmp/setup_node.sh"],
                Duration::from_secs(60),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "nodesource script failed: {}",
                res.stderr.text
            )));
        }

        // 5. Install Node.js
        let res = self
            .exec_command(
                &["apt-get", "install", "-y", "nodejs"],
                Duration::from_secs(300),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "apt-get install nodejs failed: {}",
                res.stderr.text
            )));
        }

        // 6. Download and Install Pinned Trivy Version Securely
        let res = self.exec_command(
            &["curl", "-sL", "https://github.com/aquasecurity/trivy/releases/download/v0.53.0/trivy_0.53.0_Linux-64bit.tar.gz", "-o", "/tmp/trivy.tar.gz"],
            Duration::from_secs(300),
            cancel_token.clone(),
        ).await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!("curl trivy failed: {}", res.stderr.text)));
        }

        let res = self.exec_command(&["sha256sum", "/tmp/trivy.tar.gz"], Duration::from_secs(30), cancel_token.clone()).await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) || !res.stdout.text.starts_with("0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5") {
             return Err(RunnerError::PreparationFailed(format!("Trivy SHA256 mismatch or error. Output: {}", res.stdout.text)));
        }

        let res = self.exec_command(&["tar", "-xzf", "/tmp/trivy.tar.gz", "-C", "/usr/local/bin", "trivy"], Duration::from_secs(30), cancel_token.clone()).await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!("Extract trivy failed: {}", res.stderr.text)));
        }

        // 12. Create non-root user 'ci_user'
        let res = self
            .exec_command(
                &["useradd", "-m", "-s", "/bin/bash", "-u", "1000", "ci_user"],
                Duration::from_secs(10),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded)
            && !res.stderr.text.contains("already exists")
        {
            return Err(RunnerError::PreparationFailed(format!(
                "useradd failed: {}",
                res.stderr.text
            )));
        }

        // 13. Change ownership of /workspace
        let res = self
            .exec_command(
                &["chown", "-R", "ci_user:ci_user", "/workspace"],
                Duration::from_secs(10),
                cancel_token.clone(),
            )
            .await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!(
                "chown failed: {}",
                res.stderr.text
            )));
        }

        // Verify Environment
        self.verify_dependency(&["uname", "-s"], "Linux", "OS")
            .await?;
        self.verify_dependency(&["cat", "/etc/os-release"], "ubuntu", "Distribution")
            .await?;
        self.verify_dependency(&["git", "--version"], "git", "Git")
            .await?;
        self.verify_dependency(&["node", "-v"], "v20.", "Node.js")
            .await?;
        self.verify_dependency(&["npm", "-v"], "", "npm").await?; // just ensure it doesn't fail
        self.verify_dependency(&["trivy", "--version"], "Version: 0.53.0", "Trivy")
            .await?;

        // 14. Download Trivy Vulnerability DB to isolated root-owned directory
        let res = self.exec_command(&["mkdir", "-p", "/var/lib/trivy"], Duration::from_secs(10), cancel_token.clone()).await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!("mkdir /var/lib/trivy failed: {}", res.stderr.text)));
        }

        let res = self.exec_command(&["trivy", "image", "--download-db-only", "--cache-dir", "/var/lib/trivy"], Duration::from_secs(300), cancel_token.clone()).await?;
        if !matches!(res.outcome, ProcessOutcome::Succeeded) {
            return Err(RunnerError::PreparationFailed(format!("Failed to download Trivy DB: {}", res.stderr.text)));
        }

        self.state = RunnerState::Running;
        Ok(())
    }

    async fn workspace(&self) -> PathBuf {
        self.workspace_path.clone()
    }

    async fn execute(
        &self,
        program: &str,
        args: &[String],
        envs: &HashMap<String, String>,
        timeout: Duration,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        let mut docker_args = vec![
            "exec".to_string(),
            "-u".to_string(),
            "ci_user".to_string(),
            "-w".to_string(),
            "/workspace".to_string(),
        ];

        for (k, v) in envs {
            docker_args.push("-e".to_string());
            docker_args.push(format!("{}={}", k, v));
        }

        docker_args.push(self.container_name.clone());
        docker_args.push(program.to_string());
        docker_args.extend_from_slice(args);

        let result = self
            .executor
            .execute(
                "docker",
                &docker_args,
                &self.workspace_path,
                &HashMap::new(), // Envs passed explicitly via docker arguments
                timeout,
                cancel_token,
                output_sender,
            )
            .await;

        // Process Cancellation Cleanup
        if result.outcome == ProcessOutcome::Cancelled || result.outcome == ProcessOutcome::TimedOut {
            let pkill_args = vec![
                "exec".to_string(),
                self.container_name.clone(),
                "pkill".to_string(),
                "-9".to_string(),
                "-u".to_string(),
                "ci_user".to_string(),
            ];
            let _ = self.executor.execute("docker", &pkill_args, &self.workspace_path, &HashMap::new(), Duration::from_secs(5), CancellationToken::new(), None).await;
        }

        Ok(result)
    }

    async fn build_image(
        &self,
        dockerfile: &str,
        context: &str,
        tag: &str,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<(ProcessResult, Option<String>), RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        use tokio::io::AsyncWriteExt;

        let dockerignore_path = self.workspace_path.join(".dockerignore");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&dockerignore_path)
            .await
            .map_err(|e| {
                RunnerError::PreparationFailed(format!("Failed to open .dockerignore: {}", e))
            })?;

        file.write_all(b"\n# CI/CD Secret Protection\n.env*\n*.pem\n*.key\n*id_rsa*\n*id_ed25519*\n.aws/\n.ssh/\n")
            .await
            .map_err(|e| RunnerError::PreparationFailed(format!("Failed to write to .dockerignore: {}", e)))?;

        let kaniko_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "-v".to_string(),
            format!("{}:/workspace", self.volume_name),
            "gcr.io/kaniko-project/executor:latest".to_string(),
            "--dockerfile".to_string(),
            format!("/workspace/{}", dockerfile),
            "--context".to_string(),
            format!("dir:///workspace/{}", context),
            "--destination".to_string(),
            tag.to_string(),
            "--no-push".to_string(),
            "--tar-path".to_string(),
            "/workspace/image.tar".to_string(),
            "--digest-file".to_string(),
            "/workspace/image.digest".to_string(),
        ];

        let empty_env = HashMap::new();

        let result = self
            .executor
            .execute(
                "docker",
                &kaniko_args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(900), // 15 mins for build
                cancel_token.clone(),
                output_sender.clone(),
            )
            .await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            return Ok((result, None));
        }

        // 2. Extract digest file from the container's volume (since it was written inside the tmpfs volume)
        let cp_digest_args = vec![
            "cp".to_string(),
            format!("{}:/workspace/image.digest", self.container_name),
            format!("{}/image.digest", self.workspace_path.display()),
        ];
        let _ = self.executor.execute("docker", &cp_digest_args, &self.workspace_path, &empty_env, Duration::from_secs(30), cancel_token.clone(), None).await;

        // Read digest
        let digest_path = self.workspace_path.join("image.digest");
        let digest = match tokio::fs::read_to_string(&digest_path).await {
            Ok(d) => d.trim().to_string(),
            Err(e) => {
                warn!("Failed to read image digest: {}", e);
                return Ok((result, None));
            }
        };

        // Extract image.tar from the volume to the host so it can be loaded
        let cp_tar_args = vec![
            "cp".to_string(),
            format!("{}:/workspace/image.tar", self.container_name),
            format!("{}/image.tar", self.workspace_path.display()),
        ];
        let cp_tar_res = self.executor.execute("docker", &cp_tar_args, &self.workspace_path, &empty_env, Duration::from_secs(120), cancel_token.clone(), None).await;
        if !matches!(cp_tar_res.outcome, ProcessOutcome::Succeeded) {
            warn!("Failed to extract image.tar from volume: {}", cp_tar_res.stderr.text);
            return Ok((result, None));
        }

        // Now load the image into the local docker daemon
        let load_args = vec![
            "load".to_string(),
            "-i".to_string(),
            format!("{}/image.tar", self.workspace_path.display()),
        ];

        let load_result = self
            .executor
            .execute(
                "docker",
                &load_args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(60),
                cancel_token,
                output_sender,
            )
            .await;

        if !matches!(load_result.outcome, ProcessOutcome::Succeeded) {
            return Ok((load_result, Some(digest)));
        }

        Ok((result, Some(digest)))
    }

    async fn test_image(
        &self,
        tag: &str,
        port: i32,
        health_endpoint: &str,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        // Verify the image specifies a non-root user
        let check_user_args = vec![
            "image".to_string(),
            "inspect".to_string(),
            tag.to_string(),
            "-f".to_string(),
            "{{.Config.User}}".to_string(),
        ];

        let check_user_result = self
            .executor
            .execute(
                "docker",
                &check_user_args,
                &self.workspace_path,
                &HashMap::new(),
                Duration::from_secs(10),
                cancel_token.clone(),
                None,
            )
            .await;

        if !matches!(check_user_result.outcome, ProcessOutcome::Succeeded) {
            return Ok(check_user_result);
        }

        let user = check_user_result.stdout.text.trim();
        if user.is_empty() || user == "0" || user == "root" {
            let mut failed_result = check_user_result.clone();
            failed_result.outcome = ProcessOutcome::NonZeroExit;
            failed_result.exit_code = Some(1);
            failed_result.stderr.text = format!(
                "Security Policy Violation: Target image '{}' defaults to root user (USER '{}'). \
                Images must specify a non-root user.",
                tag, user
            );
            return Ok(failed_result);
        }

        let test_container_name = format!("test-img-{}", Uuid::new_v4());

        let run_args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            test_container_name.clone(),
            "--security-opt=no-new-privileges".to_string(),
            format!("--memory={}", self.config.runner_memory_limit),
            format!("--cpus={}", self.config.runner_cpus_limit),
            format!("--pids-limit={}", self.config.runner_pids_limit),
            format!("--network={}", self.config.runner_network_policy),
            tag.to_string(),
        ];

        let empty_env = HashMap::new();

        let start_result = self
            .executor
            .execute(
                "docker",
                &run_args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(30),
                cancel_token.clone(),
                output_sender.clone(),
            )
            .await;

        if !matches!(start_result.outcome, ProcessOutcome::Succeeded) {
            return Ok(start_result);
        }

        // Get container IP address to test
        let inspect_args = vec![
            "inspect".to_string(),
            "-f".to_string(),
            "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}".to_string(),
            test_container_name.clone(),
        ];

        let inspect_result = self
            .executor
            .execute(
                "docker",
                &inspect_args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(10),
                cancel_token.clone(),
                None, // We don't want to log the IP to the user
            )
            .await;

        if !matches!(inspect_result.outcome, ProcessOutcome::Succeeded) {
            let _ = self
                .executor
                .execute(
                    "docker",
                    &[
                        "rm".to_string(),
                        "-f".to_string(),
                        test_container_name.clone(),
                    ],
                    &self.workspace_path,
                    &empty_env,
                    Duration::from_secs(10),
                    CancellationToken::new(),
                    None,
                )
                .await;
            return Ok(inspect_result);
        }

        let ip = inspect_result.stdout.text.trim();
        let health_url = format!("http://{}:{}{}", ip, port, health_endpoint);

        // Wait up to 30 seconds for health check to pass
        let mut health_passed = false;
        let mut smoke_result = None;

        for _ in 0..15 {
            if cancel_token.is_cancelled() {
                break;
            }

            let curl_args = vec![
                "run".to_string(),
                "--rm".to_string(),
                format!("--network={}", self.config.runner_network_policy),
                "curlimages/curl".to_string(),
                "-s".to_string(),
                "-f".to_string(),
                "--connect-timeout".to_string(),
                "2".to_string(),
                health_url.clone(),
            ];

            let curl_res = self
                .executor
                .execute(
                    "docker",
                    &curl_args,
                    &self.workspace_path,
                    &empty_env,
                    Duration::from_secs(60), // Increased to 60s to allow docker pull curlimages/curl to complete
                    cancel_token.clone(),
                    output_sender.clone(),
                )
                .await;

            if matches!(curl_res.outcome, ProcessOutcome::Succeeded) {
                health_passed = true;
                smoke_result = Some(curl_res);
                break;
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
            smoke_result = Some(curl_res);
        }

        // Cleanup
        let _ = self
            .executor
            .execute(
                "docker",
                &[
                    "rm".to_string(),
                    "-f".to_string(),
                    test_container_name.clone(),
                ],
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(10),
                CancellationToken::new(), // Do not cancel cleanup
                None,
            )
            .await;

        if health_passed {
            Ok(smoke_result.unwrap())
        } else {
            Ok(smoke_result.unwrap_or(start_result)) // return last error
        }
    }

    async fn scan_image(
        &self,
        tar_path: &str,
        report_output_path: &str,
        cancel_token: CancellationToken,
        output_sender: Option<mpsc::Sender<(String, String)>>,
    ) -> Result<ProcessResult, RunnerError> {
        if self.state != RunnerState::Running {
            return Err(RunnerError::ExecutionFailed(
                "Runner is not in Running state".into(),
            ));
        }

        let trivy_args = vec![
            "image".to_string(),
            "--input".to_string(),
            tar_path.to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--output".to_string(),
            report_output_path.to_string(),
            "--quiet".to_string(),
            "--skip-db-update".to_string(),
            "--offline-scan".to_string(),
            "--cache-dir".to_string(),
            "/var/lib/trivy".to_string(),
        ];

        let empty_env = HashMap::new();

        let res = self.execute(
            "trivy",
            &trivy_args,
            &empty_env,
            Duration::from_secs(300), // 5 minutes timeout for vulnerability scan
            cancel_token.clone(),
            output_sender,
        )
        .await;

        if let Ok(process_res) = &res {
            if matches!(process_res.outcome, ProcessOutcome::Succeeded) {
                // Extract report from volume to host
                let cp_report_args = vec![
                    "cp".to_string(),
                    format!("{}:{}", self.container_name, report_output_path),
                    format!("{}/trivy-report.json", self.workspace_path.display()),
                ];
                let _ = self.executor.execute(
                    "docker", 
                    &cp_report_args, 
                    &self.workspace_path, 
                    &empty_env, 
                    Duration::from_secs(30), 
                    cancel_token, 
                    None
                ).await;
            }
        }

        res
    }

    async fn cleanup(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::CleaningUp;

        info!("Cleaning up Docker container: {}", self.container_name);

        let empty_env = HashMap::new();
        let cancel_token = CancellationToken::new();

        let args = vec![
            "rm".to_string(),
            "-f".to_string(),
            self.container_name.clone(),
        ];

        let result = self
            .executor
            .execute(
                "docker",
                &args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(10),
                cancel_token,
                None,
            )
            .await;

        if !matches!(result.outcome, ProcessOutcome::Succeeded) {
            warn!(
                "Failed to remove docker container {}: {}",
                self.container_name, result.stderr.text
            );
            return Err(RunnerError::CleanupFailed(format!(
                "Failed to remove docker container: {}",
                result.stderr.text
            )));
        }

        let vol_args = vec![
            "volume".to_string(),
            "rm".to_string(),
            "-f".to_string(),
            self.volume_name.clone(),
        ];

        let _ = self
            .executor
            .execute(
                "docker",
                &vol_args,
                &self.workspace_path,
                &empty_env,
                Duration::from_secs(10),
                CancellationToken::new(),
                None,
            )
            .await;

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), RunnerError> {
        self.state = RunnerState::Destroyed;
        Ok(())
    }
}
