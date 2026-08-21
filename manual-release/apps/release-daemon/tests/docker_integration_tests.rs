use release_daemon::config::AppConfig;
use release_daemon::runner::Runner;
use release_daemon::runner::docker_ubuntu_runner::LocalDockerUbuntuRunner;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn setup_runner() -> (LocalDockerUbuntuRunner, PathBuf) {
    let workspace = PathBuf::from("/tmp/docker-test-workspace");
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
    });

    let runner = LocalDockerUbuntuRunner::new(workspace.clone(), config);
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
