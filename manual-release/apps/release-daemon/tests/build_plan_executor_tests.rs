use release_daemon::{
    config::AppConfig,
    domain::build_plan::{BuildPlan, CommandData},
    domain::project_build_config::{ApplicationType, Framework, PackageManager},
    runner::{context::RunnerExecutionContext, manager::RunnerManager},
    services::build_plan_executor::BuildPlanExecutor,
};
use sqlx::PgPool;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

async fn setup_db(pool: &PgPool) -> Uuid {
    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", project_id),
        "/tmp/dummy"
    )
    .execute(pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, job_type, status) VALUES ($1, $2, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id
    )
    .execute(pool)
    .await
    .unwrap();

    job_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_build_plan_executor_success(pool: PgPool) {
    let job_id = setup_db(&pool).await;

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

    let cancel_token = CancellationToken::new();
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let build_plan = BuildPlan {
        source_sha: "sha123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime: "20".to_string(),
        package_manager: PackageManager::Npm,
        install_command: Some(
            CommandData::new("echo".to_string(), vec!["install".to_string()]).unwrap(),
        ),
        lint_command: Some(CommandData::new("echo".to_string(), vec!["lint".to_string()]).unwrap()),
        typecheck_command: None,
        test_command: None,
        build_command: Some(
            CommandData::new("echo".to_string(), vec!["build".to_string()]).unwrap(),
        ),
        build_image_command: None,
        test_image_command: None,
        dockerfile: None,
        docker_context: None,
        application_port: None,
        health_endpoint: None,
    };

    let executor = BuildPlanExecutor::new(pool.clone(), config.clone());

    let result = executor.execute_ci(&build_plan, job_id, &context).await;

    assert!(result.is_ok(), "Expected execution to succeed");

    let steps = sqlx::query!(
        "SELECT step_key, status FROM job_steps WHERE job_id = $1 ORDER BY step_order",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].step_key, "INSTALL_DEPENDENCIES");
    assert_eq!(steps[0].status, "SUCCEEDED");

    assert_eq!(steps[1].step_key, "LINT");
    assert_eq!(steps[1].status, "SUCCEEDED");

    assert_eq!(steps[2].step_key, "BUILD_APPLICATION");
    assert_eq!(steps[2].status, "SUCCEEDED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_build_plan_executor_failure_stops_pipeline(pool: PgPool) {
    let job_id = setup_db(&pool).await;

    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: "/tmp/dummy".to_string(),
        runner_type: "MOCK_FAIL_COMMAND".to_string(),
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

    let cancel_token = CancellationToken::new();
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let build_plan = BuildPlan {
        source_sha: "sha123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime: "20".to_string(),
        package_manager: PackageManager::Npm,
        install_command: Some(
            CommandData::new("echo".to_string(), vec!["install".to_string()]).unwrap(),
        ),
        lint_command: Some(CommandData::new("echo".to_string(), vec!["lint".to_string()]).unwrap()),
        typecheck_command: None,
        test_command: None,
        build_command: Some(
            CommandData::new("echo".to_string(), vec!["build".to_string()]).unwrap(),
        ),
        build_image_command: None,
        test_image_command: None,
        dockerfile: None,
        docker_context: None,
        application_port: None,
        health_endpoint: None,
    };

    let executor = BuildPlanExecutor::new(pool.clone(), config.clone());
    let result = executor.execute_ci(&build_plan, job_id, &context).await;

    assert!(result.is_err(), "Executor should return error on failure");

    let steps = sqlx::query!(
        "SELECT step_key, status FROM job_steps WHERE job_id = $1 ORDER BY step_order",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(
        steps.len(),
        1,
        "Only first step should be persisted because it failed"
    );
    assert_eq!(steps[0].step_key, "INSTALL_DEPENDENCIES");
    assert_eq!(steps[0].status, "FAILED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_build_plan_executor_scan_image_success(pool: PgPool) {
    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", project_id),
        "/tmp/dummy"
    )
    .execute(&pool)
    .await
    .unwrap();

    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, status) VALUES ($1, $2, 'v1.0.0', '1234567890123456789012345678901234567890', 'IMAGE_TESTED')",
        release_id,
        project_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, release_id, job_type, status) VALUES ($1, $2, $3, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id,
        release_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Persist ReleaseImage
    let release_img = release_daemon::domain::release_image::ReleaseImage {
        id: Uuid::new_v4(),
        release_id,
        job_id,
        git_sha: "sha123".to_string(),
        image_tag: "target-img:sha123".to_string(),
        image_digest: "sha256:exactdigest1234567890".to_string(),
        created_at: chrono::Utc::now(),
        registry: None,
        repository: None,
        remote_digest: None,
        publication_status: None,
        updated_at: None,
    };
    let img_repo =
        release_daemon::repositories::release_image_repository::ReleaseImageRepository::new(
            pool.clone(),
        );
    img_repo.create_release_image(&release_img).await.unwrap();

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

    let cancel_token = CancellationToken::new();
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let build_plan = BuildPlan {
        source_sha: "sha123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime: "20".to_string(),
        package_manager: PackageManager::Npm,
        install_command: None,
        lint_command: None,
        typecheck_command: None,
        test_command: None,
        build_command: None,
        build_image_command: None,
        test_image_command: None,
        dockerfile: Some("Dockerfile".to_string()),
        docker_context: Some(".".to_string()),
        application_port: Some(8080),
        health_endpoint: Some("/".to_string()),
    };

    let policy = release_daemon::domain::security_policy::SecurityPolicy::default();
    let executor = BuildPlanExecutor::new(pool.clone(), config.clone());
    let passed = executor
        .execute_image_scan(&build_plan, release_id, job_id, &policy, &context)
        .await
        .unwrap();

    assert!(passed, "Scan should pass for mock empty report");

    let scan_repo =
        release_daemon::repositories::security_scan_repository::SecurityScanRepository::new(
            pool.clone(),
        );
    let scan = scan_repo
        .find_latest_by_release_id(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(scan.image_digest, "sha256:exactdigest1234567890");
    assert!(scan.passed);
    assert_eq!(scan.critical_vulnerabilities, 0);

    let steps = sqlx::query!(
        "SELECT step_key, status FROM job_steps WHERE job_id = $1 ORDER BY step_order",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].step_key, "SCAN_IMAGE");
    assert_eq!(steps[0].status, "SUCCEEDED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_build_plan_executor_scan_image_large_report(pool: PgPool) {
    let project_id = Uuid::new_v4();
    let workspace_path = format!("/tmp/dummy-{}", Uuid::new_v4());
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", project_id),
        &workspace_path
    )
    .execute(&pool)
    .await
    .unwrap();

    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, status) VALUES ($1, $2, 'v1.0.0', '1234567890123456789012345678901234567890', 'IMAGE_TESTED')",
        release_id,
        project_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, release_id, job_type, status) VALUES ($1, $2, $3, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id,
        release_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let release_img = release_daemon::domain::release_image::ReleaseImage {
        id: Uuid::new_v4(),
        release_id,
        job_id,
        git_sha: "sha123".to_string(),
        image_tag: "target-img:sha123".to_string(),
        image_digest: "sha256:exactdigest1234567890".to_string(),
        created_at: chrono::Utc::now(),
        registry: None,
        repository: None,
        remote_digest: None,
        publication_status: None,
        updated_at: None,
    };
    let img_repo =
        release_daemon::repositories::release_image_repository::ReleaseImageRepository::new(
            pool.clone(),
        );
    img_repo.create_release_image(&release_img).await.unwrap();

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

    let mut mock_runner = release_daemon::runner::mock_runner::MockRunner::new(
        PathBuf::from(&workspace_path),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    );
    mock_runner.fail_large_report = true;
    let mut runner: Box<dyn release_daemon::runner::Runner> = Box::new(mock_runner);

    let cancel_token = CancellationToken::new();
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let context = RunnerExecutionContext::new(runner.as_mut(), cancel_token);

    let build_plan = BuildPlan {
        source_sha: "sha123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime: "20".to_string(),
        package_manager: PackageManager::Npm,
        install_command: None,
        lint_command: None,
        typecheck_command: None,
        test_command: None,
        build_command: None,
        build_image_command: None,
        test_image_command: None,
        dockerfile: Some("Dockerfile".to_string()),
        docker_context: Some(".".to_string()),
        application_port: Some(8080),
        health_endpoint: Some("/".to_string()),
    };

    let policy = release_daemon::domain::security_policy::SecurityPolicy::default();
    let executor = BuildPlanExecutor::new(pool.clone(), config.clone());

    let res = executor
        .execute_image_scan(&build_plan, release_id, job_id, &policy, &context)
        .await;

    assert!(res.is_err(), "Scan should fail due to large report");

    if let Err(release_daemon::services::build_plan_executor::BuildPlanExecutorError::StepFailed(
        step,
        msg,
    )) = res
    {
        assert_eq!(step, "SCAN_IMAGE");
        assert!(msg.contains("Trivy report is too large"));
    } else {
        panic!("Expected StepFailed error");
    }

    let steps = sqlx::query!(
        "SELECT step_key, status FROM job_steps WHERE job_id = $1 ORDER BY step_order",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].step_key, "SCAN_IMAGE");
    assert_eq!(steps[0].status, "FAILED");
}
