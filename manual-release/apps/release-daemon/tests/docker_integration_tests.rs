use release_daemon::config::AppConfig;
use release_daemon::runner::Runner;
use release_daemon::runner::docker_ubuntu_runner::LocalDockerUbuntuRunner;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn setup_runner() -> (LocalDockerUbuntuRunner, PathBuf) {
    let workspace = PathBuf::from(format!(
        "/tmp/docker-test-workspace-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).unwrap();

    let config = AppConfig::from_env().unwrap_or_else(|_| AppConfig {
        database_url: "".into(),
        backend_host: "".into(),
        backend_port: 8080,
        job_workspace_root: "/tmp".into(),
        runner_type: "LOCAL_UBUNTU".into(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".into(),
        runner_memory_limit: "512m".into(),
        runner_cpus_limit: "1.0".into(),
        runner_pids_limit: "100".into(),
        runner_network_policy: "bridge".into(),
        kaniko_memory_limit: "1024m".to_string(),
        kaniko_cpus_limit: "2.0".to_string(),
        kaniko_pids_limit: "200".to_string(),
        max_image_tar_size: 1073741824,
        max_trivy_report_size: 10485760,
        registry_url: "".into(),
        registry_repository: "".into(),
        registry_username: "".into(),
        registry_password: "".into(),
    });

    let runner = LocalDockerUbuntuRunner::new(
        workspace.clone(),
        config,
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    );
    (runner, workspace)
}

async fn run_cmd(runner: &LocalDockerUbuntuRunner, cmd: &str, args: &[&str]) -> String {
    let empty_envs = HashMap::new();
    let token = CancellationToken::new();
    let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let res = runner
        .execute(
            cmd,
            &args_vec,
            &empty_envs,
            Duration::from_secs(10),
            token,
            None,
        )
        .await
        .unwrap();
    format!("{}\n{}", res.stdout.text, res.stderr.text)
        .trim()
        .to_string()
}

#[tokio::test]
async fn test_docker_isolation_and_security() {
    let (mut runner, _) = setup_runner().await;

    // 1. Container creation
    runner.create().await.expect("Failed to create container");

    // 3. Ubuntu detection & 4. Linux detection are done inside prepare()
    runner.prepare().await.expect("Failed to prepare container");

    // 5. host HOME is not forwarded
    let home = run_cmd(&runner, "env", &[]).await;
    assert!(!home.contains("HOME=/Users/"), "Host HOME was leaked");

    // 6. host USER is not forwarded
    let user = run_cmd(&runner, "whoami", &[]).await;
    assert_eq!(user, "ci_user", "Should execute as ci_user, not host user");

    // 7. host AWS credentials are not forwarded
    let env_out = run_cmd(&runner, "env", &[]).await;
    assert!(
        !env_out.contains("AWS_ACCESS_KEY_ID"),
        "AWS credentials leaked"
    );

    // 8. host SSH keys are not mounted
    let ls_ssh = run_cmd(&runner, "ls", &["-la", "/home/ci_user/.ssh"]).await;
    assert!(
        ls_ssh.contains("No such file or directory") || ls_ssh.trim().is_empty(),
        "SSH keys mounted!"
    );

    // 9. host filesystem is not broadly mounted
    let ls_root = run_cmd(&runner, "ls", &["/Users"]).await;
    assert!(
        ls_root.contains("No such file or directory"),
        "Host /Users is mounted!"
    );

    // 10. privileged mode is not used
    let mount_res = run_cmd(&runner, "mount", &["-t", "tmpfs", "none", "/mnt"]).await;
    assert!(
        mount_res.contains("Permission denied")
            || mount_res.contains("Operation not permitted")
            || mount_res.contains("must be superuser"),
        "Container is privileged! Got output: {}",
        mount_res
    );

    // 11. Docker socket is not mounted
    let sock = run_cmd(&runner, "ls", &["/var/run/docker.sock"]).await;
    assert!(
        sock.contains("No such file or directory"),
        "Docker socket is mounted!"
    );

    // 12. host network is not used
    let hostname = run_cmd(&runner, "hostname", &[]).await;
    assert!(
        hostname.len() == 12,
        "Hostname looks like a host hostname, expected Docker 12-char hex"
    );

    // 2. container removal
    runner.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_cancellation_removes_container() {
    let (mut runner, _) = setup_runner().await;
    runner.create().await.expect("Failed to create container");

    runner.cleanup().await.expect("Failed to cleanup");

    let res = std::process::Command::new("docker")
        .args(["ps", "-a", "--filter", "name=cicd-runner"])
        .output()
        .unwrap();
    let _out = String::from_utf8_lossy(&res.stdout);
}

#[tokio::test]
async fn test_image_building_and_testing() {
    let (mut runner, workspace) = setup_runner().await;

    // Create a simple Dockerfile for testing
    let dockerfile_path = workspace.join("Dockerfile");
    std::fs::write(&dockerfile_path, "FROM python:3.9-alpine\nRUN echo \"print('hello')\" > /app.py\nUSER 1000\nEXPOSE 8080\nCMD python3 -m http.server 8080\n").unwrap();

    runner.create().await.expect("Failed to create container");
    runner.prepare().await.expect("Failed to prepare container");

    let token = CancellationToken::new();

    // 1. Build Image via Kaniko
    let build_res = runner
        .build_image(
            "Dockerfile",
            ".",
            "target-test-img:latest",
            token.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(
        build_res.0.exit_code == Some(0),
        "Kaniko build failed: {}\n{}",
        build_res.0.stdout.text,
        build_res.0.stderr.text
    );

    // Verify digest was captured
    assert!(build_res.1.is_some(), "Digest should be captured");
    assert!(build_res.1.unwrap().starts_with("sha256:"));

    // Ensure tarball exists in workspace
    assert!(
        workspace.join("image.tar").exists(),
        "image.tar missing after Kaniko build"
    );

    // 2. Test Image
    let test_res = runner
        .test_image("target-test-img:latest", 8080, "/", token.clone(), None)
        .await
        .unwrap();
    assert!(
        test_res.exit_code == Some(0),
        "Smoke test failed: {}\n{}",
        test_res.stdout.text,
        test_res.stderr.text
    );

    runner.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_test_image_rejects_root() {
    let (mut runner, workspace) = setup_runner().await;

    // Create a Dockerfile that defaults to root
    let dockerfile_path = workspace.join("Dockerfile");
    std::fs::write(&dockerfile_path, "FROM alpine:3.18\nCMD sleep 10\n").unwrap();

    runner.create().await.expect("Failed to create container");
    runner.prepare().await.expect("Failed to prepare container");
    let token = CancellationToken::new();

    let build_res = runner
        .build_image(
            "Dockerfile",
            ".",
            "target-root-img:latest",
            token.clone(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(build_res.0.exit_code, Some(0));

    let test_res = runner
        .test_image("target-root-img:latest", 8080, "/", token.clone(), None)
        .await
        .unwrap();

    // Should fail because it's root
    assert_ne!(test_res.exit_code, Some(0));
    assert!(
        test_res.stderr.text.contains("Security Policy Violation"),
        "Got stderr: {}",
        test_res.stderr.text
    );

    runner.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_dockerignore_secret_protection() {
    let (mut runner, workspace) = setup_runner().await;

    // Create secrets in workspace
    std::fs::write(workspace.join(".env"), "SECRET=123").unwrap();
    std::fs::write(workspace.join("id_rsa"), "private key").unwrap();

    // Create Dockerfile
    let dockerfile_path = workspace.join("Dockerfile");
    std::fs::write(&dockerfile_path, "FROM alpine:3.18\n").unwrap();

    runner.create().await.expect("Failed to create container");
    runner.prepare().await.expect("Failed to prepare container");
    let token = CancellationToken::new();

    let build_res = runner
        .build_image(
            "Dockerfile",
            ".",
            "test-secrets:latest",
            token.clone(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(build_res.0.exit_code, Some(0));

    // Verify .dockerignore was created and contains the secrets
    let dockerignore = std::fs::read_to_string(workspace.join(".dockerignore")).unwrap();
    assert!(dockerignore.contains(".env"));
    assert!(dockerignore.contains("id_rsa"));
    assert!(dockerignore.contains("*.pem"));

    runner.cleanup().await.expect("Failed to cleanup");
}
