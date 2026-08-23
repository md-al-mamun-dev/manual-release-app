use sqlx::PgPool;
use std::path::PathBuf;
use uuid::Uuid;

use release_daemon::config::AppConfig;
use release_daemon::domain::build_plan::BuildPlan;
use release_daemon::domain::project_build_config::{ApplicationType, Framework, PackageManager};
use release_daemon::repositories::release_image_repository::ReleaseImageRepository;
use release_daemon::runner::Runner;
use release_daemon::runner::context::RunnerExecutionContext;
use release_daemon::runner::mock_runner::MockRunner;
use release_daemon::services::build_plan_executor::BuildPlanExecutor;
use tokio_util::sync::CancellationToken;

async fn setup_db(pool: &PgPool) -> (Uuid, Uuid) {
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

    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, status, git_commit, created_at, updated_at) VALUES ($1, $2, 'v1', 'IMAGE_APPROVED', 'abcd1234abcd1234abcd1234abcd1234abcd1234', NOW(), NOW())",
        release_id,
        project_id
    )
    .execute(pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, release_id, job_type, status) VALUES ($1, $2, $3, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id,
        release_id
    )
    .execute(pool)
    .await
    .unwrap();

    (release_id, job_id)
}

fn test_config() -> AppConfig {
    AppConfig {
        database_url: "".into(),
        backend_host: "".into(),
        backend_port: 8080,
        job_workspace_root: "/tmp".into(),
        runner_type: "MOCK".into(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: false,
        runner_ubuntu_image: "ubuntu:22.04".into(),
        runner_memory_limit: "512m".into(),
        runner_cpus_limit: "1.0".into(),
        runner_pids_limit: "100".into(),
        runner_network_policy: "bridge".into(),
        kaniko_memory_limit: "1024m".into(),
        kaniko_cpus_limit: "2.0".into(),
        kaniko_pids_limit: "200".into(),
        max_image_tar_size: 1024 * 1024 * 1024,
        max_trivy_report_size: 10 * 1024 * 1024,
        registry_url: "registry.example.com".into(),
        registry_repository: "myrepo/myapp".into(),
        registry_username: "user".into(),
        registry_password: "password".into(),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_publish_success_flow(pool: sqlx::PgPool) {
    let config = test_config();

    let (release_id, job_id) = setup_db(&pool).await;

    let build_plan = BuildPlan {
        source_sha: "abcd123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Express,
        runtime: "18".to_string(),
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
        application_port: None,
        health_endpoint: None,
    };

    let release_image_repo = ReleaseImageRepository::new(pool.clone());

    let mut runner = MockRunner::new(PathBuf::from("/tmp"), release_id, job_id);
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(&runner as &dyn Runner, cancel_token);

    let executor = BuildPlanExecutor::new(pool.clone(), config);

    // Provide a dummy image record for digest check
    let release_img = release_daemon::domain::release_image::ReleaseImage {
        id: Uuid::new_v4(),
        release_id,
        job_id,
        git_sha: "abcd123".to_string(),
        image_tag: "target-img:abcd123".to_string(),
        image_digest: "sha256:mockdigest".to_string(),
        created_at: chrono::Utc::now(),
        registry: None,
        repository: None,
        remote_digest: None,
        publication_status: Some("PENDING".to_string()),
        updated_at: None,
    };
    release_image_repo
        .create_release_image(&release_img)
        .await
        .unwrap();

    let res = executor
        .execute_image_publish(&build_plan, release_id, job_id, &context)
        .await;

    assert!(res.is_ok());
    assert!(res.unwrap());

    let updated_image = release_image_repo
        .get_by_release_id(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_image.publication_status.unwrap(), "PUBLISHED");
    assert_eq!(updated_image.remote_digest.unwrap(), "sha256:mockdigest");
    assert_eq!(updated_image.registry.unwrap(), "registry.example.com");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_publish_digest_mismatch(pool: sqlx::PgPool) {
    let config = test_config();

    let (release_id, job_id) = setup_db(&pool).await;

    let build_plan = BuildPlan {
        source_sha: "abcd123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Express,
        runtime: "18".to_string(),
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
        application_port: None,
        health_endpoint: None,
    };

    let release_image_repo = ReleaseImageRepository::new(pool.clone());

    let mut runner = MockRunner::new(PathBuf::from("/tmp"), release_id, job_id);
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(&runner as &dyn Runner, cancel_token);

    let executor = BuildPlanExecutor::new(pool.clone(), config);

    // Mismatched digest
    let release_img = release_daemon::domain::release_image::ReleaseImage {
        id: Uuid::new_v4(),
        release_id,
        job_id,
        git_sha: "abcd123".to_string(),
        image_tag: "target-img:abcd123".to_string(),
        image_digest: "sha256:differentdigest".to_string(),
        created_at: chrono::Utc::now(),
        registry: None,
        repository: None,
        remote_digest: None,
        publication_status: Some("PENDING".to_string()),
        updated_at: None,
    };
    release_image_repo
        .create_release_image(&release_img)
        .await
        .unwrap();

    let res = executor
        .execute_image_publish(&build_plan, release_id, job_id, &context)
        .await;

    assert!(res.is_err()); // should fail

    let updated_image = release_image_repo
        .get_by_release_id(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_image.publication_status.unwrap(), "PUBLISH_FAILED");
}
