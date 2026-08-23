use chrono::Utc;
use release_daemon::{
    config::AppConfig,
    domain::project_build_config::{
        ApplicationType, Framework, PackageManager, ProjectBuildConfig,
    },
    runner::{context::RunnerExecutionContext, manager::RunnerManager},
    services::build_plan_generator::BuildPlanGenerator,
};

use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[tokio::test]
async fn test_build_plan_generation_npm() {
    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: "/tmp/dummy".to_string(),
        runner_type: "MOCK".to_string(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".to_string(),
        runner_memory_limit: "512m".to_string(),
        runner_cpus_limit: "1.0".to_string(),
        runner_pids_limit: "100".to_string(),
        runner_network_policy: "bridge".to_string(),
        kaniko_memory_limit: "1024m".to_string(),
        kaniko_cpus_limit: "2.0".to_string(),
        kaniko_pids_limit: "200".to_string(),
        max_image_tar_size: 1073741824,
        max_trivy_report_size: 10485760,
        registry_url: "".into(),
        registry_repository: "".into(),
        registry_username: "".into(),
        registry_password: "".into(),
    };

    let runner_manager = RunnerManager::new(config.clone());
    let mut runner = runner_manager
        .create_runner(
            PathBuf::from("/tmp/dummy"),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        )
        .unwrap();

    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let project_config = ProjectBuildConfig {
        id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime_version: "20".to_string(),
        package_manager: PackageManager::Npm,
        dockerfile_path: None,
        build_context: None,
        application_port: None,
        health_endpoint: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let plan = BuildPlanGenerator::generate(
        &project_config,
        &context,
        &PathBuf::from("/tmp/dummy"),
        "sha123".to_string(),
    )
    .await
    .unwrap();

    let install = plan.install_command.unwrap();
    assert_eq!(install.program, "npm");
    assert_eq!(install.args, vec!["ci".to_string()]);
}

#[tokio::test]
async fn test_build_plan_generation_pnpm() {
    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: "/tmp/dummy".to_string(),
        runner_type: "MOCK".to_string(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".to_string(),
        runner_memory_limit: "512m".to_string(),
        runner_cpus_limit: "1.0".to_string(),
        runner_pids_limit: "100".to_string(),
        runner_network_policy: "bridge".to_string(),
        kaniko_memory_limit: "1024m".to_string(),
        kaniko_cpus_limit: "2.0".to_string(),
        kaniko_pids_limit: "200".to_string(),
        max_image_tar_size: 1073741824,
        max_trivy_report_size: 10485760,
        registry_url: "".into(),
        registry_repository: "".into(),
        registry_username: "".into(),
        registry_password: "".into(),
    };

    let runner_manager = RunnerManager::new(config.clone());
    let mut runner = runner_manager
        .create_runner(
            PathBuf::from("/tmp/dummy"),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        )
        .unwrap();

    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let project_config = ProjectBuildConfig {
        id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime_version: "20".to_string(),
        package_manager: PackageManager::Pnpm,
        dockerfile_path: None,
        build_context: None,
        application_port: None,
        health_endpoint: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let plan = BuildPlanGenerator::generate(
        &project_config,
        &context,
        &PathBuf::from("/tmp/dummy"),
        "sha123".to_string(),
    )
    .await
    .unwrap();

    let install = plan.install_command.unwrap();
    assert_eq!(install.program, "pnpm");
    assert_eq!(
        install.args,
        vec!["install".to_string(), "--frozen-lockfile".to_string()]
    );
}

#[tokio::test]
async fn test_build_plan_generation_yarn() {
    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: "/tmp/dummy".to_string(),
        runner_type: "MOCK".to_string(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".to_string(),
        runner_memory_limit: "512m".to_string(),
        runner_cpus_limit: "1.0".to_string(),
        runner_pids_limit: "100".to_string(),
        runner_network_policy: "bridge".to_string(),
        kaniko_memory_limit: "1024m".to_string(),
        kaniko_cpus_limit: "2.0".to_string(),
        kaniko_pids_limit: "200".to_string(),
        max_image_tar_size: 1073741824,
        max_trivy_report_size: 10485760,
        registry_url: "".into(),
        registry_repository: "".into(),
        registry_username: "".into(),
        registry_password: "".into(),
    };

    let runner_manager = RunnerManager::new(config.clone());
    let mut runner = runner_manager
        .create_runner(
            PathBuf::from("/tmp/dummy"),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        )
        .unwrap();

    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let project_config = ProjectBuildConfig {
        id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime_version: "20".to_string(),
        package_manager: PackageManager::Yarn,
        dockerfile_path: None,
        build_context: None,
        application_port: None,
        health_endpoint: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let plan = BuildPlanGenerator::generate(
        &project_config,
        &context,
        &PathBuf::from("/tmp/dummy"),
        "sha123".to_string(),
    )
    .await
    .unwrap();

    let install = plan.install_command.unwrap();
    assert_eq!(install.program, "yarn");
    assert_eq!(
        install.args,
        vec!["install".to_string(), "--immutable".to_string()]
    );
}
